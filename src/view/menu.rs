//! The one open toolbar menu, drawn over the window under the pill that opened it.

use rune::{Align, Color, Cursor, El, ScrollSpec, Side, col, el, row, spacer, text};

use super::App;
use super::widgets::{CHECK, CHEVRON_DOWN, glyph, pill};
use crate::menu::{self, Item, Menu, Pick};
use crate::theme::{self, p};

const CHECK_SIZE: f32 = 14.0;

/// The key of the pill a menu opens under.
pub fn anchor(m: Menu) -> &'static str {
    match m {
        Menu::CraftsSold => "crafts-sold-pill",
        Menu::Zone => "zone-pill",
    }
}

fn name(m: Menu) -> &'static str {
    match m {
        Menu::CraftsSold => "Minimum sold per day",
        Menu::Zone => "Zone",
    }
}

fn width(m: Menu) -> f32 {
    match m {
        Menu::CraftsSold => 184.0,
        Menu::Zone => 280.0,
    }
}

/// A pill that opens `m` under itself: `parts`, then a chevron.
pub fn trigger(s: &App, m: Menu, active: bool, spoken: String, parts: Vec<El>) -> El {
    let open = s.menu == Some(m);
    parts
        .into_iter()
        .fold(pill(active, open).key(anchor(m)), |out, part| out.child(part))
        .child(glyph(&CHEVRON_DOWN, 12.0, p().muted))
        .role("button")
        .label(spoken)
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut App| s.toggle_menu(m))
}

/// The open menu, if any, behind a scrim that closes it on a press anywhere outside.
pub fn overlay(s: &App) -> Vec<El> {
    let Some(m) = s.menu else { return Vec::new() };
    let scrim = el().key("menu-scrim").role("none").absolute(0.0, 0.0).w_full().h_full().on_press(|s: &mut App| s.menu = None);
    let list = s.menu_items(m).into_iter().fold(col().w_full(), |list, i| list.child(item(i)));
    // Rune borders take no layout space, so the 4pt padding grows by the 1pt border.
    let panel = col()
        .key("menu")
        .w(width(m))
        .padding(5.0)
        .radius(4.0)
        .fill(p().surface)
        .border(1.0, p().line2)
        .clip_rounded(4.0)
        .child(col().w_full().shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list))
        .role("listbox")
        .label(name(m))
        .anchored_fit(anchor(m), Side::Bottom, Align::Start, 4.0);
    vec![scrim, panel]
}

fn item(i: Item) -> El {
    let color = if i.on { p().text } else { p().muted };
    let mut out = row()
        .w_full()
        .h(32.0)
        .px(8.0)
        .gap(12.0)
        .radius(2.0)
        .center_y()
        .cursor(Cursor::Pointer)
        .child(text(&i.label).font(theme::CAPTION).color(color).truncate().shrink().min_w(0.0))
        .child(spacer());
    if let Some(hint) = &i.hint {
        out = out.child(text(hint).font(theme::CAPTION).color(p().faint));
    }
    let check = if i.on { glyph(&CHECK, CHECK_SIZE, p().text) } else { el().w(CHECK_SIZE).h(CHECK_SIZE) };
    let out = out.child(check).role("option").label(i.spoken).checked(i.on);
    let out = if i.on { out.fill(p().active) } else { out.hover(Color::WHITE.alpha(0.03)) };
    let pick = i.pick;
    out.on_press(move |s: &mut App| s.pick(pick.clone()))
}

impl App {
    /// Opens `m`, or closes it when it is already open.
    pub(super) fn toggle_menu(&mut self, m: Menu) {
        self.menu = if self.menu == Some(m) { None } else { Some(m) };
    }

    /// Closes the open menu, saying whether one was open.
    pub(super) fn close_menu(&mut self) -> bool {
        self.menu.take().is_some()
    }

    fn menu_items(&self, m: Menu) -> Vec<Item> {
        match m {
            Menu::CraftsSold => menu::sold(self.min_sold, Pick::CraftsSold),
            Menu::Zone => self.flips.zones().map_or_else(Vec::new, |zones| menu::zones(zones, self.flips.zone())),
        }
    }

    /// Sets what `pick` names and closes the menu.
    pub(super) fn pick(&mut self, pick: Pick) {
        self.menu = None;
        match pick {
            Pick::CraftsSold(step) => {
                self.min_sold = step;
                self.refresh();
            }
            Pick::Zone(zone) => self.flips.pick_zone(zone),
        }
    }
}
