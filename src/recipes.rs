//! Profession recipes from wago.tools DB2 CSV exports, joined once and cached as JSON.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub struct Profession {
    pub name: &'static str,
    pub skill_lines: &'static [u32],
}

/// 2945 is the child skill line Forever files some Leatherworking recipes under.
pub const LEATHERWORKING: Profession = Profession { name: "Leatherworking", skill_lines: &[165, 2945] };

/// The installed WoW: Forever build. Downloads pin it so the cache key names what was fetched.
pub const BUILD: &str = "1.60.1.70862";

/// A CASC file data id, here always an item's icon texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FileDataId(pub u32);

/// `ItemSparse.OverallQualityID`, the colour an item's name shows in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    Poor,
    #[default]
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Artifact,
    Heirloom,
}

impl Quality {
    pub fn from_id(id: u8) -> Option<Quality> {
        use Quality::*;
        [Poor, Common, Uncommon, Rare, Epic, Legendary, Artifact, Heirloom].get(usize::from(id)).copied()
    }
}

/// Where an item is worn or held, from `ItemSparse.InventoryType`, as the tooltip names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Slot {
    Head,
    Neck,
    Shoulder,
    Shirt,
    Chest,
    Waist,
    Legs,
    Feet,
    Wrist,
    Hands,
    Finger,
    Trinket,
    Weapon,
    Shield,
    Ranged,
    Cloak,
    Bag,
    Tabard,
    HeldInOffHand,
    Ammo,
    Quiver,
    Relic,
}

