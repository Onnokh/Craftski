//! The rail: the professions, of which only leatherworking is covered, then vendor flips.

use rune::{Color, Cursor, El, col, el};

use super::skin;
use super::{App, Tab};
use crate::skin::Piece;
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

const SLOT: f32 = 48.0;
const FRAME: f32 = 40.0;
const ICON: f32 = 34.0;

pub fn rail(s: &App) -> El {
    let icon = |label: &str, name: &str| {
        let picture = el().absolute((SLOT - ICON) / 2.0, (SLOT - ICON) / 2.0).w(ICON).h(ICON).radius(2.0);
        let picture = match s.icons.rail.get(name) {
            Some(i) => picture.pixels(i.pixels()),
            None => picture.fill(p().surface),
        };
        let frame = skin::art(Piece::SlotFrame).map(|px| el().absolute((SLOT - FRAME) / 2.0, (SLOT - FRAME) / 2.0).w(FRAME).h(FRAME).pixels(px).a11y_hidden());
        el().w(SLOT).h(SLOT).label(label).child(picture).children(frame)
    };
    let tab = |label: &str, name: &str, tab: Tab| {
        let on = s.tab == tab;
        let out = icon(label, name).role("button").checked(on).cursor(Cursor::Pointer).on_press(move |s: &mut App| s.tab = tab);
        if !on {
            return out.hover(Color::WHITE.alpha(0.04));
        }
        match skin::art(Piece::TabSelected) {
            Some(px) => out.child(el().absolute(-2.0, -4.0).w(52.0).h(56.0).pixels(px).dim(0.9).a11y_hidden()),
            None => out.child(el().absolute(-6.0, 12.0).w(2.0).h(24.0).fill(p().brass)),
        }
    };
    let separator = || el().w(32.0).h(1.0).my(6.0).fill(p().bronze);
    let (label, name) = ICONS[0];
    let mut out = col()
        .w(60.0)
        .h_full()
        .pt(16.0)
        .gap(6.0)
        .items_center()
        .fill(Color::BLACK.alpha(0.55))
        .border_sides([0.0, 1.0, 0.0, 0.0], Color::BLACK)
        .child(tab(label, name, Tab::Crafts));
    for (label, name) in &ICONS[1..] {
        out = out.child(icon(&format!("{label}, not covered yet"), name));
    }
    out.child(separator()).child(tab("Vendor flips", COIN, Tab::Flips))
}
