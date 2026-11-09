//! The vendor flips tab: goods a merchant sells for less than the auction house pays.

mod detail;
mod header;
mod table;
mod toolbar;

use std::collections::BTreeSet;

use rune::{El, TextEdit, Tx, col, row};

use super::{App, Dims, Load, message};
use crate::flip_rows::{self, Column, Filter, FilterKind, Row, Supply, Zone};
use crate::flips::{self, Flip};
use crate::items::Item;
use crate::menu::SOLD_STEPS;
use crate::merchants::{Catalog, Merchant};
use crate::recipes::{FileDataId, Quality};
use crate::sheets::Sheets;
use crate::tooltip::Card;
use crate::rows::{Hidden, Sort};

pub struct State {
    shelf: Shelf,
    zone: String,
    sort: Sort<Column>,
    min_sold: usize,
    hide_unprofitable: bool,
    supply: Supply,
    /// The flip the detail panel shows, by merchant npc and item. Its item's row shows while
    /// visible, whichever merchant leads it; else this flip, kept while filters hide it.
    selected: Option<(u32, u32)>,
    pub search: TextEdit,
}

enum Shelf {
    Loading,
    Failed(String),
    Ready(Box<Stocked>),
}

struct Stocked {
    catalog: Catalog,
    flips: Vec<Flip>,
    zones: Vec<Zone>,
    rows: Vec<Row>,
    hidden: Option<Hidden<FilterKind>>,
}

impl Stocked {
    fn at(&self, i: usize) -> (&Flip, &Merchant, Option<&Item>) {
        let f = &self.flips[i];
        (f, &self.catalog.merchants[f.merchant], self.catalog.items.get(&f.item))
    }

    fn key(&self, i: usize) -> (u32, u32) {
        let f = &self.flips[i];
        (self.catalog.merchants[f.merchant].npc, f.item)
    }
}

/// Every icon the catalog's goods draw.
pub fn icon_ids(c: &Catalog) -> BTreeSet<FileDataId> {
    c.items.values().filter_map(|i| i.icon).collect()
}

impl State {
    pub fn new() -> State {
        State {
            shelf: Shelf::Loading,
            zone: flip_rows::HOME.to_owned(),
            sort: Sort { column: Column::Profit, descending: true },
            // "1+": most vendor stock never sells, and with no sale rate its deposit loss is unknown.
            min_sold: 2,
            hide_unprofitable: true,
            supply: Supply::All,
            selected: None,
            search: TextEdit::new(""),
        }
    }

    fn stocked(&self) -> Option<&Stocked> {
        match &self.shelf {
            Shelf::Ready(f) => Some(f),
            _ => None,
        }
    }

    /// The tooltip of the flip at `i` in the flips list.
    pub fn tip(&self, i: usize, sheets: Option<&Sheets>) -> Option<Card> {
        let f = self.stocked().filter(|f| i < f.flips.len())?;
        let (flip, _, item) = f.at(i);
        let name = item.map_or_else(|| format!("Item #{}", flip.item), |i| i.name.clone());
        Some(Card::item(name, item.map_or(Quality::Common, |i| i.quality), flip.item, sheets))
    }

    fn refilter(&mut self) {
        let Shelf::Ready(f) = &mut self.shelf else { return };
        let filter = Filter { zone: &self.zone, hide_unprofitable: self.hide_unprofitable, min_sold_per_day: SOLD_STEPS[self.min_sold].min, query: &self.search.text, supply: self.supply };
        f.rows = flip_rows::visible(&f.flips, &f.catalog.merchants, &f.catalog.items, filter, self.sort);
        f.hidden = flip_rows::hidden(&f.flips, &f.catalog.merchants, &f.catalog.items, filter);
    }

    /// Turns off the filters that hide flips the zone or search shows. The zone and the search
    /// stay.
    pub fn show_all(&mut self) {
        let Some(h) = self.stocked().and_then(|f| f.hidden.clone()) else { return };
        for k in h.by {
            match k {
                FilterKind::Sells => self.min_sold = 0,
                FilterKind::Supply => self.supply = Supply::All,
                FilterKind::HideUnprofitable => self.hide_unprofitable = false,
            }
        }
        self.refilter();
    }

    /// The filters as the toolbar shows them, for the line that names what they hide.
    fn filter_name(&self, k: FilterKind) -> String {
        match k {
            FilterKind::Sells => SOLD_STEPS[self.min_sold].named(),
            FilterKind::Supply => format!("Supply {}", self.supply.label()),
            FilterKind::HideUnprofitable => "Profitable only".to_string(),
        }
    }

