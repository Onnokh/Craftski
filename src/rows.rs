//! Which recipes the table shows, and in what order. UI-free.

use std::cmp::Ordering;

use crate::profit::Evaluation;
use crate::recipes::Recipe;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Item,
    Skill,
    Cost,
    Resale,
    Profit,
    Margin,
    SoldPerDay,
    Auctions,
}

impl Column {
    pub const ALL: [Column; 8] = [Column::Item, Column::Skill, Column::Cost, Column::Resale, Column::Profit, Column::Margin, Column::SoldPerDay, Column::Auctions];

    pub fn title(self) -> &'static str {
        match self {
            Column::Item => "Item",
            Column::Skill => "Skill",
            Column::Cost => "Mats cost",
            Column::Resale => "Sells for",
            Column::Profit => "Profit",
            Column::Margin => "Margin",
            Column::SoldPerDay => "Sold/day",
            Column::Auctions => "Listed",
        }
    }

    fn number(self, r: &Recipe, e: &Evaluation) -> Option<f64> {
        match self {
            Column::Item => None,
            Column::Skill => r.skill.map(|s| f64::from(s.learn)),
            Column::Cost => e.cost.map(|c| c.0 as f64),
            Column::Resale => e.resale.map(|c| c.0 as f64),
            Column::Profit => e.profit.map(|p| p as f64),
            Column::Margin => e.margin.map(f64::from),
            Column::SoldPerDay => e.sold_per_day,
            Column::Auctions => e.auctions.map(f64::from),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Filter<'a> {
    pub hide_unpriced: bool,
    pub min_sold_per_day: f64,
    /// Matches the crafted item or any reagent by name, ignoring case. Blank matches all.
    pub query: &'a str,
}

/// A filter beyond the search that can hide recipes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterKind {
    Sells,
    HideUnpriced,
}

impl Filter<'_> {
    fn on(&self) -> Vec<FilterKind> {
        [(FilterKind::Sells, self.min_sold_per_day > 0.0), (FilterKind::HideUnpriced, self.hide_unpriced)].into_iter().filter_map(|(k, on)| on.then_some(k)).collect()
    }

    fn without(self, off: &[FilterKind]) -> Self {
        let mut f = self;
        for k in off {
            match k {
                FilterKind::Sells => f.min_sold_per_day = 0.0,
                FilterKind::HideUnpriced => f.hide_unpriced = false,
            }
        }
        f
    }
}

/// Rows the filters hide that the search would otherwise show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hidden<K> {
    /// The rows that show once every filter in `by` is off.
    pub count: usize,
    /// The filters that hide at least one of them, in the order `on` lists them.
    pub by: Vec<K>,
}

impl<K: Copy> Hidden<K> {
    /// `on` lists the filters that are on, and `shown(off)` counts the rows that show with the
    /// filters in `off` turned off. A filter counts as hiding when turning it off shows more,
    /// with or without some of the others off too, so two filters that each hide one row are
    /// both named. None when turning them all off shows nothing more.
    pub fn find(on: &[K], shown: impl Fn(&[K]) -> usize) -> Option<Hidden<K>> {
        let subset = |mask: usize| -> Vec<K> { on.iter().enumerate().filter(|(i, _)| mask >> i & 1 == 1).map(|(_, &k)| k).collect() };
        let counts: Vec<usize> = (0..1 << on.len()).map(|mask| shown(&subset(mask))).collect();
        let count = counts[counts.len() - 1] - counts[0];
        let hides = |i: usize| (0..counts.len()).any(|mask| mask >> i & 1 == 1 && counts[mask] > counts[mask ^ 1 << i]);
        (count > 0).then(|| Hidden { count, by: (0..on.len()).filter(|&i| hides(i)).map(|i| on[i]).collect() })
    }
}

