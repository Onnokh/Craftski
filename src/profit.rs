use std::collections::HashMap;

use crate::copper::Copper;
use crate::prices::PriceBook;
use crate::recipes::Recipe;

/// The auction house keeps 5% of every sale. Kept as a percent so whole results divide exactly.
pub const SELLER_PERCENT: f64 = 95.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation {
    /// None when any reagent is unpriced; `unpriced` names them.
    pub cost: Option<Copper>,
    pub unpriced: Vec<u32>,
    /// Crafted output after the auction house cut.
    pub resale: Option<Copper>,
    pub profit: Option<i64>,
    /// Profit as a fraction of cost.
    pub margin: Option<f32>,
    pub sold_per_day: Option<f64>,
    pub auctions: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceOrigin {
    Vendor,
    Auction,
    /// Made by this recipe of the loaded profession.
    Crafted { spell_id: u32 },
}

/// What one unit of a reagent costs, and where to buy it at that price.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReagentPrice {
    pub unit: Copper,
    pub origin: PriceOrigin,
}

/// Which auction figure priced an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuctionBasis {
    /// The cheapest listing on the realm in the latest scan.
    MinBuyout,
    /// TSM's realm market value, a roughly 14-day average, when nothing is listed now.
    MarketValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuctionPrice {
    pub unit: Copper,
    pub basis: AuctionBasis,
}

/// The auction house price, the only one a crafted item sells at.
pub fn auction_price(book: &PriceBook, item: u32) -> Option<AuctionPrice> {
    let at = |unit: Option<Copper>, basis| unit.map(|unit| AuctionPrice { unit, basis });
    book.realm_price(item).and_then(|p| at(p.min_buyout, AuctionBasis::MinBuyout).or(at(p.market_value, AuctionBasis::MarketValue)))
}

/// The cheaper of the vendor and auction prices, like TSM's default material price
/// `min(dbmarket, vendorbuy, ...)`. A tie goes to the vendor, which never runs out.
pub fn bought_price(book: &PriceBook, item: u32) -> Option<ReagentPrice> {
    let vendor = book.vendor.get(item).map(|unit| ReagentPrice { unit, origin: PriceOrigin::Vendor });
    let auction = auction_price(book, item).map(|p| ReagentPrice { unit: p.unit, origin: PriceOrigin::Auction });
    match (vendor, auction) {
        (Some(v), Some(a)) => Some(if a.unit < v.unit { a } else { v }),
        (v, a) => v.or(a),
    }
}

/// What one unit of each item the loaded recipes make or use costs on the chosen realm: the
/// cheapest of a vendor, the auction house, and crafting it from reagents priced the same way.
#[derive(Clone, Debug, PartialEq)]
pub struct CostBook {
    unit: HashMap<u32, ReagentPrice>,
}

impl CostBook {
    /// Starts from bought prices and lowers an item to a recipe's cost per unit made, rounded up
    /// to whole copper, whenever that is strictly cheaper, until a pass lowers nothing. Costs only
    /// fall and stay whole, so the passes end. A recipe whose output feeds itself through crafted
    /// reagents is skipped, so a cycle prices nothing and crafted origins never loop.
    pub fn new(recipes: &[Recipe], book: &PriceBook) -> CostBook {
        let items = recipes.iter().flat_map(|r| std::iter::once(r.item_id).chain(r.reagents.iter().map(|g| g.item_id)));
        let mut unit: HashMap<u32, ReagentPrice> = items.filter_map(|item| Some((item, bought_price(book, item)?))).collect();
        let by_spell: HashMap<u32, &Recipe> = recipes.iter().map(|r| (r.spell_id, r)).collect();
        loop {
            let mut lowered = false;
            for r in recipes {
                let Some(total) = r.reagents.iter().map(|g| unit.get(&g.item_id).map(|p| p.unit.0 * u64::from(g.count))).sum::<Option<u64>>() else { continue };
                let made = Copper((total as f64 / f64::from(r.quantity)).ceil() as u64);
                if unit.get(&r.item_id).is_some_and(|p| p.unit <= made) || feeds(r.item_id, r, &unit, &by_spell) {
                    continue;
                }
                unit.insert(r.item_id, ReagentPrice { unit: made, origin: PriceOrigin::Crafted { spell_id: r.spell_id } });
                lowered = true;
            }
            if !lowered {
                return CostBook { unit };
            }
        }
    }

