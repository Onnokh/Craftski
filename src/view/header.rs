//! A tab's title over its meta line: how much the filters leave.

use rune::{El, col, text};

use super::{Dims, Ready};
use crate::overview::shown;
use crate::theme::{self, p};

pub fn header(r: &Ready, d: Dims) -> El {
    page("Leatherworking", shown(r.rows.len(), r.market.recipes.len(), "recipe"), d)
}

pub(super) fn page(title: &str, meta: String, d: Dims) -> El {
    col()
        .w_full()
        .h(72.0)
        .pt(20.0)
        .px(d.pad)
        .child(text(title).font(theme::TITLE).color(p().text).truncate())
        .child(text(meta).font(theme::CAPTION).color(p().muted).mt(4.0).truncate())
}
