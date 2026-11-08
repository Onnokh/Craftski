//! Which flips the table shows, and in what order, and the zones to choose from. UI-free.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::flips::{Flip, Paid};
use crate::items::Item;
use crate::merchants::Merchant;
use crate::rows::{Hidden, Sort, SortColumn, order, sells};

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
    /// The fewest sales a day that show, any above none; `None` shows those without sales too.
    pub min_sold_per_day: Option<f64>,
    /// Matches the item, merchant or zone by name, ignoring case, in every zone. Blank keeps
    /// to `zone`.
    pub query: &'a str,
    pub supply: Supply,
}

/// Which merchants' stock to show by how much of it they have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Supply {
    All,
    Endless,
    /// Stock that runs out until the merchant restocks.
    Limited,
}

impl Supply {
    pub const ALL: [Supply; 3] = [Supply::All, Supply::Endless, Supply::Limited];

    pub fn label(self) -> &'static str {
        match self {
            Supply::All => "All",
            Supply::Endless => "Endless",
            Supply::Limited => "Limited",
        }
    }

    fn admits(self, limit: Option<u32>) -> bool {
        match self {
            Supply::All => true,
            Supply::Endless => limit.is_none(),
            Supply::Limited => limit.is_some(),
        }
    }
}

/// One item in the table: the merchant offer that represents it and the other offers of it
/// that pass the filter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Index into the flips: the best offer for this character.
    pub flip: usize,
    /// Indices into the flips, best first.
    pub others: Vec<usize>,
}

/// A filter beyond the zone and the search that can hide flips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterKind {
    Sells,
    Supply,
    HideUnprofitable,
}

impl Filter<'_> {
    fn on(&self) -> Vec<FilterKind> {
        let on = [(FilterKind::Sells, self.min_sold_per_day.is_some()), (FilterKind::Supply, self.supply != Supply::All), (FilterKind::HideUnprofitable, self.hide_unprofitable)];
        on.into_iter().filter_map(|(k, on)| on.then_some(k)).collect()
    }

    fn without(self, off: &[FilterKind]) -> Self {
        let mut f = self;
        for k in off {
            match k {
                FilterKind::Sells => f.min_sold_per_day = None,
                FilterKind::Supply => f.supply = Supply::All,
                FilterKind::HideUnprofitable => f.hide_unprofitable = false,
            }
        }
        f
    }
}

/// One row per item that passes `filter`, in order.
pub fn visible(flips: &[Flip], merchants: &[Merchant], items: &HashMap<u32, Item>, filter: Filter, sort: Sort<Column>) -> Vec<Row> {
    let item_name = |f: &Flip| items.get(&f.item).map_or("", |i| i.name.as_str());
    let by_name = |a: usize, b: usize| {
        let (fa, fb) = (&flips[a], &flips[b]);
        let item = item_name(fa).cmp(item_name(fb));
        let vendor = merchants[fa.merchant].name.cmp(&merchants[fb.merchant].name);
        let names = if sort.column == Column::Vendor { vendor.then(item) } else { item.then(vendor) };
        names.then(fa.merchant.cmp(&fb.merchant)).then(fa.item.cmp(&fb.item))
    };
    let groups = grouped(flips, merchants, items, filter);
    let mut at: Vec<usize> = (0..groups.len()).collect();
    order(&mut at, sort, |g| sort.column.number(&flips[groups[g].flip]), |a, b| by_name(groups[a].flip, groups[b].flip));
    at.into_iter().map(|g| groups[g].clone()).collect()
}

/// One row per item that passes `filter`, by item id.
fn grouped(flips: &[Flip], merchants: &[Merchant], items: &HashMap<u32, Item>, filter: Filter) -> Vec<Row> {
    let needle = filter.query.trim().to_lowercase();
    let item_name = |f: &Flip| items.get(&f.item).map_or("", |i| i.name.as_str());
    let kept: Vec<usize> = (0..flips.len())
        .filter(|&i| {
            let (f, m) = (&flips[i], &merchants[flips[i].merchant]);
            let place = if needle.is_empty() { m.zone == filter.zone } else { [item_name(f), &m.name, &m.zone].iter().any(|n| n.to_lowercase().contains(&needle)) };
            let profitable = !filter.hide_unprofitable || f.profit.is_some_and(|p| p > 0);
            let selling = sells(f.sold_per_day, filter.min_sold_per_day);
            place && profitable && selling && filter.supply.admits(f.limit)
        })
        .collect();
    dedupe(flips, merchants, kept)
}

