//! What the detail panel says about one recipe: where the sale price goes, what each reagent
//! costs, and when to be careful. UI-free.

use crate::copper::Copper;
use crate::prices::PriceBook;
use crate::profit::{self, Evaluation, PriceSource, ReagentPrice};
use crate::recipes::{Reagent, Recipe};

/// Realm listings at or below this count make a price easy to move.
const FEW_LISTED: u32 = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct Breakdown<'a> {
    /// The crafted output at market value, before the auction house cut.
    pub gross: Option<Copper>,
    pub cut: Option<Copper>,
    pub reagents: Vec<ReagentLine<'a>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReagentLine<'a> {
    pub reagent: &'a Reagent,
    pub price: Option<ReagentPrice>,
    pub total: Option<Copper>,
    /// Auctions up on the realm, whichever source prices the reagent.
    pub listed: Option<u32>,
}

pub fn breakdown<'a>(recipe: &'a Recipe, eval: &Evaluation, book: &PriceBook, source: PriceSource) -> Breakdown<'a> {
    let gross = profit::auction_price(book, recipe.item_id, source).map(|Copper(c)| Copper((c as f64 * f64::from(recipe.quantity)).round() as u64));
    let reagents = recipe
        .reagents
        .iter()
        .map(|reagent| {
            let price = profit::reagent_price(book, reagent.item_id, source);
            ReagentLine { reagent, price, total: price.map(|p| Copper(p.unit.0 * u64::from(reagent.count))), listed: book.realm.get(&reagent.item_id).and_then(|p| p.num_auctions) }
        })
        .collect();
    Breakdown { gross, cut: gross.zip(eval.resale).map(|(g, r)| Copper(g.0.saturating_sub(r.0))), reagents }
}

/// A bar on a track that runs from 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bar {
    pub start: f32,
    pub width: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Waterfall {
    pub sells: Bar,
    pub cut: Bar,
    pub mats: Bar,
    /// Sits left of the mats on a profit; on a loss it is the shortfall past the sale price.
    pub profit: Bar,
}

/// Lays the sale price and its deductions on one scale. Mats sit just left of the cut so the
/// profit is what remains at the left; a loss stretches the scale past the sale price.
pub fn waterfall(gross: Copper, cut: Copper, cost: Copper) -> Waterfall {
    let (gross, cut, cost) = (gross.0, cut.0, cost.0);
    let spent = cost + cut;
    let scale = gross.max(spent);
    if scale == 0 {
        return Waterfall::default();
    }
    let bar = |from: u64, to: u64| Bar { start: from as f32 / scale as f32, width: to.saturating_sub(from) as f32 / scale as f32 };
    let kept = gross.saturating_sub(spent);
    Waterfall {
        sells: bar(0, gross),
        mats: bar(kept, kept + cost),
        cut: bar(kept + cost, kept + spent),
        profit: if gross >= spent { bar(0, kept) } else { bar(gross, spent) },
    }
}

/// A plain warning when the profit rests on thin evidence, or None when the market looks sound.
pub fn caution(sold_per_day: Option<f64>, listed: Option<u32>) -> Option<String> {
    let few = listed.filter(|&n| n <= FEW_LISTED);
    match (sold_per_day, few) {
        (None, Some(n)) => Some(format!("No region sales recorded and only {n} listed. This margin is real on paper but untested; list one before crafting a stack.")),
        (None, None) => Some("No region sales recorded. This margin is real on paper but untested; list one before crafting a stack.".into()),
        (Some(s), _) if s < 1.0 => Some(format!("Sells about {s:.1} a day across the region. Expect to wait for a buyer; list one before crafting a stack.")),
        (Some(_), Some(n)) => Some(format!("Only {n} listed on your realm, so a single seller can move the price. Check the auction house before crafting.")),
        (Some(_), None) => None,
    }
}

/// The reagents for one craft as plain text for the clipboard, one `12x Thick Leather` line
/// each, then the total.
pub fn shopping_list(lines: &[ReagentLine], cost: Option<Copper>) -> String {
    let mut out: String = lines.iter().map(|l| format!("{}x {}\n", l.reagent.count, l.reagent.item_name)).collect();
    match cost {
        Some(c) => out.push_str(&format!("Total: {c}")),
        None => {
            let unpriced: Vec<&str> = lines.iter().filter(|l| l.price.is_none()).map(|l| l.reagent.item_name.as_str()).collect();
            out.push_str(&format!("Total: unknown, no price for {}", unpriced.join(", ")));
        }
    }
    out
}

/// "Cloak · makes 1 · learned at skill 225", leaving out what the data does not know.
pub fn subtitle(recipe: &Recipe) -> String {
    let mut parts: Vec<String> = recipe.slot.map(|s| s.name().to_string()).into_iter().collect();
    parts.push(format!("makes {}", recipe.quantity));
    if let Some(skill) = recipe.skill {
        parts.push(format!("learned at skill {}", skill.learn));
    }
    let mut out = parts.join(" · ");
    if let Some(first) = out.get(..1) {
        out = first.to_uppercase() + &out[1..];
    }
    out
}

