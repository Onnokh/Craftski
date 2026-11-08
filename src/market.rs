use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::prices::{self, PriceBook};
use crate::profit::CostBook;
use crate::recipes::{self, Profession, Recipe};
use crate::vendor;

pub struct Market {
    pub book: PriceBook,
    pub read: Read,
    pub recipes: Vec<Recipe>,
    costs: CostBook,
}

/// When the app last read AppData.lua, and the file's modified time as it read it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Read {
    pub at: SystemTime,
    pub modified: Option<SystemTime>,
}

impl Market {
    /// Prices from `realm`, or from the first realm when the book has none by that name.
    pub fn new(mut book: PriceBook, read: Read, recipes: Vec<Recipe>, realm: &str) -> Market {
        book.choose_realm(realm);
        let costs = CostBook::new(&recipes, &book);
        Market { book, read, recipes, costs }
    }

    pub fn costs(&self) -> &CostBook {
        &self.costs
    }

    /// Prices `recipes` in place of the ones it held.
    pub fn set_recipes(&mut self, recipes: Vec<Recipe>) {
        self.costs = CostBook::new(&recipes, &self.book);
        self.recipes = recipes;
    }

    /// Prices from `realm` when the book has it, saying whether that changed anything.
    pub fn choose_realm(&mut self, realm: &str) -> bool {
        let moved = self.book.choose_realm(realm);
        if moved {
            self.costs = CostBook::new(&self.recipes, &self.book);
        }
        moved
    }
}

const DEFAULT_APPDATA: &str = r"C:\Program Files (x86)\World of Warcraft\_forever_\Interface\AddOns\TradeSkillMaster_AppHelper\AppData.lua";

/// `--appdata PATH`, else `TSM_APPDATA`, else the default Forever install.
pub fn appdata_path() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    let flag = args.iter().position(|a| a == "--appdata").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    flag.or_else(|| std::env::var_os("TSM_APPDATA").map(PathBuf::from)).unwrap_or_else(|| PathBuf::from(DEFAULT_APPDATA))
}

/// AppData.lua's modified time, None when the file or its time can't be read.
pub fn modified(appdata: &Path) -> Option<SystemTime> {
    std::fs::metadata(appdata).and_then(|m| m.modified()).ok()
}

/// TSM's prices from AppData.lua and its vendor prices from the files beside it.
pub fn read_book(appdata: &Path) -> Result<(PriceBook, Read), String> {
    // Taken before the read, so a write that lands during it still differs next check.
    let modified = modified(appdata);
    let mut book = prices::load(appdata)?;
    book.vendor = vendor::load(appdata);
    Ok((book, Read { at: SystemTime::now(), modified }))
}

/// `profession`'s recipes priced from `realm` as [`Market::new`] does. Blocks on file IO and,
/// without a recipe cache, the network.
pub fn load(realm: &str, profession: Profession) -> Result<Market, String> {
    let (book, read) = read_book(&appdata_path())?;
    let recipes = recipes::load(profession)?;
    Ok(Market::new(book, read, recipes, realm))
}
