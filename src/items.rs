//! What the client knows about items merchants sell, from wago.tools DB2 exports, joined once
//! and cached as JSON.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::copper::Copper;
use crate::recipes::{self, BUILD, FileDataId, IconTables, Quality, each_row, num};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub name: String,
    pub quality: Quality,
    pub icon: Option<FileDataId>,
    /// None for an item the client's tables lack, known only by a vendor list's name for it.
    pub facts: Option<Facts>,
}

/// `ItemSparse` columns that decide whether and how an item can be flipped.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Facts {
    /// What a merchant pays for one, the base of the auction deposit.
    pub sell: Copper,
    pub stack: u32,
    pub binding: Binding,
    /// The merchant's list price, before any discount. None where the client has none.
    pub buy: Option<Purchase>,
}

/// `copper` buys `count` items, as merchants sell some goods in bundles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Purchase {
    pub copper: u64,
    pub count: u32,
}

impl Purchase {
    pub fn unit(self) -> f64 {
        self.copper as f64 / f64::from(self.count.max(1))
    }
}

/// `ItemSparse.Bonding`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Binding {
    None,
    OnPickup,
    OnEquip,
    OnUse,
    Quest,
}

impl Binding {
    pub fn from_id(id: u8) -> Binding {
        match id {
            1 => Binding::OnPickup,
            2 => Binding::OnEquip,
            3 => Binding::OnUse,
            4 => Binding::Quest,
            _ => Binding::None,
        }
    }

    /// Whether a bought one can go up on the auction house.
    pub fn tradable(self) -> bool {
        !matches!(self, Binding::OnPickup | Binding::Quest)
    }
}

/// The `wanted` items `ItemSparse` lists, with their icons.
pub fn join(item_sparse: &str, icons: &IconTables, wanted: &BTreeSet<u32>) -> Result<HashMap<u32, Item>, String> {
    let mut items = HashMap::new();
    let cols = ["ID", "Display_lang", "OverallQualityID", "SellPrice", "BuyPrice", "VendorStackCount", "Stackable", "Bonding"];
    each_row(item_sparse, "ItemSparse", cols, |[id, name, quality, sell, buy, bundle, stack, binding]| {
        let id = num(id)?;
        if !wanted.contains(&id) {
            return Ok(());
        }
        let buy = Some(Purchase { copper: num(buy)?, count: num::<u32>(bundle)?.max(1) }).filter(|p| p.copper > 0);
        let facts = Facts { sell: Copper(num(sell)?), stack: num::<u32>(stack)?.max(1), binding: Binding::from_id(num(binding)?), buy };
        items.insert(id, Item { name: name.to_owned(), quality: Quality::from_id(num(quality)?).unwrap_or_default(), icon: None, facts: Some(facts) });
        Ok(())
    })?;
    for (id, icon) in recipes::icons(icons, wanted)? {
        if let Some(item) = items.get_mut(&id) {
            item.icon = Some(icon);
        }
    }
    Ok(items)
}

/// Bump when `Item` changes shape, so a file an older version wrote is never read.
const CACHE_SCHEMA: &str = "v1";

#[derive(Serialize, Deserialize)]
struct Cache {
    build: String,
    /// Every id the join looked for, found or not, so a missing one is not looked up again.
    asked: BTreeSet<u32>,
    items: BTreeMap<u32, Item>,
}

fn cache_path() -> Option<PathBuf> {
    Some(crate::cache::root()?.join(format!("items-{CACHE_SCHEMA}-{BUILD}.json")))
}

/// The `wanted` items from the cache, or downloaded and joined when the cache lacks any.
pub fn load(wanted: &BTreeSet<u32>) -> Result<HashMap<u32, Item>, String> {
    let path = cache_path();
    let cached = path.as_ref().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<Cache>(&b).ok());
    if let Some(cache) = cached.filter(|c| c.build == BUILD && c.asked.is_superset(wanted)) {
        return Ok(cache.items.into_iter().filter(|(id, _)| wanted.contains(id)).collect());
    }
    let items = join(&recipes::fetch("ItemSparse")?, &IconTables::fetch()?, wanted)?;
    if let Some(path) = path {
        let cache = Cache { build: BUILD.into(), asked: wanted.clone(), items: items.iter().map(|(id, item)| (*id, item.clone())).collect() };
        if let Err(e) = recipes::write_json(&path, &cache) {
            eprintln!("item cache not written to {}: {e}", path.display());
        }
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEM_SPARSE: &str = "ID,Display_lang,OverallQualityID,Stackable,SellPrice,BuyPrice,VendorStackCount,Bonding\n\
                               6256,\"Fishing Pole\",1,1,4,23,1,0\n\
                               2678,\"Mild Spices\",1,20,0,10,5,0\n\
                               5956,\"Blacksmith Hammer\",1,1,3,18,1,1\n\
                               9999,\"Not Wanted\",1,1,1,1,1,0\n";

    fn icon_tables() -> IconTables {
        IconTables {
            item: "ID,IconFileDataID\n6256,132932\n2678,0\n".into(),
            item_modified_appearance: "ID,ItemID,ItemAppearanceModifierID,ItemAppearanceID,OrderIndex\n".into(),
            item_appearance: "ID,DefaultIconFileDataID\n".into(),
        }
    }

    #[test]
    fn joins_the_flip_columns_for_wanted_items_only() {
        let items = join(ITEM_SPARSE, &icon_tables(), &BTreeSet::from([6256, 2678, 5956, 1])).unwrap();
        let mut ids: Vec<u32> = items.keys().copied().collect();
        ids.sort();
        assert_eq!(ids, vec![2678, 5956, 6256]);
        assert_eq!(items[&6256], Item {
            name: "Fishing Pole".into(),
            quality: Quality::Common,
            icon: Some(FileDataId(132932)),
            facts: Some(Facts { sell: Copper(4), stack: 1, binding: Binding::None, buy: Some(Purchase { copper: 23, count: 1 }) }),
        });
        assert_eq!(items[&2678].facts, Some(Facts { sell: Copper(0), stack: 20, binding: Binding::None, buy: Some(Purchase { copper: 10, count: 5 }) }));
        assert_eq!((items[&2678].icon, items[&5956].facts.map(|f| f.binding.tradable())), (None, Some(false)));
    }

    #[test]
    fn a_bundle_price_divides_over_its_count() {
        assert_eq!((Purchase { copper: 10, count: 5 }.unit(), Purchase { copper: 25, count: 0 }.unit()), (2.0, 25.0));
    }
}
