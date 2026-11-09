use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::merchants::Side;
use crate::prices::{self, PriceBook};
use crate::public_data::{self, PublicHouse, Refresh};
use crate::profit::CostBook;
use crate::recipes::{self, Profession, Recipe};
use crate::vendor;

pub struct Market {
    pub book: PriceBook,
    pub read: Read,
    pub recipes: Vec<Recipe>,
    costs: CostBook,
}

/// Where prices come from, as the player chose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PriceSource {
    /// The AppData.lua the TSM desktop app writes.
    #[default]
    App,
    /// TSM's public files, for a player without the desktop app.
    Public(PublicHouse),
}

/// What prices are read from: an AppData.lua, or a public auction house for a faction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    AppData(PathBuf),
    Public(PublicHouse, Side),
}

/// When the app last read its prices, and for AppData.lua the file's modified time as it read
/// it.
#[derive(Clone, Debug, PartialEq)]
pub struct Read {
    pub at: SystemTime,
    pub modified: Option<SystemTime>,
    /// Why the prices read are older than they should be, such as a download held back.
    pub warning: Option<String>,
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
    Ok((book, Read { at: crate::platform::now(), modified, warning: None }))
}

/// Prices from `origin`; public data downloads as `refresh` allows.
pub fn read_prices(origin: &Origin, refresh: Refresh) -> Result<(PriceBook, Read), String> {
    match origin {
        Origin::AppData(path) => read_book(path),
        Origin::Public(house, side) => public_data::load(*house, *side, refresh),
    }
}

/// `profession`'s recipes priced from `realm` in `origin` as [`Market::new`] does. Blocks on
/// file IO and, without a cache, the network.
pub fn load(origin: &Origin, refresh: Refresh, realm: &str, profession: Profession) -> Result<Market, String> {
    let (book, read) = read_prices(origin, refresh)?;
    let recipes = recipes::load(profession)?;
    Ok(Market::new(book, read, recipes, realm))
}
