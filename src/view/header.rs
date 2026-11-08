//! A tab's title over its meta line, and the banner it shares with the toolbar.

use std::f32::consts::FRAC_PI_2;

use rune::{Color, El, col, el, text};

use super::skin;
use super::{Dims, Ready};
use crate::overview::shown;
use crate::skin::Piece;
use crate::theme::{self, p};

/// The banner's height: the title block and the toolbar under it.
const BANNER: f32 = 128.0;
/// The overview art's drawn height, which overhangs the banner top and bottom.
const ART: f32 = 300.0;

pub fn header(r: &Ready, d: Dims) -> El {
    page("Leatherworking", shown(r.rows.len(), r.market.recipes.len(), "recipe"), d)
}

pub(super) fn page(title: &str, meta: String, d: Dims) -> El {
    col()
        .w_full()
        .h(72.0)
        .pt(20.0)
        .px(d.pad)
        .child(text(title).font(theme::TITLE).color(p().gold).truncate())
        .child(text(meta).font(theme::CAPTION).color(p().muted).mt(4.0).truncate())
}

/// `header` over `toolbar`, on `art` faded into the dark from the left when given and decoded.
pub(super) fn banner(art: Option<Piece>, header: El, toolbar: El) -> El {
    let out = col().w_full().h(BANNER).clip_rounded(0.0).border_sides([0.0, 0.0, 1.0, 0.0], Color::BLACK);
    let out = match art.and_then(|a| Some((a, skin::art(a)?))) {
        Some((piece, px)) => {
            let (w, h) = piece.size();
            let shade = |a: f32| p().ground.alpha(a);
            out.child(el().pin(None, Some((BANNER - ART) / 2.0), Some(-40.0), None).w(ART * w / h).h(ART).pixels(px).a11y_hidden())
                .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).linear_gradient(FRAC_PI_2, [(0.0, shade(0.92)), (0.45, shade(0.55)), (0.75, shade(0.0))]).a11y_hidden())
        }
        None => out,
    };
    out.child(header).child(toolbar)
}
