//! The toolbars' menus, free of UI: which one is open, what each lists, and what choosing an
//! item changes.

use crate::flip_rows::Zone;

/// A toolbar menu, at most one open at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    Zone,
}

/// What choosing a menu item sets.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
