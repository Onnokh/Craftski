//! The states the desktop build renders headless with `--scene NAME --snap PATH`.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::profit::PriceOrigin;
use crate::public_data::PublicHouse;
use crate::{icons, merchants, prices};

/// The profession `--profession` names, else the one last shown, with every icon and piece of
/// art it draws fetched and decoded, so the snapshot shows them.
pub fn scene_loaded(s: &mut App) {
    s.sheets = crate::sheets::load().inspect_err(|e| eprintln!("item tooltips: {e}")).ok();
    land_now(s);
    let Load::Ready(r) = &s.load else { return };
    show_icons(s, icons::referenced(&r.market.recipes));
}

/// Prices read on this thread from where they come from, as the loader thread reads them, but
/// public data from the cache alone.
fn land_now(s: &mut App) {
    let market = s.origin().ok_or_else(|| "no AppData.lua found".to_string()).and_then(|origin| market::load(&origin, Refresh::CacheOnly, &s.realm, s.profession));
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

/// The settings tab over a loaded catalog, its reputations listed for the saved faction.
pub fn scene_settings(s: &mut App) {
    scene_flips(s);
    s.tab = Tab::Settings;
}

/// `loaded` priced from TSM's cached public data for EU PvP, the saved faction's house.
pub fn scene_public(s: &mut App) {
    s.source = PriceSource::Public(PublicHouse::default());
    scene_loaded(s);
}

/// `settings` with TSM's cached public data for EU PvP chosen.
pub fn scene_settings_public(s: &mut App) {
    s.source = PriceSource::Public(PublicHouse::default());
    scene_settings(s);
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
    jobs::prefetch(&ids, |id, path| {
        let _ = tx.send((id, path));
    });
    drop(tx);
    s.icons.cache = Some(ImageCache::new(Arc::new(|| {})));
    for (id, path) in rx {
        s.icons.show(id, &path);
    }
    let rail = std::cell::RefCell::new(Vec::new());
    jobs::fetch_rail(|name, path| rail.borrow_mut().push((name, path)));
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
            skin::show(cache, piece, crate::skin::Art::File(path));
        }
    }
    for image in s.icons.items.values().chain(s.icons.rail.values()) {
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
