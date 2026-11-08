mod detail;
mod flips;
mod header;
mod menu;
mod rail;
mod status;
mod table;
mod toolbar;
mod widgets;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rune::{Cx, El, Image, ImageCache, Key, KeyEvent, TextEdit, Tx, Waker, col, row, text};

use crate::icons;
use crate::market::{self, Market};
use crate::menu::{Menu, SOLD_STEPS};
use crate::merchants::{self, Catalog};
use crate::prices;
use crate::profit::{self, Evaluation, PriceOrigin};
use crate::recipes::{FileDataId, Recipe};
use crate::rows::{self, Column, Filter, FilterKind, Hidden, Sort};
use crate::settings;
use crate::theme::{self, p};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Crafts,
    Flips,
}

pub struct App {
    tab: Tab,
    load: Load,
    /// The loader thread's messages, until it finishes.
    inbox: Option<Receiver<Msg>>,
    icons: Icons,
    hide_unpriced: bool,
    min_sold: usize,
    sort: Sort<Column>,
    /// The recipe the detail panel shows, by spell id, kept while filters hide its row.
    selected: Option<u32>,
    search: TextEdit,
    /// The player's profession rank as typed; blank shows every rank.
    skill: TextEdit,
    flips: flips::State,
    menu: Option<Menu>,
    /// The realm the player last chose, which prices come from whenever AppData.lua holds it.
    realm: String,
    /// Unix seconds at the last minute tick: the clock the status bar's ages read from.
    now: u64,
}

enum Load {
    Loading,
    Failed(String),
    Ready(Box<Ready>),
}

enum Msg {
    Market(Box<Result<Market, String>>),
    Catalog(Box<Result<Catalog, String>>),
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
            RailImage::File(path) => cache.file(path, 32.0, 2.0),
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
    rows: Vec<usize>,
    hidden: Option<Hidden<FilterKind>>,
    /// Why the last reload failed; the prices before it stay.
    reload_error: Option<String>,
}

/// Sizes that change between a roomy window and a narrow one.
#[derive(Clone, Copy)]
struct Dims {
    compact: bool,
    pad: f32,
    detail: f32,
    gap: f32,
    /// The status bar's width at the last layout, which decides what it leaves out.
    status: Option<f32>,
}

impl Dims {
    fn new(compact: bool) -> Dims {
        if compact {
            Dims { compact, pad: 16.0, detail: 360.0, gap: 16.0, status: None }
        } else {
            Dims { compact, pad: 24.0, detail: 400.0, gap: 24.0, status: None }
        }
    }
}

impl App {
    pub fn new() -> App {
        App {
            tab: Tab::Crafts,
            load: Load::Loading,
            inbox: None,
            icons: Icons::default(),
            hide_unpriced: true,
            min_sold: 0,
            sort: Sort { column: Column::Profit, descending: true },
            selected: None,
            search: TextEdit::new(""),
            skill: TextEdit::new(settings::load_skill()),
            flips: flips::State::new(),
            menu: None,
            realm: settings::load_realm(),
            now: unix_secs(SystemTime::now()),
        }
    }

    fn land(&mut self, result: Result<Market, String>) {
        self.load = match result {
            Ok(market) => Load::Ready(Box::new(Ready { market, evals: Vec::new(), rows: Vec::new(), hidden: None, reload_error: None })),
            Err(e) => Load::Failed(e),
        };
        self.refresh();
        if let Load::Ready(r) = &self.load {
            self.selected = r.rows.first().map(|&i| r.market.recipes[i].spell_id);
        }
    }

    /// Reads TSM's prices again and reprices every recipe and flip. The selection, filters,
    /// tab and scroll stay; a failed read keeps the old prices and says why in the status bar.
    pub(super) fn reload(&mut self, appdata: &Path) {
        self.now = unix_secs(SystemTime::now());
        let Load::Ready(r) = &mut self.load else { return };
        match market::read_book(appdata) {
            Ok((book, read)) => {
                r.market = Market::new(book, read, std::mem::take(&mut r.market.recipes), &self.realm);
                r.reload_error = None;
            }
            Err(e) => {
                r.reload_error = Some(e);
                return;
            }
        }
        self.refresh();
        self.reprice_flips();
    }

