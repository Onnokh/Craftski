//! What a hover tooltip says, as the game's tooltips say it: the item's name in its quality
//! colour, the lines the game would show, then the auction figures TSM keeps for it. UI-free.

use crate::breakdown::subtitle;
use crate::copper::Copper;
use crate::flips::Flip;
use crate::items::{Binding, Item};
use crate::merchants::Merchant;
use crate::prices::PriceBook;
use crate::profit::{Evaluation, PriceOrigin, ReagentPrice};
use crate::recipes::{Quality, Reagent, Recipe, Skill};
use crate::rows::per_day;

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
    pub lines: Vec<Line>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    /// A white line, wrapped to the tooltip's width.
    Text(String),
    /// A label on the left, its figure on the right.
    Pair(&'static str, Value),
    /// The ranks where a craft turns yellow, green and grey.
    Skill(Skill),
    Gap,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Money(Copper),
    /// A profit, green when it gains and red when it loses.
    Signed(i64),
    Count(String),
}

/// TSM's figures for `item`: the realm's listings, then the region's sales. Figures TSM lacks
/// are left out, as the game leaves out lines that do not apply.
pub fn market(book: &PriceBook, item: u32) -> Vec<Line> {
    let mut lines = Vec::new();
    if let Some(p) = book.realm_price(item) {
        lines.extend(p.min_buyout.map(|v| Line::Pair("Min buyout", Value::Money(v))));
        lines.extend(p.num_auctions.map(|n| Line::Pair("Listed here", Value::Count(n.to_string()))));
        lines.extend(p.market_value.map(|v| Line::Pair("Market value", Value::Money(v))));
    }
    if let Some(r) = book.region.get(&item) {
        lines.extend(r.market_value.map(|v| Line::Pair("Region market value", Value::Money(v))));
        lines.extend(r.sold_per_day.map(|v| Line::Pair("Region sold per day", Value::Count(per_day(v)))));
        lines.extend(r.sale_rate.map(|v| Line::Pair("Region sale rate", Value::Count(format!("{:.0}%", v * 100.0)))));
    }
    lines
}

/// A recipe's output with what it takes to make, its market and what a craft earns.
pub fn recipe(recipe: &Recipe, e: &Evaluation, book: &PriceBook) -> Card {
    let reagents: Vec<String> = recipe.reagents.iter().map(|r| format!("{} ({})", r.item_name, r.count)).collect();
    let mut lines = vec![Line::Text(subtitle(recipe)), Line::Text(format!("Reagents: {}", reagents.join(", ")))];
    lines.extend(recipe.skill.map(Line::Skill));
    section(&mut lines, market(book, recipe.item_id));
    let mut craft: Vec<Line> = e.cost.map(|c| Line::Pair("Crafting cost", Value::Money(c))).into_iter().collect();
    craft.extend(e.profit.map(|p| Line::Pair("Profit per craft", Value::Signed(p))));
    if e.cost.is_none() {
        let unpriced = recipe.reagents.iter().filter(|r| e.unpriced.contains(&r.item_id)).map(|r| r.item_name.as_str()).collect::<Vec<_>>();
        craft.push(Line::Text(format!("No price for {}.", unpriced.join(", "))));
    }
    section(&mut lines, craft);
    Card { name: recipe.item_name.clone(), quality: recipe.quality, lines }
}

/// A reagent with where it is cheapest to get, then its market.
pub fn reagent(reagent: &Reagent, price: Option<ReagentPrice>, book: &PriceBook) -> Card {
    let mut lines = Vec::new();
    if let Some(price) = price {
        lines.push(Line::Text(
            match price.origin {
                PriceOrigin::Vendor => "Cheapest from a merchant.",
                PriceOrigin::Auction => "Cheapest on the auction house.",
                PriceOrigin::Crafted { .. } => "Cheapest to craft yourself.",
            }
            .into(),
        ));
        lines.push(Line::Pair("Cost each", Value::Money(price.unit)));
    }
    lines.extend(book.vendor.get(reagent.item_id).map(|v| Line::Pair("Merchant price", Value::Money(v))));
    section(&mut lines, market(book, reagent.item_id));
    Card { name: reagent.item_name.clone(), quality: reagent.quality, lines }
}

