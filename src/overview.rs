//! The page header's meta line: where the prices come from and how many recipes the filters
//! leave. UI-free.

use crate::profit::PriceSource;

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

/// "116 of 592 recipes shown."
pub fn shown(shown: usize, total: usize) -> String {
    format!("{shown} of {total} recipe{} shown.", if total == 1 { "" } else { "s" })
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

    #[test]
    fn the_count_says_how_many_recipes_the_filters_leave() {
        assert_eq!(shown(116, 592), "116 of 592 recipes shown.");
        assert_eq!(shown(1, 1), "1 of 1 recipe shown.");
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
