//! The rail: the professions, of which only leatherworking is covered, then vendor flips.

use rune::{Color, Cursor, El, col, el};

use super::{App, Tab};
use crate::theme::p;

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
    let icon = |label: &str, name: &str| {
        let picture = el().w(32.0).h(32.0).radius(4.0).label(label);
        match s.icons.rail.get(name) {
            Some(i) => picture.pixels(i.pixels()),
            None => picture.fill(p().surface),
        }
    };
    let tab = |label: &str, name: &str, tab: Tab| {
        let on = s.tab == tab;
        let out = icon(label, name).role("button").checked(on).cursor(Cursor::Pointer).on_press(move |s: &mut App| s.tab = tab);
        if on { out.child(el().absolute(-14.0, 4.0).w(2.0).h(24.0).fill(p().brass)) } else { out.hover(Color::WHITE.alpha(0.04)) }
    };
    let separator = || el().w(32.0).h(1.0).my(8.0).fill(p().line);
    let (label, name) = ICONS[0];
    let mut out = col()
        .w(60.0)
        .h_full()
        .pt(20.0)
        .gap(8.0)
        .items_center()
        .fill(p().rail)
        .border_sides([0.0, 1.0, 0.0, 0.0], p().rule)
        .child(tab(label, name, Tab::Crafts));
    for (label, name) in &ICONS[1..] {
        out = out.child(icon(&format!("{label}, not covered yet"), name));
    }
    out.child(separator()).child(tab("Vendor flips", COIN, Tab::Flips))
}
