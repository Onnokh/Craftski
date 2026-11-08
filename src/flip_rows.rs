//! Which flips the table shows, and in what order, and the zones to choose from. UI-free.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::flips::{Flip, Paid};
use crate::items::Item;
use crate::merchants::Merchant;
use crate::rows::{Sort, SortColumn, order};

pub const HOME: &str = "Thunder Bluff";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Item,
    Vendor,
    Paid,
    Sells,
    DepositLoss,
    Profit,
    SaleRate,
    SoldPerDay,
    Limit,
}

impl Column {
    pub const ALL: [Column; 9] = [Column::Item, Column::Vendor, Column::Paid, Column::Sells, Column::DepositLoss, Column::Profit, Column::SaleRate, Column::SoldPerDay, Column::Limit];

    pub fn title(self) -> &'static str {
        match self {
            Column::Item => "Item",
            Column::Vendor => "Vendor",
            Column::Paid => "Paid",
            Column::Sells => "Sells for",
            Column::DepositLoss => "Deposit loss",
            Column::Profit => "Profit",
            Column::SaleRate => "Sale rate",
            Column::SoldPerDay => "Sold/day",
            Column::Limit => "Limit",
        }
    }

    fn number(self, f: &Flip) -> Option<f64> {
        match self {
            Column::Item | Column::Vendor => None,
            Column::Paid => f.paid.map(Paid::unit),
            Column::Sells => f.sale.map(|s| s.unit.0 as f64),
            Column::DepositLoss => f.deposit.loss,
            Column::Profit => f.profit.map(|p| p as f64),
            Column::SaleRate => f.sale_rate,
            Column::SoldPerDay => f.sold_per_day,
            Column::Limit => Some(f.limit.map_or(f64::INFINITY, f64::from)),
        }
    }
}

