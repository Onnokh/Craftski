mod detail;
mod flips;
mod folder;
mod header;
mod menu;
mod rail;
mod skin;
mod status;
mod table;
mod toolbar;
mod tooltip;
mod widgets;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rune::{Align, Cx, El, Image, ImageCache, Key, KeyEvent, TextEdit, Tx, Waker, col, row, text};

use crate::icons;
use crate::market::{self, Market};
use crate::menu::{Menu, SOLD_STEPS};
use crate::merchants::{self, Catalog};
use crate::prices;
use crate::profit::{self, Evaluation, PriceOrigin};
use crate::recipes::{self, FileDataId, Profession, Recipe};
use crate::rows::{self, Column, Filter, FilterKind, Hidden, Sort};
use crate::settings;
use crate::sheets::Sheets;
use crate::skin::Piece;
use crate::theme::{self, p};
use crate::tooltip::Tip;
use crate::wow::{self, AppData};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Crafts,
    Flips,
}

pub struct App {
    tab: Tab,
    /// The AppData.lua prices come from, resolved once at start and on each folder the player
    /// picks.
    appdata: Option<AppData>,
    /// The folders looked in at start, shown while no AppData.lua was found.
    searched: Vec<PathBuf>,
    /// Why the last folder picked gave no AppData.lua.
    folder_note: Option<String>,
    load: Load,
    /// The loader threads' messages.
    inbox: Option<Receiver<Msg>>,
    /// Where loader threads post, once the window is set up.
    outbox: Option<Outbox>,
    /// Asks the art thread for a profession's cards ahead of the rest.
    art_asks: Option<Sender<Profession>>,
    /// The profession the crafts tab shows.
    profession: Profession,
    icons: Icons,
    hide_unpriced: bool,
    min_sold: usize,
    sort: Sort<Column>,
    /// The recipe the detail panel shows, by spell id, kept while filters hide its row.
    selected: Option<u32>,
    search: TextEdit,
    /// The player's rank in the shown profession as typed; blank shows every rank.
    skill: TextEdit,
    flips: flips::State,
    menu: Option<Menu>,
    /// What the pointer rests on that opens a tooltip.
    tip: Option<Tip>,
    /// foreverdb's item data, which tooltips read once it loads.
    sheets: Option<Sheets>,
    /// Where the pointer last moved over something with a tooltip, in window points.
    pointer: (f32, f32),
    /// The realm the player last chose, which prices come from whenever AppData.lua holds it.
    realm: String,
    /// Unix seconds at the last minute tick: the clock the status bar's ages read from.
    now: u64,
}

enum Load {
    Loading,
    /// No AppData.lua to read: none was found, or the one named is missing.
    Unlocated,
    Failed(String),
    Ready(Box<Ready>),
}

enum Msg {
    /// Prices with the recipes of the profession named, which the player may have left since.
    Market(Profession, Box<Result<Market, String>>),
    Recipes(Profession, Box<Result<Vec<Recipe>, String>>),
    Catalog(Box<Result<Catalog, String>>),
    Sheets(Box<Result<Sheets, String>>),
    Icon(FileDataId, PathBuf),
    Rail(&'static str, PathBuf),
    Skin(Piece, PathBuf),
}

#[derive(Clone)]
struct Outbox {
    tx: Sender<Msg>,
    wake: Waker,
}

impl Outbox {
    fn send(&self, m: Msg) {
        let _ = self.tx.send(m);
        (self.wake)();
    }
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

    fn show_rail(&mut self, name: &'static str, path: &Path) {
        if let Some(cache) = &self.cache {
            self.rail.insert(name, cache.file(path, 32.0, 2.0));
        }
    }

    fn item(&self, id: Option<FileDataId>) -> Option<&Image> {
        self.items.get(&id?)
    }
}

struct Ready {
    market: Market,
    recipes: Recipes,
    evals: Vec<Evaluation>,
    rows: Vec<usize>,
    hidden: Option<Hidden<FilterKind>>,
    /// Why the last reload failed; the prices before it stay.
    reload_error: Option<String>,
}

/// Whether the market holds the shown profession's recipes.
#[derive(Debug, PartialEq)]
enum Recipes {
    Shown,
    /// Loading on a background thread; the market holds none meanwhile.
    Loading,
    Failed(String),
}

/// Sizes that change between a roomy window and a narrow one.
#[derive(Clone, Copy)]
struct Dims {
    compact: bool,
    pad: f32,
    /// The table's margin from the rail and the detail panel. Its cells pad by the rest of
    /// `pad`, so their text lines up with the title and toolbar above.
    inset: f32,
    detail: f32,
    gap: f32,
    /// The status bar's width at the last layout, which decides what it leaves out.
    status: Option<f32>,
}

impl Dims {
    fn new(compact: bool) -> Dims {
        if compact {
            Dims { compact, pad: 16.0, inset: 8.0, detail: 360.0, gap: 16.0, status: None }
        } else {
            Dims { compact, pad: 24.0, inset: 12.0, detail: 400.0, gap: 24.0, status: None }
        }
    }

