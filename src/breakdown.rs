//! What the detail panel says about one recipe: where the sale price goes, what each reagent
//! costs, and when to be careful. UI-free.

use crate::copper::Copper;
use crate::prices::PriceBook;
use crate::profit::{self, AuctionBasis, CostBook, Evaluation, PriceOrigin, ReagentPrice};
use crate::recipes::{Reagent, Recipe};

/// Realm listings at or below this count make a price easy to move.
const FEW_LISTED: u32 = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct Breakdown<'a> {
    pub sale: Option<Sale>,
    pub reagents: Vec<ReagentLine<'a>>,
}

/// The crafted output on the auction house.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sale {
    /// Before the auction house cut.
    pub gross: Copper,
    pub cut: Copper,
    pub basis: AuctionBasis,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReagentLine<'a> {
    pub reagent: &'a Reagent,
    pub price: Option<ReagentPrice>,
    pub total: Option<Copper>,
}

pub fn breakdown<'a>(recipe: &'a Recipe, eval: &Evaluation, book: &PriceBook, costs: &CostBook) -> Breakdown<'a> {
    let sale = profit::auction_price(book, recipe.item_id).zip(eval.resale).map(|(price, resale)| {
        let gross = Copper((price.unit.0 as f64 * f64::from(recipe.quantity)).round() as u64);
        Sale { gross, cut: Copper(gross.0.saturating_sub(resale.0)), basis: price.basis }
    });
    let reagents = recipe
        .reagents
        .iter()
        .map(|reagent| {
            let price = costs.get(reagent.item_id);
            ReagentLine { reagent, price, total: price.map(|p| Copper(p.unit.0 * u64::from(reagent.count))) }
        })
        .collect();
    Breakdown { sale, reagents }
}

/// Why a profit rests on thin evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caution {
    /// No region sales recorded.
    Untested,
    /// Under one sale a day across the region.
    SlowSeller,
    /// So few listed on the realm that one seller can move the price.
    FewListed,
}

impl Caution {
    pub fn note(self) -> &'static str {
        match self {
            Caution::Untested => "Margin untested. Post one before crafting a stack.",
            Caution::SlowSeller => "Sells under once a day. Expect to wait for a buyer.",
            Caution::FewListed => "Few listed, so one seller can move the price.",
        }
    }

    /// Whether the caution is about realm listings rather than region sales.
    pub fn on_listings(self) -> bool {
        self == Caution::FewListed
    }
}

/// The caution that applies, or None when the market looks sound.
pub fn caution(sold_per_day: Option<f64>, listed: Option<u32>) -> Option<Caution> {
    match (sold_per_day, listed.filter(|&n| n <= FEW_LISTED)) {
        (None, _) => Some(Caution::Untested),
        (Some(s), _) if s < 1.0 => Some(Caution::SlowSeller),
        (Some(_), Some(_)) => Some(Caution::FewListed),
        (Some(_), None) => None,
    }
}

/// What to buy for one craft as plain text for the clipboard. A reagent cheapest to craft
/// expands into its recipe's mats, rounded up to whole crafts, and a short first line names
/// those crafts in the order to make them. Then one `12x Thick Leather` line per mat to buy,
/// merged across the recipes that need it, with `(vendor)` after those cheapest at a merchant,
/// then what the mats cost.
pub fn shopping_list(recipe: &Recipe, recipes: &[Recipe], costs: &CostBook) -> String {
    let crafted = |item: u32| match costs.get(item)?.origin {
        PriceOrigin::Crafted { spell_id } => recipes.iter().find(|r| r.spell_id == spell_id),
        _ => None,
    };
    let mut order = Vec::new();
    craft_order(recipe, &crafted, &mut order);

    let mut need = Vec::new();
    add_mats(&mut need, recipe, 1);
    let mut crafts = Vec::new();
    for sub in order.iter().rev() {
        let Some(at) = need.iter().position(|(n, _)| n.item_id == sub.item_id) else { continue };
        let (reagent, wanted) = need.remove(at);
        let times = (wanted as f64 / f64::from(sub.quantity)).ceil() as u64;
        add_mats(&mut need, sub, times);
        crafts.push(match sub.quantity {
            1.0 => format!("{times}x {}", reagent.item_name),
            each => format!("{times}x {} (makes {})", reagent.item_name, times as f32 * each),
        });
    }
    crafts.reverse();

    let mut out = String::new();
    if !crafts.is_empty() {
        out.push_str(&format!("Craft first: {}\n", crafts.join(", ")));
    }
    let mut total = 0u64;
    let mut unpriced = Vec::new();
    for (reagent, count) in &need {
        let price = costs.get(reagent.item_id);
        let vendor = if price.is_some_and(|p| p.origin == PriceOrigin::Vendor) { " (vendor)" } else { "" };
        out.push_str(&format!("{count}x {}{vendor}\n", reagent.item_name));
        match price {
            Some(p) => total += p.unit.0 * count,
            None => unpriced.push(reagent.item_name.as_str()),
        }
    }
    if unpriced.is_empty() {
        out.push_str(&format!("Total: {}", Copper(total)));
    } else {
        out.push_str(&format!("Total: unknown, no price for {}", unpriced.join(", ")));
    }
    out
}

