//! The rail: the professions, of which only leatherworking is covered, then vendor flips.

use rune::{Color, Cursor, El, Shadow, col, el, text};

use super::{App, Tab};
use crate::theme::{self, p};

pub const ACTIVE: &str = "trade_leatherworking";
pub const COIN: &str = "inv_misc_coin_01";

pub const ICONS: [(&str, &str); 6] = [
    ("Leatherworking", ACTIVE),
    ("Blacksmithing", "trade_blacksmithing"),
    ("Tailoring", "trade_tailoring"),
    ("Alchemy", "trade_alchemy"),
    ("Engineering", "trade_engineering"),
    ("Engraving", "trade_engraving"),
];

pub fn rail(s: &App) -> El {
    let [hi, body, ink] = p().mark;
    let mark = col()
        .w(40.0)
        .h(40.0)
        .radius(12.0)
        .center()
        .fill(body)
        .radial_gradient([0.35, 0.30], 38.2, [(0.0, hi), (0.7, body), (1.0, body)])
        .box_shadow(Shadow { spread: 1.0, color: Color::BLACK.alpha(0.25), inset: true, ..Shadow::default() })
        .child(text("T").font(theme::display(18.0).lh(18.0)).color(ink))
        .mb(18.0)
        .a11y_hidden();
    let icon = |label: &str, name: &str| {
        let picture = el().w(30.0).h(30.0).radius(7.0).a11y_hidden();
        let picture = match s.icons.rail.get(name) {
            Some(i) => picture.pixels(i.pixels()),
            None => picture.fill(p().raised),
        };
        col().w(44.0).h(44.0).radius(12.0).center().child(picture).label(label)
    };
    let tab = |label: &str, name: &str, tab: Tab| {
        let on = s.tab == tab;
        let out = icon(label, name).role("button").checked(on).cursor(Cursor::Pointer).on_press(move |s: &mut App| s.tab = tab);
        if on {
            out.fill(p().raised)
                .box_shadow(Shadow { spread: 1.0, color: p().line2, inset: true, ..Shadow::default() })
                .child(el().absolute(-16.0, 12.0).w(3.0).h(20.0).radius(2.0).fill(p().brass))
        } else {
            out.hover(Color::WHITE.alpha(0.04))
        }
    };
    let separator = || el().w(28.0).h(1.0).my(6.0).fill(p().line);
    let (label, name) = ICONS[0];
    let mut out = col()
        .w(76.0)
        .h_full()
        .pt(20.0)
        .gap(10.0)
        .items_center()
        .fill(p().rail)
        .border_sides([0.0, 1.0, 0.0, 0.0], p().line)
        .child(mark)
        .child(tab(label, name, Tab::Crafts))
        .child(separator());
    for (label, name) in &ICONS[1..] {
        out = out.child(icon(&format!("{label}, not covered yet"), name));
    }
    out.child(separator()).child(tab("Vendor flips", COIN, Tab::Flips))
}
