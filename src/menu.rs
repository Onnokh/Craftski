//! The toolbars' filter pills and menus, free of UI: which menu is open, what each lists,
//! what choosing an item changes, and when a pill reads as narrowing the rows.

use crate::flip_rows::{Supply, Zone};
use crate::overview::Feed;
use crate::settings;

/// A minimum demand, TSM's region sale rate read as an indicator: the pill's and menu's words and the threshold.
pub struct SoldStep {
    pub value: &'static str,
    pub min: Option<f64>,
}

impl SoldStep {
    /// "Demand 5+", as the line under a table names the filter.
    pub fn named(&self) -> String {
        format!("Demand {}", self.value)
    }
}

pub const SOLD_STEPS: [SoldStep; 5] = [
    SoldStep { value: "Any", min: None },
    SoldStep { value: ">0", min: Some(0.0) },
    SoldStep { value: "1+", min: Some(1.0) },
    SoldStep { value: "5+", min: Some(5.0) },
    SoldStep { value: "20+", min: Some(20.0) },
];

/// A toolbar menu, at most one open at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    CraftsSold,
    FlipsSold,
    Supply,
    Zone,
    Realm,
}

/// What choosing a menu item sets.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    /// A step of `SOLD_STEPS` for the recipes.
    CraftsSold(usize),
    /// A step of `SOLD_STEPS` for the flips.
    FlipsSold(usize),
    Supply(Supply),
    Zone(String),
    /// A realm in AppData.lua, by name.
    Realm(String),
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

/// The demand minimums, `current` chosen, each picked through `pick`.
pub fn sold(current: usize, pick: fn(usize) -> Pick) -> Vec<Item> {
    SOLD_STEPS.iter().enumerate().map(|(i, step)| Item::plain(step.value, i == current, pick(i))).collect()
}

/// Which stock to show by how much of it a merchant has.
pub fn supply(current: Supply) -> Vec<Item> {
    Supply::ALL.into_iter().map(|s| Item::plain(s.label(), s == current, Pick::Supply(s))).collect()
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

/// Every realm AppData.lua holds, each noting how old its prices are.
pub fn realms(feeds: &[Feed], current: &str) -> Vec<Item> {
    feeds
        .iter()
        .map(|f| Item { label: f.name.clone(), hint: f.age.as_ref().map(|(ago, _)| ago.clone()), spoken: f.line(), on: f.name == current, pick: Pick::Realm(f.name.clone()) })
        .collect()
}

// A pill fills while its filter hides rows a fresh look would show. "Priced only" and
// "Profitable only" never do: they start on, and what they hide has no profit to read.

/// Any minimum above "Any", on either tab, though flips start at 1+.
pub fn sold_active(step: usize) -> bool {
    step > 0
}

/// Endless or limited stock alone.
pub fn supply_active(s: Supply) -> bool {
    s != Supply::All
}

/// A typed rank that parses; anything else filters nothing.
pub fn skill_active(typed: &str) -> bool {
    settings::parse_skill(typed).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sold_menu_lists_each_minimum_and_marks_the_current_one() {
        let items = sold(3, Pick::CraftsSold);
        let seen: Vec<(&str, bool, Pick)> = items.iter().map(|i| (i.label.as_str(), i.on, i.pick.clone())).collect();
        assert_eq!(
            seen,
            vec![("Any", false, Pick::CraftsSold(0)), (">0", false, Pick::CraftsSold(1)), ("1+", false, Pick::CraftsSold(2)), ("5+", true, Pick::CraftsSold(3)), ("20+", false, Pick::CraftsSold(4))]
        );
        assert_eq!((items[3].hint.clone(), items[3].spoken.as_str()), (None, "5+"));
    }

    #[test]
    fn the_sold_pill_shows_the_short_value_and_the_hint_names_it() {
        let values: Vec<&str> = SOLD_STEPS.iter().map(|s| s.value).collect();
        assert_eq!(values, ["Any", ">0", "1+", "5+", "20+"]);
        assert_eq!(SOLD_STEPS[3].named(), "Demand 5+");
    }

    #[test]
    fn the_supply_menu_lists_all_endless_and_limited() {
        let seen: Vec<(String, bool, Pick)> = supply(Supply::Endless).into_iter().map(|i| (i.label, i.on, i.pick)).collect();
        assert_eq!(seen, vec![("All".into(), false, Pick::Supply(Supply::All)), ("Endless".into(), true, Pick::Supply(Supply::Endless)), ("Limited".into(), false, Pick::Supply(Supply::Limited))]);
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
    fn the_realm_menu_lists_each_realm_with_its_age_and_marks_the_one_priced() {
        let feeds = [Feed::new("PvE-Alliance".into(), None, 0), Feed::new("PvP-Horde".into(), Some(0), 3600)];
        assert_eq!(
            realms(&feeds, "PvP-Horde"),
            vec![
                Item { label: "PvE-Alliance".into(), hint: None, spoken: "PvE-Alliance".into(), on: false, pick: Pick::Realm("PvE-Alliance".into()) },
                Item { label: "PvP-Horde".into(), hint: Some("1 hour ago".into()), spoken: "PvP-Horde, 1 hour ago".into(), on: true, pick: Pick::Realm("PvP-Horde".into()) },
            ]
        );
    }

    #[test]
    fn a_pill_fills_only_when_its_filter_is_past_its_widest() {
        assert_eq!((sold_active(0), sold_active(1), sold_active(3)), (false, true, true));
        assert_eq!((supply_active(Supply::All), supply_active(Supply::Endless), supply_active(Supply::Limited)), (false, true, true));
        assert_eq!((skill_active(""), skill_active(" 225 "), skill_active("22a")), (false, true, false));
    }
}