/// Appends the crafted sub-recipes `recipe` uses, each after the sub-recipes it uses in turn.
/// CostBook keeps crafted origins acyclic, so this ends.
fn craft_order<'a>(recipe: &'a Recipe, crafted: &impl Fn(u32) -> Option<&'a Recipe>, order: &mut Vec<&'a Recipe>) {
    for sub in recipe.reagents.iter().filter_map(|g| crafted(g.item_id)) {
        if !order.iter().any(|o| o.spell_id == sub.spell_id) {
            craft_order(sub, crafted, order);
            order.push(sub);
        }
    }
}

/// Adds `times` crafts' worth of `recipe`'s reagents to `need`, merging repeats.
fn add_mats<'a>(need: &mut Vec<(&'a Reagent, u64)>, recipe: &'a Recipe, times: u64) {
    for g in &recipe.reagents {
        match need.iter_mut().find(|(n, _)| n.item_id == g.item_id) {
            Some((_, count)) => *count += u64::from(g.count) * times,
            None => need.push((g, u64::from(g.count) * times)),
        }
    }
}

/// "Cloak, makes 1. Learned at 225.", leaving out what the data does not know.
pub fn subtitle(recipe: &Recipe) -> String {
    let makes = match recipe.slot {
        Some(slot) => format!("{}, makes {}.", slot.name(), recipe.quantity),
        None => format!("Makes {}.", recipe.quantity),
    };
    match recipe.skill {
        Some(skill) => format!("{makes} Learned at {}.", skill.learn),
        None => makes,
    }
}

