//! The toolbars' filter pills and menus, free of UI: which menu is open, what each lists,
//! what choosing an item changes, and when a pill reads as narrowing the rows.

use crate::flip_rows::Zone;
use crate::settings;

/// A minimum of sales a day: the pill's value, the menu's words and the threshold.
pub struct SoldStep {
    pub value: &'static str,
    pub item: &'static str,
    pub min: f64,
}

impl SoldStep {
    /// "Sold 5+ a day", as the line under a table names the filter.
    pub fn named(&self) -> String {
        format!("Sold {}", self.item)
    }
}

pub const SOLD_STEPS: [SoldStep; 4] = [
    SoldStep { value: "Any", item: "Any", min: 0.0 },
    SoldStep { value: "1+", item: "1+ a day", min: 1.0 },
    SoldStep { value: "5+", item: "5+ a day", min: 5.0 },
    SoldStep { value: "20+", item: "20+ a day", min: 20.0 },
];

/// A toolbar menu, at most one open at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    CraftsSold,
    Zone,
}

/// What choosing a menu item sets.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    /// A step of `SOLD_STEPS` for the recipes.
    CraftsSold(usize),
    Zone(String),
}

/// One line of a menu: its words, a quiet note at its right, and whether it is the current
/// choice.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub label: String,
    pub hint: Option<String>,
    /// What a screen reader says for it.
    pub spoken: String,
    pub on: bool,
    pub pick: Pick,
}

impl Item {
    fn plain(label: &str, on: bool, pick: Pick) -> Item {
        Item { label: label.to_owned(), hint: None, spoken: label.to_owned(), on, pick }
    }
}

/// The sales minimums, `current` chosen, each picked through `pick`.
pub fn sold(current: usize, pick: fn(usize) -> Pick) -> Vec<Item> {
    SOLD_STEPS.iter().enumerate().map(|(i, step)| Item::plain(step.item, i == current, pick(i))).collect()
}

/// Every zone with something to flip, each noting its merchants.
pub fn zones(zones: &[Zone], current: &str) -> Vec<Item> {
    zones
        .iter()
        .map(|z| Item {
            label: z.name.clone(),
            hint: Some(z.merchants.to_string()),
            spoken: format!("{}, {} merchants", z.name, z.merchants),
            on: z.name == current,
            pick: Pick::Zone(z.name.clone()),
        })
        .collect()
}

// A pill fills while its filter hides rows a fresh look would show. "Priced only" never
// does: it starts on, and an unpriced recipe has nothing to read.

/// Any minimum above "Any", on either tab, though flips start at 1+.
pub fn sold_active(step: usize) -> bool {
    step > 0
}

/// A typed rank that parses; anything else filters nothing.
pub fn skill_active(typed: &str) -> bool {
    settings::parse_skill(typed).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sold_menu_lists_each_minimum_a_day_and_marks_the_current_one() {
        let items = sold(2, Pick::CraftsSold);
        let seen: Vec<(&str, bool, Pick)> = items.iter().map(|i| (i.label.as_str(), i.on, i.pick.clone())).collect();
        assert_eq!(
            seen,
            vec![("Any", false, Pick::CraftsSold(0)), ("1+ a day", false, Pick::CraftsSold(1)), ("5+ a day", true, Pick::CraftsSold(2)), ("20+ a day", false, Pick::CraftsSold(3))]
        );
        assert_eq!((items[2].hint.clone(), items[2].spoken.as_str()), (None, "5+ a day"));
    }

    #[test]
    fn the_sold_pill_shows_the_short_value_and_the_hint_names_it_in_words() {
        let values: Vec<&str> = SOLD_STEPS.iter().map(|s| s.value).collect();
        assert_eq!(values, ["Any", "1+", "5+", "20+"]);
        assert_eq!(SOLD_STEPS[2].named(), "Sold 5+ a day");
    }

    #[test]
    fn the_zone_menu_lists_each_zone_with_its_merchants_and_marks_the_current_one() {
        let zones = zones(&[Zone { name: "Mulgore".into(), merchants: 4 }, Zone { name: "Thunder Bluff".into(), merchants: 37 }], "Thunder Bluff");
        assert_eq!(
            zones,
            vec![
                Item { label: "Mulgore".into(), hint: Some("4".into()), spoken: "Mulgore, 4 merchants".into(), on: false, pick: Pick::Zone("Mulgore".into()) },
                Item { label: "Thunder Bluff".into(), hint: Some("37".into()), spoken: "Thunder Bluff, 37 merchants".into(), on: true, pick: Pick::Zone("Thunder Bluff".into()) },
            ]
        );
    }

    #[test]
    fn a_pill_fills_only_when_its_filter_is_past_its_widest() {
        assert_eq!((sold_active(0), sold_active(1), sold_active(3)), (false, true, true));
        assert_eq!((skill_active(""), skill_active(" 225 "), skill_active("22a")), (false, true, false));
    }
}