impl Slot {
    pub fn from_inventory_type(id: u8) -> Option<Slot> {
        use Slot::*;
        Some(match id {
            1 => Head,
            2 => Neck,
            3 => Shoulder,
            4 => Shirt,
            5 | 20 => Chest,
            6 => Waist,
            7 => Legs,
            8 => Feet,
            9 => Wrist,
            10 => Hands,
            11 => Finger,
            12 => Trinket,
            13 | 17 | 21 | 22 => Weapon,
            14 => Shield,
            15 | 25 | 26 => Ranged,
            16 => Cloak,
            18 => Bag,
            19 => Tabard,
            23 => HeldInOffHand,
            24 => Ammo,
            27 => Quiver,
            28 => Relic,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Slot::Head => "Head",
            Slot::Neck => "Neck",
            Slot::Shoulder => "Shoulder",
            Slot::Shirt => "Shirt",
            Slot::Chest => "Chest",
            Slot::Waist => "Waist",
            Slot::Legs => "Legs",
            Slot::Feet => "Feet",
            Slot::Wrist => "Wrist",
            Slot::Hands => "Hands",
            Slot::Finger => "Finger",
            Slot::Trinket => "Trinket",
            Slot::Weapon => "Weapon",
            Slot::Shield => "Shield",
            Slot::Ranged => "Ranged",
            Slot::Cloak => "Cloak",
            Slot::Bag => "Bag",
            Slot::Tabard => "Tabard",
            Slot::HeldInOffHand => "Held in off-hand",
            Slot::Ammo => "Ammo",
            Slot::Quiver => "Quiver",
            Slot::Relic => "Relic",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reagent {
    pub item_id: u32,
    pub item_name: String,
    pub quality: Quality,
    pub icon: Option<FileDataId>,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub spell_id: u32,
    pub item_id: u32,
    pub item_name: String,
    pub quality: Quality,
    /// None for items that are not worn or held, such as leather and armor kits.
    pub slot: Option<Slot>,
    pub icon: Option<FileDataId>,
    pub quantity: f32,
    /// None where the data has no ranks for the recipe, as for Winter Boots.
    pub skill: Option<Skill>,
    pub reagents: Vec<Reagent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    /// Rank needed to learn it.
    pub learn: u16,
    /// Rank at which it turns grey and stops raising skill.
    pub grey: u16,
}

impl Skill {
    pub fn green(self) -> u16 {
        self.learn.midpoint(self.grey)
    }
}

pub struct Tables {
    pub skill_line_ability: String,
    pub spell_effect: String,
    pub spell_reagents: String,
    pub item_sparse: String,
    pub icons: IconTables,
}

/// The tables that name an item's icon.
pub struct IconTables {
    pub item: String,
    pub item_modified_appearance: String,
    pub item_appearance: String,
}

impl IconTables {
    pub fn fetch() -> Result<IconTables, String> {
        Ok(IconTables { item: fetch("Item")?, item_modified_appearance: fetch("ItemModifiedAppearance")?, item_appearance: fetch("ItemAppearance")? })
    }
}

const CREATE_ITEM: u32 = 24;
const REAGENT_SLOTS: usize = 8;

pub fn join(t: &Tables, profession: &Profession) -> Result<Vec<Recipe>, String> {
    // MinSkillLineRank is 1 on every Forever row; the trainer gates a recipe at its trivial-low rank.
    let mut spells: BTreeMap<u32, Option<Skill>> = BTreeMap::new();
    each_row(&t.skill_line_ability, "SkillLineAbility", ["SkillLine", "Spell", "TrivialSkillLineRankLow", "TrivialSkillLineRankHigh"], |[line, spell, low, high]| {
        if profession.skill_lines.contains(&num(line)?) {
            let (learn, grey) = (num(low)?, num(high)?);
            spells.entry(num(spell)?).or_insert((learn != 0).then_some(Skill { learn, grey }));
        }
        Ok(())
    })?;

    let mut creates: HashMap<u32, (u32, u32, f32)> = HashMap::new();
    each_row(&t.spell_effect, "SpellEffect", ["SpellID", "Effect", "EffectIndex", "EffectItemType", "EffectBasePointsF"], |[spell, effect, index, item, qty]| {
        let spell = num(spell)?;
        if !spells.contains_key(&spell) || num::<u32>(effect)? != CREATE_ITEM {
            return Ok(());
        }
        let candidate = (num(index)?, num(item)?, num(qty)?);
        let slot = creates.entry(spell).or_insert(candidate);
        if candidate.0 < slot.0 {
            *slot = candidate;
        }
        Ok(())
    })?;

    let slots: [String; 2 * REAGENT_SLOTS] =
        std::array::from_fn(|i| if i < REAGENT_SLOTS { format!("Reagent_{i}") } else { format!("ReagentCount_{}", i - REAGENT_SLOTS) });
    let cols: [&str; 1 + 2 * REAGENT_SLOTS] = std::array::from_fn(|i| if i == 0 { "SpellID" } else { slots[i - 1].as_str() });
    let mut reagents: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
    each_row(&t.spell_reagents, "SpellReagents", cols, |row| {
        let spell = num(row[0])?;
        if !spells.contains_key(&spell) {
            return Ok(());
        }
        let (ids, counts) = row[1..].split_at(REAGENT_SLOTS);
        let mut list = Vec::new();
        for (id, count) in ids.iter().zip(counts) {
            let (id, count) = (num::<u32>(id)?, num::<i64>(count)?);
            if id != 0 && count > 0 {
                list.push((id, count as u32));
            }
        }
        if !list.is_empty() {
            reagents.insert(spell, list);
        }
        Ok(())
    })?;

    let mut recipes: Vec<Recipe> = spells
        .iter()
        .filter_map(|(&spell_id, &skill)| {
            let (_, item_id, quantity) = *creates.get(&spell_id)?;
            let reagents = reagents
                .remove(&spell_id)?
                .into_iter()
                .map(|(item_id, count)| Reagent { item_id, item_name: String::new(), quality: Quality::Common, icon: None, count })
                .collect();
            (item_id != 0).then(|| Recipe { spell_id, item_id, item_name: String::new(), quality: Quality::Common, slot: None, icon: None, quantity, skill, reagents })
        })
        .collect();

    let wanted: BTreeSet<u32> = recipes.iter().flat_map(|r| std::iter::once(r.item_id).chain(r.reagents.iter().map(|g| g.item_id))).collect();
    let mut facts: BTreeMap<u32, ItemFacts> = BTreeMap::new();
    each_row(&t.item_sparse, "ItemSparse", ["ID", "Display_lang", "OverallQualityID", "InventoryType"], |[id, name, quality, slot]| {
        let id = num(id)?;
        if wanted.contains(&id) {
            let quality = Quality::from_id(num(quality)?).unwrap_or_default();
            facts.insert(id, ItemFacts { name: name.to_owned(), quality, slot: Slot::from_inventory_type(num(slot)?) });
        }
        Ok(())
    })?;
    let icon = icons(&t.icons, &wanted)?;
    let unknown = |id: u32| ItemFacts { name: format!("Item #{id}"), quality: Quality::Common, slot: None };
    for r in &mut recipes {
        let f = facts.get(&r.item_id).cloned().unwrap_or_else(|| unknown(r.item_id));
        (r.item_name, r.quality, r.slot) = (f.name, f.quality, f.slot);
        r.icon = icon.get(&r.item_id).copied();
        for g in &mut r.reagents {
            let f = facts.get(&g.item_id).cloned().unwrap_or_else(|| unknown(g.item_id));
            (g.item_name, g.quality) = (f.name, f.quality);
            g.icon = icon.get(&g.item_id).copied();
        }
    }
    Ok(recipes)
}

#[derive(Clone)]
struct ItemFacts {
    name: String,
    quality: Quality,
    slot: Option<Slot>,
}

/// Each wanted item's icon: its own, else its base appearance's, as the client resolves it.
/// Forever leaves `Item.IconFileDataID` 0 on most of its added armor.
pub fn icons(t: &IconTables, wanted: &BTreeSet<u32>) -> Result<HashMap<u32, FileDataId>, String> {
    let mut icons: HashMap<u32, FileDataId> = HashMap::new();
    each_row(&t.item, "Item", ["ID", "IconFileDataID"], |[id, icon]| {
        let (id, icon) = (num(id)?, num(icon)?);
        if wanted.contains(&id) && icon != 0 {
            icons.insert(id, FileDataId(icon));
        }
        Ok(())
    })?;
    let mut appearance: HashMap<u32, ((u32, u32), u32)> = HashMap::new();
    each_row(&t.item_modified_appearance, "ItemModifiedAppearance", ["ItemID", "ItemAppearanceModifierID", "OrderIndex", "ItemAppearanceID"], |[item, modifier, order, id]| {
        let item = num(item)?;
        if wanted.contains(&item) && !icons.contains_key(&item) {
            let candidate = ((num(modifier)?, num(order)?), num(id)?);
            let slot = appearance.entry(item).or_insert(candidate);
            if candidate.0 < slot.0 {
                *slot = candidate;
            }
        }
        Ok(())
    })?;
    let mut appearance_icon: HashMap<u32, FileDataId> = HashMap::new();
    let used: BTreeSet<u32> = appearance.values().map(|&(_, id)| id).collect();
    each_row(&t.item_appearance, "ItemAppearance", ["ID", "DefaultIconFileDataID"], |[id, icon]| {
        let (id, icon) = (num(id)?, num(icon)?);
        if used.contains(&id) && icon != 0 {
            appearance_icon.insert(id, FileDataId(icon));
        }
        Ok(())
    })?;
    for (item, (_, id)) in appearance {
        if let Some(&icon) = appearance_icon.get(&id) {
            icons.insert(item, icon);
        }
    }
    Ok(icons)
}

pub fn each_row<const N: usize>(text: &str, table: &str, cols: [&str; N], mut f: impl FnMut([&str; N]) -> Result<(), String>) -> Result<(), String> {
    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let header = reader.headers().map_err(|e| format!("{table} header: {e}"))?.clone();
    let mut at = [0usize; N];
    for (slot, col) in at.iter_mut().zip(cols) {
        *slot = header.iter().position(|h| h == col).ok_or_else(|| format!("{table} has no {col} column"))?;
    }
    for record in reader.records() {
        let record = record.map_err(|e| format!("{table}: {e}"))?;
        f(at.map(|i| record.get(i).unwrap_or_default())).map_err(|e| format!("{table}: {e}"))?;
    }
    Ok(())
}

pub fn num<T: FromStr>(s: &str) -> Result<T, String> {
    s.trim().parse().map_err(|_| format!("not a number: {s:?}"))
}

/// Bump when `Recipe` changes shape, so a file an older version wrote is never read.
const CACHE_SCHEMA: &str = "v4";

#[derive(Serialize, Deserialize)]
struct Cache {
    build: String,
    profession: String,
    recipes: Vec<Recipe>,
}

/// Where downloads and joined data are kept between runs.
pub fn cache_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("tsm-crafts"))
}

fn cache_path(profession: &Profession) -> Option<PathBuf> {
    Some(cache_dir()?.join(format!("recipes-{CACHE_SCHEMA}-{}-{BUILD}.json", profession.name.to_lowercase())))
}

/// Recipes from the cache, or downloaded, joined and cached when the cache lacks this build.
pub fn load(profession: &Profession) -> Result<Vec<Recipe>, String> {
    let path = cache_path(profession);
    let cached = path.as_ref().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<Cache>(&b).ok());
    if let Some(cache) = cached.filter(|c| c.build == BUILD && c.profession == profession.name) {
        return Ok(cache.recipes);
    }
    let tables = Tables {
        skill_line_ability: fetch("SkillLineAbility")?,
        spell_effect: fetch("SpellEffect")?,
        spell_reagents: fetch("SpellReagents")?,
        item_sparse: fetch("ItemSparse")?,
        icons: IconTables::fetch()?,
    };
    let recipes = join(&tables, profession)?;
    if let Some(path) = path {
        let cache = Cache { build: BUILD.into(), profession: profession.name.into(), recipes };
        if let Err(e) = write_json(&path, &cache) {
            eprintln!("recipe cache not written to {}: {e}", path.display());
        }
        return Ok(cache.recipes);
    }
    Ok(recipes)
}

pub fn write_json(path: &std::path::Path, value: &impl Serialize) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().expect("cache path has a parent"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(value)?)?;
    std::fs::rename(tmp, path)
}

