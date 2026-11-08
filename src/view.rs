use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, OnceLock};

use rune::{Align, Cx, El, Image, ImageCache, Tx, Waker, col, el, row, spacer, text};
use rune_kit::{Cell, Kit, SegmentedControl, SortDir, Switch, Table, TableColumn, TableState, TableStyle};

use crate::copper::{Copper, signed};
use crate::icons;
use crate::market::{self, Market};
use crate::profit::{self, Evaluation, PriceSource};
use crate::recipes::FileDataId;
use crate::rows::{self, Column, Filter, Sort};

const SOLD_STEPS: [(&str, f64); 4] = [("Any", 0.0), ("1+", 1.0), ("5+", 5.0), ("20+", 20.0)];
const NA: &str = "n/a";
const ROW_ICON: f32 = 22.0;
const TITLE_ICON: f32 = 36.0;
const REAGENT_ICON: f32 = 20.0;

pub struct App {
    load: Load,
    /// The loader thread's messages, until it finishes.
    inbox: Option<Receiver<Msg>>,
    icons: Icons,
    source: PriceSource,
    hide_unpriced: bool,
    min_sold: usize,
    table: TableState,
}

enum Load {
    Loading,
    Failed(String),
    Ready(Ready),
}

enum Msg {
    Market(Result<Market, String>),
    Icon(FileDataId, PathBuf),
}

/// Decoded icons by file data id. An icon without an image, or still decoding, draws as an
/// empty square.
#[derive(Default)]
struct Icons {
    cache: Option<ImageCache>,
    images: HashMap<FileDataId, Image>,
}

impl Icons {
    fn show(&mut self, id: FileDataId, path: &Path) {
        if let Some(cache) = &self.cache {
            self.images.insert(id, cache.file(path, ROW_ICON, 1.0));
        }
    }

    fn square(&self, id: Option<FileDataId>, size: f32, k: &Kit) -> El {
        let square = el().w(size).h(size).radius(3.0).fill(k.c.layer_03).a11y_hidden();
        match id.and_then(|id| self.images.get(&id)) {
            Some(image) => square.pixels(image.pixels()),
            None => square,
        }
    }
}

struct Ready {
    market: Market,
    evals: Vec<Evaluation>,
    rows: Vec<usize>,
}

impl App {
    pub fn new() -> App {
        let mut table = TableState::new();
        table.set_sort(Column::Profit.id(), SortDir::Descending);
        App { load: Load::Loading, inbox: None, icons: Icons::default(), source: PriceSource::Realm, hide_unpriced: true, min_sold: 0, table }
    }

    fn land(&mut self, result: Result<Market, String>) {
        self.load = match result {
            Ok(market) => Load::Ready(Ready { market, evals: Vec::new(), rows: Vec::new() }),
            Err(e) => Load::Failed(e),
        };
        self.refresh();
    }

    fn refresh(&mut self) {
        let Load::Ready(r) = &mut self.load else { return };
        r.evals = r.market.recipes.iter().map(|recipe| profit::evaluate(recipe, &r.market.book, self.source)).collect();
        let sort = self.table.sort().and_then(|(id, dir)| Some(Sort { column: Column::from_id(id)?, descending: dir == SortDir::Descending }));
        let filter = Filter { hide_unpriced: self.hide_unpriced, min_sold_per_day: SOLD_STEPS[self.min_sold].1, query: "" };
        r.rows = rows::visible(&r.market.recipes, &r.evals, filter, sort.unwrap_or(Sort { column: Column::Profit, descending: true }));
    }

    fn ready(&self) -> Option<&Ready> {
        match &self.load {
            Load::Ready(r) => Some(r),
            _ => None,
        }
    }
}

pub fn setup(s: &mut App, wake: Waker) {
    if s.icons.cache.is_none() {
        s.icons.cache = Some(ImageCache::new(wake.clone()));
    }
    if !matches!(s.load, Load::Loading) || s.inbox.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    s.inbox = Some(rx);
    std::thread::spawn(move || {
        let market = market::load();
        let ids = market.as_ref().map(|m| icons::referenced(&m.recipes)).unwrap_or_default();
        let _ = tx.send(Msg::Market(market));
        wake();
        let on_ready = |id, path| {
            let _ = tx.send(Msg::Icon(id, path));
            wake();
        };
        match icons::prefetch(&ids, on_ready) {
            Ok(report) if !report.failed.is_empty() => eprintln!("{} of {} icons failed, first: {}", report.failed.len(), report.icons, report.failed[0].1),
            Ok(_) => {}
            Err(e) => eprintln!("icons not fetched: {e}"),
        }
    });
}

