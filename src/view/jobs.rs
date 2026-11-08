//! The loaders that run beside the window, each on a thread of its own. Each posts what it
//! loaded to the outbox as a [`Msg`], and `drain` lands it.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};

use super::{Msg, Outbox, rail};
use crate::icons;
use crate::market::{self, Origin};
use crate::public_data::Refresh;
use crate::recipes::{self, FileDataId, Profession};

/// Asks the art loader for a profession's cards ahead of the rest.
pub struct Art(Sender<Profession>);

impl Art {
    pub fn ask(&self, profession: Profession) {
        let _ = self.0.send(profession);
    }
}

/// Starts the loaders that run once: the interface art, `first`'s cards first, the item
/// sheets, and the rail's icons followed by the merchants.
pub fn start(out: &Outbox, first: Profession) -> Art {
    let (ask, asked) = mpsc::channel();
    let art = out.clone();
    std::thread::spawn(move || crate::skin::fetch_in_order(first, &asked, |pieces| crate::skin::fetch(pieces, |piece, path| art.send(Msg::Skin(piece, path)))));
    let sheets = out.clone();
    std::thread::spawn(move || sheets.send(Msg::Sheets(Box::new(settled(crate::sheets::load)))));
    let out = out.clone();
    std::thread::spawn(move || {
        fetch_rail(|name, path| out.send(Msg::Rail(name, path)));
        let catalog = crate::merchants::load();
        let ids = catalog.as_ref().map(super::flips::icon_ids).unwrap_or_default();
        out.send(Msg::Catalog(Box::new(catalog)));
        prefetch(&ids, |id, path| out.send(Msg::Icon(id, path)));
    });
    Art(ask)
}

/// TSM's prices with `profession`'s recipes, then their icons.
pub fn market(out: &Outbox, origin: Origin, realm: String, profession: Profession) {
    let out = out.clone();
    std::thread::spawn(move || {
        let market = settled(|| market::load(&origin, Refresh::IfDue, &realm, profession));
        let ids = market.as_ref().map(|m| icons::referenced(&m.recipes)).unwrap_or_default();
        out.send(Msg::Market(origin, profession, Box::new(market)));
        prefetch(&ids, |id, path| out.send(Msg::Icon(id, path)));
    });
}

/// `profession`'s recipes, then their icons.
pub fn recipes(out: &Outbox, profession: Profession) {
    let out = out.clone();
    std::thread::spawn(move || {
        let recipes = settled(|| recipes::load(profession));
        let ids = recipes.as_deref().map(icons::referenced).unwrap_or_default();
        out.send(Msg::Recipes(profession, Box::new(recipes)));
        prefetch(&ids, |id, path| out.send(Msg::Icon(id, path)));
    });
}

/// Prices read again from `origin`.
pub fn prices(out: &Outbox, origin: Origin, refresh: Refresh) {
    let out = out.clone();
    std::thread::spawn(move || {
        let prices = settled(|| market::read_prices(&origin, refresh));
        out.send(Msg::Prices(origin, Box::new(prices)));
    });
}

/// `load`'s result, or an error should it panic, so a loading state always ends.
fn settled<T>(load: impl FnOnce() -> Result<T, String> + std::panic::UnwindSafe) -> Result<T, String> {
    std::panic::catch_unwind(load).unwrap_or_else(|_| Err("the loader stopped without a result".into()))
}

pub fn prefetch(ids: &BTreeSet<FileDataId>, on_ready: impl Fn(FileDataId, PathBuf) + Sync) {
    match icons::prefetch(ids, on_ready) {
        Ok(report) if !report.failed.is_empty() => eprintln!("{} of {} icons failed, first: {}", report.failed.len(), report.icons, report.failed[0].1),
        Ok(_) => {}
        Err(e) => eprintln!("icons not fetched: {e}"),
    }
}

pub fn fetch_rail(on_ready: impl Fn(&'static str, PathBuf)) {
    let names: Vec<&'static str> = rail::icons().collect();
    for ((name, result), icon) in icons::fetch_named(&names).into_iter().zip(names.iter().copied()) {
        match result {
            Ok(path) => on_ready(icon, path),
            Err(e) => eprintln!("rail icon {name}: {e}"),
        }
    }
}
