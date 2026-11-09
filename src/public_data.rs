//! TSM's free public price files for WoW Forever, for a player without the TSM desktop app: one
//! auction house's file and its region's, kept under `%LOCALAPPDATA%\Craftski\public`.
//!
//! The host answers a burst of requests with HTTP 429 and an hour's `Retry-After`, though its
//! docs promise no limits. So each house costs two downloads, a cached file is downloaded again
//! only once it is due, a refused or failed download holds every download back for a while, and
//! nothing retries in a loop.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cache;
use crate::copper::Copper;
use crate::market::Read;
use crate::merchants::Side;
use crate::overview;
use crate::prices::{PriceBook, RealmBook, RealmPrice, RegionPrice};

const BASE: &str = "https://public-data.tradeskillmaster.com/forever";
/// TSM refreshes a realm's file about hourly and the region's a few times a day.
const REALM_MAX_AGE: u64 = 3600;
const REGION_MAX_AGE: u64 = 6 * 3600;
/// The youngest a file may be for a Reload to download it again, and how long a failed
/// download holds the next one back.
const FLOOR: u64 = 5 * 60;
/// The hold after a 429 that names no `Retry-After` in seconds, the length the host has sent.
const DEFAULT_RETRY_AFTER: u64 = 3600;
/// The hold on every download, shared by both files and every house: the host limits by address.
const HOLD: &str = "hold.txt";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Region {
    #[default]
    Eu,
    Us,
    Kr,
    Tw,
}

impl Region {
    pub const ALL: [Region; 4] = [Region::Eu, Region::Us, Region::Kr, Region::Tw];

    pub fn slug(self) -> &'static str {
        match self {
            Region::Eu => "eu",
            Region::Us => "us",
            Region::Kr => "kr",
            Region::Tw => "tw",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Region::Eu => "EU",
            Region::Us => "US",
            Region::Kr => "KR",
            Region::Tw => "TW",
        }
    }

    /// The region as AppData.lua scopes it, such as "Forever-EU".
    pub fn name(self) -> String {
        format!("Forever-{}", self.label())
    }

    pub fn parse(slug: &str) -> Option<Region> {
        Region::ALL.into_iter().find(|r| r.slug() == slug)
    }
}

/// Forever's four realms, each in every region.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ForeverRealm {
    Normal,
    #[default]
    PvP,
    Roleplay,
    RoleplayPvp,
}

impl ForeverRealm {
    pub const ALL: [ForeverRealm; 4] = [ForeverRealm::Normal, ForeverRealm::PvP, ForeverRealm::Roleplay, ForeverRealm::RoleplayPvp];

    pub fn slug(self) -> &'static str {
        match self {
            ForeverRealm::Normal => "normal",
            ForeverRealm::PvP => "pvp",
            ForeverRealm::Roleplay => "roleplay",
            ForeverRealm::RoleplayPvp => "roleplay-pvp",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ForeverRealm::Normal => "Normal",
            ForeverRealm::PvP => "PvP",
            ForeverRealm::Roleplay => "Roleplay",
            ForeverRealm::RoleplayPvp => "Roleplay PvP",
        }
    }

    pub fn parse(slug: &str) -> Option<ForeverRealm> {
        ForeverRealm::ALL.into_iter().find(|r| r.slug() == slug)
    }
}

/// A realm in a region. Its auction house is the character's faction's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PublicHouse {
    pub region: Region,
    pub realm: ForeverRealm,
}

impl PublicHouse {
    /// The auction house as AppData.lua scopes it, such as "PvP-Horde".
    pub fn scope(self, side: Side) -> String {
        format!("{}-{}", self.realm.name(), side.name())
    }

    fn realm_file(self, side: Side) -> File {
        let house = format!("{}-{}", self.realm.slug(), side.name().to_lowercase());
        File { url: format!("{BASE}/{}/realm/{house}/items.csv", self.region.slug()), name: format!("{}-realm-{house}.csv", self.region.slug()), max_age: REALM_MAX_AGE }
    }

    fn region_file(self) -> File {
        File { url: format!("{BASE}/{}/region/items.csv", self.region.slug()), name: format!("{}-region.csv", self.region.slug()), max_age: REGION_MAX_AGE }
    }
}

