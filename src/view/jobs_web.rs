//! The loaders in the browser, each a task on the page's one thread. Data comes from the site
//! that serves the page, art and icons straight from wago.tools and Wowhead. Each posts what it
//! loaded to the outbox as a [`Msg`], as the desktop's threads do, and `drain` lands it.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::rc::Rc;
use std::thread::LocalKey;

use futures_util::future::{FutureExt, LocalBoxFuture, Shared, join_all, try_join, try_join_all};
use wasm_bindgen_futures::spawn_local;

use super::{Msg, Outbox, rail};
use crate::icons;
use crate::market::{Market, Origin, Read};
use crate::merchants::{self, Catalog, Side};
use crate::platform;
use crate::prices::PriceBook;
use crate::public_data::{self, PublicHouse, Refresh, SiteFile};
use crate::recipes::{self, FileDataId, IconTables, Profession, Recipe, Tables};
use crate::sheets::Sheets;
use crate::skin::{self, Art, Piece};

/// A load every caller shares, started by the first to ask.
type Loading<T> = Shared<LocalBoxFuture<'static, Result<Rc<T>, String>>>;

thread_local! {
    static TABLES: RefCell<Option<Loading<Tables>>> = const { RefCell::new(None) };
    static ICON_NAMES: RefCell<Option<Loading<HashMap<u32, String>>>> = const { RefCell::new(None) };
    /// The professions whose cards were fetched or are on their way.
    static CARDS: RefCell<Vec<Profession>> = const { RefCell::new(Vec::new()) };
    /// When public data is next due, in Unix seconds. The site keeps each file as long as TSM
    /// says, so the page asks again hourly, or in five minutes after a failure.
    static NEXT: Cell<u64> = const { Cell::new(0) };
}

/// Fetches a profession's cards the first time it is shown.
pub struct ArtQueue(Outbox);

impl ArtQueue {
    pub fn ask(&self, profession: Profession) {
        if !CARDS.with_borrow(|c| c.contains(&profession)) {
            art(&self.0, Piece::cards(profession).to_vec(), profession);
        }
    }
}

/// Starts the loaders that run once: the shared interface art with `first`'s cards, the item
/// sheets, and the rail's icons followed by the merchants.
pub fn start(out: &Outbox, first: Profession) -> ArtQueue {
    art(out, Piece::SHARED.into_iter().chain(Piece::cards(first)).collect(), first);
    for name in rail::icons() {
        out.send(Msg::Rail(name, PathBuf::from(icons::image_url(name))));
    }
    let sheets = out.clone();
    spawn_local(async move { sheets.send(Msg::Sheets(Box::new(load_sheets().await))) });
    let catalog = out.clone();
    spawn_local(async move {
        let loaded = load_catalog().await;
        let ids = loaded.as_ref().map(super::flips::icon_ids).unwrap_or_default();
        catalog.send(Msg::Catalog(Box::new(loaded)));
        show_icons(&catalog, &ids).await;
    });
    ArtQueue(out.clone())
}

/// TSM's prices with `profession`'s recipes, then their icons.
pub fn market(out: &Outbox, origin: Origin, realm: String, profession: Profession) {
    let out = out.clone();
    spawn_local(async move {
        let market = try_join(read_prices(&origin), load_recipes(profession)).await.map(|((book, read), recipes)| Market::new(book, read, recipes, &realm));
        let ids = market.as_ref().map(|m| icons::referenced(&m.recipes)).unwrap_or_default();
        out.send(Msg::Market(origin, profession, Box::new(market)));
        show_icons(&out, &ids).await;
    });
}

/// `profession`'s recipes, then their icons.
pub fn recipes(out: &Outbox, profession: Profession) {
    let out = out.clone();
    spawn_local(async move {
        let recipes = load_recipes(profession).await;
        let ids = recipes.as_deref().map(icons::referenced).unwrap_or_default();
        out.send(Msg::Recipes(profession, Box::new(recipes)));
        show_icons(&out, &ids).await;
    });
}

/// Prices read again from `origin`. The site decides when TSM is asked again, so every refresh
/// reads its copy.
pub fn prices(out: &Outbox, origin: Origin, _: Refresh) {
    let out = out.clone();
    spawn_local(async move {
        let prices = read_prices(&origin).await;
        out.send(Msg::Prices(origin, Box::new(prices)));
    });
}

pub fn due(_: PublicHouse, _: Side, now: u64) -> bool {
    now >= NEXT.get()
}

async fn read_prices(origin: &Origin) -> Result<(PriceBook, Read), String> {
    let &Origin::Public(house, side) = origin else { return Err("the browser reads TSM's public data only".into()) };
    let [realm, region] = public_data::site_paths(house, side);
    let read = match try_join(site_file(&realm), site_file(&region)).await {
        Ok((realm, region)) => public_data::from_site(house, side, realm, region, platform::unix_now()),
        Err(e) => Err(e),
    };
    NEXT.set(platform::unix_now() + if read.is_ok() { 3600 } else { 300 });
    read
}

