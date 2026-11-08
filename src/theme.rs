//! The design tokens: one palette, the type scale and the bundled fonts.

use std::sync::LazyLock;

use rune::color::hex;
use rune::{Color, Family, TextStyle};

use crate::copper::Coin;
use crate::recipes::Quality;

pub const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/IBMPlexSans-Variable.ttf"),
    include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"),
    include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf"),
    include_bytes!("../assets/fonts/Fraunces-Variable.ttf"),
];
pub const FAMILIES: (&str, &str) = ("IBM Plex Sans", "IBM Plex Mono");
const DISPLAY: Family = Family::Named("Fraunces");

pub struct Palette {
    pub rail: Color,
    pub ground: Color,
    pub surface: Color,
    pub raised: Color,
    pub line: Color,
    pub line2: Color,
    /// Row dividers, a step quieter than `line`.
    pub rule: Color,
    pub track: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub brass: Color,
    pub on_brass: Color,
    pub profit: Color,
    pub loss: Color,
    pub sells_bar: Color,
    pub warn_text: Color,
    /// The rail's brass mark: highlight, body and the letter on it.
    pub mark: [Color; 3],
    pub knob: Color,
    /// Skill-up bands: still yellow, green, then grey.
    pub skill: [Color; 3],
    quality: [Color; 5],
    coin: [[Color; 3]; 3],
}

static PALETTE: LazyLock<Palette> = LazyLock::new(|| {
    let c = |s: &str| hex(s).expect("palette hex is valid");
    Palette {
        rail: c("#0F0D0B"),
        ground: c("#13110E"),
        surface: c("#1B1814"),
        raised: c("#231F1A"),
        line: c("#2F2923"),
        line2: c("#3A332B"),
        rule: c("#221E19"),
        track: c("#2A251F"),
        text: c("#EEE4D2"),
        muted: c("#9A8D79"),
        faint: c("#6E6355"),
        brass: c("#D8A443"),
        on_brass: c("#1E1608"),
        profit: c("#79C39E"),
        loss: c("#D96550"),
        sells_bar: c("#8C7B60"),
        warn_text: c("#E3CFA6"),
        mark: [c("#F3CE6E"), c("#B9832A"), c("#2A1D08")],
        knob: c("#1A140A"),
        skill: [c("#E0C24A"), c("#5FB86A"), c("#5E574E")],
        quality: [c("#9D9D9D"), c("#EEE4D2"), c("#4BD461"), c("#4F9CFF"), c("#B469FF")],
        coin: [
            [c("#FBE39A"), c("#E2B650"), c("#9C7420")],
            [c("#FFFFFF"), c("#C8CDD5"), c("#7F858E")],
            [c("#F0B48A"), c("#C27A48"), c("#7A4422")],
        ],
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

    /// Highlight, body and rim of a coin's radial gradient.
    pub fn coin(&self, coin: Coin) -> [Color; 3] {
        self.coin[coin as usize]
    }
}

/// The body face at the page's 1.4 line height.
pub const fn sans(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).lh(size * 1.4)
}

/// The figures face at Plex Mono's own line height, which the design's `font:` shorthand restores.
pub const fn mono(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).lh(size * 1.3).family(Family::Mono).features("'tnum' 1")
}

pub const fn display(size: f32) -> TextStyle {
    TextStyle::new(size, 600.0).family(DISPLAY)
}

/// Small uppercase labels: column heads, tile keys, section titles.
pub const CAPS: TextStyle = sans(11.0, 500.0).tracking(0.66);