impl SortColumn for Column {
    fn by_name(self) -> bool {
        matches!(self, Column::Item | Column::Vendor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Filter<'a> {
    pub zone: &'a str,
    pub hide_unprofitable: bool,
    pub min_sold_per_day: f64,
    /// Matches the item, merchant or zone by name, ignoring case, in every zone. Blank keeps
    /// to `zone`.
    pub query: &'a str,
}

/// Indices into `flips` to show, in order.
pub fn visible(flips: &[Flip], merchants: &[Merchant], items: &HashMap<u32, Item>, filter: Filter, sort: Sort<Column>) -> Vec<usize> {
    let needle = filter.query.trim().to_lowercase();
    let item_name = |f: &Flip| items.get(&f.item).map_or("", |i| i.name.as_str());
    let mut rows: Vec<usize> = (0..flips.len())
        .filter(|&i| {
            let (f, m) = (&flips[i], &merchants[flips[i].merchant]);
            let place = if needle.is_empty() { m.zone == filter.zone } else { [item_name(f), &m.name, &m.zone].iter().any(|n| n.to_lowercase().contains(&needle)) };
            let profitable = !filter.hide_unprofitable || f.profit.is_some_and(|p| p > 0);
            let selling = filter.min_sold_per_day <= 0.0 || f.sold_per_day.is_some_and(|s| s >= filter.min_sold_per_day);
            place && profitable && selling
        })
        .collect();
    let by_name = |a: usize, b: usize| {
        let (fa, fb) = (&flips[a], &flips[b]);
        let item = item_name(fa).cmp(item_name(fb));
        let vendor = merchants[fa.merchant].name.cmp(&merchants[fb.merchant].name);
        let names = if sort.column == Column::Vendor { vendor.then(item) } else { item.then(vendor) };
        names.then(fa.merchant.cmp(&fb.merchant)).then(fa.item.cmp(&fb.item))
    };
    order(&mut rows, sort, |i| sort.column.number(&flips[i]), by_name);
    rows
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zone {
    pub name: String,
    /// Merchants there with something to flip.
    pub merchants: usize,
}

/// Every zone with a merchant that has something to flip, A to Z.
pub fn zones(flips: &[Flip], merchants: &[Merchant]) -> Vec<Zone> {
    let mut by_zone: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    for f in flips {
        by_zone.entry(merchants[f.merchant].zone.as_str()).or_default().insert(f.merchant);
    }
    by_zone.into_iter().filter(|(name, _)| !name.is_empty()).map(|(name, m)| Zone { name: name.to_owned(), merchants: m.len() }).collect()
}

/// The header's summary of one zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    /// Indices into the flips.
    pub top: Option<usize>,
    /// The most profitable flip that sells at least once a day across the region.
    pub best_selling: Option<usize>,
    pub profitable: usize,
    pub total: usize,
    /// Flips the region has no sale rate for, so their deposit loss is unknown.
    pub no_rate: usize,
}

pub fn summary(flips: &[Flip], merchants: &[Merchant], zone: &str) -> Summary {
    let here: Vec<usize> = (0..flips.len()).filter(|&i| merchants[flips[i].merchant].zone == zone).collect();
    let earning = || here.iter().copied().filter(|&i| flips[i].profit.is_some_and(|p| p > 0));
    // The first of equal profits wins, so ties resolve by merchant order rather than at random.
    let best = |a: Option<usize>, i: usize| if a.is_some_and(|a| flips[a].profit >= flips[i].profit) { a } else { Some(i) };
    Summary {
        top: earning().fold(None, best),
        best_selling: earning().filter(|&i| flips[i].sold_per_day.is_some_and(|s| s >= 1.0)).fold(None, best),
        profitable: earning().count(),
        total: here.len(),
        no_rate: here.iter().filter(|&&i| flips[i].sale_rate.is_none()).count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::copper::Copper;
    use crate::flips::Deposit;
    use crate::merchants::StockSource;
    use crate::recipes::Quality;

    fn merchant(name: &str, zone: &str) -> Merchant {
        Merchant { npc: 0, name: name.into(), role: String::new(), zone: zone.into(), at: (0.0, 0.0), side: None, rep: None, source: StockSource::Classic, stock: vec![] }
    }

    fn flip(merchant: usize, item: u32, profit: Option<i64>, sold: Option<f64>, limit: Option<u32>) -> Flip {
        let deposit = Deposit { per_listing: Copper(100), post_stack: 1, loss: None };
        Flip { merchant, item, limit, paid: Some(Paid::Tsm(Copper(10))), sale: None, resale: None, deposit, sale_rate: None, sold_per_day: sold, profit }
    }

    fn fixture() -> (Vec<Flip>, Vec<Merchant>, HashMap<u32, Item>) {
        let merchants = vec![merchant("Kuna Thunderhorn", "Thunder Bluff"), merchant("Ansekhwa", "Thunder Bluff"), merchant("Zansoa", "Durotar")];
        let name = |n: &str| Item { name: n.into(), quality: Quality::Common, icon: None, facts: None };
        let items = HashMap::from([(1, name("Fine Shortbow")), (2, name("Strong Flux")), (3, name("Alterac Swiss"))]);
        let flips = vec![flip(0, 1, Some(500), Some(0.5), Some(1)), flip(1, 2, Some(-20), Some(9.0), None), flip(1, 3, None, None, None), flip(2, 2, Some(80), Some(9.0), None)];
        (flips, merchants, items)
    }

    const TB: Filter = Filter { zone: HOME, hide_unprofitable: false, min_sold_per_day: 0.0, query: "" };

    #[test]
    fn keeps_to_the_zone_until_a_search_looks_everywhere() {
        let (f, m, i) = fixture();
        let by_item = Sort { column: Column::Item, descending: false };
        assert_eq!(visible(&f, &m, &i, TB, by_item), vec![2, 0, 1]);
        assert_eq!(visible(&f, &m, &i, Filter { query: "FLUX", ..TB }, by_item), vec![1, 3]);
        assert_eq!(visible(&f, &m, &i, Filter { query: "durotar", ..TB }, by_item), vec![3]);
        assert_eq!(visible(&f, &m, &i, Filter { query: " kuna ", ..TB }, by_item), vec![0]);
    }

    #[test]
    fn hides_losses_unpriced_flips_and_slow_sellers() {
        let (f, m, i) = fixture();
        let by_profit = Sort { column: Column::Profit, descending: true };
        assert_eq!(visible(&f, &m, &i, Filter { hide_unprofitable: true, ..TB }, by_profit), vec![0]);
        assert_eq!(visible(&f, &m, &i, Filter { min_sold_per_day: 1.0, ..TB }, by_profit), vec![1]);
        assert_eq!(visible(&f, &m, &i, TB, by_profit), vec![0, 1, 2]);
    }

    #[test]
    fn sorts_by_vendor_name_and_counts_an_endless_supply_as_the_most() {
        let (f, m, i) = fixture();
        assert_eq!(visible(&f, &m, &i, TB, Sort { column: Column::Vendor, descending: false }), vec![2, 1, 0]);
        assert_eq!(visible(&f, &m, &i, TB, Sort { column: Column::Limit, descending: true }), vec![2, 1, 0]);
    }

    #[test]
    fn lists_each_zone_with_its_merchants_that_have_flips() {
        let (f, m, _) = fixture();
        assert_eq!(zones(&f, &m), vec![Zone { name: "Durotar".into(), merchants: 1 }, Zone { name: "Thunder Bluff".into(), merchants: 2 }]);
    }

    #[test]
    fn sums_up_a_zone_with_its_top_earner_and_best_daily_seller() {
        let (f, m, _) = fixture();
        assert_eq!(summary(&f, &m, HOME), Summary { top: Some(0), best_selling: None, profitable: 1, total: 3, no_rate: 3 });
        assert_eq!(summary(&f, &m, "Durotar"), Summary { top: Some(3), best_selling: Some(3), profitable: 1, total: 1, no_rate: 1 });
    }
}
