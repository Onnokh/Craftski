mod detail;
mod header;
mod rail;
mod table;
mod toolbar;
mod widgets;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use rune::{Cx, El, Image, ImageCache, Key, KeyEvent, TextEdit, Tx, Waker, col, row, text};

use crate::icons;
use crate::market::{self, Market};
use crate::overview::{Overview, overview};
use crate::profit::{self, Evaluation, PriceSource};
use crate::recipes::{FileDataId, Recipe};
use crate::rows::{self, Column, Filter, Sort};
use crate::theme::{self, p};

const SOLD_STEPS: [(&str, f64); 4] = [("Any", 0.0), ("1+", 1.0), ("5+", 5.0), ("20+", 20.0)];

pub struct App {
    load: Load,
    /// The loader thread's messages, until it finishes.
    inbox: Option<Receiver<Msg>>,
    icons: Icons,
    source: PriceSource,
    hide_unpriced: bool,
    min_sold: usize,
    sort: Sort,
    /// The recipe the detail panel shows, by spell id, kept while filters hide its row.
    selected: Option<u32>,
    search: TextEdit,
}

enum Load {
    Loading,
    Failed(String),
    Ready(Box<Ready>),
}

enum Msg {
    Market(Result<Market, String>),
    Icon(FileDataId, PathBuf),
    Rail(&'static str, RailImage),
}

enum RailImage {
    File(PathBuf),
    Dimmed(u32, u32, Vec<u8>),
}

/// Decoded icons. An icon without an image, or still decoding, draws as its empty frame.
#[derive(Default)]
struct Icons {
    cache: Option<ImageCache>,
    items: HashMap<FileDataId, Image>,
    rail: HashMap<&'static str, Image>,
}

impl Icons {
    fn show(&mut self, id: FileDataId, path: &Path) {
        if let Some(cache) = &self.cache {
            self.items.insert(id, cache.file(path, 48.0, 2.0));
        }
    }

    fn show_rail(&mut self, name: &'static str, image: RailImage) {
        let Some(cache) = &self.cache else { return };
        let image = match image {
            RailImage::File(path) => cache.file(path, 30.0, 2.0),
            RailImage::Dimmed(w, h, rgba) => cache.rgba(w, h, rgba),
        };
        self.rail.insert(name, image);
    }

    fn item(&self, id: Option<FileDataId>) -> Option<&Image> {
        self.items.get(&id?)
    }
}

struct Ready {
    market: Market,
    evals: Vec<Evaluation>,
    overview: Overview,
    rows: Vec<usize>,
}

/// Sizes that change between a roomy window and a narrow one.
#[derive(Clone, Copy)]
struct Dims {
    compact: bool,
    pad: f32,
    detail: f32,
    gap: f32,
    /// Skill, materials, net sale, profit, margin, sold/day, listed.
    cols: [f32; 7],
}

impl Dims {
    fn new(compact: bool) -> Dims {
        if compact {
            Dims { compact, pad: 16.0, detail: 360.0, gap: 16.0, cols: [40.0, 104.0, 104.0, 104.0, 56.0, 56.0, 40.0] }
        } else {
            Dims { compact, pad: 24.0, detail: 400.0, gap: 24.0, cols: [56.0, 112.0, 112.0, 112.0, 72.0, 80.0, 64.0] }
        }
    }
}

impl App {
    pub fn new() -> App {
        App {
            load: Load::Loading,
            inbox: None,
            icons: Icons::default(),
            source: PriceSource::Realm,
            hide_unpriced: true,
            min_sold: 0,
            sort: Sort { column: Column::Profit, descending: true },
            selected: None,
            search: TextEdit::new(""),
        }
    }

    fn land(&mut self, result: Result<Market, String>) {
        self.load = match result {
            Ok(market) => Load::Ready(Box::new(Ready { market, evals: Vec::new(), overview: overview(&[]), rows: Vec::new() })),
            Err(e) => Load::Failed(e),
        };
        self.refresh();
        if let Load::Ready(r) = &self.load {
            self.selected = r.rows.first().map(|&i| r.market.recipes[i].spell_id);
        }
    }

    fn refresh(&mut self) {
        let Load::Ready(r) = &mut self.load else { return };
        r.evals = r.market.recipes.iter().map(|recipe| profit::evaluate(recipe, &r.market.book, self.source)).collect();
        r.overview = overview(&r.evals);
        let filter = Filter { hide_unpriced: self.hide_unpriced, min_sold_per_day: SOLD_STEPS[self.min_sold].1, query: &self.search.text };
        r.rows = rows::visible(&r.market.recipes, &r.evals, filter, self.sort);
    }

    fn ready(&self) -> Option<&Ready> {
        match &self.load {
            Load::Ready(r) => Some(r),
            _ => None,
        }
    }

