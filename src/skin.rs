//! WoW Forever's own interface art: pieces cut from the game's texture atlases, downloaded from
//! wago.tools once and cached as PNGs. Nothing of it ships with the app.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::blp;
use crate::cache::{self, write_atomic};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    Background,
    OverviewCard,
    BackgroundCard,
    SlotFrame,
    ProgressBg,
    ProgressFrame,
    Dropdown,
    DropdownOpen,
    MenuBg,
    SearchLeft,
    SearchMid,
    SearchRight,
    Magnifier,
    RedLeft,
    RedMid,
    RedRight,
    RowSelected,
    TabSelected,
    Checkbox,
    Checkmark,
}

/// Where a piece lies: its name in the game's atlas tables, the atlas BLP's CASC file id, and
/// its `[left, top, right, bottom]` in that BLP's pixels.
pub struct Cut {
    pub name: &'static str,
    pub atlas: u32,
    pub rect: [u32; 4],
}

const fn cut(name: &'static str, atlas: u32, rect: [u32; 4]) -> Cut {
    Cut { name, atlas, rect }
}

/// Indexed by `Piece as usize`. From the UiTextureAtlas tables of Forever build 1.60.1.70862.
const CUTS: [Cut; Piece::ALL.len()] = [
    cut("heavybronze-frame-background-c60", 8216846, [0, 0, 1024, 1024]),
    cut("profession-overview-card-leatherworking-c60-2x", 8166364, [1, 1, 1329, 285]),
    cut("profession-background-card-leatherworking-c60-2x", 8166364, [1, 287, 721, 1255]),
    cut("profession-square-frame-c60-2x", 8164397, [131, 39, 211, 119]),
    cut("profession-progressbar-bg-c60-2x", 8164397, [219, 1, 967, 47]),
    cut("profession-progressbar-frame-c60-2x", 8164397, [219, 49, 967, 95]),
    cut("common-dropdown-b-button-c60-2x", 8069110, [1, 183, 195, 235]),
    cut("common-dropdown-b-button-open-c60-2x", 8069110, [197, 237, 391, 289]),
    cut("common-dropdown-bg-c60-2x", 8069110, [183, 1, 319, 137]),
    cut("common-search-border-left-c60", 8086476, [227, 1, 243, 41]),
    cut("common-search-border-middle-c60", 8086476, [1, 1, 225, 41]),
    cut("common-search-border-right-c60", 8086476, [1, 43, 17, 83]),
    cut("common-search-magnifyingglass", 3281887, [19, 85, 43, 109]),
    cut("128-redbutton-left-c60", 8063723, [391, 781, 505, 909]),
    cut("_128-redbutton-center-c60", 8063723, [0, 1, 64, 129]),
    cut("128-redbutton-right-c60", 8063723, [1, 391, 293, 519]),
    cut("common-button-list-small-selected-c60-2x", 8124219, [681, 321, 1019, 387]),
    cut("common-sidetab-selected-c60-2x", 8124215, [113, 1, 223, 121]),
    cut("checkbox-minimal-c60", 8086474, [1, 1, 31, 30]),
    cut("checkmark-minimal", 4614134, [1, 32, 31, 61]),
];

impl Piece {
    pub const ALL: [Piece; 20] = [
        Piece::Background,
        Piece::OverviewCard,
        Piece::BackgroundCard,
        Piece::SlotFrame,
        Piece::ProgressBg,
        Piece::ProgressFrame,
        Piece::Dropdown,
        Piece::DropdownOpen,
        Piece::MenuBg,
        Piece::SearchLeft,
        Piece::SearchMid,
        Piece::SearchRight,
        Piece::Magnifier,
        Piece::RedLeft,
        Piece::RedMid,
        Piece::RedRight,
        Piece::RowSelected,
        Piece::TabSelected,
        Piece::Checkbox,
        Piece::Checkmark,
    ];

    pub fn cut(self) -> &'static Cut {
        &CUTS[self as usize]
    }

    /// Its size in points: `-2x` art draws at half its pixel size.
    pub fn size(self) -> (f32, f32) {
        let [l, t, r, b] = self.cut().rect;
        let density = if self.cut().name.ends_with("-2x") { 2.0 } else { 1.0 };
        ((r - l) as f32 / density, (b - t) as f32 / density)
    }
}

pub fn dir() -> Option<PathBuf> {
    cache::dir("ui-v1")
}

pub fn path(dir: &Path, piece: Piece) -> PathBuf {
    dir.join(format!("{piece:?}.png"))
}

/// The `[left, top, right, bottom]` rectangle of an RGBA image `width` pixels wide, as its own
/// RGBA image. None when the rectangle reaches past the image.
pub fn crop(width: u32, rgba: &[u8], rect: [u32; 4]) -> Option<Vec<u8>> {
    let [l, t, r, b] = rect.map(|v| v as usize);
    let width = width as usize;
    if l >= r || t >= b || r > width || b * width * 4 > rgba.len() {
        return None;
    }
    Some((t..b).flat_map(|y| &rgba[(y * width + l) * 4..(y * width + r) * 4]).copied().collect())
}