pub fn poll(s: &mut App, _: f64, _: &mut Tx) -> bool {
    let Some(rx) = s.inbox.take() else { return false };
    let mut changed = false;
    loop {
        match rx.try_recv() {
            Ok(Msg::Market(result)) => s.land(result),
            Ok(Msg::Icon(id, path)) => s.icons.show(id, &path),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                if matches!(s.load, Load::Loading) {
                    s.land(Err("the loader stopped without a result".into()));
                }
                return true;
            }
        }
        changed = true;
    }
    s.inbox = Some(rx);
    changed
}

fn kit(dark: bool) -> &'static Kit {
    static DARK: OnceLock<Kit> = OnceLock::new();
    static LIGHT: OnceLock<Kit> = OnceLock::new();
    if dark { DARK.get_or_init(|| Kit::neutral(true)) } else { LIGHT.get_or_init(|| Kit::neutral(false)) }
}

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let k = kit(cx.system_dark());
    let body = match &s.load {
        Load::Loading => message(k, "Loading TSM prices and recipes..."),
        Load::Failed(e) => message(k, &format!("Could not load data: {e}")),
        Load::Ready(r) => row().w_full().grow(1.0).min_h(0.0).child(col().grow(1.0).h_full().min_h(0.0).child(table(s, k).grow(1.0).min_h(0.0))).child(detail(s, r, k)),
    };
    col().w_full().h_full().fill(k.c.bg_base).child(toolbar(s, k)).child(body)
}

fn message(k: &Kit, m: &str) -> El {
    col().w_full().grow(1.0).center().padding(24.0).child(text(m).font(k.ty.body).color(k.c.text_muted).wrap())
}

fn toolbar(s: &App, k: &Kit) -> El {
    let source = SegmentedControl::new(&["Realm", "Region"], Some(matches!(s.source, PriceSource::Region) as usize))
        .label("Price source")
        .width(Some(160.0))
        .on_pick(|s: &mut App, i, _: &mut Tx| {
            s.source = if i == 0 { PriceSource::Realm } else { PriceSource::Region };
            s.refresh();
        })
        .build(k);
    let unpriced = Switch::new(s.hide_unpriced)
        .label("Hide unpriced")
        .on_toggle(|s: &mut App| {
            s.hide_unpriced = !s.hide_unpriced;
            s.refresh();
        })
        .build(k);
    let labels: Vec<&str> = SOLD_STEPS.iter().map(|(l, _)| *l).collect();
    let sold = SegmentedControl::new(&labels, Some(s.min_sold))
        .label("Minimum sold per day")
        .width(Some(200.0))
        .on_pick(|s: &mut App, i, _: &mut Tx| {
            s.min_sold = i;
            s.refresh();
        })
        .build(k);
    let label = |t: &str| text(t).font(k.ty.text_small).color(k.c.text_muted);
    row()
        .w_full()
        .h(52.0)
        .px(16.0)
        .gap(12.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], k.c.border_weak)
        .child(text("Leatherworking").font(k.ty.body_medium).color(k.c.text_strong))
        .child(source)
        .child(unpriced)
        .child(label("Sold/day"))
        .child(sold)
        .child(spacer())
        .child(label(&status(s)))
}

fn status(s: &App) -> String {
    let Some(r) = s.ready() else { return String::new() };
    let book = &r.market.book;
    let updated = match s.source {
        PriceSource::Realm => book.realm_updated,
        PriceSource::Region => book.region_updated,
    };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let age = updated.map_or_else(|| "unknown age".into(), |t| format!("{} old", crate::overview::age(now.saturating_sub(t))));
    format!("TSM data {age}, {} of {} recipes", r.rows.len(), r.market.recipes.len())
}

fn table(s: &App, k: &Kit) -> El {
    let mut t = Table::new(
        |s: &App| &s.table,
        |s: &mut App| &mut s.table,
        |s: &App| s.ready().map_or(0, |r| r.rows.len()),
        |s: &App, i| Cow::Owned(s.ready().map_or(0, |r| r.market.recipes[r.rows[i]].spell_id).to_string()),
    );
    for c in Column::ALL {
        let col = TableColumn::new(c.id(), c.title()).sortable();
        t = t.column(match c {
            Column::Item => col.flex(1.0).min(200.0),
            Column::Skill => col.width(56.0).align(Align::End),
            Column::Margin | Column::SoldPerDay | Column::Auctions => col.width(92.0).align(Align::End),
            _ => col.width(112.0).align(Align::End),
        });
    }
    let (gain, loss) = (k.c.success.fg, k.c.danger.fg);
    let style = TableStyle::from_kit(k);
    let k = kit(k.dark);
    t.cell(move |s: &App, i, id| {
        let Some(r) = s.ready() else { return Cell::text("") };
        let (recipe, e) = (&r.market.recipes[r.rows[i]], &r.evals[r.rows[i]]);
        let na = || Cell::text(NA).muted();
        let money = |c: Option<Copper>| c.map_or_else(na, |c| Cell::text(c.to_string()).mono());
        match Column::from_id(id) {
            Some(Column::Item) => Cell::el(
                row()
                    .gap(8.0)
                    .center_y()
                    .shrink()
                    .min_w(0.0)
                    .child(s.icons.square(recipe.icon, ROW_ICON, k))
                    .child(text(&recipe.item_name).font(style.font).color(style.text).truncate().shrink().min_w(0.0).help_when_truncated(&recipe.item_name)),
            ),
            Some(Column::Skill) => recipe.skill.map_or_else(na, |skill| Cell::text(skill.learn.to_string()).mono()),
            Some(Column::Cost) => money(e.cost),
            Some(Column::Resale) => money(e.resale),
            Some(Column::Profit) => e.profit.map_or_else(na, |p| Cell::text(signed(p)).mono().color(if p >= 0 { gain } else { loss })),
            Some(Column::Margin) => e.margin.map_or_else(na, |m| Cell::text(format!("{:.1}%", m * 100.0)).mono()),
            Some(Column::SoldPerDay) => e.sold_per_day.map_or_else(na, |d| Cell::text(if d >= 100.0 { format!("{d:.0}") } else { format!("{d:.2}") }).mono()),
            Some(Column::Auctions) => e.auctions.map_or_else(na, |n| Cell::text(n.to_string()).mono()),
            None => na(),
        }
    })
    .on_sort(|s: &mut App, _, _, _| s.refresh())
    .zebra(true)
    .name("Leatherworking recipes")
    .build(s, k)
}