/// How eagerly [`load`] downloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refresh {
    /// Never; reads the cache alone.
    CacheOnly,
    /// A file missing from the cache or older than TSM refreshes it.
    IfDue,
    /// A file missing or older than [`FLOOR`], as the player's Reload asks.
    Now,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Why {
    RateLimited,
    Unreachable,
}

/// No download starts before `until`, unix seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Hold {
    until: u64,
    why: Why,
}

impl Hold {
    /// The hold after a download failed at `now`: the host's `Retry-After` for a 429, none for
    /// a 404, which says only that one house has no file, else [`FLOOR`].
    fn after(failure: &Failure, now: u64) -> Option<Hold> {
        match failure {
            Failure::RateLimited { retry_after } => Some(Hold { until: now + retry_after.unwrap_or(DEFAULT_RETRY_AFTER), why: Why::RateLimited }),
            Failure::NotFound => None,
            Failure::Other(_) => Some(Hold { until: now + FLOOR, why: Why::Unreachable }),
        }
    }

    fn parse(text: &str) -> Option<Hold> {
        let (until, why) = text.trim().split_once(' ')?;
        let why = match why {
            "rate-limited" => Why::RateLimited,
            "unreachable" => Why::Unreachable,
            _ => return None,
        };
        Some(Hold { until: until.parse().ok()?, why })
    }

    fn text(self) -> String {
        format!("{} {}", self.until, if self.why == Why::RateLimited { "rate-limited" } else { "unreachable" })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Plan {
    Cached,
    Download,
    /// Due, but held back until the hold ends.
    Held(Hold),
}

/// Whether to download a file cached `age` seconds ago, None when it isn't cached.
fn plan(age: Option<u64>, max_age: u64, refresh: Refresh, hold: Option<Hold>, now: u64) -> Plan {
    let wanted = match (age, refresh) {
        (_, Refresh::CacheOnly) => false,
        (None, _) => true,
        (Some(age), Refresh::IfDue) => age >= max_age,
        (Some(age), Refresh::Now) => age >= FLOOR,
    };
    match hold.filter(|h| h.until > now) {
        _ if !wanted => Plan::Cached,
        Some(h) => Plan::Held(h),
        None => Plan::Download,
    }
}

/// Why a cached file stands in for a due one, or why there are no prices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stale {
    hold: Hold,
    /// Seconds since the cached copy was downloaded, None without one.
    age: Option<u64>,
}

impl Stale {
    fn words(self, now: u64) -> String {
        match self.hold.why {
            Why::RateLimited => format!("TSM public data rate-limited, retrying in {} min", self.hold.until.saturating_sub(now).div_ceil(60).max(1)),
            Why::Unreachable => match self.age {
                Some(age) => format!("TSM public data unreachable, showing prices from {}", overview::ago(age)),
                None => format!("TSM public data unreachable, retrying in {} min", self.hold.until.saturating_sub(now).div_ceil(60).max(1)),
            },
        }
    }
}

/// One file a house reads.
struct File {
    url: String,
    /// Its name in the cache.
    name: String,
    /// How old the cached copy may grow before it is due.
    max_age: u64,
}

/// Why a download failed.
#[derive(Debug, PartialEq)]
pub enum Failure {
    /// HTTP 429, with its `Retry-After` in seconds when it gave one.
    RateLimited { retry_after: Option<u64> },
    NotFound,
    Other(String),
}

/// The one place a request leaves the app.
fn download(url: &str) -> Result<String, Failure> {
    let mut response = ureq::get(url)
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(60)))
        .build()
        .call()
        .map_err(|e| Failure::Other(format!("{url}: {e}")))?;
    match response.status().as_u16() {
        429 => Err(Failure::RateLimited { retry_after: response.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse().ok()) }),
        404 => Err(Failure::NotFound),
        200 => response.body_mut().with_config().limit(64 << 20).read_to_string().map_err(|e| Failure::Other(format!("{url}: {e}"))),
        status => Err(Failure::Other(format!("{url}: HTTP {status}"))),
    }
}