/// The nine `[left, top, right, bottom]` rectangles, row by row, that `insets` (from each edge,
/// in the same order) cut a `width`×`height` piece into: corners, edges and the middle.
pub fn nine(width: u32, height: u32, insets: [u32; 4]) -> [[u32; 4]; 9] {
    let [l, t, r, b] = insets;
    let xs = [0, l, width.saturating_sub(r), width];
    let ys = [0, t, height.saturating_sub(b), height];
    std::array::from_fn(|i| [xs[i % 3], ys[i / 3], xs[i % 3 + 1], ys[i / 3 + 1]])
}

/// Puts every piece on disk, calling `on_ready` from a worker thread as each one is there,
/// cached or freshly cut. An atlas downloads only when one of its pieces is missing, and is
/// dropped once its pieces are written. Blocks until every atlas has landed or failed.
pub fn fetch(on_ready: impl Fn(Piece, PathBuf) + Sync) -> Result<(), String> {
    let dir = dir().ok_or("LOCALAPPDATA is not set")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut atlases: Vec<u32> = CUTS.iter().map(|c| c.atlas).collect();
    atlases.sort_unstable();
    atlases.dedup();
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).build().into();
    let failed: Vec<String> = std::thread::scope(|scope| {
        let (dir, agent, on_ready) = (&dir, &agent, &on_ready);
        let workers: Vec<_> = atlases.into_iter().map(|atlas| scope.spawn(move || fetch_atlas(agent, dir, atlas, on_ready))).collect();
        workers.into_iter().filter_map(|w| w.join().unwrap_or_else(|_| Err("panicked".into())).err()).collect()
    });
    if failed.is_empty() { Ok(()) } else { Err(failed.join("; ")) }
}

fn fetch_atlas(agent: &ureq::Agent, dir: &Path, atlas: u32, on_ready: &(impl Fn(Piece, PathBuf) + Sync)) -> Result<(), String> {
    let pieces: Vec<Piece> = Piece::ALL.into_iter().filter(|p| p.cut().atlas == atlas).collect();
    let (cached, missing): (Vec<Piece>, Vec<Piece>) = pieces.into_iter().partition(|&p| path(dir, p).exists());
    for p in cached {
        on_ready(p, path(dir, p));
    }
    if missing.is_empty() {
        return Ok(());
    }
    let url = format!("https://wago.tools/api/casc/{atlas}?download");
    let mut response = agent.get(&url).call().map_err(|e| format!("{url}: {e}"))?;
    let bytes = response.body_mut().with_config().limit(64 << 20).read_to_vec().map_err(|e| format!("{url}: {e}"))?;
    let (w, h, rgba) = blp::decode(&bytes).map_err(|e| format!("{url}: {e}"))?;
    drop(bytes);
    for p in missing {
        let [l, t, r, b] = p.cut().rect;
        let piece = crop(w, &rgba, p.cut().rect).ok_or_else(|| format!("{:?} reaches past its {w}x{h} atlas {atlas}", p))?;
        let png = encode_png(r - l, b - t, piece)?;
        let file = path(dir, p);
        write_atomic(&file, &png).map_err(|e| format!("{}: {e}", file.display()))?;
        on_ready(p, file);
    }
    Ok(())
}

fn encode_png(w: u32, h: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    let image = image::RgbaImage::from_raw(w, h, rgba).ok_or("piece size and pixels disagree")?;
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_holds_one_cut_per_piece_in_order() {
        for (i, p) in Piece::ALL.into_iter().enumerate() {
            assert_eq!(p as usize, i, "{p:?}");
        }
    }

    #[test]
    fn crops_a_rectangle_out_of_an_image() {
        let px = |v: u8| [v, v, v, 255];
        let image: Vec<u8> = (0..12).flat_map(px).collect();
        assert_eq!(crop(4, &image, [1, 1, 3, 3]), Some([5, 6, 9, 10].into_iter().flat_map(px).collect()));
        assert_eq!(crop(4, &image, [3, 0, 5, 1]), None);
        assert_eq!(crop(4, &image, [0, 2, 1, 4]), None);
    }

    #[test]
    fn cuts_a_piece_into_corners_edges_and_middle() {
        let cells = nine(194, 52, [16, 16, 56, 16]);
        assert_eq!(cells[0], [0, 0, 16, 16]);
        assert_eq!(cells[4], [16, 16, 138, 36]);
        assert_eq!(cells[8], [138, 36, 194, 52]);
        let image = vec![0; 194 * 52 * 4];
        let area: usize = cells.iter().map(|&c| crop(194, &image, c).map_or(0, |px| px.len() / 4)).sum();
        assert_eq!(area, 194 * 52);
    }

    #[test]
    fn sizes_double_density_art_at_half_its_pixels() {
        assert_eq!(Piece::SlotFrame.size(), (40.0, 40.0));
        assert_eq!(Piece::SearchLeft.size(), (16.0, 40.0));
    }

    #[test]
    fn caches_each_piece_by_name() {
        assert_eq!(path(Path::new("cache/ui-v1"), Piece::RedMid), Path::new("cache/ui-v1/RedMid.png"));
    }
}