pub fn wowhead_url(item_id: u32) -> String {
    format!("https://www.wowhead.com/forever/item={item_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prices::{RealmPrice, RegionPrice};
    use crate::profit::PriceOrigin;
    use crate::recipes::{Quality, Skill, Slot};
    use crate::vendor::VendorBook;
    use std::collections::HashMap;

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
        b.realm.insert(8195, realm(50_788, Some(3)));
        b.realm.insert(4304, realm(647, Some(437)));
        b.realm.insert(8343, realm(578, None));
        b.region.insert(4304, RegionPrice { market_value: Some(Copper(700)), ..Default::default() });
        b
    }

    #[test]
    fn breaks_a_craft_into_sale_cut_and_reagent_lines() {
        let (recipe, book) = (nightscape_cloak(), book());
        let eval = profit::evaluate(&recipe, &book, PriceSource::Realm);
        let b = breakdown(&recipe, &eval, &book, PriceSource::Realm);
        assert_eq!((b.gross, b.cut, eval.resale, eval.cost), (Some(Copper(50_788)), Some(Copper(2_540)), Some(Copper(48_248)), Some(Copper(10_076))));
        let lines: Vec<_> = b.reagents.iter().map(|l| (l.reagent.item_id, l.price.map(|p| p.unit), l.total, l.listed)).collect();
        assert_eq!(lines, vec![(4304, Some(Copper(647)), Some(Copper(7_764)), Some(437)), (8343, Some(Copper(578)), Some(Copper(2_312)), None)]);
    }

    #[test]
    fn region_prices_leave_reagents_without_a_region_price_unpriced() {
        let (recipe, book) = (nightscape_cloak(), book());
        let eval = profit::evaluate(&recipe, &book, PriceSource::Region);
        let b = breakdown(&recipe, &eval, &book, PriceSource::Region);
        assert_eq!((b.gross, b.cut), (None, None));
        assert_eq!(b.reagents.iter().map(|l| l.total).collect::<Vec<_>>(), vec![Some(Copper(8_400)), None]);
        assert_eq!(shopping_list(&b.reagents, eval.cost), "12x Thick Leather\n4x Heavy Silken Thread\nTotal: unknown, no price for Heavy Silken Thread");
    }

    #[test]
    fn a_profit_keeps_the_left_of_the_track() {
        let w = waterfall(Copper(10_000), Copper(500), Copper(2_000));
        assert_eq!(w, Waterfall {
            sells: Bar { start: 0.0, width: 1.0 },
            cut: Bar { start: 0.95, width: 0.05 },
            mats: Bar { start: 0.75, width: 0.2 },
            profit: Bar { start: 0.0, width: 0.75 },
        });
    }

    #[test]
    fn a_loss_stretches_the_track_past_the_sale() {
        let w = waterfall(Copper(800), Copper(40), Copper(960));
        assert_eq!(w, Waterfall {
            sells: Bar { start: 0.0, width: 0.8 },
            cut: Bar { start: 0.96, width: 0.04 },
            mats: Bar { start: 0.0, width: 0.96 },
            profit: Bar { start: 0.8, width: 0.2 },
        });
        assert_eq!(waterfall(Copper(0), Copper(0), Copper(0)), Waterfall::default());
    }

    #[test]
    fn warns_in_plain_words_when_sales_are_thin() {
        assert_eq!(caution(None, Some(3)).as_deref(), Some("No region sales recorded and only 3 listed. This margin is real on paper but untested; list one before crafting a stack."));
        assert_eq!(caution(None, Some(40)).as_deref(), Some("No region sales recorded. This margin is real on paper but untested; list one before crafting a stack."));
        assert_eq!(caution(Some(0.42), None).as_deref(), Some("Sells about 0.4 a day across the region. Expect to wait for a buyer; list one before crafting a stack."));
        assert_eq!(caution(Some(23.5), Some(2)).as_deref(), Some("Only 2 listed on your realm, so a single seller can move the price. Check the auction house before crafting."));
        assert_eq!(caution(Some(23.5), Some(6)), None);
        assert_eq!(caution(Some(1.0), None), None);
    }

    #[test]
    fn reagent_lines_name_where_each_price_comes_from() {
        let (recipe, mut book) = (nightscape_cloak(), book());
        book.vendor = VendorBook::new(HashMap::new(), HashMap::from([(4304, Copper(5_000)), (8343, Copper(500))]));
        let eval = profit::evaluate(&recipe, &book, PriceSource::Realm);
        let b = breakdown(&recipe, &eval, &book, PriceSource::Realm);
        let prices: Vec<_> = b.reagents.iter().map(|l| (l.reagent.item_id, l.price, l.total)).collect();
        assert_eq!(prices, vec![
            (4304, Some(ReagentPrice { unit: Copper(647), origin: PriceOrigin::Auction }), Some(Copper(7_764))),
            (8343, Some(ReagentPrice { unit: Copper(500), origin: PriceOrigin::Vendor }), Some(Copper(2_000))),
        ]);
        assert_eq!(shopping_list(&b.reagents, eval.cost), "12x Thick Leather\n4x Heavy Silken Thread\nTotal: 97s 64c");
    }

    #[test]
    fn the_subtitle_names_slot_yield_and_skill_when_known() {
        let mut recipe = nightscape_cloak();
        assert_eq!(subtitle(&recipe), "Cloak · makes 1 · learned at skill 225");
        recipe.slot = None;
        recipe.skill = None;
        recipe.quantity = 2.0;
        assert_eq!(subtitle(&recipe), "Makes 2");
        assert_eq!(wowhead_url(8195), "https://www.wowhead.com/forever/item=8195");
    }
}