/// Whether a refresh of `house` would download anything now. Reads only file times, so it is
/// cheap enough for the UI thread.
pub fn due(house: PublicHouse, side: Side, now: u64) -> bool {
    let Some(dir) = cache::dir("public") else { return false };
    let hold = read_hold(&dir);
    [house.realm_file(side), house.region_file()].iter().any(|f| plan(age(&dir.join(&f.name), now), f.max_age, Refresh::IfDue, hold, now) == Plan::Download)
}

/// `house`'s prices for `side`'s auction house, from the cache or downloaded as `refresh`
/// allows. A file that could not be downloaded falls back to its cached copy, and `Read`'s
/// warning says why; Err only when a file has no cached copy to fall back to.
pub fn load(house: PublicHouse, side: Side, refresh: Refresh) -> Result<(PriceBook, Read), String> {
    let dir = cache::dir("public").ok_or("TSM public data: no LOCALAPPDATA folder to keep it in")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("TSM public data: {}: {e}", dir.display()))?;
    let now = unix_now();
    let mut hold = read_hold(&dir);
    let mut stale = None;
    let realm = fetch(&dir, &house.realm_file(side), refresh, now, &mut hold, &mut stale)?;
    let region = fetch(&dir, &house.region_file(), refresh, now, &mut hold, &mut stale)?;
    let (region, region_updated) = parse_region(&region).map_err(|e| format!("TSM public data, {}: {e}", house.region_file().name))?;
    let realm = parse_realm(&realm).map_err(|e| format!("TSM public data, {}: {e}", house.realm_file(side).name))?;
    Ok((book(house, side, realm, region, region_updated), Read { at: SystemTime::now(), modified: None, warning: stale.map(|s: Stale| s.words(now)) }))
}

fn book(house: PublicHouse, side: Side, realm: RealmBook, region: HashMap<u32, RegionPrice>, region_updated: Option<u64>) -> PriceBook {
    let scope = house.scope(side);
    let mut book = PriceBook::default();
    (book.realms, book.region, book.region_updated, book.region_scope) = (BTreeMap::from([(scope.clone(), realm)]), region, region_updated, Some(house.region.name()));
    book.choose_realm(&scope);
    book
}

/// `file`'s text, downloaded when `plan` says so and cached, or the cached copy. A download
/// that fails sets `hold`, saved for the next run, and `stale`.
fn fetch(dir: &Path, file: &File, refresh: Refresh, now: u64, hold: &mut Option<Hold>, stale: &mut Option<Stale>) -> Result<String, String> {
    let path = dir.join(&file.name);
    let cached_age = age(&path, now);
    let cached = || std::fs::read_to_string(&path).map_err(|e| format!("TSM public data: {}: {e}", path.display()));
    let held = match plan(cached_age, file.max_age, refresh, *hold, now) {
        Plan::Cached => return cached(),
        Plan::Held(h) => h,
        Plan::Download => match download(&file.url) {
            Ok(text) => {
                if let Err(e) = cache::write_atomic(&path, text.as_bytes()) {
                    eprintln!("{} not cached: {e}", path.display());
                }
                return Ok(text);
            }
            Err(failure) => {
                eprintln!("TSM public data: {failure:?}");
                let Some(h) = Hold::after(&failure, now) else {
                    return Err(format!("TSM publishes no public data for this auction house ({})", file.url));
                };
                if let Err(e) = std::fs::write(dir.join(HOLD), h.text()) {
                    eprintln!("download hold not saved: {e}");
                }
                *hold = Some(h);
                match (cached_age, failure) {
                    (None, Failure::Other(e)) => return Err(format!("TSM public data: {e}")),
                    _ => h,
                }
            }
        },
    };
    let why = Stale { hold: held, age: cached_age };
    if cached_age.is_none() {
        return Err(why.words(now));
    }
    stale.get_or_insert(why);
    cached()
}

fn read_hold(dir: &Path) -> Option<Hold> {
    std::fs::read_to_string(dir.join(HOLD)).ok().as_deref().and_then(Hold::parse)
}

/// Seconds since `path` was written, None when it isn't there.
fn age(path: &Path, now: u64) -> Option<u64> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    Some(now.saturating_sub(unix(modified)))
}