    /// The selected recipe's index into the recipe list.
    fn picked(&self) -> Option<(&Ready, usize)> {
        let r = self.ready()?;
        let id = self.selected?;
        Some((r, r.market.recipes.iter().position(|x| x.spell_id == id)?))
    }

    /// Moves the selection `by` rows through the visible rows and scrolls it into view.
    fn step(&mut self, by: isize, tx: &mut Tx) -> bool {
        let Some(r) = self.ready() else { return false };
        if r.rows.is_empty() {
            return false;
        }
        let at = self.selected.and_then(|id| r.rows.iter().position(|&i| r.market.recipes[i].spell_id == id));
        let next = match at {
            Some(at) => at.saturating_add_signed(by).min(r.rows.len() - 1),
            None => 0,
        };
        let id = r.market.recipes[r.rows[next]].spell_id;
        self.selected = Some(id);
        tx.scroll_into_view(id.to_string());
        true
    }

    fn clear_search(&mut self) {
        self.search.clear();
        self.refresh();
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
        let send = |m: Msg| {
            let _ = tx.send(m);
            wake();
        };
        let market = market::load();
        let ids = market.as_ref().map(|m| icons::referenced(&m.recipes)).unwrap_or_default();
        send(Msg::Market(market));
        fetch_rail(|name, image| send(Msg::Rail(name, image)));
        match icons::prefetch(&ids, |id, path| send(Msg::Icon(id, path))) {
            Ok(report) if !report.failed.is_empty() => eprintln!("{} of {} icons failed, first: {}", report.failed.len(), report.icons, report.failed[0].1),
            Ok(_) => {}
            Err(e) => eprintln!("icons not fetched: {e}"),
        }
    });
}

/// The rail's profession icons: the covered one as it is, the rest greyed.
fn fetch_rail(on_ready: impl Fn(&'static str, RailImage)) {
    let names: Vec<&str> = rail::ICONS.iter().map(|(_, name)| *name).collect();
    for ((name, result), (_, icon)) in icons::fetch_named(&names).into_iter().zip(rail::ICONS) {
        let image = result.and_then(|path| {
            if icon == rail::ACTIVE {
                Ok(RailImage::File(path))
            } else {
                icons::decode_dimmed(&path).map(|(w, h, rgba)| RailImage::Dimmed(w, h, rgba))
            }
        });
        match image {
            Ok(image) => on_ready(icon, image),
            Err(e) => eprintln!("rail icon {name}: {e}"),
        }
    }
}

pub fn poll(s: &mut App, _: f64, _: &mut Tx) -> bool {
    let Some(rx) = s.inbox.take() else { return false };
    let mut changed = false;
    loop {
        match rx.try_recv() {
            Ok(Msg::Market(result)) => s.land(result),
            Ok(Msg::Icon(id, path)) => s.icons.show(id, &path),
            Ok(Msg::Rail(name, image)) => s.icons.show_rail(name, image),
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

/// Ctrl+K finds, Escape clears the search, and the arrow keys walk the rows.
pub fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if ev.is_command('k') {
        s.search.focused = true;
        return true;
    }
    match ev.key {
        Key::Escape if !s.search.text.is_empty() => {
            s.clear_search();
            true
        }
        Key::Down => s.step(1, tx),
        Key::Up => s.step(-1, tx),
        _ => false,
    }
}

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let d = Dims::new(cx.layout_with("root", |r| r[2] < 1500.0).unwrap_or(false));
    let main = match &s.load {
        Load::Loading => message("Loading TSM prices and recipes..."),
        Load::Failed(e) => message(&format!("Could not load data: {e}")),
        Load::Ready(r) => row()
            .grow(1.0)
            .h_full()
            .min_w(0.0)
            .child(
                col()
                    .grow(1.0)
                    .shrink()
                    .min_w(0.0)
                    .h_full()
                    .child(header::header(s, r, d))
                    .child(toolbar::toolbar(s, r, d))
                    .child(table::table(s, r, d)),
            )
            .child(detail::detail(s, r, d)),
    };
    row().key("root").w_full().h_full().fill(p().ground).child(rail::rail(s)).child(main)
}

fn message(m: &str) -> El {
    col().grow(1.0).h_full().center().padding(24.0).child(text(m).font(theme::sans(14.0, 400.0)).color(p().muted).wrap())
}

fn recipe_at(r: &Ready, i: usize) -> (&Recipe, &Evaluation) {
    (&r.market.recipes[i], &r.evals[i])
}

/// Fetches every icon and waits for each to decode, so the snapshot shows them.
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
    let rail = std::cell::RefCell::new(Vec::new());
    fetch_rail(|name, image| rail.borrow_mut().push((name, image)));
    for (name, image) in rail.into_inner() {
        s.icons.show_rail(name, image);
    }
    for image in s.icons.items.values().chain(s.icons.rail.values()) {
        let _ = image.wait();
    }
}
