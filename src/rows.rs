//! Which recipes the table shows, and in what order. UI-free.

use std::cmp::Ordering;

use crate::profit::Evaluation;
use crate::recipes::Recipe;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Item,
    Skill,
    Qty,
    Cost,
    Resale,
    Profit,
    Margin,
    SoldPerDay,
    Auctions,
}

impl Column {
    pub const ALL: [Column; 9] = [Column::Item, Column::Skill, Column::Qty, Column::Cost, Column::Resale, Column::Profit, Column::Margin, Column::SoldPerDay, Column::Auctions];

    pub fn id(self) -> &'static str {
        match self {
            Column::Item => "item",
            Column::Skill => "skill",
            Column::Qty => "qty",
            Column::Cost => "cost",
            Column::Resale => "resale",
            Column::Profit => "profit",
            Column::Margin => "margin",
            Column::SoldPerDay => "sold",
            Column::Auctions => "auctions",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Column::Item => "Item",
            Column::Skill => "Skill",
            Column::Qty => "Qty",
            Column::Cost => "Cost",
            Column::Resale => "Resale",
            Column::Profit => "Profit",
            Column::Margin => "Margin %",
            Column::SoldPerDay => "Sold/day",
            Column::Auctions => "Auctions",
        }
    }

    pub fn from_id(id: &str) -> Option<Column> {
        Column::ALL.into_iter().find(|c| c.id() == id)
    }

    fn number(self, r: &Recipe, e: &Evaluation) -> Option<f64> {
        match self {
            Column::Item => None,
            Column::Skill => r.skill.map(|s| f64::from(s.learn)),
            Column::Qty => Some(f64::from(r.quantity)),
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
pub struct Filter {
    pub hide_unpriced: bool,
    pub min_sold_per_day: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub column: Column,
    pub descending: bool,
}

/// Indices into `recipes` (and the parallel `evals`) to show, in order. Unknown values sort
/// last whichever way the column sorts.
pub fn visible(recipes: &[Recipe], evals: &[Evaluation], filter: Filter, sort: Sort) -> Vec<usize> {
    let mut rows: Vec<usize> = (0..recipes.len())
        .filter(|&i| {
            let e = &evals[i];
            let priced = !filter.hide_unpriced || e.profit.is_some();
            let selling = filter.min_sold_per_day <= 0.0 || e.sold_per_day.is_some_and(|s| s >= filter.min_sold_per_day);
            priced && selling
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::copper::Copper;
    use crate::recipes::{Quality, Skill};

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

    const ALL: Filter = Filter { hide_unpriced: false, min_sold_per_day: 0.0 };

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
        assert_eq!(visible(&r, &e, Filter { hide_unpriced: true, min_sold_per_day: 0.0 }, by_name), vec![0, 2, 3]);
        assert_eq!(visible(&r, &e, Filter { hide_unpriced: false, min_sold_per_day: 1.0 }, by_name), vec![0, 1]);
        assert_eq!(visible(&r, &e, Filter { hide_unpriced: true, min_sold_per_day: 1.0 }, by_name), vec![0]);
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
