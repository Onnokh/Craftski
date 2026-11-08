//! What the detail panel says about one recipe: where the sale price goes, what each reagent
//! costs, and when to be careful. UI-free.

use crate::copper::Copper;
use crate::prices::PriceBook;
use crate::profit::{self, Evaluation, PriceSource};
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
    pub unit: Option<Copper>,
    pub total: Option<Copper>,
}

pub fn breakdown<'a>(recipe: &'a Recipe, eval: &Evaluation, book: &PriceBook, source: PriceSource) -> Breakdown<'a> {
    let gross = profit::unit_price(book, recipe.item_id, source).map(|Copper(c)| Copper((c as f64 * f64::from(recipe.quantity)).round() as u64));
    let reagents = recipe
        .reagents
        .iter()
        .map(|reagent| {
            let unit = profit::unit_price(book, reagent.item_id, source);
            ReagentLine { reagent, unit, total: unit.map(|Copper(c)| Copper(c * u64::from(reagent.count))) }
        })
        .collect();
    Breakdown { gross, cut: gross.zip(eval.resale).map(|(g, r)| Copper(g.0.saturating_sub(r.0))), reagents }
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

/// The reagents for one craft as plain text for the clipboard, one `12x Thick Leather` line
/// each, then the total.
pub fn shopping_list(lines: &[ReagentLine], cost: Option<Copper>) -> String {
    let mut out: String = lines.iter().map(|l| format!("{}x {}\n", l.reagent.count, l.reagent.item_name)).collect();
    match cost {
        Some(c) => out.push_str(&format!("Total: {c}")),
        None => {
            let unpriced: Vec<&str> = lines.iter().filter(|l| l.unit.is_none()).map(|l| l.reagent.item_name.as_str()).collect();
            out.push_str(&format!("Total: unknown, no price for {}", unpriced.join(", ")));
        }
    }
    out
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
    use crate::prices::{RealmPrice, RegionPrice};
    use crate::recipes::{Quality, Skill, Slot};

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
        let lines: Vec<_> = b.reagents.iter().map(|l| (l.reagent.item_id, l.unit, l.total)).collect();
        assert_eq!(lines, vec![(4304, Some(Copper(647)), Some(Copper(7_764))), (8343, Some(Copper(578)), Some(Copper(2_312)))]);
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
    fn the_shopping_list_is_one_line_per_reagent_and_a_total() {
        let (recipe, book) = (nightscape_cloak(), book());
        let eval = profit::evaluate(&recipe, &book, PriceSource::Realm);
        let b = breakdown(&recipe, &eval, &book, PriceSource::Realm);
        assert_eq!(shopping_list(&b.reagents, eval.cost), "12x Thick Leather\n4x Heavy Silken Thread\nTotal: 1g 00s 76c");
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
}
