//! The zone's pill, search, then one pill per filter: sold per day, supply and profitable only.

use rune::{El, Key, KeyEvent, Tx, text, text_input};

use crate::menu::{Menu, SOLD_STEPS, sold_active, supply_active};
use crate::theme::{self, p};
use crate::view::menu::trigger;
use crate::view::widgets::{self, MAP_PIN, TOOLBAR_GAP, checkbox, checkbox_width, glyph, pill_width, search_box};
use crate::view::{App, Dims};

/// The longest a zone's name draws in its pill before it truncates.
const ZONE_NAME: f32 = 160.0;
const ZONE_MIN: f32 = 96.0;
/// The narrowest the search gets beside the filters before it takes a row of its own.
const SEARCH_MIN: f32 = 160.0;
/// The room the zone and search need to spell out what the search reaches.
const SEARCH_WIDE: f32 = 480.0;

/// The toolbar and its height.
pub fn toolbar(s: &App, d: Dims) -> (El, f32) {
    widgets::toolbar(d, ZONE_MIN + TOOLBAR_GAP + SEARCH_MIN, |room| vec![zone(s), search(s, room >= SEARCH_WIDE)], |short| vec![sold(s, short), supply(s), profitable(s, short)])
}

fn zone(s: &App) -> El {
    let zone = &s.flips.zone;
    let name = text(zone).font(theme::BODY).color(p().text).truncate().shrink().min_w(0.0).max_w(ZONE_NAME);
    trigger(s, Menu::Zone, false, format!("Zone: {zone}"), vec![glyph(&MAP_PIN, 14.0, p().muted), name]).shrink().min_w(ZONE_MIN)
}

fn sold(s: &App, short: bool) -> (El, f32) {
    let (label, value) = (if short { "Sold/day" } else { "Sold per day" }, SOLD_STEPS[s.flips.min_sold].value);
    let pill = trigger(s, Menu::FlipsSold, sold_active(s.flips.min_sold), format!("Sold per day: {value}"), vec![text(label).font(theme::BODY).color(p().muted), text(value).font(theme::BODY).color(p().text)]);
    (pill, pill_width(&[label, value]))
}

fn supply(s: &App) -> (El, f32) {
    let label = s.flips.supply.label();
    let pill = trigger(s, Menu::Supply, supply_active(s.flips.supply), format!("Supply: {label}"), vec![text("Supply").font(theme::BODY).color(p().muted), text(label).font(theme::BODY).color(p().text)]);
    (pill, pill_width(&["Supply", label]))
}

fn profitable(s: &App, short: bool) -> (El, f32) {
    let label = if short { "Profitable" } else { "Profitable only" };
    let flip = |s: &mut App| {
        s.flips.hide_unprofitable = !s.flips.hide_unprofitable;
        s.flips.refilter();
    };
    (checkbox(label, s.flips.hide_unprofitable, flip).key("Profitable only").label("Profitable only"), checkbox_width(label))
}

fn search(s: &App, wide: bool) -> El {
    let field = text_input(&s.flips.search, theme::BODY)
        .key("flip-search")
        .single_line()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .color(p().text)
        .placeholder(if wide { "Search items, merchants or zones" } else { "Search" }, p().faint)
        .caret(p().brass, p().brass.alpha(0.3))
        .label("Search items, merchants or zones")
        .on_edit(|s: &mut App| &mut s.flips.search)
        .on_input_change(|s: &mut App, _: &mut Tx| s.flips.refilter())
        .on_input_key(|s: &mut App, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Escape => {
                s.flips.clear_search();
                tx.blur();
                true
            }
            Key::Down => s.flips.step(1, tx),
            Key::Up => s.flips.step(-1, tx),
            _ => false,
        });
    search_box(field, s.flips.search.focused).min_w(if wide { SEARCH_MIN } else { 120.0 })
}
