//! What a hover tooltip says: the item's name in its quality colour, then the few figures that
//! decide a craft or a flip. UI-free.

use crate::copper::Copper;
use crate::flips::Flip;
use crate::items::Item;
use crate::prices::PriceBook;
use crate::profit::Evaluation;
use crate::recipes::{Quality, Reagent, Recipe};

/// What the pointer rests on that has a tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tip {
    /// A recipe's row in the crafts table, by spell id.
    Recipe(u32),
    /// A reagent's line in the recipe detail, by item id.
    Reagent(u32),
    /// A flip's row, by its index in the flips list.
    Flip(usize),
}

impl Tip {
    /// The key of the element the tooltip opens beside.
    pub fn anchor(self) -> String {
        match self {
            Tip::Recipe(spell) => format!("tip-recipe-{spell}"),
            Tip::Reagent(item) => format!("tip-reagent-{item}"),
            Tip::Flip(index) => format!("tip-flip-{index}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub name: String,
    pub quality: Quality,
    /// A label on the left, its figure on the right.
    pub lines: Vec<(&'static str, Value)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Money(Copper),
    /// A profit, green when it gains and red when it loses.
    Signed(i64),
    Percent(f64),
}

/// The realm's cheapest listing and market value and the region's sale rate. Figures TSM lacks
/// are left out.
fn market(book: &PriceBook, item: u32) -> Vec<(&'static str, Value)> {
    let realm = book.realm_price(item);
    let mut lines: Vec<_> = realm.and_then(|p| p.min_buyout).map(|v| ("Min buyout", Value::Money(v))).into_iter().collect();
    lines.extend(realm.and_then(|p| p.market_value).map(|v| ("Market value", Value::Money(v))));
    lines.extend(book.region.get(&item).and_then(|r| r.sale_rate).map(|v| ("Region sale rate", Value::Percent(v))));
    lines
}

fn with_profit(mut lines: Vec<(&'static str, Value)>, profit: Option<i64>) -> Vec<(&'static str, Value)> {
    lines.extend(profit.map(|p| ("Profit", Value::Signed(p))));
    lines
}

pub fn recipe(recipe: &Recipe, e: &Evaluation, book: &PriceBook) -> Card {
    Card { name: recipe.item_name.clone(), quality: recipe.quality, lines: with_profit(market(book, recipe.item_id), e.profit) }
}

pub fn reagent(reagent: &Reagent, book: &PriceBook) -> Card {
    Card { name: reagent.item_name.clone(), quality: reagent.quality, lines: market(book, reagent.item_id) }
}

pub fn flip(name: String, flip: &Flip, item: Option<&Item>, book: &PriceBook) -> Card {
    Card { name, quality: item.map_or(Quality::Common, |i| i.quality), lines: with_profit(market(book, flip.item), flip.profit) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prices::{RealmPrice, RegionPrice};

    fn book() -> PriceBook {
        let mut book = PriceBook::default();
        book.realm_prices_mut().insert(9, RealmPrice { min_buyout: Some(Copper(12_00)), market_value: Some(Copper(14_50)), num_auctions: Some(3), ..Default::default() });
        book.region.insert(9, RegionPrice { market_value: Some(Copper(15_00)), sold_per_day: Some(2.5), sale_rate: Some(0.124), ..Default::default() });
        book
    }

    fn bracers() -> Recipe {
        Recipe { spell_id: 1, item_id: 9, item_name: "Barbaric Bracers".into(), quality: Quality::Uncommon, slot: None, icon: None, quantity: 1.0, skill: None, reagents: Vec::new() }
    }

    #[test]
    fn a_recipe_shows_min_buyout_market_value_sale_rate_and_profit() {
        let e = Evaluation { profit: Some(-5_00), ..Default::default() };
        let card = super::recipe(&bracers(), &e, &book());
        assert_eq!(card.name, "Barbaric Bracers");
        assert_eq!(
            card.lines,
            vec![("Min buyout", Value::Money(Copper(12_00))), ("Market value", Value::Money(Copper(14_50))), ("Region sale rate", Value::Percent(0.124)), ("Profit", Value::Signed(-5_00))]
        );
    }

    #[test]
    fn figures_tsm_lacks_are_left_out() {
        let recipe = Recipe { item_id: 8, ..bracers() };
        assert_eq!(super::recipe(&recipe, &Evaluation::default(), &book()).lines, vec![]);
    }
}