fn detail(s: &App, r: &Ready, k: &Kit) -> El {
    let panel = col().w(340.0).h_full().padding(16.0).gap(10.0).fill(k.c.layer_01).border_sides([0.0, 0.0, 0.0, 1.0], k.c.border_weak);
    let picked = s.table.selected().next().and_then(|id| r.market.recipes.iter().position(|x| x.spell_id.to_string() == id));
    let Some(i) = picked else {
        return panel.child(text("Select a recipe to see its reagents.").font(k.ty.text_small).color(k.c.text_muted).wrap());
    };
    let (recipe, e) = (&r.market.recipes[i], &r.evals[i]);
    let small = |t: String| text(t).font(k.ty.text_small).color(k.c.text_base);
    let muted = |t: String| text(t).font(k.ty.text_small).color(k.c.text_muted);
    let title = text(recipe.item_name.clone()).font(k.ty.body_medium).color(k.c.text_strong).wrap().shrink().min_w(0.0);
    let mut panel = panel
        .child(row().w_full().gap(10.0).center_y().child(s.icons.square(recipe.icon, TITLE_ICON, k)).child(title))
        .child(muted(format!("Makes {} per craft, spell {}", recipe.quantity, recipe.spell_id)))
        .child(muted(recipe.skill.map_or_else(|| format!("Skill {NA}"), |s| format!("Skill {} (green {}, grey {})", s.learn, s.green(), s.grey))));
    for reagent in &recipe.reagents {
        let unit = profit::unit_price(&r.market.book, reagent.item_id, s.source);
        let line = unit.map(|Copper(c)| Copper(c * u64::from(reagent.count)));
        let price = match (unit, line) {
            (Some(u), Some(l)) => muted(format!("{u} each, {l} total")),
            _ => text("No price").font(k.ty.text_small).color(k.c.danger.fg),
        };
        panel = panel.child(
            col()
                .w_full()
                .gap(2.0)
                .child(
                    row()
                        .w_full()
                        .gap(6.0)
                        .center_y()
                        .child(s.icons.square(reagent.icon, REAGENT_ICON, k))
                        .child(small(format!("{}x", reagent.count)))
                        .child(small(reagent.item_name.clone()).truncate().shrink().min_w(0.0)),
                )
                .child(price),
        );
    }
    let total = |label: &str, v: String| row().w_full().child(muted(label.into())).child(spacer()).child(small(v));
    panel
        .child(total("Cost", e.cost.map_or(NA.into(), |c| c.to_string())))
        .child(total("Resale after 5% cut", e.resale.map_or(NA.into(), |c| c.to_string())))
        .child(total("Profit", e.profit.map_or(NA.into(), signed)))
}

/// Prefetches icons and waits for every one to decode, so the snapshot shows them.
pub fn scene_loaded(s: &mut App) {
    s.land(market::load());
    let Load::Ready(r) = &s.load else { return };
    let (tx, rx) = mpsc::channel();
    if let Err(e) = icons::prefetch(&icons::referenced(&r.market.recipes), |id, path| {
        let _ = tx.send((id, path));
    }) {
        eprintln!("icons not fetched: {e}");
    }
    drop(tx);
    s.icons.cache = Some(ImageCache::new(Arc::new(|| {})));
    for (id, path) in rx {
        s.icons.show(id, &path);
    }
    for image in s.icons.images.values() {
        let _ = image.wait();
    }
    if let Some(&first) = r.rows.first() {
        s.table.select(r.market.recipes[first].spell_id.to_string());
    }
}

