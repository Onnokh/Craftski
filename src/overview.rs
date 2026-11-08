//! The page header's meta line and the toolbar's count, for a whole profession under one price
//! source. UI-free.

use crate::profit::{Evaluation, PriceSource};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Overview {
    /// Recipes with a known profit.
    pub priced: usize,
    pub total: usize,
}

pub fn overview(evals: &[Evaluation]) -> Overview {
    Overview { priced: evals.iter().filter(|e| e.profit.is_some()).count(), total: evals.len() }
}

/// "WoW Forever, PvP-Horde. TSM realm prices, 9 minutes old." The realm and the age drop out
/// when the snapshot does not record them.
pub fn origin(scope: Option<&str>, source: PriceSource, age_secs: Option<u64>) -> String {
    let realm = scope.map_or_else(String::new, |s| format!(", {s}"));
    let source = match source {
        PriceSource::Realm => "realm",
        PriceSource::Region => "region",
    };
    let age = age_secs.map_or_else(|| " of unknown age".into(), |s| format!(", {} old", age(s)));
    format!("WoW Forever{realm}. TSM {source} prices{age}.")
}

/// How long ago a snapshot was taken, in its largest unit.
fn age(secs: u64) -> String {
    let (count, unit) = match (secs / 60, secs / 3600, secs / 86_400) {
        (_, _, d) if d > 0 => (d, "day"),
        (_, h, _) if h > 0 => (h, "hour"),
        (m, _, _) => (m, "minute"),
    };
    format!("{count} {unit}{}", if count == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::copper::Copper;

    fn eval(profit: Option<i64>) -> Evaluation {
        Evaluation { cost: profit.map(|_| Copper(100)), unpriced: vec![], resale: None, profit, margin: None, sold_per_day: None, auctions: None }
    }

    #[test]
    fn counts_the_recipes_with_a_known_profit() {
        assert_eq!(overview(&[eval(Some(38_173)), eval(None), eval(Some(-500))]), Overview { priced: 2, total: 3 });
    }

    #[test]
    fn the_meta_line_names_realm_source_and_age() {
        assert_eq!(origin(Some("PvP-Horde"), PriceSource::Realm, Some(9 * 60 + 30)), "WoW Forever, PvP-Horde. TSM realm prices, 9 minutes old.");
        assert_eq!(origin(None, PriceSource::Region, Some(3 * 3600 + 5 * 60)), "WoW Forever. TSM region prices, 3 hours old.");
        assert_eq!(origin(Some("PvP-Horde"), PriceSource::Realm, None), "WoW Forever, PvP-Horde. TSM realm prices of unknown age.");
    }

    #[test]
    fn ages_read_in_their_largest_unit() {
        assert_eq!(age(59), "0 minutes");
        assert_eq!(age(60), "1 minute");
        assert_eq!(age(86_400 + 3600), "1 day");
        assert_eq!(age(2 * 86_400 + 4 * 3600), "2 days");
    }
}