    /// A table cell's side padding, inside the table's inset.
    fn cell_pad(self) -> f32 {
        self.pad - self.inset
    }
}

impl App {
    pub fn new() -> App {
        let profession = settings::start_profession().unwrap_or(Profession::Leatherworking);
        let appdata = wow::locate();
        let readable = appdata.as_ref().is_some_and(|a| a.path.is_file());
        App {
            tab: Tab::Crafts,
            searched: if appdata.is_none() { wow::install_dirs() } else { Vec::new() },
            appdata,
            folder_note: None,
            load: if readable { Load::Loading } else { Load::Unlocated },
            inbox: None,
            outbox: None,
            art_asks: None,
            profession,
            icons: Icons::default(),
            hide_unpriced: true,
            min_sold: 0,
            sort: Sort { column: Column::Profit, descending: true },
            selected: None,
            search: TextEdit::new(""),
            skill: TextEdit::new(settings::load_skill(profession)),
            flips: flips::State::new(),
            menu: None,
            tip: None,
            sheets: None,
            pointer: (0.0, 0.0),
            realm: settings::load_realm(),
            now: unix_secs(SystemTime::now()),
        }
    }

    /// Lands the first load, made for `profession`, and prices the flips should the merchants
    /// have landed first.
    fn land(&mut self, profession: Profession, result: Result<Market, String>) {
        self.load = match result {
            Ok(market) => Load::Ready(Box::new(Ready { market, recipes: Recipes::Shown, evals: Vec::new(), rows: Vec::new(), hidden: None, reload_error: None })),
            Err(e) => Load::Failed(e),
        };
        self.reprice_flips();
        if profession != self.profession {
            self.show_profession(self.profession);
            return;
        }
        self.refresh();
        self.select_first();
    }

    /// Reads TSM's prices with the shown profession's recipes on a loader thread, or asks for
    /// the folder when there is no AppData.lua to read.
    fn load_market(&mut self) {
        let Some(appdata) = self.appdata.as_ref().map(|a| a.path.clone()).filter(|p| p.is_file()) else {
            self.load = Load::Unlocated;
            return;
        };
        self.load = Load::Loading;
        let Some(out) = self.outbox.clone() else { return };
        let (realm, profession) = (self.realm.clone(), self.profession);
        std::thread::spawn(move || {
            let market = settled(|| market::load(&appdata, &realm, profession));
            let ids = market.as_ref().map(|m| icons::referenced(&m.recipes)).unwrap_or_default();
            out.send(Msg::Market(profession, Box::new(market)));
            prefetch(&ids, |id, path| out.send(Msg::Icon(id, path)));
        });
    }

    fn appdata_path(&self) -> Option<PathBuf> {
        self.appdata.as_ref().map(|a| a.path.clone())
    }

    fn select_first(&mut self) {
        self.selected = self.ready().and_then(|r| r.rows.first().map(|&i| r.market.recipes[i].spell_id));
    }

    /// The rail's press: the crafts tab with `profession`'s recipes, remembered for next time.
    /// Pressing the shown profession again retries recipes that failed to load.
    pub(super) fn choose_profession(&mut self, profession: Profession) {
        self.tab = Tab::Crafts;
        let failed = self.ready().is_some_and(|r| matches!(r.recipes, Recipes::Failed(_)));
        if profession == self.profession && !failed {
            return;
        }
        settings::save_profession(profession);
        self.skill = TextEdit::new(settings::load_skill(profession));
        self.show_profession(profession);
    }

    /// Drops the shown recipes and loads `profession`'s in the background, keeping the search
    /// and filters. The selection moves to the first row once they land.
    fn show_profession(&mut self, profession: Profession) {
        self.profession = profession;
        self.selected = None;
        if let Some(ask) = &self.art_asks {
            let _ = ask.send(profession);
        }
        let Load::Ready(r) = &mut self.load else { return };
        r.market.set_recipes(Vec::new());
        r.recipes = Recipes::Loading;
        self.refresh();
        let Some(out) = self.outbox.clone() else { return };
        std::thread::spawn(move || {
            let recipes = settled(|| recipes::load(profession));
            let ids = recipes.as_deref().map(icons::referenced).unwrap_or_default();
            out.send(Msg::Recipes(profession, Box::new(recipes)));
            prefetch(&ids, |id, path| out.send(Msg::Icon(id, path)));
        });
    }