/// The items the zone and the search show but the filters hide, each counted once.
pub fn hidden(flips: &[Flip], merchants: &[Merchant], items: &HashMap<u32, Item>, filter: Filter) -> Option<Hidden<FilterKind>> {
    Hidden::find(&filter.on(), |off| grouped(flips, merchants, items, filter.without(off)).len())
}

/// Groups the `kept` flips by item, each group led by its best offer.
pub fn dedupe(flips: &[Flip], merchants: &[Merchant], mut kept: Vec<usize>) -> Vec<Row> {
    kept.sort_by(|&a, &b| flips[a].item.cmp(&flips[b].item).then_with(|| best_first(flips, merchants, a, b)));
    kept.chunk_by(|&a, &b| flips[a].item == flips[b].item).map(|g| Row { flip: g[0], others: g[1..].to_vec() }).collect()
}

/// The better of two offers of one item for this character: the higher profit, else the lower
/// price, unknowns last either way; then an endless supply, then the merchant's name.
fn best_first(flips: &[Flip], merchants: &[Merchant], a: usize, b: usize) -> Ordering {
    let (fa, fb) = (&flips[a], &flips[b]);
    let paid = match (fa.paid.map(Paid::unit), fb.paid.map(Paid::unit)) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (x, y) => y.is_some().cmp(&x.is_some()),
    };
    fb.profit
        .cmp(&fa.profit)
        .then(paid)
        .then(fa.limit.is_some().cmp(&fb.limit.is_some()))
        .then_with(|| merchants[fa.merchant].name.cmp(&merchants[fb.merchant].name))
        .then(fa.merchant.cmp(&fb.merchant))
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

    const TB: Filter = Filter { zone: HOME, hide_unprofitable: false, min_sold_per_day: None, query: "", supply: Supply::All };
    const BY_PROFIT: Sort<Column> = Sort { column: Column::Profit, descending: true };

    /// The flip leading each row.
    fn leads(rows: Vec<Row>) -> Vec<usize> {
        rows.into_iter().map(|r| r.flip).collect()
    }

    fn paying(copper: u64, f: Flip) -> Flip {
        Flip { paid: Some(Paid::Tsm(Copper(copper))), ..f }
    }

    #[test]
    fn keeps_to_the_zone_until_a_search_looks_everywhere() {
        let (f, m, i) = fixture();
        let by_item = Sort { column: Column::Item, descending: false };
        assert_eq!(leads(visible(&f, &m, &i, TB, by_item)), vec![2, 0, 1]);
        assert_eq!(visible(&f, &m, &i, Filter { query: "FLUX", ..TB }, by_item), vec![Row { flip: 3, others: vec![1] }]);
        assert_eq!(leads(visible(&f, &m, &i, Filter { query: "durotar", ..TB }, by_item)), vec![3]);
        assert_eq!(leads(visible(&f, &m, &i, Filter { query: " kuna ", ..TB }, by_item)), vec![0]);
    }

    #[test]
    fn hides_losses_unpriced_flips_and_slow_sellers() {
        let (f, m, i) = fixture();
        assert_eq!(leads(visible(&f, &m, &i, Filter { hide_unprofitable: true, ..TB }, BY_PROFIT)), vec![0]);
        assert_eq!(leads(visible(&f, &m, &i, Filter { min_sold_per_day: Some(1.0), ..TB }, BY_PROFIT)), vec![1]);
        assert_eq!(leads(visible(&f, &m, &i, TB, BY_PROFIT)), vec![0, 1, 2]);
    }

    #[test]
    fn sorts_by_vendor_name_and_counts_an_endless_supply_as_the_most() {
        let (f, m, i) = fixture();
        assert_eq!(leads(visible(&f, &m, &i, TB, Sort { column: Column::Vendor, descending: false })), vec![2, 1, 0]);
        assert_eq!(leads(visible(&f, &m, &i, TB, Sort { column: Column::Limit, descending: true })), vec![2, 1, 0]);
    }

    #[test]
    fn of_two_merchants_the_more_profitable_or_else_the_cheaper_one_shows() {
        let (_, m, i) = fixture();
        let profits = vec![flip(0, 1, Some(40), None, None), flip(1, 1, Some(90), None, None)];
        assert_eq!(visible(&profits, &m, &i, TB, BY_PROFIT), vec![Row { flip: 1, others: vec![0] }]);
        let unsold = vec![paying(300, flip(0, 1, None, None, None)), paying(250, flip(1, 1, None, None, None)), Flip { paid: None, ..flip(2, 1, None, None, None) }];
        assert_eq!(visible(&unsold, &m, &i, Filter { query: "shortbow", ..TB }, BY_PROFIT), vec![Row { flip: 1, others: vec![0, 2] }]);
    }

    #[test]
    fn on_a_tie_an_endless_supply_beats_a_limited_one_then_the_name_decides() {
        let (_, m, i) = fixture();
        let tied = vec![flip(1, 1, Some(70), None, Some(5)), flip(0, 1, Some(70), None, None)];
        assert_eq!(visible(&tied, &m, &i, TB, BY_PROFIT), vec![Row { flip: 1, others: vec![0] }]);
        let endless = vec![flip(0, 1, Some(70), None, None), flip(1, 1, Some(70), None, None)];
        assert_eq!(visible(&endless, &m, &i, TB, BY_PROFIT), vec![Row { flip: 1, others: vec![0] }]);
    }

    #[test]
    fn counts_only_the_other_merchants_that_pass_the_filter() {
        let (_, m, i) = fixture();
        let flips = vec![flip(0, 1, Some(70), None, None), flip(1, 1, Some(-5), None, None), flip(2, 1, Some(30), None, None)];
        let search = Filter { query: "shortbow", ..TB };
        assert_eq!(visible(&flips, &m, &i, search, BY_PROFIT), vec![Row { flip: 0, others: vec![2, 1] }]);
        assert_eq!(visible(&flips, &m, &i, Filter { hide_unprofitable: true, ..search }, BY_PROFIT), vec![Row { flip: 0, others: vec![2] }]);
    }

    #[test]
    fn the_supply_filter_runs_before_the_best_merchant_is_chosen() {
        let (_, m, i) = fixture();
        let flips = vec![flip(0, 1, Some(90), None, Some(3)), flip(1, 1, Some(40), None, None), flip(1, 2, Some(10), None, Some(1))];
        let rows = |supply| visible(&flips, &m, &i, Filter { supply, ..TB }, BY_PROFIT);
        assert_eq!(rows(Supply::All), vec![Row { flip: 0, others: vec![1] }, Row { flip: 2, others: vec![] }]);
        assert_eq!(rows(Supply::Endless), vec![Row { flip: 1, others: vec![] }]);
        assert_eq!(rows(Supply::Limited), vec![Row { flip: 0, others: vec![] }, Row { flip: 2, others: vec![] }]);
    }

    #[test]
    fn never_shows_an_item_twice_under_any_supply_or_search() {
        let (mut f, m, i) = fixture();
        f.extend([flip(0, 2, Some(5), Some(3.0), Some(4)), flip(2, 1, None, None, None), flip(0, 3, Some(1), None, None), flip(2, 3, Some(1), None, Some(2))]);
        let mut seen = Vec::new();
        for supply in Supply::ALL {
            for query in ["", "a", "flux", "thunder", "zansoa"] {
                let items: Vec<u32> = visible(&f, &m, &i, Filter { supply, query, ..TB }, BY_PROFIT).iter().map(|r| f[r.flip].item).collect();
                seen.push((supply.label(), query, items));
            }
        }
        let twice: Vec<_> = seen.iter().filter(|(_, _, items)| items.iter().collect::<BTreeSet<_>>().len() != items.len()).collect();
        assert_eq!(twice, Vec::<&(&str, &str, Vec<u32>)>::new());
        assert_eq!(seen[1], ("All", "a", vec![1, 2, 3]));
    }

    #[test]
    fn names_the_one_filter_that_hides_an_item_the_zone_or_search_shows() {
        let (f, m, i) = fixture();
        assert_eq!(hidden(&f, &m, &i, Filter { hide_unprofitable: true, ..TB }), Some(Hidden { count: 2, by: vec![FilterKind::HideUnprofitable] }));
        let endless_sellers = Filter { supply: Supply::Endless, min_sold_per_day: Some(1.0), ..TB };
        assert_eq!(hidden(&f, &m, &i, Filter { query: "swiss", ..endless_sellers }), Some(Hidden { count: 1, by: vec![FilterKind::Sells] }));
        assert_eq!(hidden(&f, &m, &i, Filter { query: "kuna", ..endless_sellers }), Some(Hidden { count: 1, by: vec![FilterKind::Sells, FilterKind::Supply] }));
    }

    #[test]
    fn an_item_several_filters_hide_counts_once() {
        let (f, m, i) = fixture();
        // Sells hides the shortbow and the swiss, Supply the shortbow, Hide unprofitable the flux and the swiss.
        let every = Filter { min_sold_per_day: Some(1.0), supply: Supply::Endless, hide_unprofitable: true, ..TB };
        assert_eq!(visible(&f, &m, &i, every, BY_PROFIT), vec![]);
        assert_eq!(hidden(&f, &m, &i, every), Some(Hidden { count: 3, by: vec![FilterKind::Sells, FilterKind::Supply, FilterKind::HideUnprofitable] }));
    }

    #[test]
    fn an_item_another_merchant_still_shows_is_not_hidden() {
        let (f, m, i) = fixture();
        assert_eq!(hidden(&f, &m, &i, Filter { query: "flux", hide_unprofitable: true, ..TB }), None);
        assert_eq!(hidden(&f, &m, &i, Filter { query: "flux", supply: Supply::Endless, min_sold_per_day: Some(5.0), ..TB }), None);
        assert_eq!(hidden(&f, &m, &i, TB), None);
    }

    /// The user's report: searching "patter" at Sells 5+, an endless supply and only profitable
    /// flips showed two rows, hiding the new armor kit patterns.
    /// `cargo test -- --ignored --nocapture real_data`
    #[test]
    #[ignore = "reads this machine's TSM data and the merchant catalog"]
    fn real_data_patterns_hidden_by_the_filters() {
        let (mut book, _) = crate::market::read_book(&crate::wow::locate().unwrap().path).unwrap();
        book.choose_realm(&crate::settings::load_realm());
        let c = crate::merchants::load().unwrap();
        let flips = crate::flips::evaluate(&c.merchants, &c.items, &book, &crate::flips::RULES);
        let filter = Filter { zone: HOME, hide_unprofitable: true, min_sold_per_day: Some(5.0), query: "patter", supply: Supply::Endless };
        let names = |filter| -> Vec<String> { visible(&flips, &c.merchants, &c.items, filter, BY_PROFIT).iter().map(|r| c.items[&flips[r.flip].item].name.clone()).collect() };
        let shown = names(filter);
        let h = hidden(&flips, &c.merchants, &c.items, filter).unwrap();
        let all = names(filter.without(&h.by));
        println!("shown {}: {shown:?}\nhidden {h:?}\nafter Show all {}: {all:?}", shown.len(), all.len());
        assert_eq!(all.len(), shown.len() + h.count);
        for pattern in ["Pattern: Mystic Medium Armor Kit", "Pattern: Forceful Heavy Armor Kit", "Pattern: Mystic Heavy Armor Kit"] {
            assert!(!shown.iter().any(|n| n == pattern) && all.iter().any(|n| n == pattern), "{pattern}");
        }
    }

    /// `cargo test -- --ignored --nocapture real_data`
    #[test]
    #[ignore = "reads this machine's TSM data and the merchant catalog"]
    fn real_data_never_shows_an_item_twice() {
        let (mut book, _) = crate::market::read_book(&crate::wow::locate().unwrap().path).unwrap();
        book.choose_realm(&crate::settings::load_realm());
        let c = crate::merchants::load().unwrap();
        let flips = crate::flips::evaluate(&c.merchants, &c.items, &book, &crate::flips::RULES);
        let mut twice = Vec::new();
        for zone in zones(&flips, &c.merchants) {
            for hide_unprofitable in [false, true] {
                for supply in Supply::ALL {
                    for query in ["", "a", "e", "flux", "weak", "thunder bluff", "kuna", "dye"] {
                        let filter = Filter { zone: &zone.name, hide_unprofitable, min_sold_per_day: None, query, supply };
                        let items: Vec<u32> = visible(&flips, &c.merchants, &c.items, filter, BY_PROFIT).iter().map(|r| flips[r.flip].item).collect();
                        if items.iter().collect::<BTreeSet<_>>().len() != items.len() {
                            twice.push(format!("{} {hide_unprofitable} {} {query:?}", zone.name, supply.label()));
                        }
                        if zone.name == HOME && !hide_unprofitable {
                            println!("{HOME}, {}, {query:?}: {} rows", supply.label(), items.len());
                        }
                    }
                }
            }
        }
        assert_eq!(twice, Vec::<String>::new());
    }

    #[test]
    fn lists_each_zone_with_its_merchants_that_have_flips() {
        let (f, m, _) = fixture();
        assert_eq!(zones(&f, &m), vec![Zone { name: "Durotar".into(), merchants: 1 }, Zone { name: "Thunder Bluff".into(), merchants: 2 }]);
    }
}