async fn site_file(path: &str) -> Result<SiteFile, String> {
    let reply = platform::get(path).await?;
    let (fetched_at, stale) = (reply.header("x-craftski-fetched-at"), reply.header("x-craftski-stale"));
    Ok(SiteFile { status: reply.status, fetched_at, stale, body: reply.text() })
}

async fn load_recipes(profession: Profession) -> Result<Vec<Recipe>, String> {
    recipes::join(&*tables().await?, profession)
}

async fn load_catalog() -> Result<Catalog, String> {
    let (vendors, tables) = try_join(text("/data/foreverdb/vendors.json".into()), tables()).await?;
    merchants::catalog(&vendors, |wanted| crate::items::join(&tables.item_sparse, &tables.icons, wanted))
}

async fn load_sheets() -> Result<Sheets, String> {
    let [items, stats, meta, ratings] = texts(["items.json", "stats.json", "itemmeta.json", "ratings.json"].map(|f| format!("/data/foreverdb/{f}"))).await?;
    crate::sheets::parse(&items, &stats, &meta, &ratings)
}

/// The recipe tables, downloaded once for every profession.
async fn tables() -> Result<Rc<Tables>, String> {
    once(&TABLES, || {
        async {
            let names = ["SkillLineAbility", "SpellEffect", "SpellReagents", "ItemSparse", "Item", "ItemModifiedAppearance", "ItemAppearance"];
            let [skill_line_ability, spell_effect, spell_reagents, item_sparse, item, item_modified_appearance, item_appearance] = texts(names.map(|t| format!("/data/wago/{t}.csv"))).await?;
            Ok(Tables { skill_line_ability, spell_effect, spell_reagents, item_sparse, icons: IconTables { item, item_modified_appearance, item_appearance } })
        }
        .boxed_local()
    })
    .await
}

/// Shows each icon `ids` names that the site's list of icon names knows. One it lacks draws as
/// its plain frame, as a failed download does on desktop.
async fn show_icons(out: &Outbox, ids: &BTreeSet<FileDataId>) {
    let names = once(&ICON_NAMES, || {
        async {
            let names = text("/icon-names.json".into()).await?;
            serde_json::from_str(&names).map_err(|e| format!("/icon-names.json: {e}"))
        }
        .boxed_local()
    });
    match names.await {
        Ok(names) => {
            for &id in ids {
                if let Some(name) = names.get(&id.0) {
                    out.send(Msg::Icon(id, PathBuf::from(icons::image_url(name))));
                }
            }
        }
        Err(e) => warn(&format!("icons not shown: {e}")),
    }
}

/// `pieces` cut from their atlases, each atlas fetched once and all at the same time. A
/// profession whose art failed is fetched again the next time it is shown.
fn art(out: &Outbox, pieces: Vec<Piece>, profession: Profession) {
    CARDS.with_borrow_mut(|c| c.push(profession));
    let out = out.clone();
    spawn_local(async move {
        let mut atlases: Vec<u32> = pieces.iter().map(|p| p.cut().atlas).collect();
        atlases.sort_unstable();
        atlases.dedup();
        let pieces = &pieces;
        let cut = join_all(atlases.into_iter().map(|atlas| async move { skin::cut_atlas(atlas, &fetch(&skin::atlas_url(atlas)).await?, pieces) })).await;
        let mut failed = false;
        for result in cut {
            match result {
                Ok(cut) => cut.into_iter().for_each(|(piece, rgba)| out.send(Msg::Skin(piece, Art::Rgba(rgba)))),
                Err(e) => {
                    warn(&format!("{} art not fetched: {e}", profession.name()));
                    failed = true;
                }
            }
        }
        if failed {
            CARDS.with_borrow_mut(|c| c.retain(|&p| p != profession));
        }
    });
}

/// What `slot` holds, loading it with `load` when it holds nothing. A failed load empties the
/// slot so the next caller tries again.
async fn once<T: 'static>(slot: &'static LocalKey<RefCell<Option<Loading<T>>>>, load: impl FnOnce() -> LocalBoxFuture<'static, Result<T, String>>) -> Result<Rc<T>, String> {
    let loading = slot.with_borrow_mut(|s| s.get_or_insert_with(|| load().map(|r| r.map(Rc::new)).boxed_local().shared()).clone());
    let result = loading.clone().await;
    if result.is_err() {
        slot.with_borrow_mut(|s| s.take_if(|l| l.ptr_eq(&loading)));
    }
    result
}

async fn texts<const N: usize>(urls: [String; N]) -> Result<[String; N], String> {
    let texts = try_join_all(urls.map(text)).await?;
    Ok(texts.try_into().unwrap_or_else(|_| unreachable!("one text per url")))
}

async fn text(url: String) -> Result<String, String> {
    fetch(&url).await.map(|body| String::from_utf8_lossy(&body).into_owned())
}

/// `url`'s body, or why there is none.
async fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let reply = platform::get(url).await?;
    match reply.status {
        200 => Ok(reply.body),
        status => Err(format!("{url}: HTTP {status}")),
    }
}

fn warn(what: &str) {
    web_sys::console::warn_1(&what.into());
}
