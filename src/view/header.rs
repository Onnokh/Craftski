//! A tab's title over its meta line, and the banner it shares with the toolbar.

use std::f32::consts::FRAC_PI_2;

use rune::{Color, Cursor, El, Image, col, el, row, text};

use super::skin;
use super::{Dims, Ready, Recipes};
use crate::overview::shown;
use crate::recipes::Profession;
use crate::skin::Piece;
use crate::theme::{self, p};

/// The banner's height: the title block and the toolbar under it.
const BANNER: f32 = 128.0;
/// The overview art's drawn height, which overhangs the banner top and bottom.
const ART: f32 = 300.0;

pub const COFFEE: &[u8] = include_bytes!("../../assets/buymeacoffee.png");
const COFFEE_URL: &str = "https://buymeacoffee.com/onnokh1i";
/// The 312 by 88 pixel button, drawn at its own aspect.
const COFFEE_SIZE: (f32, f32) = (128.0, 36.0);

pub fn header(profession: Profession, r: &Ready, coffee: Option<&Image>, d: Dims) -> El {
    let meta = match r.recipes {
        Recipes::Shown => shown(r.rows.len(), r.market.recipes.len(), "recipe"),
        Recipes::Loading => "Loading recipes...".into(),
        Recipes::Failed(_) => "No recipes loaded.".into(),
    };
    page(profession.name(), meta, coffee, d)
}

/// The title over its meta line, with the Buy Me a Coffee button at the far end.
pub(super) fn page(title: &str, meta: String, coffee: Option<&Image>, d: Dims) -> El {
    let words = col()
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .child(text(title).font(theme::TITLE).color(p().gold).truncate())
        .child(text(meta).font(theme::CAPTION).color(p().muted).mt(4.0).truncate());
    row().w_full().h(72.0).pt(20.0).px(d.pad).gap(16.0).center_y().child(words).children(coffee.map(coffee_button))
}

fn coffee_button(image: &Image) -> El {
    el().key("coffee")
        .w(COFFEE_SIZE.0)
        .h(COFFEE_SIZE.1)
        .radius(8.0)
        .pixels(image.pixels())
        .shadow(6.0, 12.0, Color::BLACK.alpha(0.5))
        .press_scale(0.97)
        .cursor(Cursor::Pointer)
        .label("Buy me a coffee")
        .href(COFFEE_URL)
}

/// `header` over `toolbar`, on `art` faded into the dark from the left when given and decoded.
pub(super) fn banner(art: Option<Piece>, header: El, toolbar: El) -> El {
    let out = col().w_full().h(BANNER).clip_rounded(0.0).border_sides([0.0, 0.0, 1.0, 0.0], Color::BLACK);
    let out = match art.and_then(|a| Some((a, skin::art(a)?))) {
        Some((piece, px)) => {
            let (w, h) = piece.size();
            let shade = |a: f32| p().ground.alpha(a);
            out.child(el().key("banner-art").pin(None, Some((BANNER - ART) / 2.0), Some(-40.0), None).w(ART * w / h).h(ART).pixels(px).a11y_hidden())
                .child(el().key("banner-shade").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).linear_gradient(FRAC_PI_2, [(0.0, shade(0.92)), (0.45, shade(0.55)), (0.75, shade(0.0))]).a11y_hidden())
        }
        None => out,
    };
    out.child(header.key("title")).child(toolbar.key("toolbar"))
}
