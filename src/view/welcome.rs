//! The first visit's one screen: where prices come from and the character's faction, then
//! Start.

use rune::{El, ScrollSpec, Tx, col, row, text};

use super::detail::band;
use super::settings_tab::{caption, faction, prices};
use super::widgets::{CHECK, button};
use super::{App, Dims};
use crate::settings;
use crate::theme::{self, p};

/// The whole window while the welcome screen is up: its card centred in a window `tall`
/// points high, scrolling once the window is shorter than the card.
pub fn screen(s: &App, d: Dims, tall: f32) -> El {
    let start = button("Start", "Start", &CHECK, |s: &mut App, _: &mut Tx| s.start()).key("welcome-start").w(120.0);
    let card = col()
        .w_full()
        .max_w(560.0)
        .padding(d.pad + 8.0)
        .fill(p().surface)
        .border(1.0, p().bronze)
        .radius(4.0)
        .child(text("Welcome to Craftski").font(theme::TITLE).color(p().gold).wrap())
        .child(caption("Crafting and merchant flip profits for WoW Forever, ranked from TSM's auction house prices.").mt(8.0))
        .child(band("Prices").mt(24.0))
        .child(prices(s))
        .child(band("Character").mt(32.0))
        .child(faction(s))
        .child(row().w_full().mt(32.0).justify_end().child(start));
    let page = col().w_full().min_h(tall).center().padding(d.pad).child(card);
    col().key("welcome").w_full().h_full().scroll(ScrollSpec { stick_bottom: false }).child(page)
}

impl App {
    /// Leaves the welcome screen for good with what it shows saved, the house the player never
    /// changed from included. Its prices are already loading, or landed.
    pub(super) fn start(&mut self) {
        settings::save_source(self.source);
        settings::save_rules(&self.rules);
        settings::save_welcomed();
        self.welcome = false;
    }
}