    /// Reloads when AppData.lua's modified time differs from the one last read, as it does
    /// after each TSM desktop app sync. A failed read retries on the next check.
    fn reload_if_changed(&mut self, appdata: &Path) {
        let Some(r) = self.ready() else { return };
        if market::modified(appdata) != r.market.read.modified {
            self.reload(appdata);
        }
    }

    fn refresh(&mut self) {
        let Load::Ready(r) = &mut self.load else { return };
        r.evals = r.market.recipes.iter().map(|recipe| profit::evaluate(recipe, &r.market.book, r.market.costs())).collect();
        let filter = Filter {
            hide_unpriced: self.hide_unpriced,
            min_sold_per_day: SOLD_STEPS[self.min_sold].min,
            max_skill: settings::parse_skill(&self.skill.text),
            query: &self.search.text,
        };
        r.rows = rows::visible(&r.market.recipes, &r.evals, filter, self.sort);
        r.hidden = rows::hidden(&r.market.recipes, &r.evals, filter);
    }

    /// Turns off the filters that hide recipes the search finds.
    fn show_all(&mut self) {
        let Some(h) = self.ready().and_then(|r| r.hidden.clone()) else { return };
        for k in h.by {
            match k {
                FilterKind::Sells => self.min_sold = 0,
                FilterKind::HideUnpriced => self.hide_unpriced = false,
                FilterKind::Skill => {
                    self.skill.clear();
                    settings::save_skill("");
                }
            }
        }
        self.refresh();
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
    widgets::load_coins(s.icons.cache.get_or_insert_with(|| ImageCache::new(wake.clone())));
    if !matches!(s.load, Load::Loading) || s.inbox.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    s.inbox = Some(rx);
    let realm = s.realm.clone();
    std::thread::spawn(move || {
        let send = |m: Msg| {
            let _ = tx.send(m);
            wake();
        };
        let market = market::load(&realm);
        let ids = market.as_ref().map(|m| icons::referenced(&m.recipes)).unwrap_or_default();
        send(Msg::Market(Box::new(market)));
        fetch_rail(|name, image| send(Msg::Rail(name, image)));
        prefetch(&ids, |id, path| send(Msg::Icon(id, path)));
        let catalog = merchants::load();
        let ids = catalog.as_ref().map(flips::icon_ids).unwrap_or_default();
        send(Msg::Catalog(Box::new(catalog)));
        prefetch(&ids, |id, path| send(Msg::Icon(id, path)));
    });
}

fn prefetch(ids: &BTreeSet<FileDataId>, on_ready: impl Fn(FileDataId, PathBuf) + Sync) {
    match icons::prefetch(ids, on_ready) {
        Ok(report) if !report.failed.is_empty() => eprintln!("{} of {} icons failed, first: {}", report.failed.len(), report.icons, report.failed[0].1),
        Ok(_) => {}
        Err(e) => eprintln!("icons not fetched: {e}"),
    }
}

/// The rail's icons: the covered profession and the flips coin as they are, the rest greyed.
fn fetch_rail(on_ready: impl Fn(&'static str, RailImage)) {
    let names: Vec<&'static str> = rail::ICONS.iter().map(|(_, name)| *name).chain([rail::COIN]).collect();
    for ((name, result), icon) in icons::fetch_named(&names).into_iter().zip(names.iter().copied()) {
        let image = result.and_then(|path| {
            if icon == rail::ACTIVE || icon == rail::COIN {
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

/// Lands the loader's messages, and on each new minute moves the clock the ages read from and
/// reloads when TSM has rewritten AppData.lua.
pub fn poll(s: &mut App, _: f64, tx: &mut Tx) -> bool {
    let landed = drain(s);
    let now = unix_secs(SystemTime::now());
    tx.wake_after(Duration::from_secs(60 - now % 60));
    let ticked = now / 60 != s.now / 60;
    if ticked {
        s.now = now;
        s.reload_if_changed(&market::appdata_path());
    }
    landed || ticked
}

/// Takes the loader thread's messages; true when any arrived or the thread finished.
fn drain(s: &mut App) -> bool {
    let Some(rx) = s.inbox.take() else { return false };
    let mut changed = false;
    loop {
        match rx.try_recv() {
            Ok(Msg::Market(result)) => s.land(*result),
            Ok(Msg::Catalog(result)) => s.land_catalog(*result),
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

/// Ctrl+K finds, Escape closes the open menu or clears the search, and the arrow keys walk the
/// rows, all on the open tab.
pub fn on_key(s: &mut App, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if ev.key == Key::Escape && s.close_menu() {
        return true;
    }
    if s.tab == Tab::Flips {
        let f = &mut s.flips;
        if ev.is_command('k') {
            f.search.focused = true;
            return true;
        }
        return match ev.key {
            Key::Escape if !f.search.text.is_empty() => {
                f.clear_search();
                true
            }
            Key::Down => f.step(1, tx),
            Key::Up => f.step(-1, tx),
            _ => false,
        };
    }
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
    let compact = cx.layout_with("root", |r| r[2] < 1500.0).unwrap_or(false);
    let d = Dims { status: cx.layout_with("status", |r| r[2]), ..Dims::new(compact) };
    let main = match &s.load {
        Load::Loading => message("Loading TSM prices and recipes..."),
        Load::Failed(e) => message(&format!("Could not load data: {e}")),
        Load::Ready(_) if s.tab == Tab::Flips => flips::main(s, d),
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
                    .child(header::header(r, d))
                    .child(toolbar::toolbar(s, d))
                    .child(table::table(s, r, d))
                    .child(status::status(s, r, d)),
            )
            .child(detail::detail(s, r, d)),
    };
    let root = row().key("root").w_full().h_full().fill(p().ground).child(rail::rail(s)).child(main);
    menu::overlay(s).into_iter().fold(root, |r, e| r.child(e))
}

fn unix_secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn message(m: &str) -> El {
    col().grow(1.0).h_full().center().padding(24.0).child(text(m).font(theme::BODY).color(p().muted).wrap())
}

fn recipe_at(r: &Ready, i: usize) -> (&Recipe, &Evaluation) {
    (&r.market.recipes[i], &r.evals[i])
}

/// Fetches every icon and waits for each to decode, so the snapshot shows them.
pub fn scene_loaded(s: &mut App) {
    s.land(market::load(&s.realm));
    let Load::Ready(r) = &s.load else { return };
    show_icons(s, icons::referenced(&r.market.recipes));
}

/// The flips tab in Thunder Bluff, priced as the window would price it.
pub fn scene_flips(s: &mut App) {
    s.tab = Tab::Flips;
    s.land(market::load(&s.realm));
    let catalog = merchants::load();
    let ids = catalog.as_ref().map(flips::icon_ids).unwrap_or_default();
    s.land_catalog(catalog);
    show_icons(s, ids);
}

/// `flips` with the first listed item that more than one merchant sells selected.
pub fn scene_flips_shared(s: &mut App) {
    scene_flips(s);
    s.flips.select_first_shared();
}

/// `flips` searched for patterns with filters that hide most of them.
pub fn scene_flips_hidden(s: &mut App) {
    scene_flips(s);
    s.flips.search_patterns_narrowly();
}

/// The flips tab with the zone menu open.
pub fn scene_zones(s: &mut App) {
    scene_flips(s);
    s.menu = Some(Menu::Zone);
}

/// The flips tab with the Sold per day menu open.
pub fn scene_flips_sold(s: &mut App) {
    scene_flips(s);
    s.menu = Some(Menu::FlipsSold);
}

fn show_icons(s: &mut App, ids: BTreeSet<FileDataId>) {
    let (tx, rx) = mpsc::channel();
    prefetch(&ids, |id, path| {
        let _ = tx.send((id, path));
    });
    drop(tx);
    let coins = widgets::load_coins(s.icons.cache.insert(ImageCache::new(Arc::new(|| {}))));
    for (id, path) in rx {
        s.icons.show(id, &path);
    }
    let rail = std::cell::RefCell::new(Vec::new());
    fetch_rail(|name, image| rail.borrow_mut().push((name, image)));
    for (name, image) in rail.into_inner() {
        s.icons.show_rail(name, image);
    }
    for image in s.icons.items.values().chain(s.icons.rail.values()).chain(coins) {
        let _ = image.wait();
    }
}

/// `loaded` with Sold 5+ a day, so the line under the table says what it hides.
pub fn scene_crafts_hidden(s: &mut App) {
    scene_loaded(s);
    s.min_sold = 2;
    s.refresh();
}

/// `crafts-hidden` with the Sold per day menu open.
pub fn scene_crafts_sold(s: &mut App) {
    scene_crafts_hidden(s);
    s.menu = Some(Menu::CraftsSold);
}

/// `loaded` with the first listed recipe that uses a reagent cheapest to craft selected.
pub fn scene_crafted(s: &mut App) {
    scene_loaded(s);
    let Load::Ready(r) = &s.load else { return };
    let costs = r.market.costs();
    let crafted = |recipe: &&Recipe| recipe.reagents.iter().any(|g| costs.get(g.item_id).is_some_and(|p| matches!(p.origin, PriceOrigin::Crafted { .. })));
    s.selected = r.rows.iter().map(|&i| &r.market.recipes[i]).find(crafted).map(|recipe| recipe.spell_id);
}

/// `loaded` with realm prices five hours old and region prices thirty, so the status bar
/// shows both stale colours.
pub fn scene_stale(s: &mut App) {
    scene_loaded(s);
    let Load::Ready(r) = &mut s.load else { return };
    for realm in r.market.book.realms.values_mut() {
        realm.updated = Some(s.now - 5 * 3600);
    }
    r.market.book.region_updated = Some(s.now - 30 * 3600);
}

/// `loaded` with the status bar's realm menu open over a second realm, PvE-Alliance, made up
/// from the first with prices five hours older, since AppData.lua holds one realm.
pub fn scene_realms(s: &mut App) {
    scene_loaded(s);
    let Load::Ready(r) = &mut s.load else { return };
    if let Some((_, realm)) = r.market.book.realm() {
        let older = prices::RealmBook { updated: realm.updated.map(|t| t - 5 * 3600), ..realm.clone() };
        r.market.book.realms.insert("PvE-Alliance".into(), older);
    }
    s.menu = Some(Menu::Realm);
}

/// `flips` after a reload from a file that isn't there: the status bar says why and the old
/// prices stay.
pub fn scene_reload_failed(s: &mut App) {
    scene_flips(s);
    s.reload(Path::new(r"C:\no-such-wow\AppData.lua"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::copper::Copper;
    use crate::menu::Pick;
    use crate::flip_rows::Supply;

    fn appdata(scope: &str, time: u64, min_buyout: &str) -> String {
        format!(r#"x.LoadData("AUCTIONDB_NON_COMMODITY_DATA","{scope}",[[return {{downloadTime={time},fields={{"itemString","minBuyout"}},data={{{{4234,{min_buyout}}}}}}}]])"#)
    }

    #[test]
    fn a_pill_toggles_its_menu_and_closing_says_whether_one_was_open() {
        let mut s = App::new();
        s.toggle_menu(Menu::Zone);
        assert_eq!(s.menu, Some(Menu::Zone));
        s.toggle_menu(Menu::Zone);
        assert_eq!(s.menu, None);
        s.toggle_menu(Menu::Zone);
        assert!(s.close_menu());
        assert!(!s.close_menu());
    }

    #[test]
    fn choosing_a_sold_minimum_sets_the_recipe_filter_and_closes_the_menu() {
        let mut s = App::new();
        s.toggle_menu(Menu::CraftsSold);
        s.pick(Pick::CraftsSold(2));
        assert_eq!((s.menu, s.min_sold), (None, 2));
    }

    #[test]
    fn the_flips_menus_set_the_flips_filters_and_leave_the_recipes_alone() {
        let mut s = App::new();
        s.toggle_menu(Menu::FlipsSold);
        s.pick(Pick::FlipsSold(3));
        assert_eq!((s.menu, s.flips.min_sold(), s.min_sold), (None, 3, 0));
        s.toggle_menu(Menu::Supply);
        s.pick(Pick::Supply(Supply::Limited));
        assert_eq!((s.menu, s.flips.supply()), (None, Supply::Limited));
    }

    #[test]
    fn choosing_a_zone_moves_to_it_and_closes_the_menu() {
        let mut s = App::new();
        s.toggle_menu(Menu::Zone);
        s.pick(Pick::Zone("Mulgore".into()));
        assert_eq!((s.menu, s.flips.zone()), (None, "Mulgore"));
    }

    #[test]
    fn choosing_a_realm_reprices_from_it_and_holds_through_a_reload() {
        let dir = std::env::temp_dir().join(format!("tsm-crafts-realm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("AppData.lua");
        std::fs::write(&path, [appdata("PvE-Alliance", 100, "6I"), appdata("PvP-Horde", 200, "7P")].join("
")).unwrap();
        let mut s = App::new();
        s.realm = "Gone-Realm".into();
        s.land(market::read_book(&path).map(|(book, read)| Market::new(book, read, Vec::new(), &s.realm)));
        let seen = |s: &App| {
            let book = &s.ready().unwrap().market.book;
            (book.realm().map(|(name, _)| name.to_owned()), book.realm_price(4234).and_then(|p| p.min_buyout))
        };
        assert_eq!(seen(&s), (Some("PvE-Alliance".into()), Some(Copper(210))));

        let items: Vec<(String, bool)> = s.menu_items(Menu::Realm).into_iter().map(|i| (i.label, i.on)).collect();
        assert_eq!(items, [("PvE-Alliance".into(), true), ("PvP-Horde".into(), false)]);

        s.use_realm("PvP-Horde".into());
        assert_eq!(seen(&s), (Some("PvP-Horde".into()), Some(Copper(249))));
        s.reload(&path);
        assert_eq!(seen(&s), (Some("PvP-Horde".into()), Some(Copper(249))));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reload_takes_the_new_prices_and_keeps_the_old_ones_when_the_read_fails() {
        let dir = std::env::temp_dir().join(format!("tsm-crafts-reload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("AppData.lua");
        std::fs::write(&path, appdata("PvP-Horde", 100, "6I")).unwrap();
        let mut s = App::new();
        s.land(market::read_book(&path).map(|(book, read)| Market::new(book, read, Vec::new(), "")));
        let seen = |s: &App| {
            let r = s.ready().unwrap();
            let book = &r.market.book;
            (book.realm().map(|(name, _)| name.to_owned()), book.realm().and_then(|(_, b)| b.updated), book.realm_price(4234).and_then(|p| p.min_buyout), r.reload_error.clone())
        };
        assert_eq!(seen(&s), (Some("PvP-Horde".into()), Some(100), Some(Copper(210)), None));

        std::fs::write(&path, appdata("PvE-Horde", 200, "7P")).unwrap();
        s.reload(&path);
        assert_eq!(seen(&s), (Some("PvE-Horde".into()), Some(200), Some(Copper(249)), None));

        let missing = dir.join("missing.lua");
        s.reload(&missing);
        let (scope, updated, price, error) = seen(&s);
        assert_eq!((scope, updated, price), (Some("PvE-Horde".into()), Some(200), Some(Copper(249))));
        assert!(error.is_some_and(|e| e.starts_with(&format!("reading {}: ", missing.display()))));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_minute_check_reloads_only_after_appdata_is_rewritten() {
        let dir = std::env::temp_dir().join(format!("tsm-crafts-watch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("AppData.lua");
        std::fs::write(&path, appdata("PvP-Horde", 100, "6I")).unwrap();
        let mut s = App::new();
        s.land(market::read_book(&path).map(|(book, read)| Market::new(book, read, Vec::new(), "")));
        let read_at = |s: &App| s.ready().unwrap().market.read.at;
        let first = read_at(&s);

        s.reload_if_changed(&path);
        assert_eq!(read_at(&s), first);

        std::fs::write(&path, appdata("PvE-Horde", 200, "7P")).unwrap();
        // The clock that stamps writes can tick slower than this test runs.
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(first + Duration::from_secs(5)).unwrap();
        s.reload_if_changed(&path);
        assert_eq!(s.ready().unwrap().market.book.realm().map(|(name, _)| name), Some("PvE-Horde"));
        assert_ne!(read_at(&s), first);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
