use crate::copper::Copper;
use crate::prices::PriceBook;
use crate::recipes::Recipe;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceSource {
    Realm,
    Region,
}

/// The auction house keeps 5% of every sale. Kept as a percent so whole results divide exactly.
const SELLER_PERCENT: f64 = 95.0;

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
    /// The region's market value.
    Region,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuctionPrice {
    pub unit: Copper,
    pub basis: AuctionBasis,
}

/// The auction house price, the only one a crafted item sells at.
pub fn auction_price(book: &PriceBook, item: u32, source: PriceSource) -> Option<AuctionPrice> {
    let at = |unit: Option<Copper>, basis| unit.map(|unit| AuctionPrice { unit, basis });
    match source {
        PriceSource::Realm => book.realm.get(&item).and_then(|p| at(p.min_buyout, AuctionBasis::MinBuyout).or(at(p.market_value, AuctionBasis::MarketValue))),
        PriceSource::Region => book.region.get(&item).and_then(|p| at(p.market_value, AuctionBasis::Region)),
    }
}

/// The cheaper of the vendor and auction prices, like TSM's default material price
/// `min(dbmarket, vendorbuy, ...)`. A tie goes to the vendor, which never runs out.
pub fn reagent_price(book: &PriceBook, item: u32, source: PriceSource) -> Option<ReagentPrice> {
    let vendor = book.vendor.get(item).map(|unit| ReagentPrice { unit, origin: PriceOrigin::Vendor });
    let auction = auction_price(book, item, source).map(|p| ReagentPrice { unit: p.unit, origin: PriceOrigin::Auction });
    match (vendor, auction) {
        (Some(v), Some(a)) => Some(if a.unit < v.unit { a } else { v }),
        (v, a) => v.or(a),
    }
}

pub fn evaluate(recipe: &Recipe, book: &PriceBook, source: PriceSource) -> Evaluation {
    let mut total = 0u64;
    let mut unpriced = Vec::new();
    for r in &recipe.reagents {
        match reagent_price(book, r.item_id, source) {
            Some(p) => total += p.unit.0 * u64::from(r.count),
            None => unpriced.push(r.item_id),
        }
    }
    let cost = unpriced.is_empty().then_some(Copper(total));
    let resale = auction_price(book, recipe.item_id, source).map(|AuctionPrice { unit: Copper(c), .. }| Copper((c as f64 * f64::from(recipe.quantity) * SELLER_PERCENT / 100.0).floor() as u64));
    let profit = cost.zip(resale).map(|(c, r)| r.0 as i64 - c.0 as i64);
    let margin = profit.zip(cost).filter(|(_, c)| c.0 > 0).map(|(p, c)| p as f32 / c.0 as f32);
    Evaluation {
        cost,
        unpriced,
        resale,
        profit,
        margin,
        sold_per_day: book.region.get(&recipe.item_id).and_then(|p| p.sold_per_day),
        auctions: book.realm.get(&recipe.item_id).and_then(|p| p.num_auctions),
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
        b.realm.insert(15407, realm(Some(1000), None, Some(12)));
        b.realm.insert(8171, realm(Some(300), Some(250), None));
        b.realm.insert(15409, realm(None, Some(200), None));
        b.region.insert(15407, RegionPrice { market_value: Some(Copper(2000)), sold_per_day: Some(3.5), ..Default::default() });
        b.region.insert(8171, RegionPrice { market_value: Some(Copper(400)), ..Default::default() });
        b
    }

    fn cured_rugged_hide() -> Recipe {
        let reagent = |item_id, count| Reagent { item_id, item_name: String::new(), quality: Quality::Common, icon: None, count };
        Recipe { spell_id: 19047, item_id: 15407, item_name: "Cured Rugged Hide".into(), quality: Quality::Common, slot: None, icon: None, quantity: 2.0, skill: None, reagents: vec![reagent(8171, 2), reagent(15409, 1)] }
    }

    #[test]
    fn realm_prices_take_the_current_min_buyout_first_and_the_cut_on_every_crafted_unit() {
        assert_eq!(evaluate(&cured_rugged_hide(), &book(), PriceSource::Realm), Evaluation {
            cost: Some(Copper(700)),
            unpriced: vec![],
            resale: Some(Copper(1900)),
            profit: Some(1200),
            margin: Some(1.7142857),
            sold_per_day: Some(3.5),
            auctions: Some(12),
        });
        let b = book();
        assert_eq!(auction_price(&b, 8171, PriceSource::Realm), Some(AuctionPrice { unit: Copper(250), basis: AuctionBasis::MinBuyout }));
        assert_eq!(auction_price(&b, 15407, PriceSource::Realm), Some(AuctionPrice { unit: Copper(1000), basis: AuctionBasis::MarketValue }));
        assert_eq!(auction_price(&b, 15407, PriceSource::Region), Some(AuctionPrice { unit: Copper(2000), basis: AuctionBasis::Region }));
        assert_eq!(auction_price(&b, 15409, PriceSource::Region), None);
    }

    #[test]
    fn an_unpriced_reagent_leaves_cost_and_profit_unknown_and_is_named() {
        assert_eq!(evaluate(&cured_rugged_hide(), &book(), PriceSource::Region), Evaluation {
            cost: None,
            unpriced: vec![15409],
            resale: Some(Copper(3800)),
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
        let realm = evaluate(&cured_rugged_hide(), &b, PriceSource::Realm);
        assert_eq!((realm.cost, realm.resale, realm.profit), (Some(Copper(650)), Some(Copper(1900)), Some(1250)));
        let region = evaluate(&cured_rugged_hide(), &b, PriceSource::Region);
        assert_eq!((region.cost, region.unpriced, region.resale, region.profit), (Some(Copper(850)), vec![], Some(Copper(3800)), Some(2950)));
        assert_eq!(reagent_price(&b, 8171, PriceSource::Realm), Some(ReagentPrice { unit: Copper(250), origin: PriceOrigin::Auction }));
        assert_eq!(reagent_price(&b, 8171, PriceSource::Region), Some(ReagentPrice { unit: Copper(350), origin: PriceOrigin::Vendor }));
        assert_eq!(reagent_price(&b, 2321, PriceSource::Realm), None);
    }

    #[test]
    fn a_tie_goes_to_the_vendor() {
        let mut b = book();
        b.vendor = VendorBook::new(HashMap::new(), HashMap::from([(8171, Copper(250))]));
        assert_eq!(reagent_price(&b, 8171, PriceSource::Realm), Some(ReagentPrice { unit: Copper(250), origin: PriceOrigin::Vendor }));
    }

    #[test]
    fn a_loss_is_negative_profit() {
        let mut recipe = cured_rugged_hide();
        recipe.quantity = 1.0;
        recipe.reagents[0].count = 4;
        let e = evaluate(&recipe, &book(), PriceSource::Realm);
        assert_eq!((e.cost, e.resale, e.profit), (Some(Copper(1200)), Some(Copper(950)), Some(-250)));
    }
}