pub fn wowhead_url(item_id: u32) -> String {
    format!("https://www.wowhead.com/forever/item={item_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prices::RealmPrice;
    use crate::recipes::{Quality, Skill, Slot};
    use crate::vendor::VendorBook;
    use std::collections::HashMap;

    fn priced<'a>(recipe: &'a Recipe, book: &PriceBook) -> (Evaluation, Breakdown<'a>) {
        let costs = CostBook::new(std::slice::from_ref(recipe), book);
        let eval = profit::evaluate(recipe, book, &costs);
        let b = breakdown(recipe, &eval, book, &costs);
        (eval, b)
    }

    fn list(recipe: &Recipe, book: &PriceBook) -> String {
        let recipes = std::slice::from_ref(recipe);
        shopping_list(recipe, recipes, &CostBook::new(recipes, book))
    }

    fn nightscape_cloak() -> Recipe {
        let reagent = |item_id, name: &str, count| Reagent { item_id, item_name: name.into(), quality: Quality::Common, icon: None, count };
        Recipe {
            spell_id: 10550,
            item_id: 8195,
            item_name: "Nightscape Cloak".into(),
            quality: Quality::Uncommon,
            slot: Some(Slot::Cloak),
            icon: None,
            quantity: 1.0,
            skill: Some(Skill { learn: 225, grey: 245 }),
            reagents: vec![reagent(4304, "Thick Leather", 12), reagent(8343, "Heavy Silken Thread", 4)],
        }
    }

    fn book() -> PriceBook {
        let mut b = PriceBook::default();
        let realm = |mv, n| RealmPrice { market_value: Some(Copper(mv)), num_auctions: n, ..Default::default() };
        b.realm_prices_mut().insert(8195, realm(50_788, Some(3)));
        b.realm_prices_mut().insert(4304, realm(647, Some(437)));
        b.realm_prices_mut().insert(8343, realm(578, None));
        b
    }

    #[test]
    fn breaks_a_craft_into_sale_cut_and_reagent_lines() {
        let (recipe, book) = (nightscape_cloak(), book());
        let (eval, b) = priced(&recipe, &book);
        assert_eq!((b.sale, eval.resale, eval.cost), (Some(Sale { gross: Copper(50_788), cut: Copper(2_540), basis: AuctionBasis::MarketValue }), Some(Copper(48_248)), Some(Copper(10_076))));
        let lines: Vec<_> = b.reagents.iter().map(|l| (l.reagent.item_id, l.price.map(|p| p.unit), l.total)).collect();
        assert_eq!(lines, vec![(4304, Some(Copper(647)), Some(Copper(7_764))), (8343, Some(Copper(578)), Some(Copper(2_312)))]);
    }

    #[test]
    fn a_craft_sells_at_its_current_listing_over_the_two_week_average() {
        let mut recipe = nightscape_cloak();
        recipe.item_id = 4252;
        let mut book = book();
        book.realm_prices_mut().insert(4252, RealmPrice { min_buyout: Some(Copper(2_900)), market_value: Some(Copper(6_586)), num_auctions: Some(57), ..Default::default() });
        let (eval, b) = priced(&recipe, &book);
        assert_eq!((b.sale, eval.resale), (Some(Sale { gross: Copper(2_900), cut: Copper(145), basis: AuctionBasis::MinBuyout }), Some(Copper(2_755))));
    }

    #[test]
    fn a_reagent_without_a_price_leaves_its_line_and_the_total_unknown() {
        let (recipe, mut book) = (nightscape_cloak(), book());
        book.realm_prices_mut().remove(&8343);
        let (eval, b) = priced(&recipe, &book);
        assert_eq!((eval.cost, eval.unpriced), (None, vec![8343]));
        assert_eq!(b.reagents.iter().map(|l| l.total).collect::<Vec<_>>(), vec![Some(Copper(7_764)), None]);
        assert_eq!(list(&recipe, &book), "12x Thick Leather\n4x Heavy Silken Thread\nTotal: unknown, no price for Heavy Silken Thread");
    }

    #[test]
    fn cautions_when_sales_are_missing_slow_or_listings_thin() {
        assert_eq!(caution(None, Some(3)), Some(Caution::Untested));
        assert_eq!(caution(None, None), Some(Caution::Untested));
        assert_eq!(caution(Some(0.42), Some(2)), Some(Caution::SlowSeller));
        assert_eq!(caution(Some(23.5), Some(2)), Some(Caution::FewListed));
        assert_eq!(caution(Some(23.5), Some(6)), None);
        assert_eq!(caution(Some(1.0), None), None);
        assert_eq!(Caution::Untested.note(), "Margin untested. Post one before crafting a stack.");
    }

    #[test]
    fn the_shopping_list_is_one_line_per_reagent_and_marks_vendor_items() {
        let (recipe, mut book) = (nightscape_cloak(), book());
        book.vendor = VendorBook::new(HashMap::new(), HashMap::from([(4304, Copper(5_000)), (8343, Copper(500))]));
        let (eval, b) = priced(&recipe, &book);
        let prices: Vec<_> = b.reagents.iter().map(|l| (l.reagent.item_id, l.price, l.total)).collect();
        assert_eq!(prices, vec![
            (4304, Some(ReagentPrice { unit: Copper(647), origin: PriceOrigin::Auction }), Some(Copper(7_764))),
            (8343, Some(ReagentPrice { unit: Copper(500), origin: PriceOrigin::Vendor }), Some(Copper(2_000))),
        ]);
        assert_eq!((eval.cost, list(&recipe, &book).as_str()), (Some(Copper(9_764)), "12x Thick Leather\n4x Heavy Silken Thread (vendor)\nTotal: 97s 64c"));
    }

    #[test]
    fn the_subtitle_names_slot_yield_and_skill_when_known() {
        let mut recipe = nightscape_cloak();
        assert_eq!(subtitle(&recipe), "Cloak, makes 1. Learned at 225.");
        recipe.slot = None;
        recipe.skill = None;
        recipe.quantity = 2.0;
        assert_eq!(subtitle(&recipe), "Makes 2.");
        assert_eq!(wowhead_url(8195), "https://www.wowhead.com/forever/item=8195");
    }

    #[test]
    fn the_shopping_list_crafts_intermediates_first_and_buys_their_merged_mats() {
        let reagent = |item_id, name: &str, count| Reagent { item_id, item_name: name.into(), quality: Quality::Common, icon: None, count };
        let recipe = |spell_id, item_id, name: &str, quantity, reagents| Recipe { spell_id, item_id, item_name: name.into(), quality: Quality::Common, slot: None, icon: None, quantity, skill: None, reagents };
        let recipes = [
            recipe(19047, 15407, "Cured Rugged Hide", 2.0, vec![reagent(8171, "Rugged Hide", 1), reagent(15409, "Refined Deeprock Salt", 1)]),
            recipe(2881, 2318, "Light Leather", 1.0, vec![reagent(2934, "Ruined Leather Scraps", 3)]),
            recipe(20648, 2319, "Medium Leather", 1.0, vec![reagent(2318, "Light Leather", 4)]),
            recipe(19097, 15062, "Leggings", 1.0, vec![reagent(15407, "Cured Rugged Hide", 3), reagent(8171, "Rugged Hide", 2), reagent(2319, "Medium Leather", 1)]),
        ];
        let mut book = PriceBook::default();
        for (item, unit) in [(8171, 250), (2934, 10), (2318, 50), (2319, 200)] {
            book.realm_prices_mut().insert(item, RealmPrice { min_buyout: Some(Copper(unit)), ..Default::default() });
        }
        book.vendor = VendorBook::new(HashMap::new(), HashMap::from([(15409, Copper(200))]));
        let costs = CostBook::new(&recipes, &book);
        assert_eq!(
            shopping_list(&recipes[3], &recipes, &costs),
            "Craft first: 2x Cured Rugged Hide (makes 4), 4x Light Leather, 1x Medium Leather\n4x Rugged Hide\n12x Ruined Leather Scraps\n2x Refined Deeprock Salt (vendor)\nTotal: 15s 20c"
        );
        assert_eq!(profit::evaluate(&recipes[3], &book, &costs).cost, Some(Copper(1_295)));
    }
}
