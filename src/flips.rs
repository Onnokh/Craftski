//! Goods to buy from a merchant and sell on the auction house: what one costs this character,
//! what it sells for, and what the deposit takes. UI-free.

use std::collections::HashMap;

use crate::copper::Copper;
use crate::items::{Facts, Item, Purchase};
use crate::merchants::{Merchant, Price, Side};
use crate::prices::PriceBook;
use crate::profit::{self, AuctionPrice, SELLER_PERCENT};

/// Who buys and how they post.
pub struct Rules {
    pub side: Side,
    /// Factions this character is Honored or better with.
    pub honored: &'static [&'static str],
    pub listing_hours: u64,
}

/// A Tauren on PvP-Horde without Bartering, Honored with Thunder Bluff, posting for 8 hours.
pub const RULES: Rules = Rules { side: Side::Horde, honored: &["Thunder Bluff"], listing_hours: 8 };

const REP_DISCOUNT_PERCENT: u8 = 10;
/// A faction house's deposit per 2 hours listed, as a percent of the merchant's buy price
/// (AuctionHouse.db2 `DepositRate`).
const DEPOSIT_PERCENT: u64 = 5;
const MIN_DEPOSIT: Copper = Copper(100);
const POST_STACK: u32 = 20;

/// What one costs this character, and how that was worked out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paid {
    /// TSM's vendor price: what a merchant last charged this character, discount included.
    Tsm(Copper),
    /// A list price per item, less this character's reputation discount.
    List { list: f64, from: ListSource, discount_percent: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListSource {
    /// The client's `ItemSparse.BuyPrice`.
    Client,
    /// foreverdb's price at this merchant.
    ForeverDb,
}

impl Paid {
    pub fn unit(self) -> f64 {
        match self {
            Paid::Tsm(c) => c.0 as f64,
            Paid::List { list, discount_percent, .. } => list * f64::from(100 - discount_percent) / 100.0,
        }
    }

    /// To the nearest copper, as the game charges.
    pub fn copper(self) -> Copper {
        Copper(self.unit().round() as u64)
    }

    /// Where the price came from, as in `TSM` or `list −10%`.
    pub fn origin(self) -> String {
        match self {
            Paid::Tsm(_) => "TSM".into(),
            Paid::List { discount_percent: 0, .. } => "list".into(),
            Paid::List { discount_percent, .. } => format!("list \u{2212}{discount_percent}%"),
        }
    }
}

/// The deposit a listing holds, refunded when it sells and lost when it expires.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Deposit {
    pub per_listing: Copper,
    /// Items per listing.
    pub post_stack: u32,
    /// The deposit expected to be lost for each one sold; None without a sale rate.
    pub loss: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flip {
    /// Index into the merchant list.
    pub merchant: usize,
    pub item: u32,
    pub limit: Option<u32>,
    /// None when neither TSM, the client nor foreverdb has a price.
    pub paid: Option<Paid>,
    pub sale: Option<AuctionPrice>,
    /// The sale price after the auction house cut.
    pub resale: Option<Copper>,
    pub deposit: Deposit,
    pub sale_rate: Option<f64>,
    pub sold_per_day: Option<f64>,
    /// Per item: the resale less the price paid and the expected deposit loss, an unknown loss
    /// counting as none.
    pub profit: Option<i64>,
}

/// One flip per gold-priced good a merchant on `rules.side` sells that can go up on the auction
/// house, priced on the chosen realm.
pub fn evaluate(merchants: &[Merchant], items: &HashMap<u32, Item>, book: &PriceBook, rules: &Rules) -> Vec<Flip> {
    let mut flips = Vec::new();
    for (index, merchant) in merchants.iter().enumerate() {
        if merchant.side.is_some_and(|s| s != rules.side) {
            continue;
        }
        let discount = if merchant.rep.as_deref().is_some_and(|r| rules.honored.contains(&r)) { REP_DISCOUNT_PERCENT } else { 0 };
        for stock in &merchant.stock {
            let Price::Gold(listed) = stock.price else { continue };
            let facts = items.get(&stock.item).and_then(|i| i.facts);
            if facts.is_some_and(|f| !f.binding.tradable()) {
                continue;
            }
            let paid = paid(book, stock.item, facts, listed, discount);
            let sale = profit::auction_price(book, stock.item);
            let resale = sale.map(|s| Copper((s.unit.0 as f64 * SELLER_PERCENT / 100.0).floor() as u64));
            let region = book.region.get(&stock.item);
            let sale_rate = region.and_then(|r| r.sale_rate);
            let deposit = deposit(facts, sale_rate, rules.listing_hours);
            let profit = paid.zip(resale).map(|(p, r)| (r.0 as f64 - p.unit() - deposit.loss.unwrap_or(0.0)).round() as i64);
            flips.push(Flip { merchant: index, item: stock.item, limit: stock.limit, paid, sale, resale, deposit, sale_rate, sold_per_day: region.and_then(|r| r.sold_per_day), profit });
        }
    }
    flips
}

/// TSM's price as it stands; else the client's list price, else foreverdb's, less `discount`.
fn paid(book: &PriceBook, item: u32, facts: Option<Facts>, listed: Option<Purchase>, discount: u8) -> Option<Paid> {
    if let Some(c) = book.vendor.get(item) {
        return Some(Paid::Tsm(c));
    }
    let (purchase, from) = facts.and_then(|f| f.buy).map(|p| (p, ListSource::Client)).or(listed.map(|p| (p, ListSource::ForeverDb)))?;
    Some(Paid::List { list: purchase.unit(), from, discount_percent: discount })
}

/// An item the client's tables lack posts one at a time at the minimum deposit.
fn deposit(facts: Option<Facts>, sale_rate: Option<f64>, hours: u64) -> Deposit {
    let (sell, stack) = facts.map_or((0, 1), |f| (f.sell.0, f.stack));
    let post_stack = stack.clamp(1, POST_STACK);
    let per_listing = Copper((sell * u64::from(post_stack) * DEPOSIT_PERCENT * hours / 200).max(MIN_DEPOSIT.0));
    // Each listing sells with chance `rate`, so every sale carries (1 - rate) / rate expired ones.
    let loss = sale_rate.map(|rate| (per_listing.0 as f64 / f64::from(post_stack) * (1.0 - rate) / rate).max(0.0));
    Deposit { per_listing, post_stack, loss }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::Binding;
    use crate::merchants::{Stock, StockSource};
    use crate::prices::{RealmPrice, RegionPrice};
    use crate::profit::AuctionBasis;
    use crate::recipes::Quality;
    use crate::vendor::VendorBook;

    fn merchant(npc: u32, side: Option<Side>, rep: &str, stock: Vec<Stock>) -> Merchant {
        let rep = (!rep.is_empty()).then(|| rep.to_string());
        Merchant { npc, name: format!("NPC {npc}"), role: String::new(), zone: "Thunder Bluff".into(), at: (50.0, 50.0), side, rep, source: StockSource::ForeverSeen, stock }
    }

    fn gold(item: u32, copper: u64) -> Stock {
        Stock { item, price: Price::Gold(Some(Purchase { copper, count: 1 })), limit: None }
    }

    fn item(sell: u64, stack: u32, binding: Binding, buy: Option<u64>) -> Item {
        let buy = buy.map(|copper| Purchase { copper, count: 1 });
        Item { name: String::new(), quality: Quality::Common, icon: None, facts: Some(Facts { sell: Copper(sell), stack, binding, buy }) }
    }

    /// 6365 Strong Fishing Pole: TSM knows the price. 11303 Fine Shortbow: only the client's
    /// list. 3371 Crystal Vial: stacks of 20. 5956 Blacksmith Hammer: binds on pickup.
    fn world() -> (HashMap<u32, Item>, PriceBook) {
        let items = HashMap::from([
            (6365, item(181, 1, Binding::None, Some(906))),
            (11303, item(400, 1, Binding::OnEquip, Some(2000))),
            (3371, item(50, 20, Binding::None, Some(20))),
            (5956, item(3, 1, Binding::OnPickup, Some(18))),
        ]);
        let mut book = PriceBook::default();
        for (id, min_buyout) in [(6365, 3000), (11303, 5000), (3371, 400), (5956, 900)] {
            book.realm_prices_mut().insert(id, RealmPrice { min_buyout: Some(Copper(min_buyout)), ..Default::default() });
        }
        book.region.insert(6365, RegionPrice { sale_rate: Some(0.5), sold_per_day: Some(2.0), ..Default::default() });
        book.region.insert(11303, RegionPrice { sale_rate: Some(0.25), ..Default::default() });
        book.vendor = VendorBook::new(HashMap::from([(6365, Copper(815))]), HashMap::new());
        (items, book)
    }

    fn sale(unit: u64) -> Option<AuctionPrice> {
        Some(AuctionPrice { unit: Copper(unit), basis: AuctionBasis::MinBuyout })
    }

    #[test]
    fn a_tsm_price_is_paid_as_it_stands_without_another_discount() {
        let (items, book) = world();
        let flips = evaluate(&[merchant(3027, Some(Side::Horde), "Thunder Bluff", vec![gold(6365, 906)])], &items, &book, &RULES);
        assert_eq!(flips, vec![Flip {
            merchant: 0,
            item: 6365,
            limit: None,
            paid: Some(Paid::Tsm(Copper(815))),
            sale: sale(3000),
            resale: Some(Copper(2850)),
            deposit: Deposit { per_listing: Copper(100), post_stack: 1, loss: Some(100.0) },
            sale_rate: Some(0.5),
            sold_per_day: Some(2.0),
            profit: Some(2850 - 815 - 100),
        }]);
    }

    #[test]
    fn a_list_price_takes_ten_percent_off_only_where_the_character_is_honored() {
        let (items, book) = world();
        let stock = || vec![gold(11303, 1900)];
        let merchants = [merchant(3015, Some(Side::Horde), "Thunder Bluff", stock()), merchant(3322, Some(Side::Horde), "Orgrimmar", stock())];
        let flips = evaluate(&merchants, &items, &book, &RULES);
        let paid: Vec<_> = flips.iter().map(|f| (f.paid, f.paid.map(Paid::unit), f.profit)).collect();
        assert_eq!(paid, vec![
            (Some(Paid::List { list: 2000.0, from: ListSource::Client, discount_percent: 10 }), Some(1800.0), Some(4750 - 1800 - 300)),
            (Some(Paid::List { list: 2000.0, from: ListSource::Client, discount_percent: 0 }), Some(2000.0), Some(4750 - 2000 - 300)),
        ]);
    }

    #[test]
    fn foreverdbs_price_stands_in_where_the_client_has_none() {
        let (mut items, book) = world();
        items.remove(&11303);
        let bundle = Stock { item: 11303, price: Price::Gold(Some(Purchase { copper: 30, count: 5 })), limit: Some(2) };
        let flips = evaluate(&[merchant(3015, Some(Side::Horde), "Thunder Bluff", vec![bundle])], &items, &book, &RULES);
        assert_eq!((flips[0].paid, flips[0].limit), (Some(Paid::List { list: 6.0, from: ListSource::ForeverDb, discount_percent: 10 }), Some(2)));
        assert_eq!(flips[0].deposit, Deposit { per_listing: Copper(100), post_stack: 1, loss: Some(300.0) });
    }

    #[test]
    fn the_deposit_is_five_percent_of_the_sell_price_per_two_hours_and_lost_on_unsold_listings() {
        assert_eq!(deposit(Some(Facts { sell: Copper(50), stack: 20, binding: Binding::None, buy: None }), Some(0.2), 8), Deposit { per_listing: Copper(200), post_stack: 20, loss: Some(40.0) });
        assert_eq!(deposit(Some(Facts { sell: Copper(50), stack: 200, binding: Binding::None, buy: None }), Some(1.0), 24), Deposit { per_listing: Copper(600), post_stack: 20, loss: Some(0.0) });
        assert_eq!(deposit(Some(Facts { sell: Copper(181), stack: 1, binding: Binding::None, buy: None }), None, 8), Deposit { per_listing: Copper(100), post_stack: 1, loss: None });
    }

    #[test]
    fn an_unknown_sale_rate_costs_no_deposit_but_says_so() {
        let (items, book) = world();
        let flips = evaluate(&[merchant(3015, None, "", vec![gold(3371, 20)])], &items, &book, &RULES);
        assert_eq!((flips[0].deposit.loss, flips[0].paid.map(Paid::unit), flips[0].profit), (None, Some(20.0), Some(380 - 20)));
    }

    #[test]
    fn skips_currency_goods_binding_goods_and_the_other_factions_merchants() {
        let (items, book) = world();
        let currency = Stock { item: 6365, price: Price::Currency, limit: None };
        let merchants = [
            merchant(1, Some(Side::Horde), "", vec![currency, gold(5956, 18), gold(3371, 20)]),
            merchant(2, Some(Side::Alliance), "", vec![gold(6365, 906)]),
            merchant(3, None, "", vec![gold(11303, 2000)]),
        ];
        let kept: Vec<(usize, u32)> = evaluate(&merchants, &items, &book, &RULES).iter().map(|f| (f.merchant, f.item)).collect();
        assert_eq!(kept, vec![(0, 3371), (2, 11303)]);
    }
}
