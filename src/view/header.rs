//! A tab's title over its meta line: where its prices come from and how much the filters
//! leave.

use rune::{El, col, text};

use super::{App, Dims, Ready};
use crate::overview::{origin, shown};
use crate::profit::PriceSource;
use crate::theme::{self, p};

pub fn header(s: &App, r: &Ready, d: Dims) -> El {
    page("Leatherworking", format!("{} {}", prices(s, r), shown(r.rows.len(), r.market.recipes.len(), "recipe")), d)
}

/// "WoW Forever, PvP-Horde. TSM realm prices, 33 minutes old." for the open price source.
pub(super) fn prices(s: &App, r: &Ready) -> String {
    let book = &r.market.book;
    let (scope, updated) = match s.source {
        PriceSource::Realm => (book.realm_scope.as_deref(), book.realm_updated),
        PriceSource::Region => (book.region_scope.as_deref(), book.region_updated),
    };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    origin(scope, s.source, updated.map(|t| now.saturating_sub(t)))
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
