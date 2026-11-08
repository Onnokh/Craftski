//! Vendor buy prices, read the way TSM builds its `VendorBuy` source: the vendor table in TSM's
//! SavedVariables, over the built-in Retail list that Forever uses. TSM writes the list into
//! that table and updates it at each merchant the player opens, so it usually holds both.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;

use crate::copper::Copper;

const LIST_TABLE: &str = "VendorSell.Retail = {";
/// Not `["g@internalData@vendorItems"]`, which holds the table's schema version.
const SAVED_TABLE: &str = r#"["g@ @internalData@vendorItems"] = {"#;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VendorBook {
    prices: HashMap<u32, Copper>,
    /// Entries each source held before the merge.
    pub from_list: usize,
    pub from_saved: usize,
}

impl VendorBook {
    /// A saved price wins over the list's, as TSM's `itemsTable[k] = itemsTable[k] or cost`.
    pub fn new(saved: HashMap<u32, Copper>, list: HashMap<u32, Copper>) -> VendorBook {
        let (from_list, from_saved) = (list.len(), saved.len());
        let mut prices = saved;
        for (id, cost) in list {
            prices.entry(id).or_insert(cost);
        }
        VendorBook { prices, from_list, from_saved }
    }

    pub fn get(&self, item: u32) -> Option<Copper> {
        self.prices.get(&item).copied()
    }

    pub fn len(&self) -> usize {
        self.prices.len()
    }
}

/// Reads TSM's files around the AppHelper's `...\Interface\AddOns\TradeSkillMaster_AppHelper\AppData.lua`:
/// the list inside the TSM addon and every account's SavedVariables under `WTF`, a sibling of
/// `Interface`. A missing file adds no prices. Accounts that disagree keep the lower price.
pub fn load(appdata: &Path) -> VendorBook {
    let Some(addons) = appdata.parent().and_then(Path::parent) else { return VendorBook::default() };
    let list = read(&addons.join("TradeSkillMaster").join("LibTSMData").join("Item").join("VendorSell.lua")).map_or_else(HashMap::new, |src| parse_table(&src, LIST_TABLE));
    let mut saved: HashMap<u32, Copper> = HashMap::new();
    let accounts = addons.parent().and_then(Path::parent).map(|game| game.join("WTF").join("Account"));
    for account in accounts.and_then(|dir| std::fs::read_dir(dir).ok()).into_iter().flatten().flatten() {
        let Some(src) = read(&account.path().join("SavedVariables").join("TradeSkillMaster.lua")) else { continue };
        for (id, cost) in parse_table(&src, SAVED_TABLE) {
            saved.entry(id).and_modify(|c| *c = (*c).min(cost)).or_insert(cost);
        }
    }
    VendorBook::new(saved, list)
}

fn read(path: &Path) -> Option<String> {
    match std::fs::read(path) {
        Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => {
            eprintln!("vendor prices: reading {}: {e}", path.display());
            None
        }
    }
}

/// The `["i:4291"] = 500,` entries of the Lua table that opens on the line `opener`, up to its
/// closing brace. Comments, item variants (`i:4291::i55`), pets and zero prices are skipped.
fn parse_table(src: &str, opener: &str) -> HashMap<u32, Copper> {
    src.lines().map(str::trim).skip_while(|l| *l != opener).skip(1).take_while(|l| !l.starts_with('}')).filter_map(entry).collect()
}