fn unix(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn unix_now() -> u64 {
    unix(SystemTime::now())
}

/// A realm's `items.csv`: `itemId,name,marketValue,minBuyout,recent,historical,updatedAt`.
pub fn parse_realm(csv: &str) -> Result<RealmBook, String> {
    let mut book = RealmBook::default();
    for_rows(csv, ["itemId", "marketValue", "minBuyout", "recent", "historical", "updatedAt"], |[id, market_value, min_buyout, recent, historical, updated]| {
        let price = RealmPrice { market_value: copper(market_value)?, min_buyout: copper(min_buyout)?, market_value_recent: copper(recent)?, historical: copper(historical)?, num_auctions: None };
        book.prices.insert(item(id)?, price);
        book.updated = book.updated.max(Some(iso_secs(updated).ok_or_else(|| format!("updatedAt {updated:?}"))?));
        Ok(())
    })?;
    Ok(book)
}

/// The region's `items.csv`: `itemId,name,marketValue,historical,avgSalePrice,saleRate,soldPerDay,updatedAt`,
/// with the newest `updatedAt`.
pub fn parse_region(csv: &str) -> Result<(HashMap<u32, RegionPrice>, Option<u64>), String> {
    let (mut prices, mut updated) = (HashMap::new(), None);
    for_rows(csv, ["itemId", "marketValue", "historical", "avgSalePrice", "saleRate", "soldPerDay", "updatedAt"], |[id, market_value, historical, sale_avg, sale_rate, sold_per_day, at]| {
        let price = RegionPrice { market_value: copper(market_value)?, historical: copper(historical)?, sale_avg: copper(sale_avg)?, sale_rate: decimal(sale_rate)?, sold_per_day: decimal(sold_per_day)? };
        prices.insert(item(id)?, price);
        updated = updated.max(Some(iso_secs(at).ok_or_else(|| format!("updatedAt {at:?}"))?));
        Ok(())
    })?;
    Ok((prices, updated))
}

/// Calls `row` with each record's cells in the order `columns` names them, found by header.
fn for_rows<const N: usize>(csv: &str, columns: [&str; N], mut row: impl FnMut([&str; N]) -> Result<(), String>) -> Result<(), String> {
    let mut reader = csv::Reader::from_reader(csv.as_bytes());
    let headers = reader.headers().map_err(|e| e.to_string())?.clone();
    let mut at = [0; N];
    for (slot, name) in at.iter_mut().zip(columns) {
        *slot = headers.iter().position(|h| h == name).ok_or_else(|| format!("no {name} column"))?;
    }
    for (n, record) in reader.records().enumerate() {
        let record = record.map_err(|e| e.to_string())?;
        let cells = at.map(|i| record.get(i).unwrap_or_default());
        row(cells).map_err(|e| format!("row {}: {e}", n + 1))?;
    }
    Ok(())
}

fn item(cell: &str) -> Result<u32, String> {
    cell.parse().map_err(|_| format!("itemId {cell:?}"))
}

/// TSM writes 0 where it has no data, so a zero price is a missing one.
fn copper(cell: &str) -> Result<Option<Copper>, String> {
    let raw: u64 = cell.parse().map_err(|_| format!("price {cell:?}"))?;
    Ok((raw > 0).then_some(Copper(raw)))
}

fn decimal(cell: &str) -> Result<Option<f64>, String> {
    let v: f64 = cell.parse().map_err(|_| format!("number {cell:?}"))?;
    Ok((v > 0.0).then_some(v))
}

/// Unix seconds of a UTC time written `2026-11-08T19:12:28Z`.
pub fn iso_secs(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<u64>().ok();
    let (y, m, d, hh, mm, ss) = (num(0..4)?, num(5..7)?, num(8..10)?, num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1970..=9999).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    // Days from 1970-01-01 to the date, counting years from March so the leap day ends one.
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y / 400;
    let year_of_era = y % 400;
    let day_of_year = (153 * m + 2) / 5 + d - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = (era * 146_097 + day_of_era).checked_sub(719_468)?;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REALM: &str = r#"itemId,name,marketValue,minBuyout,recent,historical,updatedAt
2318,Light Leather,32,28,34,32,2026-11-08T19:12:28Z
2391,Rusted Chain Gloves,119,0,0,150,2026-11-08T19:12:28Z
3041,"""Mage-Eye"" Blunderbuss",8198,3800,3800,18460,2026-11-08T19:12:28Z
"#;

    const REGION: &str = r#"itemId,name,marketValue,historical,avgSalePrice,saleRate,soldPerDay,updatedAt
2318,Light Leather,33,30,28,0.961,9140.071,2026-11-07T00:55:06Z
791,Gnarled Ash Staff,35000,38700,0,0,0,2026-11-07T00:55:06Z
4831,Stalking Pants,10661,12300,9405,0.5,0,2026-11-07T00:55:06Z
"#;

    #[test]
    fn utc_times_read_as_unix_seconds() {
        assert_eq!(iso_secs("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_secs("2026-11-08T19:12:28Z"), Some(1_794_165_148));
        assert_eq!(iso_secs("2024-02-29T12:00:00Z"), Some(1_709_208_000));
        assert_eq!(iso_secs("2000-03-01T00:00:00Z"), Some(951_868_800));
        assert_eq!((iso_secs("2026-11-08 19:12:28Z"), iso_secs("2026-11-08T19:12:28"), iso_secs("2026-13-08T19:12:28Z")), (None, None, None));
    }

    #[test]
    fn a_realm_file_reads_by_header_with_zero_as_no_price() {
        let book = parse_realm(REALM).unwrap();
        assert_eq!(book.updated, Some(1_794_165_148));
        assert_eq!(book.prices.len(), 3);
        assert_eq!(book.prices[&2318], RealmPrice { market_value: Some(Copper(32)), min_buyout: Some(Copper(28)), market_value_recent: Some(Copper(34)), historical: Some(Copper(32)), num_auctions: None });
        assert_eq!(book.prices[&2391], RealmPrice { market_value: Some(Copper(119)), min_buyout: None, market_value_recent: None, historical: Some(Copper(150)), num_auctions: None });
        assert_eq!(book.prices[&3041].historical, Some(Copper(18_460)));

        let reordered = "updatedAt,historical,recent,minBuyout,marketValue,itemId\n2026-11-08T19:12:28Z,32,34,28,31,2318\n";
        assert_eq!(parse_realm(reordered).unwrap().prices[&2318].market_value, Some(Copper(31)));
    }

    #[test]
    fn a_region_file_reads_sale_rates_as_decimals_and_zero_as_unknown() {
        let (prices, updated) = parse_region(REGION).unwrap();
        assert_eq!(updated, Some(1_794_012_906));
        assert_eq!(prices[&2318], RegionPrice { market_value: Some(Copper(33)), historical: Some(Copper(30)), sale_avg: Some(Copper(28)), sale_rate: Some(0.961), sold_per_day: Some(9140.071) });
        assert_eq!(prices[&791], RegionPrice { market_value: Some(Copper(35_000)), historical: Some(Copper(38_700)), sale_avg: None, sale_rate: None, sold_per_day: None });
        assert_eq!((prices[&4831].sale_rate, prices[&4831].sold_per_day), (Some(0.5), None));
    }

    #[test]
    fn a_bad_file_says_what_is_wrong() {
        assert_eq!(parse_realm("itemId,name,marketValue\n1,x,2\n"), Err("no minBuyout column".into()));
        assert_eq!(parse_region(&REGION.replace("0.961", "high")), Err("row 1: number \"high\"".into()));
    }

    #[test]
    fn the_book_names_the_house_and_region_as_appdata_does() {
        let house = PublicHouse { region: Region::Eu, realm: ForeverRealm::PvP };
        let book = book(house, Side::Horde, parse_realm(REALM).unwrap(), HashMap::new(), Some(5));
        assert_eq!((book.realm().map(|(name, _)| name), book.realms.len(), book.region_scope.as_deref(), book.region_updated), (Some("PvP-Horde"), 1, Some("Forever-EU"), Some(5)));
        assert_eq!(book.realm_price(2318).and_then(|p| p.min_buyout), Some(Copper(28)));
        let rp = PublicHouse { region: Region::Tw, realm: ForeverRealm::RoleplayPvp };
        assert_eq!((rp.scope(Side::Alliance), rp.region.name()), ("Roleplay PvP-Alliance".to_string(), "Forever-TW".to_string()));
    }

    #[test]
    fn each_house_downloads_its_own_file_and_its_regions() {
        let rp = PublicHouse { region: Region::Us, realm: ForeverRealm::RoleplayPvp };
        let (realm, region) = (rp.realm_file(Side::Alliance), rp.region_file());
        assert_eq!((realm.url.as_str(), realm.name.as_str()), ("https://public-data.tradeskillmaster.com/forever/us/realm/roleplay-pvp-alliance/items.csv", "us-realm-roleplay-pvp-alliance.csv"));
        assert_eq!((region.url.as_str(), region.name.as_str()), ("https://public-data.tradeskillmaster.com/forever/us/region/items.csv", "us-region.csv"));
    }

    #[test]
    fn a_file_downloads_when_missing_or_due_and_never_while_held() {
        let now = 1_000_000;
        let held = Hold { until: now + 600, why: Why::RateLimited };
        let ended = Hold { until: now, why: Why::RateLimited };
        assert_eq!(plan(None, 3600, Refresh::IfDue, None, now), Plan::Download);
        assert_eq!(plan(Some(3599), 3600, Refresh::IfDue, None, now), Plan::Cached);
        assert_eq!(plan(Some(3600), 3600, Refresh::IfDue, None, now), Plan::Download);
        assert_eq!(plan(Some(3600), 3600, Refresh::IfDue, Some(held), now), Plan::Held(held));
        assert_eq!(plan(Some(3600), 3600, Refresh::IfDue, Some(ended), now), Plan::Download);
        assert_eq!(plan(None, 3600, Refresh::IfDue, Some(held), now), Plan::Held(held));
        assert_eq!(plan(Some(299), 3600, Refresh::Now, None, now), Plan::Cached);
        assert_eq!(plan(Some(300), 3600, Refresh::Now, None, now), Plan::Download);
        assert_eq!(plan(Some(10), 3600, Refresh::IfDue, Some(held), now), Plan::Cached);
        assert_eq!((plan(None, 3600, Refresh::CacheOnly, None, now), plan(Some(90_000), 3600, Refresh::CacheOnly, None, now)), (Plan::Cached, Plan::Cached));
    }

    #[test]
    fn a_429_holds_for_its_retry_after_a_404_not_at_all_and_another_failure_for_five_minutes() {
        let now = 1_000_000;
        assert_eq!(Hold::after(&Failure::RateLimited { retry_after: Some(3540) }, now), Some(Hold { until: now + 3540, why: Why::RateLimited }));
        assert_eq!(Hold::after(&Failure::RateLimited { retry_after: None }, now), Some(Hold { until: now + 3600, why: Why::RateLimited }));
        assert_eq!(Hold::after(&Failure::Other("offline".into()), now), Some(Hold { until: now + 300, why: Why::Unreachable }));
        assert_eq!(Hold::after(&Failure::NotFound, now), None);
        let hold = Hold { until: now + 3540, why: Why::RateLimited };
        assert_eq!((hold.text(), Hold::parse(&hold.text())), ("1003540 rate-limited".to_string(), Some(hold)));
        assert_eq!(Hold::parse("soon"), None);
    }

    #[test]
    fn stale_prices_say_why_and_for_how_long() {
        let now = 1_000_000;
        let limited = Stale { hold: Hold { until: now + 52 * 60 - 20, why: Why::RateLimited }, age: Some(7200) };
        let unreachable = Stale { hold: Hold { until: now + 300, why: Why::Unreachable }, age: Some(3 * 3600 + 60) };
        assert_eq!(limited.words(now), "TSM public data rate-limited, retrying in 52 min");
        assert_eq!(unreachable.words(now), "TSM public data unreachable, showing prices from 3 hours ago");
        assert_eq!(Stale { age: None, ..unreachable }.words(now), "TSM public data unreachable, retrying in 5 min");
    }
}