/// The line under the table that says what the filters hide, as in `6 more match “patter” but
/// are hidden by Sells 5+ and Hide unprofitable`. `names` are the hiding filters as the toolbar
/// shows them; `more` says the table shows some rows already.
pub fn hint(count: usize, query: &str, names: &[String], more: bool) -> String {
    let list = match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    let more = if more { " more" } else { "" };
    let (verb, be) = if count == 1 { ("matches", "is") } else { ("match", "are") };
    match query.trim() {
        "" => format!("{count}{more} {be} hidden by {list}"),
        q => format!("{count}{more} {verb} “{q}” but {be} hidden by {list}"),
    }
}

/// A column a table sorts by: by name, or by a number.
pub trait SortColumn: Copy + PartialEq {
    fn by_name(self) -> bool;
}

impl SortColumn for Column {
    fn by_name(self) -> bool {
        self == Column::Item
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort<C> {
    pub column: C,
    pub descending: bool,
}

impl<C: SortColumn> Sort<C> {
    /// The sort after a click on `column`'s header: the same column flips, a new one starts
    /// with names A to Z and numbers largest first.
    pub fn clicked(self, column: C) -> Sort<C> {
        if column == self.column {
            Sort { column, descending: !self.descending }
        } else {
            Sort { column, descending: !column.by_name() }
        }
    }
}

/// Orders `rows` under `sort`: a name column by `by_name`, a number column by `number` with
/// unknown values last whichever way it sorts and ties by `by_name`.
pub fn order<C: SortColumn>(rows: &mut [usize], sort: Sort<C>, number: impl Fn(usize) -> Option<f64>, by_name: impl Fn(usize, usize) -> Ordering) {
    let directed = |o: Ordering| if sort.descending { o.reverse() } else { o };
    rows.sort_by(|&a, &b| {
        if sort.column.by_name() {
            return directed(by_name(a, b));
        }
        match (number(a), number(b)) {
            (Some(x), Some(y)) => directed(x.total_cmp(&y)).then_with(|| by_name(a, b)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => by_name(a, b),
        }
    });
}

fn matches(recipe: &Recipe, needle: &str) -> bool {
    needle.is_empty() || std::iter::once(&recipe.item_name).chain(recipe.reagents.iter().map(|r| &r.item_name)).any(|name| name.to_lowercase().contains(needle))
}

/// Indices into `recipes` (and the parallel `evals`) to show, in order. Unknown values sort
/// last whichever way the column sorts.
pub fn visible(recipes: &[Recipe], evals: &[Evaluation], filter: Filter, sort: Sort<Column>) -> Vec<usize> {
    let mut rows = kept(recipes, evals, filter);
    let by_name = |a: usize, b: usize| recipes[a].item_name.cmp(&recipes[b].item_name).then(recipes[a].spell_id.cmp(&recipes[b].spell_id));
    order(&mut rows, sort, |i| sort.column.number(&recipes[i], &evals[i]), by_name);
    rows
}

fn kept(recipes: &[Recipe], evals: &[Evaluation], filter: Filter) -> Vec<usize> {
    let needle = filter.query.trim().to_lowercase();
    (0..recipes.len())
        .filter(|&i| {
            let e = &evals[i];
            let priced = !filter.hide_unpriced || e.profit.is_some();
            let selling = filter.min_sold_per_day <= 0.0 || e.sold_per_day.is_some_and(|s| s >= filter.min_sold_per_day);
            priced && selling && matches(&recipes[i], &needle)
        })
        .collect()
}

/// The recipes that the search matches but the filters hide.
pub fn hidden(recipes: &[Recipe], evals: &[Evaluation], filter: Filter) -> Option<Hidden<FilterKind>> {
    Hidden::find(&filter.on(), |off| kept(recipes, evals, filter.without(off)).len())
}

/// Sales a day with one decimal, or none once the count is large enough not to need it.
pub fn per_day(d: f64) -> String {
    if d >= 100.0 { format!("{d:.0}") } else { format!("{d:.1}") }
}

/// Margin as a whole percent, as in `379%`.
pub fn percent(margin: f32) -> String {
    format!("{:.0}%", margin * 100.0)
}

/// How much of the margin bar to fill: full at 250%, so typical margins still differ.
pub fn margin_fill(margin: f32) -> f32 {
    (margin.abs() / 2.5).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::copper::Copper;
    use crate::recipes::{Quality, Reagent, Skill};

    fn recipe(spell_id: u32, name: &str, skill: u16) -> Recipe {
        Recipe { spell_id, item_id: spell_id, item_name: name.into(), quality: Quality::Common, slot: None, icon: None, quantity: 1.0, skill: Some(Skill { learn: skill, grey: skill + 10 }), reagents: vec![] }
    }

    fn eval(profit: Option<i64>, sold: Option<f64>) -> Evaluation {
        Evaluation { cost: profit.map(|_| Copper(100)), unpriced: vec![], resale: None, profit, margin: None, sold_per_day: sold, auctions: None }
    }

    fn fixture() -> (Vec<Recipe>, Vec<Evaluation>) {
        (
            vec![recipe(1, "Armor Kit", 125), recipe(2, "Boots", 20), recipe(3, "Cloak", 225), recipe(4, "Drums", 125)],
            vec![eval(Some(50), Some(2.0)), eval(None, Some(9.0)), eval(Some(-10), None), eval(Some(300), Some(0.5))],
        )
    }

    const ALL: Filter = Filter { hide_unpriced: false, min_sold_per_day: 0.0, query: "" };

    #[test]
    fn unknown_values_sort_last_in_both_directions() {
        let (r, e) = fixture();
        assert_eq!(visible(&r, &e, ALL, Sort { column: Column::Profit, descending: true }), vec![3, 0, 2, 1]);
        assert_eq!(visible(&r, &e, ALL, Sort { column: Column::Profit, descending: false }), vec![2, 0, 3, 1]);
        assert_eq!(visible(&r, &e, ALL, Sort { column: Column::SoldPerDay, descending: false }), vec![3, 0, 1, 2]);
        assert_eq!(visible(&r, &e, ALL, Sort { column: Column::SoldPerDay, descending: true }), vec![1, 0, 3, 2]);
    }

    #[test]
    fn filters_hide_unpriced_rows_and_slow_sellers() {
        let (r, e) = fixture();
        let by_name = Sort { column: Column::Item, descending: false };
        assert_eq!(visible(&r, &e, Filter { hide_unpriced: true, min_sold_per_day: 0.0, query: "" }, by_name), vec![0, 2, 3]);
        assert_eq!(visible(&r, &e, Filter { hide_unpriced: false, min_sold_per_day: 1.0, query: "" }, by_name), vec![0, 1]);
        assert_eq!(visible(&r, &e, Filter { hide_unpriced: true, min_sold_per_day: 1.0, query: "" }, by_name), vec![0]);
    }

    #[test]
    fn search_matches_item_or_reagent_names_ignoring_case() {
        let (mut r, e) = fixture();
        let reagent = |name: &str| Reagent { item_id: 0, item_name: name.into(), quality: Quality::Common, icon: None, count: 1 };
        r[1].reagents = vec![reagent("Thick Leather")];
        r[3].reagents = vec![reagent("Rugged Leather"), reagent("Thick Leather")];
        let by_name = Sort { column: Column::Item, descending: false };
        let find = |query| visible(&r, &e, Filter { query, ..ALL }, by_name);
        assert_eq!(find("THICK leath"), vec![1, 3]);
        assert_eq!(find("cloak"), vec![2]);
        assert_eq!(find("  kit "), vec![0]);
        assert_eq!(find(""), vec![0, 1, 2, 3]);
        assert_eq!(find("mithril"), Vec::<usize>::new());
    }

    const SELLING_PRICED: Filter = Filter { hide_unpriced: true, min_sold_per_day: 1.0, query: "" };

    #[test]
    fn names_only_the_filter_that_hides_what_the_search_finds() {
        let (r, e) = fixture();
        assert_eq!(hidden(&r, &e, Filter { query: "boots", ..SELLING_PRICED }), Some(Hidden { count: 1, by: vec![FilterKind::HideUnpriced] }));
        assert_eq!(hidden(&r, &e, Filter { query: "drums", ..SELLING_PRICED }), Some(Hidden { count: 1, by: vec![FilterKind::Sells] }));
    }

    #[test]
    fn a_row_two_filters_hide_counts_once() {
        let (mut r, mut e) = fixture();
        r.push(recipe(5, "Elixir", 300));
        e.push(eval(None, None));
        // Sells hides the cloak, drums and elixir, Hide unpriced the boots and elixir.
        assert_eq!(hidden(&r, &e, SELLING_PRICED), Some(Hidden { count: 4, by: vec![FilterKind::Sells, FilterKind::HideUnpriced] }));
        assert_eq!(hidden(&r, &e, Filter { query: "elixir", ..SELLING_PRICED }), Some(Hidden { count: 1, by: vec![FilterKind::Sells, FilterKind::HideUnpriced] }));
    }

    #[test]
    fn nothing_hidden_is_none() {
        let (r, e) = fixture();
        assert_eq!(hidden(&r, &e, Filter { query: "kit", ..SELLING_PRICED }), None);
        assert_eq!(hidden(&r, &e, ALL), None);
        assert_eq!(hidden(&r, &e, Filter { min_sold_per_day: 1.0, ..ALL }), Some(Hidden { count: 2, by: vec![FilterKind::Sells] }));
    }

    #[test]
    fn the_hint_says_how_many_and_why() {
        let names = |n: &[&str]| n.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(hint(6, " patter ", &names(&["Sells 5+", "Hide unprofitable"]), true), "6 more match “patter” but are hidden by Sells 5+ and Hide unprofitable");
        assert_eq!(hint(212, "", &names(&["Sells 5+"]), true), "212 more are hidden by Sells 5+");
        assert_eq!(hint(1, "flux", &names(&["Sells 1+", "Supply Endless", "Hide unprofitable"]), false), "1 matches “flux” but is hidden by Sells 1+, Supply Endless and Hide unprofitable");
        assert_eq!(hint(3, "", &names(&["Hide unpriced"]), false), "3 are hidden by Hide unpriced");
    }

    #[test]
    fn clicking_a_header_flips_it_or_starts_a_sensible_direction() {
        let profit_desc = Sort { column: Column::Profit, descending: true };
        assert_eq!(profit_desc.clicked(Column::Profit), Sort { column: Column::Profit, descending: false });
        assert_eq!(profit_desc.clicked(Column::Item), Sort { column: Column::Item, descending: false });
        assert_eq!(Sort { column: Column::Item, descending: false }.clicked(Column::Margin), Sort { column: Column::Margin, descending: true });
    }

    #[test]
    fn numbers_read_as_the_table_prints_them() {
        assert_eq!((per_day(23.46), per_day(7.0), per_day(0.04), per_day(140.6)), ("23.5".into(), "7.0".into(), "0.0".into(), "141".into()));
        assert_eq!((percent(3.79), percent(-0.124), percent(0.0)), ("379%".into(), "-12%".into(), "0%".into()));
        assert_eq!((margin_fill(1.25), margin_fill(3.79), margin_fill(-0.5)), (0.5, 1.0, 0.2));
    }

    #[test]
    fn unknown_skill_sorts_last_in_both_directions() {
        let (mut r, e) = fixture();
        r[1].skill = None;
        assert_eq!(visible(&r, &e, ALL, Sort { column: Column::Skill, descending: false }), vec![0, 3, 2, 1]);
        assert_eq!(visible(&r, &e, ALL, Sort { column: Column::Skill, descending: true }), vec![2, 0, 3, 1]);
    }

    #[test]
    fn skill_sorts_by_required_rank_then_name() {
        let (r, e) = fixture();
        assert_eq!(visible(&r, &e, ALL, Sort { column: Column::Skill, descending: false }), vec![1, 0, 3, 2]);
        assert_eq!(visible(&r, &e, ALL, Sort { column: Column::Skill, descending: true }), vec![2, 0, 3, 1]);
    }
}
