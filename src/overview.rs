//! What the page says about its data: how many rows the filters leave, and the status bar's
//! price feeds with their ages. UI-free.

use crate::prices::PriceBook;
use crate::profit::PriceSource;

/// "116 of 592 recipes shown." for `noun` "recipe".
pub fn shown(shown: usize, total: usize, noun: &str) -> String {
    format!("{shown} of {total} {noun}{} shown.", if total == 1 { "" } else { "s" })
}

/// How far to trust prices of a given age. TSM's AppHelper refreshes them about hourly while
/// the desktop app runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    /// Three hours or more: the app has likely stopped syncing.
    Aging,
    /// A day or more.
    Stale,
}

impl Freshness {
    pub fn of(age_secs: u64) -> Freshness {
        match age_secs {
            s if s >= 24 * 3600 => Freshness::Stale,
            s if s >= 3 * 3600 => Freshness::Aging,
            _ => Freshness::Fresh,
        }
    }
}

/// "just now", "17 min ago", "1 hour ago", "2 days ago": the age in its largest whole unit.
pub fn ago(secs: u64) -> String {
    let plural = |n: u64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match (secs / 60, secs / 3600, secs / 86_400) {
        (0, _, _) => "just now".into(),
        (m, 0, _) => format!("{m} min ago"),
        (_, h, 0) => plural(h, "hour"),
        (_, _, d) => plural(d, "day"),
    }
}

/// One price feed as the status bar names it.
#[derive(Debug, PartialEq, Eq)]
pub struct Feed {
    /// "Realm PvP-Horde", "PvP-Horde" when short, or "Realm prices" when AppData names no scope.
    pub name: String,
    /// "17 min ago" and how fresh that is; None when AppData records no download time.
    pub age: Option<(String, Freshness)>,
}

impl Feed {
    pub fn new(source: PriceSource, scope: Option<&str>, age_secs: Option<u64>, short: bool) -> Feed {
        let kind = match source {
            PriceSource::Realm => "Realm",
            PriceSource::Region => "Region",
        };
        let name = match scope {
            Some(scope) if short => scope.to_owned(),
            Some(scope) => format!("{kind} {scope}"),
            None => format!("{kind} prices"),
        };
        Feed { name, age: age_secs.map(|s| (ago(s), Freshness::of(s))) }
    }

    /// The realm and region feeds of `book` at unix time `now`.
    pub fn both(book: &PriceBook, now: u64, short: bool) -> [Feed; 2] {
        let age = |updated: Option<u64>| updated.map(|t| now.saturating_sub(t));
        [
            Feed::new(PriceSource::Realm, book.realm().map(|(name, _)| name), age(book.realm().and_then(|(_, b)| b.updated)), short),
            Feed::new(PriceSource::Region, book.region_scope.as_deref(), age(book.region_updated), short),
        ]
    }

    /// "Realm PvP-Horde, 17 min ago", or the name alone when the age is unknown.
    pub fn line(&self) -> String {
        match &self.age {
            Some((ago, _)) => format!("{}, {ago}", self.name),
            None => self.name.clone(),
        }
    }
}

/// "TSM AppHelper, read 14:02", from the local wall-clock hour and minute of the last read.
pub fn read_at(hour: u32, minute: u32) -> String {
    format!("TSM AppHelper, read {hour:02}:{minute:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_count_says_how_many_recipes_the_filters_leave() {
        assert_eq!(shown(116, 592, "recipe"), "116 of 592 recipes shown.");
        assert_eq!(shown(1, 1, "recipe"), "1 of 1 recipe shown.");
    }

    #[test]
    fn ages_read_in_their_largest_short_unit() {
        assert_eq!(ago(0), "just now");
        assert_eq!(ago(59), "just now");
        assert_eq!(ago(60), "1 min ago");
        assert_eq!(ago(17 * 60 + 40), "17 min ago");
        assert_eq!(ago(3599), "59 min ago");
        assert_eq!(ago(3600), "1 hour ago");
        assert_eq!(ago(7 * 3600 + 59 * 60), "7 hours ago");
        assert_eq!(ago(86_400 + 3600), "1 day ago");
        assert_eq!(ago(2 * 86_400 + 4 * 3600), "2 days ago");
    }

    #[test]
    fn prices_turn_aging_at_three_hours_and_stale_at_a_day() {
        assert_eq!(Freshness::of(0), Freshness::Fresh);
        assert_eq!(Freshness::of(3 * 3600 - 1), Freshness::Fresh);
        assert_eq!(Freshness::of(3 * 3600), Freshness::Aging);
        assert_eq!(Freshness::of(24 * 3600 - 1), Freshness::Aging);
        assert_eq!(Freshness::of(24 * 3600), Freshness::Stale);
        assert_eq!(Freshness::of(30 * 86_400), Freshness::Stale);
    }

    #[test]
    fn a_feed_names_its_scope_and_age() {
        let feed = Feed::new(PriceSource::Realm, Some("PvP-Horde"), Some(17 * 60), false);
        assert_eq!(feed, Feed { name: "Realm PvP-Horde".into(), age: Some(("17 min ago".into(), Freshness::Fresh)) });
        assert_eq!(feed.line(), "Realm PvP-Horde, 17 min ago");
        assert_eq!(Feed::new(PriceSource::Region, Some("Forever-EU"), Some(7 * 3600), false).line(), "Region Forever-EU, 7 hours ago");
    }

    #[test]
    fn a_short_feed_drops_its_kind_but_keeps_it_without_a_scope() {
        assert_eq!(Feed::new(PriceSource::Realm, Some("PvP-Horde"), Some(17 * 60), true).line(), "PvP-Horde, 17 min ago");
        assert_eq!(Feed::new(PriceSource::Region, None, Some(7 * 3600), true).line(), "Region prices, 7 hours ago");
    }

    #[test]
    fn a_feed_drops_what_appdata_does_not_record() {
        assert_eq!(Feed::new(PriceSource::Realm, None, Some(30), false).line(), "Realm prices, just now");
        let undated = Feed::new(PriceSource::Region, Some("Forever-EU"), None, false);
        assert_eq!((undated.line().as_str(), undated.age), ("Region Forever-EU", None));
        assert_eq!(Feed::new(PriceSource::Realm, None, None, false).line(), "Realm prices");
    }

    #[test]
    fn both_feeds_age_from_now_and_never_go_negative() {
        let mut book = PriceBook::default();
        (book.region_updated, book.region_scope) = (Some(10_000 + 120), Some("Forever-EU".into()));
        book.realm_prices_mut();
        book.realms.get_mut("PvP-Horde").unwrap().updated = Some(10_000);
        let [realm, region] = Feed::both(&book, 10_000 + 26 * 3600, false);
        assert_eq!((realm.line().as_str(), realm.age.map(|a| a.1)), ("Realm PvP-Horde, 1 day ago", Some(Freshness::Stale)));
        assert_eq!(region.line(), "Region Forever-EU, 1 day ago");
        assert_eq!(Feed::both(&book, 10_000, false)[1].line(), "Region Forever-EU, just now");
    }

    #[test]
    fn the_read_time_is_a_24_hour_clock() {
        assert_eq!(read_at(14, 2), "TSM AppHelper, read 14:02");
        assert_eq!(read_at(9, 0), "TSM AppHelper, read 09:00");
    }
}