    fn select_first(&mut self) {
        self.selected = self.stocked().and_then(|f| Some(f.key(f.rows.first()?.flip)));
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Selects the first row with more than one merchant, for a snapshot scene.
    pub fn select_first_shared(&mut self) {
        self.selected = self.stocked().and_then(|f| Some(f.key(f.rows.iter().find(|r| !r.others.is_empty())?.flip)));
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// The state a user reported missing patterns in: "patter" searched at Sold 5+ a day,
    /// an endless supply and only profitable flips, for a snapshot scene.
    pub fn search_patterns_narrowly(&mut self) {
        self.search = TextEdit::new("patter");
        self.min_sold = 3;
        self.supply = Supply::Endless;
        self.hide_unprofitable = true;
        self.refilter();
        self.select_first();
    }

    /// The selected item's position in the rows.
    fn selected_row(&self) -> Option<usize> {
        let (f, (_, item)) = (self.stocked()?, self.selected?);
        f.rows.iter().position(|r| f.flips[r.flip].item == item)
    }

    /// The flip the detail panel shows, as an index into the flips, and the other merchants'
    /// offers of its item that pass the filters.
    fn picked(&self) -> Option<(&Stocked, usize, &[usize])> {
        let f = self.stocked()?;
        if let Some(at) = self.selected_row() {
            return Some((f, f.rows[at].flip, &f.rows[at].others));
        }
        let key = self.selected?;
        Some((f, (0..f.flips.len()).find(|&i| f.key(i) == key)?, &[]))
    }

    pub fn step(&mut self, by: isize, tx: &mut Tx) -> bool {
        let Some(f) = self.stocked() else { return false };
        if f.rows.is_empty() {
            return false;
        }
        let next = match self.selected_row() {
            Some(at) => at.saturating_add_signed(by).min(f.rows.len() - 1),
            None => 0,
        };
        let key = f.key(f.rows[next].flip);
        self.selected = Some(key);
        tx.scroll_into_view(row_key(key.1));
        true
    }

    pub fn clear_search(&mut self) {
        self.search.clear();
        self.refilter();
    }

    pub fn pick_zone(&mut self, zone: String) {
        self.zone = zone;
        self.refilter();
        self.select_first();
    }

    pub fn min_sold(&self) -> usize {
        self.min_sold
    }

    pub fn set_min_sold(&mut self, step: usize) {
        self.min_sold = step;
        self.refilter();
    }

    pub fn supply(&self) -> Supply {
        self.supply
    }

    pub fn set_supply(&mut self, supply: Supply) {
        self.supply = supply;
        self.refilter();
    }

    pub fn zone(&self) -> &str {
        &self.zone
    }

    /// The merchants, once they have loaded.
    pub fn merchants(&self) -> Option<&[Merchant]> {
        self.stocked().map(|f| f.catalog.merchants.as_slice())
    }

    /// Every zone with something to flip, once the merchants are priced.
    pub fn zones(&self) -> Option<&[Zone]> {
        self.stocked().map(|f| f.zones.as_slice())
    }
}

/// A row's key: its item, which no other row shows.
fn row_key(item: u32) -> String {
    format!("item-{item}")
}

impl App {
    pub(super) fn land_catalog(&mut self, result: Result<Catalog, String>) {
        self.flips.shelf = match result {
            Ok(catalog) => Shelf::Ready(Box::new(Stocked { catalog, flips: Vec::new(), zones: Vec::new(), rows: Vec::new(), hidden: None })),
            Err(e) => Shelf::Failed(e),
        };
        self.reprice_flips();
    }

    /// Prices every flip on the chosen realm for this character, selecting the first when the
    /// selection is gone. Needs the market loaded.
    pub(super) fn reprice_flips(&mut self) {
        let (Load::Ready(r), Shelf::Ready(f)) = (&self.load, &mut self.flips.shelf) else { return };
        f.flips = flips::evaluate(&f.catalog.merchants, &f.catalog.items, &r.market.book, &self.rules);
        f.zones = flip_rows::zones(&f.flips, &f.catalog.merchants);
        self.flips.refilter();
        if self.flips.picked().is_none() {
            self.flips.select_first();
        }
    }
}

pub fn main(s: &App, d: Dims) -> El {
    let f = match &s.flips.shelf {
        Shelf::Loading => return message("Loading merchants and their goods..."),
        Shelf::Failed(e) => return message(&format!("Could not load merchants: {e}")),
        Shelf::Ready(f) => f,
    };
    if !matches!(s.load, Load::Ready(_)) {
        return message("Loading TSM prices...");
    }
    row()
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .child(
            col()
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .h_full()
                .child(super::header::banner(None, header::header(s, f, d), toolbar::toolbar(s, d)))
                .child(table::table(s, f, d)),
        )
        .child(detail::detail(s, f, d))
}

