use std::path::PathBuf;

use crate::prices::{self, PriceBook};
use crate::profit::{CostBook, PriceSource};
use crate::recipes::{self, LEATHERWORKING, Recipe};
use crate::vendor;

pub struct Market {
    pub book: PriceBook,
    pub recipes: Vec<Recipe>,
    realm_costs: CostBook,
    region_costs: CostBook,
}

impl Market {
    pub fn new(book: PriceBook, recipes: Vec<Recipe>) -> Market {
        let costs = |source| CostBook::new(&recipes, &book, source);
        let (realm_costs, region_costs) = (costs(PriceSource::Realm), costs(PriceSource::Region));
        Market { book, recipes, realm_costs, region_costs }
    }

    pub fn costs(&self, source: PriceSource) -> &CostBook {
        match source {
            PriceSource::Realm => &self.realm_costs,
            PriceSource::Region => &self.region_costs,
        }
    }
}

const DEFAULT_APPDATA: &str = r"C:\Program Files (x86)\World of Warcraft\_forever_\Interface\AddOns\TradeSkillMaster_AppHelper\AppData.lua";

/// `--appdata PATH`, else `TSM_APPDATA`, else the default Forever install.
pub fn appdata_path() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    let flag = args.iter().position(|a| a == "--appdata").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    flag.or_else(|| std::env::var_os("TSM_APPDATA").map(PathBuf::from)).unwrap_or_else(|| PathBuf::from(DEFAULT_APPDATA))
}

/// Blocks on file IO and, without a recipe cache, the network.
pub fn load() -> Result<Market, String> {
    let appdata = appdata_path();
    let mut book = prices::load(&appdata)?;
    book.vendor = vendor::load(&appdata);
    let recipes = recipes::load(&LEATHERWORKING)?;
    Ok(Market::new(book, recipes))
}
