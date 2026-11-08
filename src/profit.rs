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

pub fn unit_price(book: &PriceBook, item: u32, source: PriceSource) -> Option<Copper> {
    match source {
        PriceSource::Realm => book.realm.get(&item).and_then(|p| p.market_value.or(p.min_buyout)),
        PriceSource::Region => book.region.get(&item).and_then(|p| p.market_value),
    }
}

pub fn evaluate(recipe: &Recipe, book: &PriceBook, source: PriceSource) -> Evaluation {
    let mut total = 0u64;
    let mut unpriced = Vec::new();
    for r in &recipe.reagents {
        match unit_price(book, r.item_id, source) {
            Some(Copper(c)) => total += c * u64::from(r.count),
            None => unpriced.push(r.item_id),
        }
    }
    let cost = unpriced.is_empty().then_some(Copper(total));
    let resale = unit_price(book, recipe.item_id, source).map(|Copper(c)| Copper((c as f64 * f64::from(recipe.quantity) * SELLER_PERCENT / 100.0).floor() as u64));
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
    fn realm_prices_fall_back_to_min_buyout_and_take_the_cut_on_every_crafted_unit() {
        assert_eq!(evaluate(&cured_rugged_hide(), &book(), PriceSource::Realm), Evaluation {
            cost: Some(Copper(800)),
            unpriced: vec![],
            resale: Some(Copper(1900)),
            profit: Some(1100),
            margin: Some(1.375),
            sold_per_day: Some(3.5),
            auctions: Some(12),
        });
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
    fn a_loss_is_negative_profit() {
        let mut recipe = cured_rugged_hide();
        recipe.quantity = 1.0;
        recipe.reagents[0].count = 3;
        let e = evaluate(&recipe, &book(), PriceSource::Realm);
        assert_eq!((e.cost, e.resale, e.profit), (Some(Copper(1100)), Some(Copper(950)), Some(-150)));
    }
}