    /// Lands recipes loaded for `profession`, unless the player has moved on from it or they
    /// already landed.
    fn land_recipes(&mut self, profession: Profession, result: Result<Vec<Recipe>, String>) {
        let Load::Ready(r) = &mut self.load else { return };
        if profession != self.profession || r.recipes != Recipes::Loading {
            return;
        }
        match result {
            Ok(recipes) => {
                r.market.set_recipes(recipes);
                r.recipes = Recipes::Shown;
            }
            Err(e) => r.recipes = Recipes::Failed(e),
        }
        self.refresh();
        self.select_first();
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
                    settings::save_skill(self.profession, "");
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
    if !matches!(s.load, Load::Loading | Load::Unlocated) || s.inbox.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    s.inbox = Some(rx);
    let out = Outbox { tx, wake };
    s.outbox = Some(out.clone());
    let (ask, asked) = mpsc::channel();
    s.art_asks = Some(ask);
    let (art, first) = (out.clone(), s.profession);
    std::thread::spawn(move || crate::skin::fetch_in_order(first, &asked, |pieces| crate::skin::fetch(pieces, |piece, path| art.send(Msg::Skin(piece, path)))));
    let sheets = out.clone();
    std::thread::spawn(move || sheets.send(Msg::Sheets(Box::new(settled(crate::sheets::load)))));
    s.load_market();
    std::thread::spawn(move || {
        fetch_rail(|name, path| out.send(Msg::Rail(name, path)));
        let catalog = merchants::load();
        let ids = catalog.as_ref().map(flips::icon_ids).unwrap_or_default();
        out.send(Msg::Catalog(Box::new(catalog)));
        prefetch(&ids, |id, path| out.send(Msg::Icon(id, path)));
    });
}

/// `load`'s result, or an error should it panic, so a loading state always ends.
fn settled<T>(load: impl FnOnce() -> Result<T, String> + std::panic::UnwindSafe) -> Result<T, String> {
    std::panic::catch_unwind(load).unwrap_or_else(|_| Err("the loader stopped without a result".into()))
}

fn prefetch(ids: &BTreeSet<FileDataId>, on_ready: impl Fn(FileDataId, PathBuf) + Sync) {
    match icons::prefetch(ids, on_ready) {
        Ok(report) if !report.failed.is_empty() => eprintln!("{} of {} icons failed, first: {}", report.failed.len(), report.icons, report.failed[0].1),
        Ok(_) => {}
        Err(e) => eprintln!("icons not fetched: {e}"),
    }
}

fn fetch_rail(on_ready: impl Fn(&'static str, PathBuf)) {
    let names: Vec<&'static str> = rail::icons().collect();
    for ((name, result), icon) in icons::fetch_named(&names).into_iter().zip(names.iter().copied()) {
        match result {
            Ok(path) => on_ready(icon, path),
            Err(e) => eprintln!("rail icon {name}: {e}"),
        }
    }
}

/// Lands the loader's messages, and on each new minute moves the clock the ages read from and
/// reloads when TSM has rewritten AppData.lua.
pub fn poll(s: &mut App, _: f64, tx: &mut Tx) -> bool {
    let landed = drain(s) | skin::newly_ready();
    let now = unix_secs(SystemTime::now());
    tx.wake_after(Duration::from_secs(60 - now % 60));
    let ticked = now / 60 != s.now / 60;
    if ticked {
        s.now = now;
        if let Some(path) = s.appdata_path() {
            s.reload_if_changed(&path);
        }
    }
    landed || ticked
}

/// Takes the loader threads' messages; true when any arrived.
fn drain(s: &mut App) -> bool {
    let Some(rx) = s.inbox.take() else { return false };
    let mut changed = false;
    for m in rx.try_iter() {
        changed = true;
        match m {
            Msg::Market(profession, result) => s.land(profession, *result),
            Msg::Recipes(profession, result) => s.land_recipes(profession, *result),
            Msg::Catalog(result) => s.land_catalog(*result),
            Msg::Sheets(result) => match *result {
                Ok(sheets) => s.sheets = Some(sheets),
                Err(e) => eprintln!("item tooltips: {e}"),
            },
            Msg::Icon(id, path) => s.icons.show(id, &path),
            Msg::Rail(name, path) => s.icons.show_rail(name, &path),
            Msg::Skin(piece, path) => {
                if let Some(cache) = &s.icons.cache {
                    skin::show(cache, piece, &path);
                }
            }
        }
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
        Load::Unlocated => folder::not_found(s),
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
                    .child(header::banner(Some(Piece::OverviewCard(s.profession)), header::header(s.profession, r, d), toolbar::toolbar(s, d)))
                    .child(table::table(s, r, d)),
            )
            .child(detail::detail(s, r, d)),
    };
    let root = col()
        .key("root")
        .w_full()
        .h_full()
        .fill(p().ground)
        .children(skin::cover(Piece::Background, (Align::Center, Align::Center)))
        .child(row().w_full().grow(1.0).min_h(0.0).child(rail::rail(s)).child(main))
        .children(s.ready().map(|r| status::status(s, r, d)));
    menu::overlay(s).into_iter().fold(root, |r, e| r.child(e)).children(tooltip::overlay(s))
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

/// The profession `--profession` names, else the one last shown, with every icon and piece of
/// art it draws fetched and decoded, so the snapshot shows them.
pub fn scene_loaded(s: &mut App) {
    s.sheets = crate::sheets::load().inspect_err(|e| eprintln!("item tooltips: {e}")).ok();
    land_now(s);
    let Load::Ready(r) = &s.load else { return };
    show_icons(s, icons::referenced(&r.market.recipes));
}

/// Prices read on this thread from the AppData.lua located, as the loader thread reads them.
fn land_now(s: &mut App) {
    let market = s.appdata_path().ok_or_else(|| "no AppData.lua found".to_string()).and_then(|path| market::load(&path, &s.realm, s.profession));
    s.land(s.profession, market);
}

/// A first launch where no AppData.lua was found, after picking a folder without one.
pub fn scene_no_appdata(s: &mut App) {
    (s.appdata, s.searched, s.load) = (None, wow::install_dirs(), Load::Unlocated);
    s.picked_folder(Ok(Some(PathBuf::from(r"D:\Games\Warcraft"))));
    show_icons(s, BTreeSet::new());
    // An inbox keeps `setup` from starting loaders, which would land data after the snapshot.
    s.inbox = Some(mpsc::channel().1);
}

/// The flips tab in Thunder Bluff, priced as the window would price it.
pub fn scene_flips(s: &mut App) {
    s.sheets = crate::sheets::load().inspect_err(|e| eprintln!("item tooltips: {e}")).ok();
    s.tab = Tab::Flips;
    land_now(s);
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
    fetch_rail(|name, path| rail.borrow_mut().push((name, path)));
    for (name, path) in rail.into_inner() {
        s.icons.show_rail(name, &path);
    }
    let (tx, rx) = mpsc::channel();
    let pieces: Vec<Piece> = Piece::SHARED.into_iter().chain(Piece::cards(s.profession)).collect();
    if let Err(e) = crate::skin::fetch(&pieces, |piece, path| {
        let _ = tx.send((piece, path));
    }) {
        eprintln!("interface art not fetched: {e}");
    }
    drop(tx);
    if let Some(cache) = &s.icons.cache {
        for (piece, path) in rx {
            skin::show(cache, piece, &path);
        }
    }
    for image in s.icons.items.values().chain(s.icons.rail.values()).chain(coins) {
        let _ = image.wait();
    }
    skin::wait();
    skin::newly_ready();
}

/// `loaded` with Sold 5+ a day, so the line under the table says what it hides.
pub fn scene_crafts_hidden(s: &mut App) {
    scene_loaded(s);
    s.min_sold = 3;
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
    use crate::recipes::Quality;
    use crate::menu::Pick;
    use crate::flip_rows::Supply;

    fn appdata(scope: &str, time: u64, min_buyout: &str) -> String {
        format!(r#"x.LoadData("AUCTIONDB_NON_COMMODITY_DATA","{scope}",[[return {{downloadTime={time},fields={{"itemString","minBuyout"}},data={{{{4234,{min_buyout}}}}}}}]])"#)
    }

    #[test]
    fn a_folder_without_apphelper_data_says_why_and_a_cancel_changes_nothing() {
        let dir = std::env::temp_dir().join(format!("tsm-crafts-pick-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = App::new();
        (s.appdata, s.load) = (None, Load::Unlocated);
        let why = format!("No TSM AppHelper data under {}. Pick the folder that holds _forever_, and make sure the TSM desktop app has synced once.", dir.display());
        s.picked_folder(Ok(Some(dir.clone())));
        assert_eq!((s.folder_note.as_deref(), s.appdata.is_none(), matches!(s.load, Load::Unlocated)), (Some(why.as_str()), true, true));
        s.picked_folder(Ok(None));
        assert_eq!(s.folder_note.as_deref(), Some(why.as_str()));
        std::fs::remove_dir_all(&dir).unwrap();
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
        s.pick(Pick::CraftsSold(3));
        assert_eq!((s.menu, s.min_sold), (None, 3));
    }

    #[test]
    fn the_flips_menus_set_the_flips_filters_and_leave_the_recipes_alone() {
        let mut s = App::new();
        s.toggle_menu(Menu::FlipsSold);
        s.pick(Pick::FlipsSold(4));
        assert_eq!((s.menu, s.flips.min_sold(), s.min_sold), (None, 4, 0));
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
        s.land(s.profession, market::read_book(&path).map(|(book, read)| Market::new(book, read, Vec::new(), &s.realm)));
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
    fn a_switch_drops_the_old_recipes_lands_only_the_new_professions_and_keeps_the_search() {
        let recipe = |spell_id, name: &str| Recipe { spell_id, item_id: spell_id, item_name: name.into(), quality: Quality::Common, slot: None, icon: None, quantity: 1.0, skill: None, reagents: vec![] };
        let market = |recipes| Market::new(prices::PriceBook::default(), market::Read { at: UNIX_EPOCH, modified: None }, recipes, "");
        let seen = |s: &App| {
            let r = s.ready().unwrap();
            let rows: Vec<&str> = r.rows.iter().map(|&i| r.market.recipes[i].item_name.as_str()).collect();
            (s.profession, format!("{:?}", r.recipes), rows.join(", "), s.selected, s.search.text.clone())
        };
        let mut s = App::new();
        (s.profession, s.hide_unpriced) = (Profession::Leatherworking, false);
        s.land(Profession::Leatherworking, Ok(market(vec![recipe(1, "Light Armor Kit"), recipe(2, "Linen Bag")])));
        s.search = TextEdit::new("bag");
        s.refresh();
        assert_eq!(seen(&s), (Profession::Leatherworking, "Shown".into(), "Linen Bag".into(), Some(1), "bag".into()));

        s.show_profession(Profession::Tailoring);
        assert_eq!(seen(&s), (Profession::Tailoring, "Loading".into(), "".into(), None, "bag".into()));
        s.land_recipes(Profession::Leatherworking, Ok(vec![recipe(1, "Light Armor Kit")]));
        assert_eq!(seen(&s), (Profession::Tailoring, "Loading".into(), "".into(), None, "bag".into()));
        s.land_recipes(Profession::Tailoring, Ok(vec![recipe(3, "Bolt of Linen Cloth"), recipe(4, "Red Linen Bag")]));
        assert_eq!(seen(&s), (Profession::Tailoring, "Shown".into(), "Red Linen Bag".into(), Some(4), "bag".into()));
        s.land_recipes(Profession::Tailoring, Ok(vec![]));
        assert_eq!(seen(&s).2, "Red Linen Bag");

        s.show_profession(Profession::Cooking);
        s.land_recipes(Profession::Cooking, Err("offline".into()));
        assert_eq!(seen(&s), (Profession::Cooking, "Failed(\"offline\")".into(), "".into(), None, "bag".into()));
    }

    #[test]
    fn a_first_load_for_a_profession_since_left_keeps_its_prices_and_loads_the_new_one() {
        let mut s = App::new();
        s.profession = Profession::Mining;
        let recipe = Recipe { spell_id: 1, item_id: 1, item_name: "Light Armor Kit".into(), quality: Quality::Common, slot: None, icon: None, quantity: 1.0, skill: None, reagents: vec![] };
        s.land(Profession::Leatherworking, Ok(Market::new(prices::PriceBook::default(), market::Read { at: UNIX_EPOCH, modified: None }, vec![recipe], "")));
        let r = s.ready().unwrap();
        assert_eq!((r.recipes == Recipes::Loading, r.market.recipes.len(), s.selected), (true, 0, None));
    }

    #[test]
    fn reload_takes_the_new_prices_and_keeps_the_old_ones_when_the_read_fails() {
        let dir = std::env::temp_dir().join(format!("tsm-crafts-reload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("AppData.lua");
        std::fs::write(&path, appdata("PvP-Horde", 100, "6I")).unwrap();
        let mut s = App::new();
        s.land(s.profession, market::read_book(&path).map(|(book, read)| Market::new(book, read, Vec::new(), "")));
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
        s.land(s.profession, market::read_book(&path).map(|(book, read)| Market::new(book, read, Vec::new(), "")));
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