    pub fn get(&self, item: u32) -> Option<ReagentPrice> {
        self.unit.get(&item).copied()
    }
}

/// Whether `item` goes into `recipe`, directly or through the recipes its crafted reagents come from.
fn feeds(item: u32, recipe: &Recipe, unit: &HashMap<u32, ReagentPrice>, by_spell: &HashMap<u32, &Recipe>) -> bool {
    recipe.reagents.iter().any(|g| {
        g.item_id == item
            || matches!(unit.get(&g.item_id), Some(ReagentPrice { origin: PriceOrigin::Crafted { spell_id }, .. }) if feeds(item, by_spell[spell_id], unit, by_spell))
    })
}

pub fn evaluate(recipe: &Recipe, book: &PriceBook, costs: &CostBook) -> Evaluation {
    let mut total = 0u64;
    let mut unpriced = Vec::new();
    for r in &recipe.reagents {
        match costs.get(r.item_id) {
            Some(p) => total += p.unit.0 * u64::from(r.count),
            None => unpriced.push(r.item_id),
        }
    }
    let cost = unpriced.is_empty().then_some(Copper(total));
    let resale = auction_price(book, recipe.item_id).map(|AuctionPrice { unit: Copper(c), .. }| Copper((c as f64 * f64::from(recipe.quantity) * SELLER_PERCENT / 100.0).floor() as u64));
    let profit = cost.zip(resale).map(|(c, r)| r.0 as i64 - c.0 as i64);
    let margin = profit.zip(cost).filter(|(_, c)| c.0 > 0).map(|(p, c)| p as f32 / c.0 as f32);
    Evaluation {
        cost,
        unpriced,
        resale,
        profit,
        margin,
        sold_per_day: book.region.get(&recipe.item_id).and_then(|p| p.sold_per_day),
        auctions: book.realm_price(recipe.item_id).and_then(|p| p.num_auctions),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prices::{RealmPrice, RegionPrice};
    use crate::recipes::{Quality, Reagent};
    use crate::vendor::VendorBook;
    use std::collections::HashMap;

    fn book() -> PriceBook {
        let mut b = PriceBook::default();
        let realm = |mv: Option<u64>, mb: Option<u64>, n| RealmPrice { market_value: mv.map(Copper), min_buyout: mb.map(Copper), num_auctions: n, ..Default::default() };
        b.realm_prices_mut().insert(15407, realm(Some(1000), None, Some(12)));
        b.realm_prices_mut().insert(8171, realm(Some(300), Some(250), None));
        b.realm_prices_mut().insert(15409, realm(None, Some(200), None));
        b.region.insert(15407, RegionPrice { sold_per_day: Some(3.5), ..Default::default() });
        b
    }

    fn evaluated(recipe: &Recipe, book: &PriceBook) -> Evaluation {
        evaluate(recipe, book, &CostBook::new(std::slice::from_ref(recipe), book))
    }

    fn cured_rugged_hide() -> Recipe {
        let reagent = |item_id, count| Reagent { item_id, item_name: String::new(), quality: Quality::Common, icon: None, count };
        Recipe { spell_id: 19047, item_id: 15407, item_name: "Cured Rugged Hide".into(), quality: Quality::Common, slot: None, icon: None, quantity: 2.0, skill: None, reagents: vec![reagent(8171, 2), reagent(15409, 1)] }
    }

    #[test]
    fn realm_prices_take_the_current_min_buyout_first_and_the_cut_on_every_crafted_unit() {
        assert_eq!(evaluated(&cured_rugged_hide(), &book()), Evaluation {
            cost: Some(Copper(700)),
            unpriced: vec![],
            resale: Some(Copper(1900)),
            profit: Some(1200),
            margin: Some(1.7142857),
            sold_per_day: Some(3.5),
            auctions: Some(12),
        });
        let b = book();
        assert_eq!(auction_price(&b, 8171), Some(AuctionPrice { unit: Copper(250), basis: AuctionBasis::MinBuyout }));
        assert_eq!(auction_price(&b, 15407), Some(AuctionPrice { unit: Copper(1000), basis: AuctionBasis::MarketValue }));
        assert_eq!(auction_price(&b, 2321), None);
    }

    #[test]
    fn an_unpriced_reagent_leaves_cost_and_profit_unknown_and_is_named() {
        let mut b = book();
        b.realm_prices_mut().remove(&15409);
        assert_eq!(evaluated(&cured_rugged_hide(), &b), Evaluation {
            cost: None,
            unpriced: vec![15409],
            resale: Some(Copper(1900)),
            profit: None,
            margin: None,
            sold_per_day: Some(3.5),
            auctions: Some(12),
        });
    }

    #[test]
    fn reagents_cost_the_cheaper_of_vendor_and_auction_but_the_craft_sells_at_auction() {
        let mut b = book();
        b.vendor = VendorBook::new(HashMap::from([(8171, Copper(350)), (15409, Copper(150)), (15407, Copper(10))]), HashMap::new());
        let realm = evaluated(&cured_rugged_hide(), &b);
        assert_eq!((realm.cost, realm.resale, realm.profit), (Some(Copper(650)), Some(Copper(1900)), Some(1250)));
        assert_eq!(bought_price(&b, 8171), Some(ReagentPrice { unit: Copper(250), origin: PriceOrigin::Auction }));
        assert_eq!(bought_price(&b, 15409), Some(ReagentPrice { unit: Copper(150), origin: PriceOrigin::Vendor }));
        assert_eq!(bought_price(&b, 2321), None);
    }

    #[test]
    fn a_tie_goes_to_the_vendor() {
        let mut b = book();
        b.vendor = VendorBook::new(HashMap::new(), HashMap::from([(8171, Copper(250))]));
        assert_eq!(bought_price(&b, 8171), Some(ReagentPrice { unit: Copper(250), origin: PriceOrigin::Vendor }));
    }

    #[test]
    fn a_loss_is_negative_profit() {
        let mut recipe = cured_rugged_hide();
        recipe.quantity = 1.0;
        recipe.reagents[0].count = 4;
        let e = evaluated(&recipe, &book());
        assert_eq!((e.cost, e.resale, e.profit), (Some(Copper(1200)), Some(Copper(950)), Some(-250)));
    }

    fn recipe(spell_id: u32, item_id: u32, quantity: f32, reagents: &[(u32, u32)]) -> Recipe {
        let reagents = reagents.iter().map(|&(item_id, count)| Reagent { item_id, item_name: String::new(), quality: Quality::Common, icon: None, count }).collect();
        Recipe { spell_id, item_id, item_name: String::new(), quality: Quality::Common, slot: None, icon: None, quantity, skill: None, reagents }
    }

    /// Rugged Hide and Refined Deeprock Salt listed on the realm, Cured Rugged Hide not at all.
    fn new_server() -> PriceBook {
        let mut b = PriceBook::default();
        b.realm_prices_mut().insert(8171, RealmPrice { min_buyout: Some(Copper(250)), ..Default::default() });
        b.realm_prices_mut().insert(15409, RealmPrice { min_buyout: Some(Copper(200)), ..Default::default() });
        b.realm_prices_mut().insert(15062, RealmPrice { min_buyout: Some(Copper(5000)), ..Default::default() });
        b
    }

    /// Cured Rugged Hide, and Devilsaur Leggings-like pants made from three of them and two Rugged Hide.
    fn hide_and_pants() -> Vec<Recipe> {
        vec![recipe(19047, 15407, 2.0, &[(8171, 1), (15409, 1)]), recipe(19097, 15062, 1.0, &[(15407, 3), (8171, 2)])]
    }

    #[test]
    fn an_intermediate_costs_its_recipe_per_unit_made_and_flows_into_its_parent() {
        let (recipes, book) = (hide_and_pants(), new_server());
        let costs = CostBook::new(&recipes, &book);
        assert_eq!(costs.get(15407), Some(ReagentPrice { unit: Copper(225), origin: PriceOrigin::Crafted { spell_id: 19047 } }));
        let pants = evaluate(&recipes[1], &book, &costs);
        assert_eq!((pants.cost, pants.unpriced, pants.resale, pants.profit), (Some(Copper(1175)), vec![], Some(Copper(4750)), Some(3575)));
    }

    #[test]
    fn an_intermediate_with_an_unpriced_reagent_leaves_its_parent_unpriced() {
        let (recipes, mut book) = (hide_and_pants(), new_server());
        book.realm_prices_mut().remove(&15409);
        let costs = CostBook::new(&recipes, &book);
        assert_eq!(costs.get(15407), None);
        let pants = evaluate(&recipes[1], &book, &costs);
        assert_eq!((pants.cost, pants.unpriced, pants.profit), (None, vec![15407], None));
    }

    #[test]
    fn crafting_wins_only_when_strictly_cheaper_than_buying() {
        let recipes = hide_and_pants();
        let listed_at = |unit| {
            let mut b = new_server();
            b.realm_prices_mut().insert(15407, RealmPrice { min_buyout: Some(Copper(unit)), ..Default::default() });
            CostBook::new(&recipes, &b).get(15407)
        };
        assert_eq!(listed_at(300), Some(ReagentPrice { unit: Copper(225), origin: PriceOrigin::Crafted { spell_id: 19047 } }));
        assert_eq!(listed_at(225), Some(ReagentPrice { unit: Copper(225), origin: PriceOrigin::Auction }));
        assert_eq!(listed_at(200), Some(ReagentPrice { unit: Copper(200), origin: PriceOrigin::Auction }));
    }

    #[test]
    fn a_recipe_making_two_halves_the_unit_cost_rounding_up() {
        let mut book = new_server();
        book.realm_prices_mut().insert(15409, RealmPrice { min_buyout: Some(Copper(201)), ..Default::default() });
        let costs = CostBook::new(&hide_and_pants(), &book);
        assert_eq!(costs.get(15407), Some(ReagentPrice { unit: Copper(226), origin: PriceOrigin::Crafted { spell_id: 19047 } }));
    }

    #[test]
    fn a_two_recipe_cycle_prices_nothing_and_ends() {
        let up_and_down = [recipe(1, 101, 1.0, &[(102, 1)]), recipe(2, 102, 1.0, &[(101, 1)])];
        let costs = CostBook::new(&up_and_down, &PriceBook::default());
        assert_eq!((costs.get(101), costs.get(102)), (None, None));
    }

    #[test]
    fn a_cycle_that_would_make_an_item_cheaper_than_itself_is_refused() {
        let split_and_join = [recipe(1, 101, 2.0, &[(102, 1)]), recipe(2, 102, 1.0, &[(101, 1)])];
        let mut book = PriceBook::default();
        book.realm_prices_mut().insert(102, RealmPrice { min_buyout: Some(Copper(100)), ..Default::default() });
        let costs = CostBook::new(&split_and_join, &book);
        assert_eq!(costs.get(101), Some(ReagentPrice { unit: Copper(50), origin: PriceOrigin::Crafted { spell_id: 1 } }));
        assert_eq!(costs.get(102), Some(ReagentPrice { unit: Copper(100), origin: PriceOrigin::Auction }));
    }
}
