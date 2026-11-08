//! The profession title and where its prices come from.

use rune::{El, col, text};

use super::{App, Dims, Ready};
use crate::overview::origin;
use crate::profit::PriceSource;
use crate::theme::{self, p};

pub fn header(s: &App, r: &Ready, d: Dims) -> El {
    let book = &r.market.book;
    let (scope, updated) = match s.source {
        PriceSource::Realm => (book.realm_scope.as_deref(), book.realm_updated),
        PriceSource::Region => (book.region_scope.as_deref(), book.region_updated),
    };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    col()
        .w_full()
        .h(72.0)
        .pt(20.0)
        .px(d.pad)
        .child(text("Leatherworking").font(theme::TITLE).color(p().text).truncate())
        .child(text(origin(scope, s.source, updated.map(|t| now.saturating_sub(t)))).font(theme::CAPTION).color(p().muted).mt(4.0).truncate())
}
