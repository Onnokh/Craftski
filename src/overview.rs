//! What the page says about its data: how many rows the filters leave, and the status bar's
//! price feeds with their ages. UI-free.

use crate::prices::PriceBook;

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
    /// "PvP-Horde", "Region Forever-EU", or "Realm prices" when AppData names no scope.
    pub name: String,
    /// "17 min ago" and how fresh that is; None when AppData records no download time.
    pub age: Option<(String, Freshness)>,
}

impl Feed {
    /// A feed TSM downloaded at unix time `updated`, aged from `now`.
    pub fn new(name: String, updated: Option<u64>, now: u64) -> Feed {
        let age = updated.map(|t| now.saturating_sub(t));
        Feed { name, age: age.map(|s| (ago(s), Freshness::of(s))) }
    }

    /// The realm prices are read from, or "Realm prices" when AppData holds none.
    pub fn realm(book: &PriceBook, now: u64) -> Feed {
        match book.realm() {
            Some((name, realm)) => Feed::new(name.to_owned(), realm.updated, now),
            None => Feed::new("Realm prices".into(), None, now),
        }
    }

    /// The region's sales: "Region Forever-EU", or "Region prices" when AppData names no scope.
    pub fn region(book: &PriceBook, now: u64) -> Feed {
        let name = book.region_scope.as_deref().map_or_else(|| "Region prices".into(), |scope| format!("Region {scope}"));
        Feed::new(name, book.region_updated, now)
    }

    /// "PvP-Horde, 17 min ago", or the name alone when the age is unknown.
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
    fn a_feed_names_its_scope_and_age_and_never_ages_below_zero() {
        let feed = Feed::new("PvP-Horde".into(), Some(10_000), 10_000 + 17 * 60);
        assert_eq!(feed, Feed { name: "PvP-Horde".into(), age: Some(("17 min ago".into(), Freshness::Fresh)) });
        assert_eq!(feed.line(), "PvP-Horde, 17 min ago");
        assert_eq!(Feed::new("PvP-Horde".into(), Some(10_120), 10_000).line(), "PvP-Horde, just now");
        let undated = Feed::new("PvP-Horde".into(), None, 10_000);
        assert_eq!((undated.line().as_str(), undated.age), ("PvP-Horde", None));
    }

    #[test]
    fn the_status_bar_feeds_are_the_chosen_realm_and_the_region() {
        let mut book = PriceBook::default();
        (book.region_updated, book.region_scope) = (Some(10_000 + 120), Some("Forever-EU".into()));
        book.realm_prices_mut();
        book.realms.get_mut("PvP-Horde").unwrap().updated = Some(10_000);
        let now = 10_000 + 26 * 3600;
        let (realm, region) = (Feed::realm(&book, now), Feed::region(&book, now));
        assert_eq!((realm.line().as_str(), realm.age.map(|a| a.1)), ("PvP-Horde, 1 day ago", Some(Freshness::Stale)));
        assert_eq!(region.line(), "Region Forever-EU, 1 day ago");
    }

    #[test]
    fn feeds_name_their_kind_when_appdata_names_no_scope() {
        let book = PriceBook::default();
        assert_eq!((Feed::realm(&book, 0).line(), Feed::region(&book, 0).line()), ("Realm prices".into(), "Region prices".into()));
    }

    #[test]
    fn the_read_time_is_a_24_hour_clock() {
        assert_eq!(read_at(14, 2), "TSM AppHelper, read 14:02");
        assert_eq!(read_at(9, 0), "TSM AppHelper, read 09:00");
    }
}
