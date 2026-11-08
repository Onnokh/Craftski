//! The settings tab: where prices come from, and the character the flips are priced for.

use std::collections::BTreeSet;

use rune::{El, ScrollSpec, col, row, text};

use super::detail::band;
use super::header::page;
use super::widgets::checkbox;
use super::{App, Dims, Load, folder};
use crate::flips::{self, Bartering, Rules};
use crate::market::PriceSource;
use crate::merchants::Side;
use crate::overview::Feed;
use crate::public_data::{ForeverRealm, PublicHouse, Region};
use crate::settings;
use crate::theme::{self, p};

pub fn main(s: &App, d: Dims) -> El {
    let r = &s.rules;
    let faction = Side::ALL.into_iter().map(|side| choice(side.name(), r.side == side, move |r| r.side = side));
    let bartering = Bartering::ALL.into_iter().map(|b| choice(&b.label(), r.bartering == b, move |r| r.bartering = b));
    let body = col()
        .w_full()
        .max_w(720.0)
        .child(band("Prices"))
        .child(prices(s))
        .child(band("Character").mt(32.0))
        .child(setting("Faction", row().gap(8.0).children(faction)))
        .child(setting("Honored or better with", honored(s)))
        .child(setting("Bartering", row().gap(8.0).children(bartering)))
        .child(caption("Honored takes 10% off the merchants of that faction, and Bartering its percent off every merchant, the two added together.").mt(16.0));
    #[cfg(target_arch = "wasm32")]
    let body = body.child(band("About").mt(32.0)).child(about());
    col()
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .child(page("Settings", "Where prices come from, and who buys from merchants.".into(), d))
        .child(col().key("settings-body").w_full().grow(1.0).min_h(0.0).px(d.pad).pb(24.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
}

/// The two sources: the desktop app with the AppData.lua it reads, or TSM's public data with
/// the region and realm whose auction house it reads.
fn prices(s: &App) -> El {
    let house = match s.source {
        PriceSource::App => None,
        PriceSource::Public(house) => Some(house),
    };
    let sources = row()
        .gap(8.0)
        .flex_wrap()
        .child(radio("TSM desktop app", house.is_none(), |s| s.set_source(PriceSource::App)))
        .child(radio("TSM public data, no TSM needed", house.is_some(), |s| s.set_source(PriceSource::Public(settings::load_house()))));
    // The browser reads public data only.
    let out = if cfg!(target_arch = "wasm32") { col().w_full() } else { col().w_full().child(sources.mt(12.0)) };
    let Some(house) = house else {
        return out.child(caption("Reads the prices the TSM desktop app saves for WoW Forever in AppData.lua.").mt(16.0)).child(folder::block(s, false).mt(16.0));
    };
    let pick = move |house: PublicHouse| move |s: &mut App| s.set_source(PriceSource::Public(house));
    let regions = Region::ALL.map(|region| radio(region.label(), house.region == region, pick(PublicHouse { region, ..house })));
    let realms = ForeverRealm::ALL.map(|realm| radio(realm.name(), house.realm == realm, pick(PublicHouse { realm, ..house })));
    out.child(setting("Region", row().gap(8.0).children(regions)))
        .child(setting("Realm", row().gap(8.0).flex_wrap().children(realms)))
        .child(house_caption(s, house).mt(16.0))
}

/// The auction house in use, and how old its prices are once they load, or why they are older
/// than they should be.
fn house_caption(s: &App, house: PublicHouse) -> El {
    let scope = house.scope(s.rules.side);
    let mut out = col().w_full().gap(4.0).child(caption(&format!("Auction house {scope}, from your faction below.")));
    let shown = s.ready().filter(|r| r.market.book.realm().is_some_and(|(name, _)| name == scope) && r.market.book.region_scope == Some(house.region.name()));
    match (&s.load, shown) {
        (_, Some(r)) => {
            let (realm, region) = (Feed::realm(&r.market.book, s.now), Feed::region(&r.market.book, s.now));
            if let (Some((realm, _)), Some((region, _))) = (realm.age, region.age) {
                out = out.child(caption(&format!("Its prices are from {realm}, the region's from {region}.")));
            }
            if let Some(warning) = &r.reload_error {
                out = out.child(text(warning).font(theme::CAPTION).color(p().loss).wrap());
            }
        }
        (Load::Failed(e), None) => out = out.child(text(e).font(theme::CAPTION).color(p().loss).wrap()),
        _ => {}
    }
    out
}

/// The page's place beside Blizzard and TSM, and the desktop app.
#[cfg(target_arch = "wasm32")]
fn about() -> El {
    const DESKTOP_APP: &str = "https://github.com/Onnokh/tsm-crafts/releases/latest";
    col()
        .w_full()
        .gap(12.0)
        .mt(16.0)
        .child(caption("Not affiliated with Blizzard Entertainment or TradeSkillMaster. Prices are TSM's public data. Item art and icons load from wago.tools and Wowhead."))
        .child(caption("The desktop app for Windows also reads the prices your own TSM desktop app saves."))
        .child(row().child(super::widgets::link::<App>("Desktop app", "Download the desktop app", DESKTOP_APP.into())))
}

fn radio(label: &str, on: bool, pick: impl Fn(&mut App) + 'static) -> El {
    checkbox(label, on, pick).role("radio")
}

/// A checkbox per faction whose merchants serve this side, and per one saved but not among
/// them. Before the merchants load, the saved ones alone.
fn honored(s: &App) -> El {
    let r = &s.rules;
    let merchants = s.flips.merchants();
    let mut names: BTreeSet<&str> = merchants.map(|m| flips::reputations(m, r.side)).unwrap_or_default();
    names.extend(r.honored.iter().map(String::as_str));
    let boxes = names.into_iter().map(|name| {
        let faction = name.to_string();
        checkbox(name, r.honored.iter().any(|h| h == name), move |s: &mut App| s.set_rules(|r| r.toggle_honored(&faction)))
    });
    let out = row().w_full().gap(8.0).flex_wrap().children(boxes);
    if merchants.is_some() { out } else { col().w_full().gap(8.0).child(out).child(caption("The rest show once the merchants load.")) }
}

fn setting(label: &str, control: El) -> El {
    col().w_full().gap(8.0).mt(16.0).child(text(label).font(theme::LABEL).color(p().muted)).child(control)
}

fn choice(label: &str, on: bool, pick: impl Fn(&mut Rules) + Copy + 'static) -> El {
    radio(label, on, move |s| s.set_rules(pick))
}

fn caption(words: &str) -> El {
    text(words).font(theme::CAPTION).color(p().muted).wrap()
}

impl App {
    /// Changes the character, remembered for next time, and prices the flips for it. Under
    /// public data, a new faction reads its own auction house.
    fn set_rules(&mut self, change: impl FnOnce(&mut Rules)) {
        let side = self.rules.side;
        change(&mut self.rules);
        settings::save_rules(&self.rules);
        if self.rules.side != side && self.source != PriceSource::App {
            self.load_market();
        }
        self.reprice_flips();
    }
}
