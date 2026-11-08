//! TSM AppHelper `AppData.lua` reader. Each line is one `LoadData(block, scope, [[return {...}]])`
//! call whose table names its columns in `fields` and lists rows in `data`.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::copper::Copper;
use crate::vendor::VendorBook;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RealmPrice {
    pub market_value: Option<Copper>,
    pub min_buyout: Option<Copper>,
    pub market_value_recent: Option<Copper>,
    pub historical: Option<Copper>,
    pub num_auctions: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegionPrice {
    pub market_value: Option<Copper>,
    pub historical: Option<Copper>,
    pub sale_avg: Option<Copper>,
    pub sold_per_day: Option<f64>,
    pub sale_rate: Option<f64>,
}

/// One auction house's listings and when TSM last downloaded them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RealmBook {
    pub prices: HashMap<u32, RealmPrice>,
    /// Newest `downloadTime` (unix seconds) among the blocks feeding `prices`.
    pub updated: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PriceBook {
    /// Each realm's auction house by its `LoadData` scope, such as "PvP-Horde".
    pub realms: BTreeMap<String, RealmBook>,
    /// Commodities trade region-wide, so their rows price an item on every realm.
    pub commodities: RealmBook,
    /// The realm in `realms` that prices are read from: the first by name until one is chosen.
    realm: Option<String>,
    pub region: HashMap<u32, RegionPrice>,
    pub region_updated: Option<u64>,
    /// The region's `LoadData` scope, such as "Forever-EU".
    pub region_scope: Option<String>,
    /// Merchant prices, read from TSM's own files rather than AppData.
    pub vendor: VendorBook,
}

impl PriceBook {
    /// The realm prices are read from, by name.
    pub fn realm(&self) -> Option<(&str, &RealmBook)> {
        let name = self.realm.as_deref()?;
        Some((name, self.realms.get(name)?))
    }

    /// An item's auction figures on the chosen realm, or region-wide for a commodity.
    pub fn realm_price(&self, item: u32) -> Option<&RealmPrice> {
        self.realm().and_then(|(_, b)| b.prices.get(&item)).or_else(|| self.commodities.prices.get(&item))
    }

    /// The chosen realm's prices, making a "PvP-Horde" realm to hold them in a book built by
    /// hand.
    #[cfg(test)]
    pub fn realm_prices_mut(&mut self) -> &mut HashMap<u32, RealmPrice> {
        let name = self.realm.get_or_insert_with(|| "PvP-Horde".into()).clone();
        &mut self.realms.entry(name).or_default().prices
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stat {
    MinBuyout,
    NumAuctions,
    MarketValueRecent,
    MarketValue,
    Historical,
    RegionMarketValue,
    RegionHistorical,
    RegionSale,
    RegionSoldPerDay,
    RegionSalePercent,
}

impl Stat {
    fn from_field(name: &str) -> Option<Stat> {
        Some(match name {
            "minBuyout" => Stat::MinBuyout,
            "numAuctions" => Stat::NumAuctions,
            "marketValueRecent" => Stat::MarketValueRecent,
            "marketValue" => Stat::MarketValue,
            "historical" => Stat::Historical,
            "regionMarketValue" => Stat::RegionMarketValue,
            "regionHistorical" => Stat::RegionHistorical,
            "regionSale" => Stat::RegionSale,
            "regionSoldPerDay" => Stat::RegionSoldPerDay,
            "regionSalePercent" => Stat::RegionSalePercent,
            _ => return None,
        })
    }

    fn is_region(self) -> bool {
        matches!(self, Stat::RegionMarketValue | Stat::RegionHistorical | Stat::RegionSale | Stat::RegionSoldPerDay | Stat::RegionSalePercent)
    }

    /// Writes one value into the region's prices, or into `realm`'s for a realm-kind stat.
    fn apply(self, region: &mut HashMap<u32, RegionPrice>, realm: Option<&mut RealmBook>, id: u32, raw: u64) {
        // TSM writes 0 where it has no data, so a zero price or sale rate is a missing one.
        let price = (raw > 0).then_some(Copper(raw));
        let thousandths = (raw > 0).then_some(raw as f64 / 1000.0);
        if self.is_region() {
            let p = region.entry(id).or_default();
            match self {
                Stat::RegionMarketValue => p.market_value = price,
                Stat::RegionHistorical => p.historical = price,
                Stat::RegionSale => p.sale_avg = price,
                Stat::RegionSoldPerDay => p.sold_per_day = thousandths,
                _ => p.sale_rate = thousandths,
            }
        } else if let Some(realm) = realm {
            let p = realm.prices.entry(id).or_default();
            match self {
                Stat::MinBuyout => p.min_buyout = price,
                Stat::NumAuctions => p.num_auctions = Some(u32::try_from(raw).unwrap_or(u32::MAX)),
                Stat::MarketValueRecent => p.market_value_recent = price,
                Stat::MarketValue => p.market_value = price,
                _ => p.historical = price,
            }
        }
    }
}

/// TSM's base-32 number (digits 0-9 then A-V, any case). TSM splits strings longer than six
/// digits into `rest * 2^30 + last6` to stay inside Lua's float precision; 32^6 is 2^30, so
/// that is plain positional base 32.
pub fn decode(s: &str) -> Result<u64, String> {
    if s.is_empty() {
        return Err("empty value".into());
    }
    s.bytes().try_fold(0u64, |acc, b| {
        let d = match b {
            b'0'..=b'9' => b - b'0',
            b'A'..=b'V' => b - b'A' + 10,
            b'a'..=b'v' => b - b'a' + 10,
            _ => return Err(format!("bad base-32 digit in {s:?}")),
        };
        acc.checked_mul(32).and_then(|v| v.checked_add(u64::from(d))).ok_or_else(|| format!("{s:?} overflows"))
    })
}

pub fn load(path: &Path) -> Result<PriceBook, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    parse(&String::from_utf8_lossy(&bytes))
}

pub fn parse(src: &str) -> Result<PriceBook, String> {
    let mut book = PriceBook::default();
    for (n, line) in src.lines().enumerate() {
        parse_block(line, &mut book).map_err(|e| format!("AppData line {}: {e}", n + 1))?;
    }
    book.realm = book.realms.keys().next().cloned();
    Ok(book)
}

/// Lines without a `fields` table (APP_INFO, blank lines) carry no prices and are skipped.
fn parse_block(line: &str, book: &mut PriceBook) -> Result<(), String> {
    let Some(body) = between(line, "[[", "]]") else { return Ok(()) };
    let Some(fields) = between(body, "fields={", "}") else { return Ok(()) };
    let fields: Vec<&str> = fields.split(',').map(|f| f.trim().trim_matches('"')).collect();
    if fields.first() != Some(&"itemString") {
        return Ok(());
    }
    let stats: Vec<Option<Stat>> = fields[1..].iter().map(|f| Stat::from_field(f)).collect();
    let time = between(body, "downloadTime=", ",").map(|t| t.parse::<u64>().map_err(|e| format!("downloadTime {t:?}: {e}"))).transpose()?;
    let rows = body.split_once("data={").ok_or("block has fields but no data")?.1;

    let named = scope(line);
    // Commodities trade region-wide, so their blocks carry the region's scope even though
    // their fields are the realm kind.
    let commodity = named.is_some_and(|(block, _)| block.contains("COMMODITY") && !block.contains("NON_COMMODITY"));
    let region = stats.iter().flatten().any(|s| s.is_region());
    let mut realm = match named {
        _ if stats.iter().flatten().all(|s| s.is_region()) => None,
        _ if commodity => Some(&mut book.commodities),
        Some((_, scope)) => Some(book.realms.entry(scope.to_owned()).or_default()),
        None => return Err("realm prices without a realm scope".into()),
    };

    for row in rows.split('{').skip(1) {
        let row = row.split('}').next().unwrap_or_default();
        let mut cells = row.split(',');
        let key = cells.next().unwrap_or_default();
        // String keys are item-level variants ("i:180162::i55") and pets ("p:2163").
        let Ok(id) = key.parse::<u32>() else { continue };
        let values: Vec<&str> = cells.collect();
        if values.len() != stats.len() {
            return Err(format!("item {id} has {} values for {} fields", values.len(), stats.len()));
        }
        for (stat, value) in stats.iter().zip(values) {
            if let Some(stat) = stat {
                stat.apply(&mut book.region, realm.as_deref_mut(), id, decode(value)?);
            }
        }
    }

    if let Some(time) = time {
        if region {
            book.region_updated = book.region_updated.max(Some(time));
        }
        if let Some(realm) = realm {
            realm.updated = realm.updated.max(Some(time));
        }
    }
    if let Some((_, scope)) = named.filter(|_| region || commodity) {
        book.region_scope.get_or_insert_with(|| scope.to_owned());
    }
    Ok(())
}

/// The block name and scope that open a line: `LoadData("BLOCK","PvP-Horde",[[...`.
fn scope(line: &str) -> Option<(&str, &str)> {
    let args = &line[line.find("LoadData(")? + "LoadData(".len()..line.find("[[")?];
    let mut args = args.split(',').map(|a| a.trim().trim_matches('"'));
    let (block, scope) = (args.next()?, args.next()?);
    (!scope.is_empty()).then_some((block, scope))
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let rest = &s[s.find(open)? + open.len()..];
    Some(&rest[..rest.find(close)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_base32_like_tsm() {
        assert_eq!(decode("IL"), Ok(597));
        assert_eq!(decode("il"), Ok(597));
        assert_eq!(decode("8MKI0"), Ok(9_130_560));
        assert_eq!(decode("UGKEOQ"), Ok(1_024_080_666));
        assert_eq!(decode("1UGKEOQ"), Ok(1_024_080_666 + (1 << 30)));
        assert_eq!(decode("21UGKEOQ"), Ok(1_024_080_666 + 65 * (1 << 30)));
        assert!(decode("W").is_err());
        assert!(decode("").is_err());
    }

    const SNIPPET: &str = r#"select(2, ...).LoadData("AUCTIONDB_NON_COMMODITY_DATA","PvP-Horde",[[return {downloadTime=1794129134,fields={"itemString","marketValueRecent","minBuyout","numAuctions"},data={{4234,6Q,6I,7P},{"i:180162::i55",34,34,1},{2319,38,0,8F}}}]]) --<AUCTIONDB_NON_COMMODITY_DATA,PvP-Horde,1794129134>
select(2, ...).LoadData("AUCTIONDB_REGION_SALE","Forever-EU",[[return {downloadTime=1794099200,fields={"itemString","regionSale","regionSoldPerDay","regionSalePercent"},data={{2318,T,8MKI0,U0}}}]]) --<AUCTIONDB_REGION_SALE,Forever-EU,1794099200>
select(2, ...).LoadData("APP_INFO","Global",[[return {version="1.0.9",lastSync=1794131961}]]) --<APP_INFO,Global,1794131961>
"#;

    #[test]
    fn parses_blocks_by_field_name_and_skips_string_keys() {
        let book = parse(SNIPPET).unwrap();
        let (name, realm) = book.realm().unwrap();
        let mut prices: Vec<_> = realm.prices.iter().map(|(id, p)| (*id, p.clone())).collect();
        prices.sort_by_key(|(id, _)| *id);
        assert_eq!(prices, vec![
            (2319, RealmPrice { market_value_recent: Some(Copper(104)), min_buyout: None, num_auctions: Some(271), ..Default::default() }),
            (4234, RealmPrice { market_value_recent: Some(Copper(218)), min_buyout: Some(Copper(210)), num_auctions: Some(249), ..Default::default() }),
        ]);
        assert_eq!(book.region.len(), 1);
        assert_eq!(book.region[&2318], RegionPrice { sale_avg: Some(Copper(29)), sold_per_day: Some(9130.56), sale_rate: Some(0.96), ..Default::default() });
        assert_eq!((name, realm.updated, book.region_updated), ("PvP-Horde", Some(1_794_129_134), Some(1_794_099_200)));
        assert_eq!((book.realms.len(), book.region_scope.as_deref()), (1, Some("Forever-EU")));
    }

    #[test]
    fn each_realm_keeps_its_own_prices_and_age_and_the_first_by_name_is_read() {
        let src = r#"x.LoadData("AUCTIONDB_NON_COMMODITY_DATA","PvP-Horde",[[return {downloadTime=100,fields={"itemString","minBuyout"},data={{4234,6I},{2319,38}}}]])
x.LoadData("AUCTIONDB_NON_COMMODITY_DATA","PvE-Alliance",[[return {downloadTime=200,fields={"itemString","minBuyout"},data={{4234,7P}}}]])
x.LoadData("AUCTIONDB_NON_COMMODITY_HISTORICAL","PvP-Horde",[[return {downloadTime=50,fields={"itemString","historical"},data={{4234,T}}}]])"#;
        let book = parse(src).unwrap();
        let seen = |b: &RealmBook| (b.prices.len(), b.prices[&4234].min_buyout, b.prices[&4234].historical, b.updated);
        assert_eq!(book.realms.keys().collect::<Vec<_>>(), ["PvE-Alliance", "PvP-Horde"]);
        assert_eq!(seen(&book.realms["PvP-Horde"]), (2, Some(Copper(210)), Some(Copper(29)), Some(100)));
        assert_eq!(seen(&book.realms["PvE-Alliance"]), (1, Some(Copper(249)), None, Some(200)));

        let read = |b: &PriceBook| (b.realm().unwrap().0.to_owned(), b.realm_price(4234).unwrap().min_buyout, b.realm_price(2319).is_some());
        assert_eq!(read(&book), ("PvE-Alliance".into(), Some(Copper(249)), false));
    }

    #[test]
    fn commodity_blocks_price_any_realm_and_name_the_region() {
        let src = r#"x.LoadData("AUCTIONDB_COMMODITY_HISTORICAL","Forever-EU",[[return {downloadTime=5,fields={"itemString","historical"},data={{2318,T}}}]])
x.LoadData("AUCTIONDB_NON_COMMODITY_DATA","PvP-Horde",[[return {downloadTime=6,fields={"itemString","minBuyout"},data={{4234,6I}}}]])
x.LoadData("AUCTIONDB_NON_COMMODITY_DATA","PvE-Alliance",[[return {downloadTime=7,fields={"itemString","minBuyout"},data={{4234,7P}}}]])"#;
        let book = parse(src).unwrap();
        assert_eq!((book.realms.keys().map(String::as_str).collect::<Vec<_>>(), book.region_scope.as_deref()), (vec!["PvE-Alliance", "PvP-Horde"], Some("Forever-EU")));
        assert_eq!((book.realm_price(2318).unwrap().historical, book.realms["PvE-Alliance"].updated), (Some(Copper(29)), Some(7)));
    }

    #[test]
    fn a_zero_sale_rate_is_unknown_like_a_zero_price() {
        let src = r#"x[[return {downloadTime=1,fields={"itemString","regionSale","regionSoldPerDay","regionSalePercent"},data={{2319,T,0,0},{2318,0,8MKI0,U0}}}]]"#;
        let book = parse(src).unwrap();
        assert_eq!(book.region[&2319], RegionPrice { sale_avg: Some(Copper(29)), sold_per_day: None, sale_rate: None, ..Default::default() });
        assert_eq!(book.region[&2318], RegionPrice { sale_avg: None, sold_per_day: Some(9130.56), sale_rate: Some(0.96), ..Default::default() });
    }

    #[test]
    fn a_row_with_the_wrong_width_names_its_line() {
        let src = r#"x.LoadData("AUCTIONDB_NON_COMMODITY_DATA","PvP-Horde",[[return {downloadTime=1,fields={"itemString","marketValue"},data={{1,2},{3,4,5}}}]])"#;
        assert_eq!(parse(src), Err("AppData line 1: item 3 has 2 values for 1 fields".into()));
    }

    #[test]
    fn realm_prices_need_a_realm_to_belong_to() {
        let src = r#"x[[return {downloadTime=1,fields={"itemString","marketValue"},data={{1,2}}}]]"#;
        assert_eq!(parse(src), Err("AppData line 1: realm prices without a realm scope".into()));
    }
}
