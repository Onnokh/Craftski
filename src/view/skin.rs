//! The game's art as widgets draw it: one decoded image per piece, and the layers that lay
//! them behind an element. A widget whose pieces have not decoded draws its flat look.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use rune::{Align, El, Image, ImageCache, Pixels, SurfaceFit, el};

use crate::skin::{self, Art, Piece};

static ART: [OnceLock<Image>; skin::COUNT] = [const { OnceLock::new() }; skin::COUNT];
static SHOWN: AtomicU64 = AtomicU64::new(0);
const _: () = assert!(skin::COUNT <= u64::BITS as usize);

/// Decodes the piece's art, once per process.
pub fn show(cache: &ImageCache, piece: Piece, art: Art) {
    ART[piece.index()].get_or_init(|| match art {
        Art::File(path) => {
            let (w, h) = piece.size();
            cache.file(path, w.max(h), 2.0)
        }
        Art::Rgba(piece) => cache.rgba(piece.width, piece.height, piece.pixels),
    });
}

/// Waits for every piece shown so far to decode, for a snapshot.
#[cfg(not(target_arch = "wasm32"))]
pub fn wait() {
    for image in ART.iter().filter_map(OnceLock::get) {
        let _ = image.wait();
    }
}

/// True when pieces have finished decoding since the last call, so the view redraws them.
pub fn newly_ready() -> bool {
    let mask = Piece::all().filter(|&p| art(p).is_some()).fold(0, |m, p| m | 1 << p.index());
    SHOWN.swap(mask, Ordering::Relaxed) != mask
}

/// The piece as one image, once it has decoded.
pub fn art(piece: Piece) -> Option<&'static Pixels> {
    let image = ART[piece.index()].get()?;
    (image.is_ready() && image.error().is_none()).then(|| image.pixels())
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

/// Eight pieces around their parent's box, top row first, where the game cut a border's corners
/// and edges apart: corners `edge` points square, edges `edge` points thick and stretched
/// between them.
pub fn frame(pieces: [Piece; 8], edge: f32) -> Option<Vec<El>> {
    let arts = pieces.map(art);
    let near = (Some(0.0), None);
    let far = (None, Some(0.0));
    let between = (Some(edge), Some(edge));
    let spans = [(near, near), (between, near), (far, near), (near, between), (far, between), (near, far), (between, far), (far, far)];
    let mut out = Vec::with_capacity(8);
    for (px, ((left, right), (top, bottom))) in arts.into_iter().zip(spans) {
        let mut piece = layer(px?).pin(left, top, right, bottom);
        if left.is_none() || right.is_none() {
            piece = piece.w(edge);
        }
        if top.is_none() || bottom.is_none() {
            piece = piece.h(edge);
        }
        out.push(piece);
    }
    Some(out)
}

