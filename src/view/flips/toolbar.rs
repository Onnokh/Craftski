//! The zone, price source, filters and search, and the zone menu.

use rune::{Align, Color, Cursor, El, Icon, Key, KeyEvent, ScrollSpec, Shadow, Side, Tx, col, el, row, spacer, text, text_input};

use super::Stocked;
use crate::flip_rows::Supply;
use crate::profit::PriceSource;
use crate::theme::{self, p};
use crate::view::widgets::{SEARCH, Segment, caps, glyph, segmented, toggle};
use crate::view::{App, Dims, SOLD_STEPS};

const TRIGGER: &str = "zone-trigger";

static CHEVRON: Icon = Icon { name: "tsm-chevron", view: 24.0, body: r#"<path d="m6 9 6 6 6-6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"# };

pub fn toolbar(s: &App, d: Dims) -> El {
    let f = &s.flips;
    let zone = row()
        .key(TRIGGER)
        .gap(8.0)
        .px(13.0)
        .py(8.0)
        .center_y()
        .radius(10.0)
        .fill(if f.zone_menu { p().raised } else { p().surface })
        .border(1.0, if f.zone_menu { p().line2 } else { p().line })
        .cursor(Cursor::Pointer)
        .hover(Color::WHITE.alpha(0.03))
        .child(text(&f.zone).font(theme::sans(13.0, 500.0)).color(p().brass))
        .child(glyph(&CHEVRON, 14.0, p().muted))
        .role("button")
        .label(format!("Zone: {}", f.zone))
        .on_press(|s: &mut App| s.flips.zone_menu = !s.flips.zone_menu);
    let source = segmented(
        "Price source",
        vec![
            Segment { label: "Realm", on: s.source == PriceSource::Realm, brass: true, pick: Some(|s: &mut App| s.set_source(PriceSource::Realm)) },
            Segment { label: "Region", on: s.source == PriceSource::Region, brass: true, pick: Some(|s: &mut App| s.set_source(PriceSource::Region)) },
        ],
    );
    let unprofitable = toggle("Hide unprofitable", f.hide_unprofitable, |s: &mut App| {
        s.flips.hide_unprofitable = !s.flips.hide_unprofitable;
        s.flips.refilter();
    });
    let picks: [fn(&mut App); 4] = [|s| s.flips.set_min_sold(0), |s| s.flips.set_min_sold(1), |s| s.flips.set_min_sold(2), |s| s.flips.set_min_sold(3)];
    let mut steps: Vec<Segment<App>> = SOLD_STEPS.iter().zip(picks).enumerate().map(|(i, ((label, _), pick))| Segment { label, on: f.min_sold == i, brass: false, pick: Some(pick) }).collect();
    steps.push(Segment { label: "/ day", on: false, brass: false, pick: None });
    let supply_picks: [fn(&mut App); 3] = [|s| s.flips.set_supply(Supply::All), |s| s.flips.set_supply(Supply::Endless), |s| s.flips.set_supply(Supply::Limited)];
    let supply = Supply::ALL.into_iter().zip(supply_picks).map(|(supply, pick)| Segment { label: supply.label(), on: f.supply == supply, brass: false, pick: Some(pick) }).collect();
    let label = |l: &str| caps(l, p().faint).font(theme::sans(12.0, 500.0).tracking(0.72));
    let place = row().w_full().gap(12.0).center_y().child(zone).child(source).child(spacer()).child(search(s, d));
    let filters = row()
        .w_full()
        .gap(12.0)
        .center_y()
        .child(unprofitable)
        .child(label("Sells").ml(8.0))
        .child(segmented("Minimum sold per day", steps))
        .child(label("Supply").ml(8.0))
        .child(segmented("Supply", supply));
    col().w_full().px(d.pad).pt(14.0).pb(15.0).gap(10.0).border_sides([0.0, 0.0, 1.0, 0.0], p().line).child(place).child(filters)
}

fn search(s: &App, d: Dims) -> El {
    let field = text_input(&s.flips.search, theme::sans(13.0, 400.0))
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
        .gap(8.0)
        .px(13.0)
        .py(9.0)
        .center_y()
        .radius(10.0)
        .fill(p().surface)
        .border(1.0, if s.flips.search.focused { p().line2 } else { p().line })
        .clip_rounded(10.0)
        .cursor(Cursor::Text)
        .child(glyph(&SEARCH, 14.0, p().faint))
        .child(field);
    if d.compact { b.grow(1.0).shrink().min_w(120.0).max_w(300.0) } else { b.w(300.0) }
}

/// A press anywhere outside closes the menu; the menu lists every zone with something to flip.
pub fn zone_menu(s: &App, f: &Stocked) -> Vec<El> {
    let scrim = el().key("zone-scrim").role("none").absolute(0.0, 0.0).w_full().h_full().on_press(|s: &mut App| s.flips.zone_menu = false);
    let mut list = col().w_full().padding(6.0);
    for z in &f.zones {
        let on = z.name == s.flips.zone;
        let name = z.name.clone();
        let mut item = row()
            .w_full()
            .px(10.0)
            .py(7.0)
            .gap(12.0)
            .center_y()
            .radius(7.0)
            .cursor(Cursor::Pointer)
            .hover(Color::WHITE.alpha(0.04))
            .child(text(&z.name).font(theme::sans(13.0, 500.0)).color(if on { p().brass } else { p().text }).truncate().shrink().min_w(0.0))
            .child(spacer())
            .child(text(z.merchants.to_string()).font(theme::mono(12.0, 400.0)).color(p().faint))
            .role("option")
            .label(format!("{}, {} merchants", z.name, z.merchants))
            .checked(on)
            .on_press(move |s: &mut App| s.flips.pick_zone(name.clone()));
        if on {
            item = item.fill(p().raised);
        }
        list = list.child(item);
    }
    let menu = col()
        .key("zone-menu")
        .w(280.0)
        .radius(12.0)
        .fill(p().surface)
        .border(1.0, p().line2)
        .box_shadow(Shadow { blur: 24.0, y: 8.0, color: Color::BLACK.alpha(0.45), ..Shadow::default() })
        .clip_rounded(12.0)
        .child(col().w_full().shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list))
        .role("listbox")
        .label("Zone")
        .anchored_fit(TRIGGER, Side::Bottom, Align::Start, 6.0);
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
