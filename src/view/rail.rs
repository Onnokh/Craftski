//! The rail: the primary professions, the secondary ones, then vendor flips, each under its name.

use rune::{Color, Cursor, El, col, el, text};

use super::skin;
use super::{App, Tab};
use crate::recipes::Profession;
use crate::skin::Piece;
use crate::theme::{self, p};

pub const COIN: &str = "inv_misc_coin_01";

const SLOT: f32 = 48.0;
const FRAME: f32 = 40.0;
const ICON: f32 = 34.0;

/// Every icon the rail draws, by the name icon files go by.
pub fn icons() -> impl Iterator<Item = &'static str> {
    Profession::ALL.into_iter().map(Profession::icon).chain([COIN])
}

pub fn rail(s: &App) -> El {
    let professions = |secondary: bool| Profession::ALL.into_iter().filter(move |p| p.secondary() == secondary).map(|p| profession(s, p));
    col()
        .w(60.0)
        .h_full()
        .pt(12.0)
        .items_center()
        .fill(Color::BLACK.alpha(0.55))
        .border_sides([0.0, 1.0, 0.0, 0.0], Color::BLACK)
        .child(section("Primary", professions(false)))
        .child(rule())
        .child(section("Secondary", professions(true)))
        .child(rule())
        .child(section("Vendor", [tab(s, "Vendor flips", COIN, s.tab == Tab::Flips, |s: &mut App| s.tab = Tab::Flips)]))
}

fn profession(s: &App, profession: Profession) -> El {
    let on = s.tab == Tab::Crafts && s.profession == profession;
    tab(s, profession.name(), profession.icon(), on, move |s: &mut App| s.choose_profession(profession))
}

fn section(title: &str, tabs: impl IntoIterator<Item = El>) -> El {
    let heading = text(title).font(theme::RAIL).color(p().gold);
    col().items_center().gap(6.0).child(heading).children(tabs)
}

fn rule() -> El {
    el().w(32.0).h(1.0).my(12.0).fill(p().bronze)
}

fn tab(s: &App, label: &str, icon: &str, on: bool, press: impl Fn(&mut App) + 'static) -> El {
    let picture = el().absolute((SLOT - ICON) / 2.0, (SLOT - ICON) / 2.0).w(ICON).h(ICON).radius(2.0);
    let picture = match s.icons.rail.get(icon) {
        Some(i) => picture.pixels(i.pixels()),
        None => picture.fill(p().surface),
    };
    let frame = skin::art(Piece::SlotFrame).map(|px| el().absolute((SLOT - FRAME) / 2.0, (SLOT - FRAME) / 2.0).w(FRAME).h(FRAME).pixels(px).a11y_hidden());
    let out = el().w(SLOT).h(SLOT).label(label).child(picture).children(frame).role("button").checked(on).cursor(Cursor::Pointer).on_press(press);
    if !on {
        return out.hover(Color::WHITE.alpha(0.04));
    }
    match skin::art(Piece::TabSelected) {
        Some(px) => out.child(el().absolute(-2.0, -4.0).w(52.0).h(56.0).pixels(px).dim(0.9).a11y_hidden()),
        None => out.child(el().absolute(-6.0, 12.0).w(2.0).h(24.0).fill(p().brass)),
    }
}