/// A merchant's good with its binding, its market, what the merchant charges, what a merchant
/// pays for it and the flip's profit.
pub fn flip(name: String, flip: &Flip, merchant: &Merchant, item: Option<&Item>, book: &PriceBook) -> Card {
    let facts = item.and_then(|i| i.facts);
    let mut lines = vec![Line::Text(format!("Sold by {}, {}.", merchant.name, merchant.zone))];
    lines.extend(facts.and_then(|f| binding(f.binding)).map(|b| Line::Text(b.into())));
    section(&mut lines, market(book, flip.item));
    let mut sale: Vec<Line> = flip.paid.map(|p| Line::Pair("Merchant price", Value::Money(p.copper()))).into_iter().collect();
    sale.extend(facts.filter(|f| f.sell.0 > 0).map(|f| Line::Pair("Sell price", Value::Money(f.sell))));
    sale.extend(flip.profit.map(|p| Line::Pair("Profit per item", Value::Signed(p))));
    section(&mut lines, sale);
    Card { name, quality: item.map_or(Quality::Common, |i| i.quality), lines }
}

fn binding(b: Binding) -> Option<&'static str> {
    match b {
        Binding::None => None,
        Binding::OnPickup => Some("Binds when picked up"),
        Binding::OnEquip => Some("Binds when equipped"),
        Binding::OnUse => Some("Binds when used"),
        Binding::Quest => Some("Quest Item"),
    }
}

/// Adds `more` after a gap, when there is any.
fn section(lines: &mut Vec<Line>, more: Vec<Line>) {
    if !more.is_empty() {
        lines.push(Line::Gap);
        lines.extend(more);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prices::{RealmPrice, RegionPrice};

    fn book() -> PriceBook {
        let mut book = PriceBook::default();
        book.realm_prices_mut().insert(7, RealmPrice { min_buyout: Some(Copper(12_00)), num_auctions: Some(3), ..Default::default() });
        book.region.insert(7, RegionPrice { market_value: Some(Copper(15_00)), sold_per_day: Some(2.5), sale_rate: Some(0.124), ..Default::default() });
        book
    }

    #[test]
    fn market_lists_the_figures_tsm_has_and_skips_the_rest() {
        assert_eq!(
            market(&book(), 7),
            vec![
                Line::Pair("Min buyout", Value::Money(Copper(12_00))),
                Line::Pair("Listed here", Value::Count("3".into())),
                Line::Pair("Region market value", Value::Money(Copper(15_00))),
                Line::Pair("Region sold per day", Value::Count("2.5".into())),
                Line::Pair("Region sale rate", Value::Count("12%".into())),
            ]
        );
    }

    #[test]
    fn an_item_tsm_never_saw_has_no_market_lines() {
        assert_eq!(market(&book(), 8), vec![]);
    }

    #[test]
    fn a_recipe_names_its_reagents_as_the_game_does_and_what_a_craft_earns() {
        let reagent = |item_id, name: &str, count| Reagent { item_id, item_name: name.into(), quality: Quality::Common, icon: None, count };
        let recipe = Recipe {
            spell_id: 1,
            item_id: 9,
            item_name: "Barbaric Bracers".into(),
            quality: Quality::Uncommon,
            slot: None,
            icon: None,
            quantity: 1.0,
            skill: None,
            reagents: vec![reagent(2, "Heavy Leather", 8), reagent(3, "Large Fang", 4)],
        };
        let e = Evaluation { cost: Some(Copper(6_732)), profit: Some(-5_00), ..Default::default() };
        let card = super::recipe(&recipe, &e, &book());
        assert_eq!(card.name, "Barbaric Bracers");
        assert_eq!(
            card.lines,
            vec![
                Line::Text("Makes 1.".into()),
                Line::Text("Reagents: Heavy Leather (8), Large Fang (4)".into()),
                Line::Gap,
                Line::Pair("Crafting cost", Value::Money(Copper(6_732))),
                Line::Pair("Profit per craft", Value::Signed(-5_00)),
            ]
        );
    }

    #[test]
    fn a_recipe_with_an_unpriced_reagent_says_which() {
        let recipe = Recipe {
            spell_id: 1,
            item_id: 9,
            item_name: "Rugged Armor Kit".into(),
            quality: Quality::Common,
            slot: None,
            icon: None,
            quantity: 1.0,
            skill: None,
            reagents: vec![Reagent { item_id: 4, item_name: "Rugged Leather".into(), quality: Quality::Common, icon: None, count: 5 }],
        };
        let e = Evaluation { unpriced: vec![4], ..Default::default() };
        assert_eq!(super::recipe(&recipe, &e, &book()).lines.last(), Some(&Line::Text("No price for Rugged Leather.".into())));
    }
}
