//! The zone's pill, search, then one pill per filter: sold per day, supply and profitable only.

use rune::{El, Key, KeyEvent, Tx, el, row, text, text_input};

use crate::menu::{Menu, SOLD_STEPS, sold_active, supply_active};
use crate::theme::{self, p};
use crate::view::menu::trigger;
use crate::view::widgets::{MAP_PIN, TOOLBAR_GAP, checkbox, glyph, search_box};
use crate::view::{App, Dims};

/// The longest a zone's name draws in its pill before it truncates.
const ZONE_NAME: f32 = 160.0;

pub fn toolbar(s: &App, d: Dims) -> El {
    let f = &s.flips;
    let zone = trigger(
        s,
        Menu::Zone,
        false,
        format!("Zone: {}", f.zone),
        vec![glyph(&MAP_PIN, 14.0, p().muted), text(&f.zone).font(theme::BODY).color(p().text).truncate().shrink().min_w(0.0).max_w(ZONE_NAME)],
    )
    .shrink()
    .min_w(96.0);
    let step = SOLD_STEPS[f.min_sold].value;
    let sold = trigger(
        s,
        Menu::FlipsSold,
        sold_active(f.min_sold),
        format!("Sold per day: {step}"),
        vec![text(if d.compact { "Sold/day" } else { "Sold per day" }).font(theme::BODY).color(p().muted), text(step).font(theme::BODY).color(p().text)],
    );
    let supply = trigger(
        s,
        Menu::Supply,
        supply_active(f.supply),
        format!("Supply: {}", f.supply.label()),
        vec![text("Supply").font(theme::BODY).color(p().muted), text(f.supply.label()).font(theme::BODY).color(p().text)],
    );
    let profitable = checkbox("Profitable only", f.hide_unprofitable, |s: &mut App| {
        s.flips.hide_unprofitable = !s.flips.hide_unprofitable;
        s.flips.refilter();
    });
    row().w_full().h(56.0).pt(16.0).pb(8.0).px(d.pad).gap(TOOLBAR_GAP).center_y().child(zone).child(search(s, d)).child(el().grow(1.0)).child(sold).child(supply).child(profitable)
}

fn search(s: &App, d: Dims) -> El {
    let field = text_input(&s.flips.search, theme::BODY)
        .key("flip-search")
        .single_line()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .color(p().text)
        .placeholder(if d.compact { "Search" } else { "Search items, merchants or zones" }, p().faint)
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
    search_box(field, s.flips.search.focused).min_w(if d.compact { 120.0 } else { 160.0 })
}