pub fn fetch(table: &str) -> Result<String, String> {
    let url = format!("https://wago.tools/db2/{table}/csv?build={BUILD}");
    let mut response = ureq::get(&url).call().map_err(|e| format!("{url}: {e}"))?;
    response.body_mut().with_config().limit(64 << 20).read_to_string().map_err(|e| format!("{url}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> Tables {
        Tables {
            skill_line_ability: "ID,SkillLine,Spell,MinSkillLineRank,TrivialSkillLineRankHigh,TrivialSkillLineRankLow\n\
                                 1,165,3818,1,135,125\n\
                                 2,2945,19047,1,270,250\n\
                                 3,164,2660,1,30,1\n\
                                 4,165,9999,1,40,20\n\
                                 5,165,7777,1,40,20\n\
                                 6,165,44953,1,0,0\n"
                .into(),
            spell_effect: "ID,EffectIndex,Effect,EffectItemType,EffectBasePointsF,SpellID\n\
                           10,0,24,4234,1,3818\n\
                           11,1,24,15407,2,19047\n\
                           12,0,6,0,5,19047\n\
                           13,0,24,2862,1,2660\n\
                           14,0,24,1234,1,9999\n\
                           15,0,24,8170,1,44953\n"
                .into(),
            spell_reagents: "ID,SpellID,Reagent_0,Reagent_1,Reagent_2,Reagent_3,Reagent_4,Reagent_5,Reagent_6,Reagent_7,ReagentCount_0,ReagentCount_1,ReagentCount_2,ReagentCount_3,ReagentCount_4,ReagentCount_5,ReagentCount_6,ReagentCount_7\n\
                             1,3818,2319,0,0,0,0,0,0,0,5,0,0,0,0,0,0,0\n\
                             2,19047,8171,15409,0,0,0,0,0,0,1,1,0,0,0,0,0,0\n\
                             3,7777,2319,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0\n\
                             4,44953,2319,0,0,0,0,0,0,0,3,0,0,0,0,0,0,0\n"
                .into(),
            item_sparse: "ID,Description_lang,Display_lang,InventoryType,OverallQualityID\n\
                          4234,,\"Heavy Leather\",0,1\n\
                          2319,\"Soft, supple\",\"Medium Leather\",0,1\n\
                          15407,,\"Cured Rugged Hide\",16,3\n\
                          8171,,\"Rugged Hide\",0,2\n\
                          8170,,\"Light Leather\",0,9\n"
                .into(),
            icons: icon_tables(),
        }
    }

    fn icon_tables() -> IconTables {
        IconTables {
            item: "ID,ClassID,IconFileDataID\n\
                   4234,7,134256\n\
                   2319,7,134254\n\
                   15407,7,0\n\
                   8171,7,134257\n\
                   8170,7,134251\n"
                .into(),
            item_modified_appearance: "ID,ItemID,ItemAppearanceModifierID,ItemAppearanceID,OrderIndex\n\
                                       1,15407,0,501,1\n\
                                       2,15407,0,500,0\n\
                                       3,15407,1,503,0\n\
                                       4,4234,0,502,0\n"
                .into(),
            item_appearance: "ID,DisplayType,DefaultIconFileDataID\n\
                              500,9,134255\n\
                              501,9,999\n\
                              502,9,777\n\
                              503,9,888\n"
                .into(),
        }
    }

    #[test]
    fn joins_create_item_effects_with_reagents_icons_and_ranks_for_the_profession() {
        let recipes = join(&tables(), &LEATHERWORKING).unwrap();
        let reagent = |item_id, name: &str, quality, icon: Option<u32>, count| Reagent { item_id, item_name: name.into(), quality, icon: icon.map(FileDataId), count };
        assert_eq!(recipes, vec![
            Recipe {
                spell_id: 3818,
                item_id: 4234,
                item_name: "Heavy Leather".into(),
                quality: Quality::Common,
                slot: None,
                icon: Some(FileDataId(134256)),
                quantity: 1.0,
                skill: Some(Skill { learn: 125, grey: 135 }),
                reagents: vec![reagent(2319, "Medium Leather", Quality::Common, Some(134254), 5)],
            },
            Recipe {
                spell_id: 19047,
                item_id: 15407,
                item_name: "Cured Rugged Hide".into(),
                quality: Quality::Rare,
                slot: Some(Slot::Cloak),
                icon: Some(FileDataId(134255)),
                quantity: 2.0,
                skill: Some(Skill { learn: 250, grey: 270 }),
                reagents: vec![reagent(8171, "Rugged Hide", Quality::Uncommon, Some(134257), 1), reagent(15409, "Item #15409", Quality::Common, None, 1)],
            },
            Recipe {
                spell_id: 44953,
                item_id: 8170,
                item_name: "Light Leather".into(),
                quality: Quality::Common,
                slot: None,
                icon: Some(FileDataId(134251)),
                quantity: 1.0,
                skill: None,
                reagents: vec![reagent(2319, "Medium Leather", Quality::Common, Some(134254), 3)],
            },
        ]);
    }

    #[test]
    fn quality_and_slot_ids_map_as_the_client_names_them() {
        assert_eq!([0, 2, 4, 7, 8].map(Quality::from_id), [Some(Quality::Poor), Some(Quality::Uncommon), Some(Quality::Epic), Some(Quality::Heirloom), None]);
        let slot = |id| Slot::from_inventory_type(id).map(Slot::name);
        assert_eq!([16, 20, 10, 18, 0, 29].map(slot), [Some("Cloak"), Some("Chest"), Some("Hands"), Some("Bag"), None, None]);
    }

    #[test]
    fn a_missing_column_names_its_table() {
        let mut t = tables();
        t.spell_effect = "ID,SpellID\n1,3818\n".into();
        assert_eq!(join(&t, &LEATHERWORKING), Err("SpellEffect has no Effect column".into()));
    }
}
