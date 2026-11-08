//! The tab title over where the prices come from, the zone, and how much the filters leave.

use rune::El;

use super::Stocked;
use crate::overview::shown;
use crate::view::header::{page, prices};
use crate::view::{App, Dims, Ready};

/// "… Thunder Bluff, 9 merchants. 12 of 40 items shown.", counting what the zone or search
/// finds before the filters.
pub fn header(s: &App, r: &Ready, f: &Stocked, d: Dims) -> El {
    let zone = &s.flips.zone;
    let place = if s.flips.search.text.trim().is_empty() {
        let merchants = f.zones.iter().find(|z| &z.name == zone).map_or(0, |z| z.merchants);
        format!("{zone}, {merchants} merchants.")
    } else {
        "Every zone.".to_string()
    };
    let found = f.rows.len() + f.hidden.as_ref().map_or(0, |h| h.count);
    page("Vendor flips", format!("{} {place} {}", prices(s, r), shown(f.rows.len(), found, "item")), d)
}