fn entry(line: &str) -> Option<(u32, Copper)> {
    let (id, rest) = line.strip_prefix("[\"i:")?.split_once("\"]")?;
    let value = rest.trim_start().strip_prefix('=')?.split(',').next()?.trim();
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(id) || !digits(value) {
        return None;
    }
    let cost = value.parse().ok().filter(|&c| c > 0)?;
    Some((id.parse().ok()?, Copper(cost)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VENDOR_SELL: &str = r#"local VendorSell = LibTSMData:Init("VendorSell")

VendorSell.Classic = {
	-- https://classic.wowhead.com/items/side:3?filter=92:87:186:86;1:11:1:12;0:0:0:0
	["i:2320"] = 10, -- Coarse Thread
	["i:4291"] = 450, -- Silken Thread
}

VendorSell.Retail = {
	-- https://www.wowhead.com/items/side:3/live-only:on?filter=92:87:186:86;1:11:1:12;0:0:0:0
	["i:2320"] = 10, -- Coarse Thread
	["i:2678"] = 10, -- Mild Spices
	["i:4291"] = 500, -- Silken Thread
}
"#;

    const SAVED: &str = "[\"g@internalData@vendorItems\"] = 102,\r
[\"g@ @internalData@vendorItems\"] = {\r
[\"i:2678\"] = 2,\r
[\"i:4291\"] = 450,\r
[\"i:4291::i55\"] = 7,\r
[\"p:2163\"] = 9,\r
[\"i:851\"] = 0,\r
},\r
[\"g@ @craftingOptions@ignoreGuilds\"] = {\r
[\"i:9999\"] = 1,\r
},\r
";

    fn sorted(t: HashMap<u32, Copper>) -> Vec<(u32, u64)> {
        let mut v: Vec<_> = t.into_iter().map(|(id, c)| (id, c.0)).collect();
        v.sort();
        v
    }

    #[test]
    fn reads_the_retail_list_not_the_classic_one() {
        assert_eq!(sorted(parse_table(VENDOR_SELL, LIST_TABLE)), vec![(2320, 10), (2678, 10), (4291, 500)]);
    }

    #[test]
    fn reads_only_plain_item_entries_of_the_saved_table() {
        assert_eq!(sorted(parse_table(SAVED, SAVED_TABLE)), vec![(2678, 2), (4291, 450)]);
        assert_eq!(parse_table("", SAVED_TABLE), HashMap::new());
    }

    #[test]
    fn saved_prices_override_the_list() {
        let book = VendorBook::new(parse_table(SAVED, SAVED_TABLE), parse_table(VENDOR_SELL, LIST_TABLE));
        assert_eq!((book.get(4291), book.get(2678), book.get(2320), book.get(851)), (Some(Copper(450)), Some(Copper(2)), Some(Copper(10)), None));
        assert_eq!((book.len(), book.from_list, book.from_saved), (3, 3, 2));
    }

    #[test]
    fn loads_beside_appdata_from_every_account_and_survives_missing_files() {
        let game = std::env::temp_dir().join(format!("tsm-crafts-vendor-{}", std::process::id()));
        let addons = game.join("Interface").join("AddOns");
        let appdata = addons.join("TradeSkillMaster_AppHelper").join("AppData.lua");
        let write = |path: &Path, src: &str| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, src).unwrap();
        };
        let saved = |account: &str| game.join("WTF").join("Account").join(account).join("SavedVariables").join("TradeSkillMaster.lua");

        assert_eq!(load(&appdata), VendorBook::default());

        write(&saved("ONNOZ"), SAVED);
        write(&saved("ALT"), "[\"g@ @internalData@vendorItems\"] = {\n[\"i:4291\"] = 400,\n[\"i:2321\"] = 90,\n}\n");
        let saved_only = load(&appdata);
        assert_eq!((saved_only.get(4291), saved_only.get(2678), saved_only.get(2321), saved_only.get(2320)), (Some(Copper(400)), Some(Copper(2)), Some(Copper(90)), None));

        write(&addons.join("TradeSkillMaster").join("LibTSMData").join("Item").join("VendorSell.lua"), VENDOR_SELL);
        let both = load(&appdata);
        std::fs::remove_dir_all(&game).unwrap();
        assert_eq!((both.get(4291), both.get(2320), both.len(), both.from_list, both.from_saved), (Some(Copper(400)), Some(Copper(10)), 4, 3, 3));
    }
}
