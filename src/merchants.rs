//! Merchants and what they sell: foreverdb's vendor list, with Wowhead's stock for the vendors
//! it was crawled for. `vendor.rs` is something else: TSM's vendor prices.

use std::collections::{BTreeSet, HashMap};

use serde::Deserialize;

use crate::items::{self, Item, Purchase};
use crate::recipes::{FileDataId, Quality};

/// Wowhead's stock for every Thunder Bluff merchant. foreverdb misses limited supply and new
/// Forever items, and lists the wrong stock at about a third of them.
const WOWHEAD: &str = include_str!("../assets/vendors/wowhead-thunder-bluff.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Alliance,
    Horde,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::Horde, Side::Alliance];

    pub fn name(self) -> &'static str {
        match self {
            Side::Alliance => "Alliance",
            Side::Horde => "Horde",
        }
    }

    pub fn named(name: &str) -> Option<Side> {
        Side::ALL.into_iter().find(|s| s.name() == name)
    }
}

/// Where a merchant's list of goods comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StockSource {
    Wowhead,
    /// foreverdb's addon saw the list in Forever.
    ForeverSeen,
    /// Classic's list, for a merchant nobody has opened in Forever yet.
    Classic,
}

impl StockSource {
    pub fn label(self) -> &'static str {
        match self {
            StockSource::Wowhead => "Wowhead",
            StockSource::ForeverSeen => "seen in Forever",
            StockSource::Classic => "Classic",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Merchant {
    pub npc: u32,
    pub name: String,
    /// What the merchant is, as in "Weaponsmith" or "Trade Supplies".
    pub role: String,
    pub zone: String,
    /// Map coordinates in percent.
    pub at: (f32, f32),
    /// None for a merchant who serves both factions.
    pub side: Option<Side>,
    /// The faction whose reputation lowers this merchant's prices.
    pub rep: Option<String>,
    pub source: StockSource,
    pub stock: Vec<Stock>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stock {
    pub item: u32,
    pub price: Price,
    /// How many the merchant has before restocking; None for an endless supply.
    pub limit: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Price {
    /// foreverdb's price. None for an item only Wowhead lists: its prices are the list price
    /// over 0.95 at some merchants, so they are not used.
    Gold(Option<Purchase>),
    Currency,
}

pub struct Catalog {
    pub merchants: Vec<Merchant>,
    /// Every item a merchant stocks.
    pub items: HashMap<u32, Item>,
}

#[derive(Deserialize)]
struct Db {
    vendors: HashMap<String, RawVendor>,
    /// `[name, icon, quality, ...]`.
    items: HashMap<String, Vec<serde_json::Value>>,
    factions: HashMap<String, String>,
}

#[derive(Deserialize)]
struct RawVendor {
    n: String,
    #[serde(default)]
    sub: String,
    #[serde(default)]
    side: String,
    #[serde(default)]
    zone: String,
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    rf: Option<u32>,
    fv: Option<u8>,
    sells: HashMap<String, RawSale>,
}

#[derive(Deserialize)]
struct RawSale {
    #[serde(default)]
    p: u64,
    n: Option<u32>,
    lim: Option<u32>,
    c: Option<serde_json::Value>,
}

impl RawSale {
    fn price(&self) -> Price {
        match self.c {
            Some(_) => Price::Currency,
            None => Price::Gold((self.p > 0).then(|| Purchase { copper: self.p, count: self.n.unwrap_or(1) })),
        }
    }
}

/// `[item, name, cost, available]`, where an available count of -1 is an endless supply.
#[derive(Deserialize)]
struct Crawled {
    id: u32,
    sells: Vec<(u32, String, u64, i64)>,
}

/// Merchants from foreverdb's `vendors.json`, a crawled merchant's stock replaced by Wowhead's,
/// and the names and icons the two give their items.
pub fn parse(db: &str, crawled: &str) -> Result<(Vec<Merchant>, HashMap<u32, Item>), String> {
    let db: Db = serde_json::from_str(db).map_err(|e| format!("vendors.json: {e}"))?;
    let crawled: Vec<Crawled> = serde_json::from_str(crawled).map_err(|e| format!("Wowhead stock: {e}"))?;
    let crawled: HashMap<u32, Crawled> = crawled.into_iter().map(|c| (c.id, c)).collect();
    let mut items: HashMap<u32, Item> = HashMap::new();
    for (id, fields) in &db.items {
        let id = id.parse().map_err(|_| format!("vendors.json: item id {id:?}"))?;
        let name = fields.first().and_then(|v| v.as_str()).unwrap_or_default().to_owned();
        let icon = fields.get(1).and_then(serde_json::Value::as_u64).filter(|&i| i > 0).and_then(|i| u32::try_from(i).ok()).map(FileDataId);
        let quality = fields.get(2).and_then(serde_json::Value::as_u64).and_then(|q| Quality::from_id(u8::try_from(q).ok()?)).unwrap_or_default();
        items.insert(id, Item { name, quality, icon, facts: None });
    }
    let mut merchants = Vec::with_capacity(db.vendors.len());
    for (npc, v) in db.vendors {
        let npc: u32 = npc.parse().map_err(|_| format!("vendors.json: NPC id {npc:?}"))?;
        let listed = |item: u32| v.sells.get(&item.to_string());
        let (source, mut stock) = match crawled.get(&npc) {
            Some(c) => {
                let stock = c
                    .sells
                    .iter()
                    .map(|(item, name, _, available)| {
                        items.entry(*item).or_insert_with(|| Item { name: name.clone(), quality: Quality::Common, icon: None, facts: None });
                        let price = listed(*item).map_or(Price::Gold(None), RawSale::price);
                        Stock { item: *item, price, limit: u32::try_from(*available).ok().filter(|&n| n > 0) }
                    })
                    .collect();
                (StockSource::Wowhead, stock)
            }
            None => {
                let mut stock = Vec::with_capacity(v.sells.len());
                for (item, sale) in &v.sells {
                    let item = item.parse().map_err(|_| format!("vendors.json: {} sells item {item:?}", v.n))?;
                    stock.push(Stock { item, price: sale.price(), limit: sale.lim });
                }
                (if v.fv.is_some() { StockSource::ForeverSeen } else { StockSource::Classic }, stock)
            }
        };
        stock.sort_by_key(|s| s.item);
        let side = Side::named(&v.side);
        let rep = v.rf.and_then(|f| db.factions.get(&f.to_string()).cloned());
        merchants.push(Merchant { npc, name: v.n, role: v.sub, zone: v.zone, at: (v.x, v.y), side, rep, source, stock });
    }
    merchants.sort_by_key(|m| m.npc);
    Ok((merchants, items))
}

/// Blocks on file IO and, when the copies are missing or old, the network.
pub fn load() -> Result<Catalog, String> {
    let (merchants, mut items) = parse(&crate::foreverdb::fetch("vendors.json")?, WOWHEAD)?;
    let wanted: BTreeSet<u32> = merchants.iter().flat_map(|m| m.stock.iter().map(|s| s.item)).collect();
    for (id, item) in items::load(&wanted)? {
        let icon = item.icon.or_else(|| items.get(&id).and_then(|i| i.icon));
        items.insert(id, Item { icon, ..item });
    }
    Ok(Catalog { merchants, items })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DB: &str = r#"{
        "vendors": {
            "3015": {"n": "Kuna Thunderhorn", "sub": "Bowyer", "side": "Horde", "zone": "Thunder Bluff", "x": 47.0, "y": 45.6, "rf": 81, "fv": 1,
                     "sells": {"2504": {"p": 29}, "11303": {"p": 1600, "lim": 1}}},
            "4168": {"n": "Elynna", "sub": "Tradeswoman", "side": "", "zone": "Booty Bay", "x": 27.4, "y": 77.2,
                     "sells": {"2678": {"p": 10, "n": 5}, "272151": {"p": 0, "c": [["c", 3469, 5]]}}},
            "54":   {"n": "Corina Steele", "sub": "Weaponsmith", "side": "Alliance", "zone": "Elwynn Forest", "x": 41.6, "y": 65.8, "rf": 72,
                     "sells": {"2488": {"p": 536}}}
        },
        "items": {"2504": ["Worn Shortbow", 135489, 1, 0, 2], "11303": ["Fine Shortbow", 135490, 2, 0, 2]},
        "factions": {"81": "Thunder Bluff", "72": "Stormwind"}
    }"#;

    const CRAWLED: &str = r#"[{"id": 3015, "sells": [[2504, "Worn Shortbow", 30, -1], [287063, "Serrated Knife", 3612, 1]]}]"#;

    #[test]
    fn wowhead_replaces_a_crawled_merchants_stock_and_limits_but_keeps_foreverdbs_prices() {
        let (merchants, items) = parse(DB, CRAWLED).unwrap();
        let kuna = &merchants[1];
        assert_eq!((kuna.npc, kuna.source, kuna.side, kuna.rep.as_deref(), kuna.at), (3015, StockSource::Wowhead, Some(Side::Horde), Some("Thunder Bluff"), (47.0, 45.6)));
        assert_eq!(kuna.stock, vec![
            Stock { item: 2504, price: Price::Gold(Some(Purchase { copper: 29, count: 1 })), limit: None },
            Stock { item: 287063, price: Price::Gold(None), limit: Some(1) },
        ]);
        assert_eq!((items[&287063].name.as_str(), items[&11303].icon, items[&11303].quality), ("Serrated Knife", Some(FileDataId(135490)), Quality::Uncommon));
    }

    #[test]
    fn reads_bundles_currency_limits_and_sides() {
        let (merchants, _) = parse(DB, "[]").unwrap();
        let ids: Vec<u32> = merchants.iter().map(|m| m.npc).collect();
        assert_eq!(ids, vec![54, 3015, 4168]);
        assert_eq!((merchants[0].side, merchants[0].rep.as_deref(), merchants[0].source), (Some(Side::Alliance), Some("Stormwind"), StockSource::Classic));
        assert_eq!(merchants[1].stock[1], Stock { item: 11303, price: Price::Gold(Some(Purchase { copper: 1600, count: 1 })), limit: Some(1) });
        assert_eq!((merchants[1].source, merchants[2].side, merchants[2].rep.as_deref()), (StockSource::ForeverSeen, None, None));
        assert_eq!(merchants[2].stock, vec![
            Stock { item: 2678, price: Price::Gold(Some(Purchase { copper: 10, count: 5 })), limit: None },
            Stock { item: 272151, price: Price::Currency, limit: None },
        ]);
    }
}
