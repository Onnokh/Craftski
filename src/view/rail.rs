//! The profession rail. Only leatherworking is covered; the rest show what is to come.

use rune::{El, col, el};

use super::App;
use crate::theme::p;

pub const ACTIVE: &str = "trade_leatherworking";

pub const ICONS: [(&str, &str); 6] = [
    ("Leatherworking", ACTIVE),
    ("Blacksmithing", "trade_blacksmithing"),
    ("Tailoring", "trade_tailoring"),
    ("Alchemy", "trade_alchemy"),
    ("Engineering", "trade_engineering"),
    ("Engraving", "trade_engraving"),
];

pub fn rail(s: &App) -> El {
    let profession = |label: &str, name: &str| {
        let picture = el().w(32.0).h(32.0).radius(4.0).label(label);
        match s.icons.rail.get(name) {
            Some(i) => picture.pixels(i.pixels()),
            None => picture.fill(p().surface),
        }
    };
    let (label, name) = ICONS[0];
    let mut out = col()
        .w(60.0)
        .h_full()
        .pt(20.0)
        .gap(8.0)
        .items_center()
        .fill(p().rail)
        .border_sides([0.0, 1.0, 0.0, 0.0], p().rule)
        .child(profession(label, name).child(el().absolute(-14.0, 4.0).w(2.0).h(24.0).fill(p().brass)).role("button").checked(true));
    for (label, name) in &ICONS[1..] {
        out = out.child(profession(&format!("{label}, not covered yet"), name));
    }
    out
}
