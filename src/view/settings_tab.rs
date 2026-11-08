//! The settings tab: where prices come from, and the character the flips are priced for.

use std::collections::BTreeSet;

use rune::{El, ScrollSpec, col, row, text};

use super::detail::band;
use super::header::page;
use super::widgets::checkbox;
use super::{App, Dims, folder};
use crate::flips::{self, Bartering, Rules};
use crate::market::PriceSource;
use crate::merchants::Side;
use crate::settings;
use crate::theme::{self, p};

pub fn main(s: &App, d: Dims) -> El {
    let r = &s.rules;
    let faction = Side::ALL.into_iter().map(|side| choice(side.name(), r.side == side, move |r| r.side = side));
    let bartering = Bartering::ALL.into_iter().map(|b| choice(&b.label(), r.bartering == b, move |r| r.bartering = b));
    let body = col()
        .w_full()
        .max_w(720.0)
        .child(band("World of Warcraft folder"))
        .child(folder::block(s).mt(12.0))
        .child(band("Character").mt(32.0))
        .child(setting("Faction", row().gap(8.0).children(faction)))
        .child(setting("Honored or better with", honored(s)))
        .child(setting("Bartering", row().gap(8.0).children(bartering)))
        .child(caption("Honored takes 10% off the merchants of that faction, and Bartering its percent off every merchant, the two added together.").mt(16.0));
    col()
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .child(page("Settings", "Where prices come from, and who buys from merchants.".into(), d))
        .child(col().key("settings-body").w_full().grow(1.0).min_h(0.0).px(d.pad).pb(24.0).scroll(ScrollSpec { stick_bottom: false }).child(body))
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
    checkbox(label, on, move |s: &mut App| s.set_rules(pick)).role("radio")
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
