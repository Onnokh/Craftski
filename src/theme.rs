//! The design tokens: one palette, the type scale and the bundled fonts.

use std::sync::LazyLock;

use rune::color::hex;
use rune::{Color, TextStyle};

use crate::recipes::Quality;

const PLEX: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Variable.ttf");
const FAMILIES: (&str, &str) = ("IBM Plex Sans", "monospace");
/// Friz Quadrata, the game's own face, with IBM Plex drawing what it lacks.
const FRIZ_FAMILIES: (&str, &str) = ("\"Friz Quadrata TT\", \"IBM Plex Sans\"", "monospace");
const FRIZ_FILE_ID: u32 = 615960;

/// The fonts and families the window draws with: Friz Quadrata when it is cached or downloads
/// within a few seconds, else IBM Plex alone. The game's font is never bundled.
pub fn fonts() -> (&'static [&'static [u8]], (&'static str, &'static str)) {
    match friz() {
        Ok(friz) => (Vec::leak(vec![PLEX, Vec::leak(friz)]), FRIZ_FAMILIES),
        Err(e) => {
            eprintln!("Friz Quadrata not loaded, drawing in IBM Plex: {e}");
            (Vec::leak(vec![PLEX]), FAMILIES)
        }
    }
}

fn friz() -> Result<Vec<u8>, String> {
    let dir = crate::cache::dir("fonts").ok_or("LOCALAPPDATA is not set")?;
    let path = dir.join("frizqt__.ttf");
    if let Ok(bytes) = std::fs::read(&path)
        && is_truetype(&bytes)
    {
        return Ok(bytes);
    }
    let url = format!("https://wago.tools/api/casc/{FRIZ_FILE_ID}?download");
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(4))).build().into();
    let mut response = agent.get(&url).call().map_err(|e| format!("{url}: {e}"))?;
    let bytes = response.body_mut().with_config().limit(4 << 20).read_to_vec().map_err(|e| format!("{url}: {e}"))?;
    if !is_truetype(&bytes) {
        return Err(format!("{url}: not a TrueType font"));
    }
    std::fs::create_dir_all(&dir).and_then(|()| crate::cache::write_atomic(&path, &bytes)).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(bytes)
}

fn is_truetype(bytes: &[u8]) -> bool {
    bytes.len() > 1024 && bytes.starts_with(&[0, 1, 0, 0])
}

pub struct Palette {
    pub rail: Color,
    pub ground: Color,
    pub surface: Color,
    pub line: Color,
    pub line2: Color,
    /// Row dividers, a step quieter than `line`, which rules the column heads and bands.
    pub rule: Color,
    pub selected: Color,
    /// The fill of what is chosen: a pill whose filter is on, a menu's current item.
    pub active: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub brass: Color,
    pub on_brass: Color,
    pub loss: Color,
    /// The caution icon beside a market figure the data cannot vouch for.
    pub warn: Color,
    /// Where a sale price goes, on the breakdown bar and its swatches: the auction house cut
    /// (with an edge so the faint fill reads), the materials and the profit.
    pub cut: Color,
    pub cut_edge: Color,
    pub materials: Color,
    pub profit: Color,
    /// Skill-up bands: still yellow, green, then grey.
    pub skill: [Color; 3],
    quality: [Color; 5],
}

static PALETTE: LazyLock<Palette> = LazyLock::new(|| {
    let c = |s: &str| hex(s).expect("palette hex is valid");
    let text = c("#EEE4D2");
    Palette {
        rail: c("#0F0D0B"),
        ground: c("#13110E"),
        surface: c("#1B1814"),
        line: c("#2E2822"),
        line2: c("#3A332B"),
        rule: c("#1F1B17"),
        selected: c("#221D17"),
        active: c("#26211B"),
        text,
        muted: c("#9A8D79"),
        faint: c("#6E6355"),
        brass: c("#D8A443"),
        on_brass: c("#1E1608"),
        loss: c("#E57560"),
        warn: c("#E3B860"),
        cut: text.alpha(0.08),
        cut_edge: text.alpha(0.14),
        materials: text.alpha(0.20),
        profit: text.alpha(0.64),
        skill: [c("#E0C24A"), c("#5FB86A"), c("#5E574E")],
        quality: [c("#9D9D9D"), c("#EEE4D2"), c("#4BD461"), c("#4F9CFF"), c("#B469FF")],
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
pub const BODY: TextStyle = plex(14.0, 20.0, 400.0);
pub const BODY_MEDIUM: TextStyle = plex(14.0, 20.0, 500.0);
pub const BODY_STRONG: TextStyle = plex(14.0, 20.0, 600.0);
/// The detail panel's item name.
pub const HEADING: TextStyle = plex(20.0, 28.0, 600.0);
/// The hero profit figure.
pub const FIGURE: TextStyle = plex(24.0, 32.0, 600.0);
pub const TITLE: TextStyle = FIGURE.tracking(-0.24);
