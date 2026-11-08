//! TSM AppHelper `AppData.lua` reader. Each line is one `LoadData(block, scope, [[return {...}]])`
//! call whose table names its columns in `fields` and lists rows in `data`.

use std::collections::HashMap;
use std::path::Path;

use crate::copper::Copper;

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PriceBook {
    pub realm: HashMap<u32, RealmPrice>,
    pub region: HashMap<u32, RegionPrice>,
    /// Newest `downloadTime` (unix seconds) among the blocks feeding each map.
    pub realm_updated: Option<u64>,
    pub region_updated: Option<u64>,
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

    fn apply(self, book: &mut PriceBook, id: u32, raw: u64) {
        // TSM writes 0 where it has no price, so a zero price is a missing one.
        let price = (raw > 0).then_some(Copper(raw));
        let thousandths = raw as f64 / 1000.0;
        if self.is_region() {
            let p = book.region.entry(id).or_default();
            match self {
                Stat::RegionMarketValue => p.market_value = price,
                Stat::RegionHistorical => p.historical = price,
                Stat::RegionSale => p.sale_avg = price,
                Stat::RegionSoldPerDay => p.sold_per_day = Some(thousandths),
                _ => p.sale_rate = Some(thousandths),
            }
        } else {
            let p = book.realm.entry(id).or_default();
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
                stat.apply(book, id, decode(value)?);
            }
        }
    }

    if let Some(time) = time {
        let updated = if stats.iter().flatten().any(|s| s.is_region()) { &mut book.region_updated } else { &mut book.realm_updated };
        *updated = (*updated).max(Some(time));
    }
    Ok(())
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
        let mut realm: Vec<_> = book.realm.iter().map(|(id, p)| (*id, p.clone())).collect();
        realm.sort_by_key(|(id, _)| *id);
        assert_eq!(realm, vec![
            (2319, RealmPrice { market_value_recent: Some(Copper(104)), min_buyout: None, num_auctions: Some(271), ..Default::default() }),
            (4234, RealmPrice { market_value_recent: Some(Copper(218)), min_buyout: Some(Copper(210)), num_auctions: Some(249), ..Default::default() }),
        ]);
        assert_eq!(book.region.len(), 1);
        assert_eq!(book.region[&2318], RegionPrice { sale_avg: Some(Copper(29)), sold_per_day: Some(9130.56), sale_rate: Some(0.96), ..Default::default() });
        assert_eq!((book.realm_updated, book.region_updated), (Some(1_794_129_134), Some(1_794_099_200)));
    }

    #[test]
    fn a_row_with_the_wrong_width_names_its_line() {
        let src = r#"x[[return {downloadTime=1,fields={"itemString","marketValue"},data={{1,2},{3,4,5}}}]]"#;
        assert_eq!(parse(src), Err("AppData line 1: item 3 has 2 values for 1 fields".into()));
    }
}
