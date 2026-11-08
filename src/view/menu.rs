//! The one open toolbar menu, drawn over the window under the pill that opened it.

use rune::{Align, Color, Cursor, El, ScrollSpec, Side, col, el, row, spacer, text};

use super::{App, Load};
use super::skin;
use super::widgets::{ARROW_ROOM, CHECK, CHEVRON_DOWN, DROPDOWN_GAP, glyph, pill};
use crate::menu::{self, Item, Menu, Pick};
use crate::overview::Feed;
use crate::settings;
use crate::skin::Piece;
use crate::theme::{self, p};

const CHECK_SIZE: f32 = 14.0;
/// The game's tick, which draws larger than the glyph it stands in for.
const CHECKMARK: f32 = 18.0;
/// The chevron that ends a pill opening a menu.
pub const CHEVRON: f32 = 12.0;

/// The key of the pill a menu opens under.
pub fn anchor(m: Menu) -> &'static str {
    match m {
        Menu::CraftsSold => "crafts-sold-pill",
        Menu::FlipsSold => "flips-sold-pill",
        Menu::Supply => "supply-pill",
        Menu::Zone => "zone-pill",
        Menu::Realm => "realm-pill",
    }
}

fn name(m: Menu) -> &'static str {
    match m {
        Menu::CraftsSold | Menu::FlipsSold => "Minimum sold per day",
        Menu::Supply => "Supply",
        Menu::Zone => "Zone",
        Menu::Realm => "Realm",
    }
}

fn width(m: Menu) -> f32 {
    match m {
        Menu::CraftsSold | Menu::FlipsSold | Menu::Supply => 184.0,
        Menu::Zone => 280.0,
        Menu::Realm => 240.0,
    }
}

/// The status bar sits at the window's foot, so its menu opens upward.
fn side(m: Menu) -> Side {
    match m {
        Menu::Realm => Side::Top,
        _ => Side::Bottom,
    }
}

/// A toolbar pill that opens `m` under itself.
pub fn trigger(s: &App, m: Menu, active: bool, spoken: String, parts: Vec<El>) -> El {
    opener(pill(active, s.menu == Some(m)), m, spoken, parts)
}

/// `base` holding `parts` and a gold chevron, opening `m` on a press.
pub fn opener(base: El, m: Menu, spoken: String, parts: Vec<El>) -> El {
    parts
        .into_iter()
        .fold(base.key(anchor(m)), |out, part| out.child(part))
        .child(glyph(&CHEVRON_DOWN, CHEVRON, p().gold))
        .pr(ARROW_ROOM - DROPDOWN_GAP - CHEVRON)
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
    let panel = col()
        .key("menu")
        .w(width(m))
        .padding(4.0)
        .radius(4.0)
        .fill(p().surface)
        .border(1.0, p().bronze)
        .border_layout()
        .shadow(8.0, 24.0, Color::BLACK.alpha(0.6))
        .clip_rounded(4.0)
        .child(col().w_full().shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(list))
        .role("listbox")
        .label(name(m))
        .anchored_fit(anchor(m), side(m), Align::Start, 4.0);
    vec![scrim, panel]
}

fn item(i: Item) -> El {
    let mut out = row()
        .w_full()
        .h(28.0)
        .px(8.0)
        .gap(12.0)
        .radius(3.0)
        .center_y()
        .cursor(Cursor::Pointer)
        .child(text(&i.label).font(theme::MENU).color(p().text).truncate().shrink().min_w(0.0))
        .child(spacer());
    if let Some(hint) = &i.hint {
        out = out.child(text(hint).font(theme::CAPTION).color(p().faint));
    }
    let tick = skin::art(Piece::Checkmark);
    let check = match (i.on, tick) {
        (true, Some(px)) => el().w(CHECKMARK).h(CHECKMARK).pixels(px).a11y_hidden(),
        (true, None) => glyph(&CHECK, CHECK_SIZE, p().text),
        (false, _) => {
            let size = if tick.is_some() { CHECKMARK } else { CHECK_SIZE };
            el().w(size).h(size)
        }
    };
    let out = out.child(check).role("option").label(i.spoken).checked(i.on);
    let out = if i.on { out.fill(p().active) } else { out.hover(Color::WHITE.alpha(0.04)) };
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

    pub(super) fn menu_items(&self, m: Menu) -> Vec<Item> {
        match m {
            Menu::CraftsSold => menu::sold(self.min_sold, Pick::CraftsSold),
            Menu::FlipsSold => menu::sold(self.flips.min_sold(), Pick::FlipsSold),
            Menu::Supply => menu::supply(self.flips.supply()),
            Menu::Zone => self.flips.zones().map_or_else(Vec::new, |zones| menu::zones(zones, self.flips.zone())),
            Menu::Realm => self.ready().map_or_else(Vec::new, |r| {
                let book = &r.market.book;
                menu::realms(&Feed::realms(book, self.now), book.realm().map_or("", |(name, _)| name))
            }),
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
            Pick::FlipsSold(step) => self.flips.set_min_sold(step),
            Pick::Supply(supply) => self.flips.set_supply(supply),
            Pick::Zone(zone) => self.flips.pick_zone(zone),
            Pick::Realm(realm) => {
                settings::save_realm(&realm);
                self.use_realm(realm);
            }
        }
    }

    /// Prices both tabs from `realm` from now on, this session and after a reload.
    pub(super) fn use_realm(&mut self, realm: String) {
        self.realm = realm;
        let Load::Ready(r) = &mut self.load else { return };
        if r.market.choose_realm(&self.realm) {
            self.refresh();
            self.reprice_flips();
        }
    }
}
