//! The vendor flips tab: goods a merchant sells for less than the auction house pays.

mod detail;
mod header;
mod table;
mod toolbar;

use std::collections::BTreeSet;

use rune::{El, TextEdit, Tx, col, row};

use super::{App, Dims, Load, SOLD_STEPS, message};
use crate::flip_rows::{self, Column, Filter, Summary, Zone};
use crate::flips::{self, Flip};
use crate::items::Item;
use crate::merchants::{Catalog, Merchant};
use crate::recipes::FileDataId;
use crate::rows::Sort;

pub struct State {
    shelf: Shelf,
    zone: String,
    sort: Sort<Column>,
    min_sold: usize,
    hide_unprofitable: bool,
    /// The flip the detail panel shows, by merchant npc and item, kept while filters hide it.
    selected: Option<(u32, u32)>,
    pub search: TextEdit,
    zone_menu: bool,
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
    summary: Summary,
    rows: Vec<usize>,
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
            min_sold: 1,
            hide_unprofitable: true,
            selected: None,
            search: TextEdit::new(""),
            zone_menu: false,
        }
    }

    fn stocked(&self) -> Option<&Stocked> {
        match &self.shelf {
            Shelf::Ready(f) => Some(f),
            _ => None,
        }
    }

    fn refilter(&mut self) {
        let Shelf::Ready(f) = &mut self.shelf else { return };
        let filter = Filter { zone: &self.zone, hide_unprofitable: self.hide_unprofitable, min_sold_per_day: SOLD_STEPS[self.min_sold].1, query: &self.search.text };
        f.summary = flip_rows::summary(&f.flips, &f.catalog.merchants, &self.zone);
        f.rows = flip_rows::visible(&f.flips, &f.catalog.merchants, &f.catalog.items, filter, self.sort);
    }

    fn select_first(&mut self) {
        self.selected = self.stocked().and_then(|f| Some(f.key(*f.rows.first()?)));
    }

    /// The selected flip's index into the flips.
    fn picked(&self) -> Option<(&Stocked, usize)> {
        let f = self.stocked()?;
        let key = self.selected?;
        Some((f, (0..f.flips.len()).find(|&i| f.key(i) == key)?))
    }

    pub fn step(&mut self, by: isize, tx: &mut Tx) -> bool {
        let Some(f) = self.stocked() else { return false };
        if f.rows.is_empty() {
            return false;
        }
        let at = self.selected.and_then(|key| f.rows.iter().position(|&i| f.key(i) == key));
        let next = match at {
            Some(at) => at.saturating_add_signed(by).min(f.rows.len() - 1),
            None => 0,
        };
        let key = f.key(f.rows[next]);
        self.selected = Some(key);
        tx.scroll_into_view(row_key(key));
        true
    }

    pub fn clear_search(&mut self) {
        self.search.clear();
        self.refilter();
    }

    fn pick_zone(&mut self, zone: String) {
        self.zone = zone;
        self.zone_menu = false;
        self.refilter();
        self.select_first();
    }

    pub fn open_menu(&mut self) {
        self.zone_menu = true;
    }

    /// Closes the zone menu, saying whether it was open.
    pub fn close_menu(&mut self) -> bool {
        std::mem::replace(&mut self.zone_menu, false)
    }
}

fn row_key((npc, item): (u32, u32)) -> String {
    format!("{npc}-{item}")
}

impl App {
    pub(super) fn land_catalog(&mut self, result: Result<Catalog, String>) {
        self.flips.shelf = match result {
            Ok(catalog) => Shelf::Ready(Box::new(Stocked { catalog, flips: Vec::new(), zones: Vec::new(), summary: flip_rows::summary(&[], &[], ""), rows: Vec::new() })),
            Err(e) => Shelf::Failed(e),
        };
        self.reprice_flips();
        self.flips.select_first();
    }

    /// Prices every flip under the current source. Needs the market loaded.
    pub(super) fn reprice_flips(&mut self) {
        let (Load::Ready(r), Shelf::Ready(f)) = (&self.load, &mut self.flips.shelf) else { return };
        f.flips = flips::evaluate(&f.catalog.merchants, &f.catalog.items, &r.market.book, self.source, &flips::RULES);
        f.zones = flip_rows::zones(&f.flips, &f.catalog.merchants);
        self.flips.refilter();
    }
}

pub fn main(s: &App, d: Dims) -> El {
    let f = match &s.flips.shelf {
        Shelf::Loading => return message("Loading merchants and their goods..."),
        Shelf::Failed(e) => return message(&format!("Could not load merchants: {e}")),
        Shelf::Ready(f) => f,
    };
    let Load::Ready(r) = &s.load else { return message("Loading TSM prices...") };
    row()
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .child(col().grow(1.0).shrink().min_w(0.0).h_full().child(header::header(s, r, f, d)).child(toolbar::toolbar(s, d)).child(table::table(s, f, d)))
        .child(detail::detail(s, f, d))
}

/// The open zone menu, drawn over the whole window.
pub fn overlay(s: &App) -> Vec<El> {
    match s.flips.stocked() {
        Some(f) if s.flips.zone_menu => toolbar::zone_menu(s, f),
        _ => Vec::new(),
    }
}
