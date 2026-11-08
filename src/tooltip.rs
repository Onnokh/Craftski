//! What a hover tooltip says: the item's name in its quality colour, what a craft takes, then
//! the item as the game's tooltip shows it. UI-free.

use crate::recipes::{Quality, Reagent, Recipe};
use crate::sheets::{Line, Sheets};

/// What the pointer rests on that has a tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tip {
    /// A recipe's row in the crafts table, by spell id.
    Recipe(u32),
    /// A reagent's line in the recipe detail, by item id.
    Reagent(u32),
    /// A flip's row, by its index in the flips list.
    Flip(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub name: String,
    pub quality: Quality,
    /// What a craft takes, under the name; empty for anything not crafted.
    pub mats: Vec<Reagent>,
    /// The game tooltip's lines under the name; empty until foreverdb's items load.
    pub lines: Vec<Line>,
}

impl Card {
    pub fn item(name: String, quality: Quality, item: u32, sheets: Option<&Sheets>) -> Card {
        Card { name, quality, mats: Vec::new(), lines: sheets.map(|s| s.lines(item)).unwrap_or_default() }
    }

    pub fn recipe(recipe: &Recipe, sheets: Option<&Sheets>) -> Card {
        Card { mats: recipe.reagents.clone(), ..Card::item(recipe.item_name.clone(), recipe.quality, recipe.item_id, sheets) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheets;

    #[test]
    fn a_recipe_shows_its_mats_then_the_crafted_item() {
        let sheets = sheets::parse(r#"[{"id":9,"il":32,"inv":9,"c":4,"sc":2,"ar":47,"st":[[7,6]]}]"#, r#"{"7":"Stamina"}"#, r#"{"sub":{"4.2":"Leather"}}"#, "{}").unwrap();
        let hide = Reagent { item_id: 4234, item_name: "Heavy Leather".into(), quality: Quality::Common, icon: None, count: 8 };
        let recipe = Recipe { spell_id: 1, item_id: 9, item_name: "Barbaric Bracers".into(), quality: Quality::Uncommon, slot: None, icon: None, quantity: 1.0, skill: None, reagents: vec![hide.clone()] };
        let card = Card::recipe(&recipe, Some(&sheets));
        assert_eq!((card.name.as_str(), card.mats), ("Barbaric Bracers", vec![hide]));
        assert_eq!(card.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["Item Level 32", "Wrist", "47 Armor", "+6 Stamina"]);
        assert_eq!(Card::recipe(&recipe, None).lines, vec![]);
    }
}
