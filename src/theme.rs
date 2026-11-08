//! The design tokens: one palette, the type scale and the bundled fonts.

use std::sync::LazyLock;

use rune::color::hex;
use rune::{Color, TextStyle};

use crate::recipes::Quality;

const PLEX: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Variable.ttf");
pub const FONTS: &[&[u8]] = &[PLEX];
pub const FAMILIES: (&str, &str) = ("IBM Plex Sans", "monospace");

pub struct Palette {
    pub ground: Color,
    pub surface: Color,
    /// Row dividers, quieter than the bronze that rules the column heads and bands.
    pub rule: Color,
    pub selected: Color,
    /// The fill of what is chosen: a pill whose filter is on, a menu's current item.
    pub active: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub brass: Color,
    /// The game's own gold: titles, column heads, section bands and the chosen standing.
    pub gold: Color,
    /// Each coin's face then rim, gold first.
    pub coins: [[Color; 2]; 3],
    /// The rule under column heads and section bands.
    pub bronze: Color,
    /// The profit bar's gradient, top to bottom, and the selected row's.
    pub bar: [Color; 2],
    pub bar_selected: [Color; 2],
    /// A lit sales pip.
    pub pip: Color,
    pub on_brass: Color,
    pub loss: Color,
    /// The caution icon beside a market figure the data cannot vouch for.
    pub warn: Color,
    /// Skill-up bands: still yellow, green, then grey.
    pub skill: [Color; 3],
    /// The game tooltip's ground, TOOLTIP_DEFAULT_BACKGROUND_COLOR.
    pub tooltip: Color,
    /// The game bar's skill-up segments, each top to bottom.
    pub skill_bar: [[Color; 2]; 3],
    quality: [Color; 5],
}

static PALETTE: LazyLock<Palette> = LazyLock::new(|| {
    let c = |s: &str| hex(s).expect("palette hex is valid");
    let text = c("#EEE6D6");
    Palette {
        ground: c("#0B0806"),
        surface: c("#120E0A"),
        rule: c("#FFE2AA").alpha(0.07),
        selected: c("#221D17"),
        active: c("#26211B"),
        text,
        muted: c("#A69C8B"),
        faint: c("#776D5F"),
        brass: c("#D8A443"),
        gold: c("#FFD100"),
        coins: [[c("#F2C230"), c("#7A5A10")], [c("#C8CDD4"), c("#5F656E")], [c("#C8743A"), c("#5E3214")]],
        bronze: c("#6E5536"),
        bar: [c("#C9A24E"), c("#8A6A2A")],
        bar_selected: [c("#FFE07A"), c("#C99A32")],
        pip: c("#E8C66A"),
        on_brass: c("#1E1608"),
        loss: c("#E57560"),
        warn: c("#FFB733"),
        skill: [c("#E0C24A"), c("#5FB86A"), c("#5E574E")],
        tooltip: c("#171730"),
        skill_bar: [[c("#FFE15A"), c("#C9A21E")], [c("#5BE04A"), c("#2E8A22")], [c("#8C8478"), c("#4A443C")]],
        quality: [c("#9D9D9D"), c("#FFFFFF"), c("#1EFF00"), c("#0070DD"), c("#A335EE")],
    }
});

pub fn p() -> &'static Palette {
    &PALETTE
}

impl Palette {
    /// The game's item colours; legendary and above share epic's, as nothing crafted here
    /// reaches them.
    pub fn quality(&self, q: Quality) -> Color {
        self.quality[(q as usize).min(4)]
    }
}

const fn plex(size: f32, line_height: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).lh(line_height).features("'tnum' 1")
}

/// Meta lines, notes and skill-up labels.
pub const CAPTION: TextStyle = plex(12.0, 16.0, 400.0);
/// Column heads and band labels.
pub const LABEL: TextStyle = plex(12.0, 16.0, 500.0);
/// Menu items.
pub const MENU: TextStyle = plex(13.0, 18.0, 400.0);
pub const BODY: TextStyle = plex(14.0, 20.0, 400.0);
pub const BODY_MEDIUM: TextStyle = plex(14.0, 20.0, 500.0);
pub const BODY_STRONG: TextStyle = plex(14.0, 20.0, 600.0);
/// The detail panel's item name.
pub const HEADING: TextStyle = plex(20.0, 28.0, 600.0);
/// The hero profit figure.
pub const FIGURE: TextStyle = plex(24.0, 32.0, 600.0);
pub const TITLE: TextStyle = FIGURE.tracking(-0.24);
