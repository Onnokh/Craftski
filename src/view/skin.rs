//! The game's art as widgets draw it: one decoded image per piece, the slices of the pieces
//! that stretch around their content, and the layers that lay them behind an element. A widget
//! whose pieces have not decoded draws its flat look.

use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use rune::{Align, El, Image, ImageCache, Pixels, SurfaceFit, el};

use crate::skin::{self, Piece};

struct Art {
    whole: Image,
    /// Row by row, for the pieces drawn by nine-slice; an empty cell is None.
    cells: Option<[Option<Image>; 9]>,
}

static ART: [OnceLock<Art>; skin::COUNT] = [const { OnceLock::new() }; skin::COUNT];
static SHOWN: AtomicU64 = AtomicU64::new(0);
const _: () = assert!(skin::COUNT <= u64::BITS as usize);

/// How far in from each edge, in the piece's pixels, its corners and edges reach, for the
/// pieces that stretch by nine-slice.
fn insets(piece: Piece) -> Option<[u32; 4]> {
    match piece {
        Piece::MenuBg => Some([24, 24, 24, 24]),
        _ => None,
    }
}

/// Decodes the piece cached at `path`, once per process.
pub fn show(cache: &ImageCache, piece: Piece, path: &Path) {
    ART[piece.index()].get_or_init(|| {
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
    let mask = Piece::all().filter(|&p| art(p).is_some()).fold(0, |m, p| m | 1 << p.index());
    SHOWN.swap(mask, Ordering::Relaxed) != mask
}

/// The piece as one image, once it has decoded.
pub fn art(piece: Piece) -> Option<&'static Pixels> {
    let a = ART[piece.index()].get()?;
    let cells_ready = a.cells.iter().flatten().flatten().all(ready);
    (ready(&a.whole) && cells_ready).then(|| a.whole.pixels())
}

fn cells(piece: Piece) -> Option<[Option<&'static Pixels>; 9]> {
    art(piece)?;
    let cells = ART[piece.index()].get()?.cells.as_ref()?;
    Some(std::array::from_fn(|i| cells[i].as_ref().map(Image::pixels)))
}

/// A layer under or over its parent's content, in the box `pin` places it. The image fills a
/// child, because an element given one side takes the other from the image's aspect ratio.
fn layer(pixels: &Pixels) -> El {
    el().a11y_hidden().child(el().w_full().h_full().pixels(pixels))
}

/// `piece` stretched over its parent's whole box.
pub fn stretch(piece: Piece) -> Option<El> {
    art(piece).map(|px| layer(px).key(format!("{piece:?}")).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)))
}

/// `piece` covering its parent's box at its own aspect ratio, cropped away from `anchor`.
pub fn cover(piece: Piece, anchor: (Align, Align)) -> Option<El> {
    art(piece).map(|px| el().pixels_fit(px, SurfaceFit::Fill).surface_anchor(anchor.0, anchor.1).a11y_hidden().key(format!("{piece:?}")).pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)))
}

/// `piece` around its parent's box: its corners `caps` points across (left, top, right,
/// bottom), its edges and middle stretched between them.
pub fn nine_slice(piece: Piece, caps: [f32; 4]) -> Option<Vec<El>> {
    let cells = cells(piece)?;
    let [l, t, r, b] = caps;
    // Each axis: the near cap, the stretch between the caps, the far cap.
    let span = |i: usize, near: f32, far: f32| match i {
        0 => (Some(0.0), None, Some(near)),
        1 => (Some(near), Some(far), None),
        _ => (None, Some(0.0), Some(far)),
    };
    Some(
        cells
            .into_iter()
            .enumerate()
            .filter_map(|(i, cell)| {
                let ((left, right, w), (top, bottom, h)) = (span(i % 3, l, r), span(i / 3, t, b));
                let mut cell = layer(cell?).pin(left, top, right, bottom);
                if let Some(w) = w {
                    cell = cell.w(w);
                }
                if let Some(h) = h {
                    cell = cell.h(h);
                }
                Some(cell)
            })
            .collect(),
    )
}

/// Three pieces side by side across their parent's box: `left` and `right` `caps` points wide,
/// `middle` stretched between them.
pub fn three_slice([left, middle, right]: [Piece; 3], caps: [f32; 2]) -> Option<Vec<El>> {
    Some(vec![
        layer(art(left)?).pin(Some(0.0), Some(0.0), None, Some(0.0)).w(caps[0]),
        layer(art(middle)?).pin(Some(caps[0]), Some(0.0), Some(caps[1]), Some(0.0)),
        layer(art(right)?).pin(None, Some(0.0), Some(0.0), Some(0.0)).w(caps[1]),
    ])
}
