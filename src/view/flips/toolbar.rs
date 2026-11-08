//! The zone, filters and search, and the zone menu.

use rune::{Align, Color, Cursor, El, Key, KeyEvent, ScrollSpec, Shadow, Side, Tx, col, row, spacer, text, text_input};

use super::Stocked;
use crate::flip_rows::Supply;
use crate::theme::{self, p};
use crate::view::widgets::{CHEVRON_DOWN, SEARCH, Segment, checkbox, glyph, segmented};
use crate::view::{App, Dims, SOLD_STEPS};

const TRIGGER: &str = "zone-trigger";

pub fn toolbar(s: &App, d: Dims) -> El {
    let f = &s.flips;
    let zone = row()
        .key(TRIGGER)
        .h(32.0)
        .gap(8.0)
        .px(13.0)
        .center_y()
        .radius(4.0)
        .border(1.0, if f.zone_menu { p().line2 } else { p().line })
        .cursor(Cursor::Pointer)
        .hover(Color::WHITE.alpha(0.03))
        .child(text(&f.zone).font(theme::BODY).color(p().text))
        .child(glyph(&CHEVRON_DOWN, 16.0, p().muted))
        .role("button")
        .label(format!("Zone: {}", f.zone))
        .on_press(|s: &mut App| s.flips.zone_menu = !s.flips.zone_menu);
    let unprofitable = checkbox("Hide unprofitable", f.hide_unprofitable, |s: &mut App| {
        s.flips.hide_unprofitable = !s.flips.hide_unprofitable;
        s.flips.refilter();
    });
    let picks: [fn(&mut App); 4] = [|s| s.flips.set_min_sold(0), |s| s.flips.set_min_sold(1), |s| s.flips.set_min_sold(2), |s| s.flips.set_min_sold(3)];
    let steps = SOLD_STEPS.iter().zip(picks).enumerate().map(|(i, ((label, _), pick))| Segment { label, on: f.min_sold == i, pick }).collect();
    let supply_picks: [fn(&mut App); 3] = [|s| s.flips.set_supply(Supply::All), |s| s.flips.set_supply(Supply::Endless), |s| s.flips.set_supply(Supply::Limited)];
    let supply = Supply::ALL.into_iter().zip(supply_picks).map(|(supply, pick)| Segment { label: supply.label(), on: f.supply == supply, pick }).collect();
    let labelled = |label: &str, control: El| row().gap(8.0).center_y().child(text(label).font(theme::BODY).color(p().muted)).child(control);
    let place = row().w_full().h(32.0).gap(d.gap).center_y().child(zone).child(spacer()).child(search(s, d));
    let filters = row()
        .w_full()
        .h(32.0)
        .gap(d.gap)
        .center_y()
        .child(unprofitable)
        .child(labelled("Sold per day", segmented("Minimum sold per day", steps)))
        .child(labelled("Supply", segmented("Supply", supply)));
    col().w_full().px(d.pad).py(16.0).gap(12.0).child(place).child(filters)
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
    let b = row()
        .h(32.0)
        .gap(8.0)
        .px(13.0)
        .center_y()
        .radius(4.0)
        .border(1.0, if s.flips.search.focused { p().line2 } else { p().line })
        .clip_rounded(4.0)
        .cursor(Cursor::Text)
        .child(glyph(&SEARCH, 16.0, p().faint))
        .child(field);
    if d.compact { b.grow(1.0).shrink().min_w(128.0).max_w(280.0) } else { b.w(280.0) }
}

/// A press anywhere outside closes the menu; the menu lists every zone with something to flip.
pub fn zone_menu(s: &App, f: &Stocked) -> Vec<El> {
    let scrim = rune::el().key("zone-scrim").role("none").absolute(0.0, 0.0).w_full().h_full().on_press(|s: &mut App| s.flips.zone_menu = false);
    let mut list = col().w_full().py(4.0);
    for z in &f.zones {
        let on = z.name == s.flips.zone;
        let name = z.name.clone();
        let item = row()
            .w_full()
            .h(32.0)
            .px(12.0)
            .gap(12.0)
            .center_y()
            .cursor(Cursor::Pointer)
            .child(text(&z.name).font(theme::BODY).color(p().text).truncate().shrink().min_w(0.0))
            .child(spacer())
            .child(text(z.merchants.to_string()).font(theme::CAPTION).color(p().muted))
            .role("option")
            .label(format!("{}, {} merchants", z.name, z.merchants))
            .checked(on)
            .on_press(move |s: &mut App| s.flips.pick_zone(name.clone()));
        list = list.child(if on { item.fill(p().active) } else { item.hover(Color::WHITE.alpha(0.03)) });
    }
    let menu = col()
        .key("zone-menu")
        .w(280.0)
        .radius(4.0)
        .fill(p().surface)
        .border(1.0, p().line2)
        .box_shadow(Shadow { blur: 16.0, y: 4.0, color: Color::BLACK.alpha(0.4), ..Shadow::default() })
        .clip_rounded(4.0)
        .child(col().w_full().shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list))
        .role("listbox")
        .label("Zone")
        .anchored_fit(TRIGGER, Side::Bottom, Align::Start, 4.0);
    vec![scrim, menu]
}

impl super::State {
    fn set_min_sold(&mut self, step: usize) {
        self.min_sold = step;
        self.refilter();
    }

    fn set_supply(&mut self, supply: Supply) {
        self.supply = supply;
        self.refilter();
    }
}
