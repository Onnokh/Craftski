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
            Column::Item => "Recipe",
            Column::Skill => "Skill",
            Column::Cost => "Materials",
            Column::Resale => "Net sale",
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
    /// The player's rank: recipes learned above it hide, recipes without a known rank stay.
    pub max_skill: Option<u16>,
    /// Matches the crafted item or any reagent by name, ignoring case. Blank matches all.
    pub query: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub column: Column,
    pub descending: bool,
}

impl Sort {
    /// The sort after a click on `column`'s header: the same column flips, a new one starts
    /// with names A to Z and numbers largest first.
    pub fn clicked(self, column: Column) -> Sort {
        if column == self.column {
            Sort { column, descending: !self.descending }
        } else {
            Sort { column, descending: column != Column::Item }
        }
    }
}

fn matches(recipe: &Recipe, needle: &str) -> bool {
    needle.is_empty() || std::iter::once(&recipe.item_name).chain(recipe.reagents.iter().map(|r| &r.item_name)).any(|name| name.to_lowercase().contains(needle))
}

/// Indices into `recipes` (and the parallel `evals`) to show, in order. Unknown values sort
/// last whichever way the column sorts.
pub fn visible(recipes: &[Recipe], evals: &[Evaluation], filter: Filter, sort: Sort) -> Vec<usize> {
    let needle = filter.query.trim().to_lowercase();
    let mut rows: Vec<usize> = (0..recipes.len())
        .filter(|&i| {
            let e = &evals[i];
            let priced = !filter.hide_unpriced || e.profit.is_some();
            let selling = filter.min_sold_per_day <= 0.0 || e.sold_per_day.is_some_and(|s| s >= filter.min_sold_per_day);
            let learnable = filter.max_skill.is_none_or(|max| recipes[i].skill.is_none_or(|s| s.learn <= max));
            priced && selling && learnable && matches(&recipes[i], &needle)
        })
        .collect();
    let directed = |o: Ordering| if sort.descending { o.reverse() } else { o };
    rows.sort_by(|&a, &b| {
        let by_name = || recipes[a].item_name.cmp(&recipes[b].item_name).then(recipes[a].spell_id.cmp(&recipes[b].spell_id));
        if sort.column == Column::Item {
            return directed(by_name());
        }
        match (sort.column.number(&recipes[a], &evals[a]), sort.column.number(&recipes[b], &evals[b])) {
            (Some(x), Some(y)) => directed(x.total_cmp(&y)).then_with(by_name),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => by_name(),
        }
    });
    rows
}

/// Sales a day with one decimal, or none once the count is large enough not to need it.
pub fn per_day(d: f64) -> String {
    if d >= 100.0 { format!("{d:.0}") } else { format!("{d:.1}") }
}

/// Margin as a whole percent with a true minus, as in `379%` or `−12%`.
pub fn percent(margin: f32) -> String {
    format!("{:.0}%", margin * 100.0).replacen('-', "\u{2212}", 1)
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

    const ALL: Filter = Filter { hide_unpriced: false, min_sold_per_day: 0.0, max_skill: None, query: "" };

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
        assert_eq!(visible(&r, &e, Filter { hide_unpriced: true, ..ALL }, by_name), vec![0, 2, 3]);
        assert_eq!(visible(&r, &e, Filter { min_sold_per_day: 1.0, ..ALL }, by_name), vec![0, 1]);
        assert_eq!(visible(&r, &e, Filter { hide_unpriced: true, min_sold_per_day: 1.0, ..ALL }, by_name), vec![0]);
    }

    #[test]
    fn a_skill_hides_recipes_learned_above_it_and_keeps_those_without_a_rank() {
        let (mut r, e) = fixture();
        r[1].skill = None;
        let by_name = Sort { column: Column::Item, descending: false };
        assert_eq!(visible(&r, &e, Filter { max_skill: Some(225), ..ALL }, by_name), vec![0, 1, 2, 3]);
        assert_eq!(visible(&r, &e, Filter { max_skill: Some(125), ..ALL }, by_name), vec![0, 1, 3]);
        assert_eq!(visible(&r, &e, Filter { max_skill: Some(124), ..ALL }, by_name), vec![1]);
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
        assert_eq!((percent(3.79), percent(-0.124), percent(0.0)), ("379%".into(), "\u{2212}12%".into(), "0%".into()));
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
