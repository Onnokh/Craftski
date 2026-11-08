//! The game's art as widgets draw it: one decoded image per piece, the slices of the pieces
//! that stretch around their content, and the layers that lay them behind an element. A widget
//! whose pieces have not decoded draws its flat look.

use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use rune::{Image, ImageCache, Pixels};

use crate::skin::{self, Piece};

struct Art {
    whole: Image,
    /// Row by row, for the pieces drawn by nine-slice; an empty cell is None.
    cells: Option<[Option<Image>; 9]>,
}

static ART: [OnceLock<Art>; Piece::ALL.len()] = [const { OnceLock::new() }; Piece::ALL.len()];
static SHOWN: AtomicU32 = AtomicU32::new(0);

/// How far in from each edge, in the piece's pixels, its corners and edges reach, for the
/// pieces that stretch by nine-slice.
fn insets(piece: Piece) -> Option<[u32; 4]> {
    match piece {
        Piece::Dropdown | Piece::DropdownOpen => Some([16, 16, 56, 16]),
        Piece::MenuBg => Some([24, 24, 24, 24]),
        _ => None,
    }
}

/// Decodes the piece cached at `path`, once per process.
pub fn show(cache: &ImageCache, piece: Piece, path: &Path) {
    ART[piece as usize].get_or_init(|| {
        let cells = insets(piece).and_then(|insets| match slice(cache, path, insets) {
            Ok(cells) => Some(cells),
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                None
            }
        });
        let (w, h) = piece.size();
        Art { whole: cache.file(path, w.max(h), 2.0), cells }
    });
}

fn slice(cache: &ImageCache, path: &Path, insets: [u32; 4]) -> Result<[Option<Image>; 9], String> {
    let image = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
    let (w, h) = image.dimensions();
    let rgba = image.into_raw();
    Ok(skin::nine(w, h, insets).map(|[l, t, r, b]| skin::crop(w, &rgba, [l, t, r, b]).map(|px| cache.rgba(r - l, b - t, px))))
}

/// Waits for every piece shown so far to decode, for a snapshot.
pub fn wait() {
    for art in ART.iter().filter_map(OnceLock::get) {
        let _ = art.whole.wait();
        for cell in art.cells.iter().flatten().flatten() {
            let _ = cell.wait();
        }
    }
}

fn ready(image: &Image) -> bool {
    image.is_ready() && image.error().is_none()
}

/// True when pieces have finished decoding since the last call, so the view redraws them.
pub fn newly_ready() -> bool {
    let mask = Piece::ALL.into_iter().filter(|&p| art(p).is_some()).fold(0, |m, p| m | 1 << p as u32);
    SHOWN.swap(mask, Ordering::Relaxed) != mask
}

/// The piece as one image, once it has decoded.
pub fn art(piece: Piece) -> Option<&'static Pixels> {
    let a = ART[piece as usize].get()?;
    let cells_ready = a.cells.iter().flatten().flatten().all(ready);
    (ready(&a.whole) && cells_ready).then(|| a.whole.pixels())
}
