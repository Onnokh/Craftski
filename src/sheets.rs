//! What an item's game tooltip says under its name, from foreverdb's item data: binding, slot,
//! damage, armor, stats, requirements and effects, in the order the game prints them.

use std::collections::HashMap;

use serde::Deserialize;

use crate::foreverdb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Gold,
    Green,
    Red,
}

/// A tooltip line; `right` is the column the game sets flush right, as an armor's type.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub right: Option<String>,
    pub tone: Tone,
}

fn line(tone: Tone, text: impl Into<String>) -> Line {
    Line { text: text.into(), right: None, tone }
}

/// One entry of foreverdb's `items.json`; fields a crafted or vendor item never shows are left out.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Sheet {
    id: u32,
    il: u32,
    rl: u32,
    c: u8,
    sc: i16,
    inv: u8,
    b: u8,
    qs: Option<u32>,
    u: u8,
    un: u32,
    ue: u8,
    /// Unique by family: its name, how many, and whether only equipped ones count.
    lim: Option<(String, u32, u8)>,
    slots: u32,
    /// Low, high and school.
    dmg: Option<(u32, u32, u8)>,
    /// Milliseconds between swings.
    spd: u32,
    ar: i32,
    bar: i32,
    st: Vec<(u16, i32)>,
    /// Class bits, signed: Blizzard stores a mask of most classes as a negative number.
    cls: i32,
    side: Option<String>,
    /// Faction id, rank and faction name.
    rep: Option<(u32, usize, String)>,
    /// Trigger, spell id and what the spell does.
    sp: Vec<(u8, u32, Option<String>)>,
    d: Option<String>,
}

#[derive(Deserialize)]
struct Meta {
    sub: HashMap<String, String>,
}

#[derive(Deserialize)]
struct Rating {
    per: f64,
}

pub struct Sheets {
    items: HashMap<u32, Sheet>,
    stats: HashMap<u16, String>,
    /// Item subclass names keyed `"{class}.{subclass}"`.
    subclasses: HashMap<String, String>,
    /// Rating points per percent.
    ratings: HashMap<u16, f64>,
}

/// Blocks on file IO and, when the copies are missing or old, the network.
pub fn load() -> Result<Sheets, String> {
    parse(&foreverdb::fetch("items.json")?, &foreverdb::fetch("stats.json")?, &foreverdb::fetch("itemmeta.json")?, &foreverdb::fetch("ratings.json")?)
}

pub fn parse(items: &str, stats: &str, meta: &str, ratings: &str) -> Result<Sheets, String> {
    let items: Vec<serde_json::Value> = serde_json::from_str(items).map_err(|e| format!("foreverdb items: {e}"))?;
    let meta: Meta = serde_json::from_str(meta).map_err(|e| format!("foreverdb item classes: {e}"))?;
    let ratings: HashMap<u16, Rating> = serde_json::from_str(ratings).map_err(|e| format!("foreverdb ratings: {e}"))?;
    Ok(Sheets {
        items: items.into_iter().filter_map(|v| serde_json::from_value::<Sheet>(v).ok()).map(|s| (s.id, s)).collect(),
        stats: serde_json::from_str(stats).map_err(|e| format!("foreverdb stats: {e}"))?,
        subclasses: meta.sub,
        ratings: ratings.into_iter().map(|(id, r)| (id, r.per)).collect(),
    })
}

const CLASSES: [(i32, &str); 9] = [(1, "Warrior"), (2, "Paladin"), (3, "Hunter"), (4, "Rogue"), (5, "Priest"), (7, "Shaman"), (8, "Mage"), (9, "Warlock"), (11, "Druid")];
const RANKS: [&str; 8] = ["Hated", "Hostile", "Unfriendly", "Neutral", "Friendly", "Honored", "Revered", "Exalted"];

fn slot(inv: u8) -> Option<&'static str> {
    Some(match inv {
        1 => "Head",
        2 => "Neck",
        3 => "Shoulder",
        4 => "Shirt",
        5 | 20 => "Chest",
        6 => "Waist",
        7 => "Legs",
        8 => "Feet",
        9 => "Wrist",
        10 => "Hands",
        11 => "Finger",
        12 => "Trinket",
        13 => "One-Hand",
        14 | 22 => "Off Hand",
        15 | 26 => "Ranged",
        16 => "Back",
        17 => "Two-Hand",
        19 => "Tabard",
        21 => "Main Hand",
        23 => "Held In Off-hand",
        24 => "Ammo",
        25 => "Thrown",
        27 => "Quiver",
        28 => "Relic",
        _ => return None,
    })
}

fn school(id: u8) -> &'static str {
    match id {
        1 => "Holy ",
        2 => "Fire ",
        3 => "Nature ",
        4 => "Frost ",
        5 => "Shadow ",
        6 => "Arcane ",
        _ => "",
    }
}

/// What a rating improves, before the percent it adds.
fn rating(stat: u16) -> Option<&'static str> {
    Some(match stat {
        16 | 31 => "Improves your chance to hit by",
        17 => "Improves your chance to hit with ranged weapons by",
        18 => "Improves your chance to hit with spells by",
        19 | 32 => "Improves your chance to get a critical strike by",
        20 => "Improves your chance to get a critical strike with ranged weapons by",
        21 => "Improves your chance to get a critical strike with spells by",
        13 => "Increases your chance to dodge an attack by",
        14 => "Increases your chance to parry an attack by",
        15 => "Increases your chance to block attacks with a shield by",
        28 => "Increases your melee attack speed by",
        29 => "Increases your ranged attack speed by",
        30 => "Increases your casting speed by",
        36 => "Increases your attack and casting speed by",
        44 => "Your attacks ignore armor equal to",
        _ => return None,
    })
}

fn signed(v: i32) -> String {
    if v < 0 { format!("\u{2212}{}", -v) } else { format!("+{v}") }
}

/// Up to two decimals, without trailing zeros: 1.50 is "1.5".
fn percent(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

impl Sheets {
    /// The lines under `item`'s name, empty for an item foreverdb lacks.
    pub fn lines(&self, item: u32) -> Vec<Line> {
        let Some(s) = self.items.get(&item) else { return Vec::new() };
        let sub = self.subclasses.get(&format!("{}.{}", s.c, s.sc));
        let mut out = Vec::new();
        if s.il > 1 && s.inv > 0 {
            out.push(line(Tone::Gold, format!("Item Level {}", s.il)));
        }
        let binding = match s.b {
            1 => "Binds when picked up",
            2 => "Binds when equipped",
            3 => "Binds when used",
            4 => "Quest Item",
            _ => "",
        };
        let mut plain_if = |on: bool, text: &str| {
            if on {
                out.push(line(Tone::Plain, text));
            }
        };
        plain_if(!binding.is_empty(), binding);
        plain_if(s.qs.is_some(), "This Item Begins a Quest");
        if s.u > 0 {
            out.push(line(Tone::Plain, "Unique"));
        } else if s.un > 0 {
            out.push(line(Tone::Plain, format!("Unique ({})", s.un)));
        }
        match &s.lim {
            Some((name, n, equipped)) => out.push(line(Tone::Plain, format!("{}: {name} ({n})", if *equipped > 0 { "Unique-Equipped" } else { "Unique" }))),
            None if s.ue > 0 => out.push(line(Tone::Plain, "Unique-Equipped")),
            None => {}
        }
        if let Some(name) = slot(s.inv) {
            let typed = s.c == 2 || (s.c == 4 && s.sc > 0 && s.inv != 16);
            out.push(Line { text: name.into(), right: typed.then(|| sub.cloned()).flatten(), tone: Tone::Plain });
        }
        if s.slots > 0 {
            out.push(line(Tone::Plain, format!("{} Slot {}", s.slots, sub.map_or("Bag", |x| x))));
        }
        if let Some((lo, hi, kind)) = s.dmg {
            let speed = f64::from(s.spd) / 1000.0;
            out.push(Line { text: format!("{lo} - {hi} {}Damage", school(kind)), right: Some(format!("Speed {speed:.2}")), tone: Tone::Plain });
            out.push(line(Tone::Plain, format!("({:.1} damage per second)", f64::from(lo + hi) / 2.0 / speed)));
        }
        if s.ar - s.bar > 0 {
            out.push(line(Tone::Plain, format!("{} Armor", s.ar - s.bar)));
        }
        let mut secondary = Vec::new();
        for &(stat, v) in &s.st {
            let name = self.stats.get(&stat).map_or_else(|| format!("Stat {stat}"), Clone::clone);
            match stat {
                3..=7 | 51..=56 => out.push(line(Tone::Plain, format!("{} {name}", signed(v)))),
                _ => secondary.push(self.secondary(stat, v, &name)),
            }
        }
        if s.bar > 0 {
            out.push(line(Tone::Green, format!("+{} Armor", s.bar)));
        }
        let classes: Vec<&str> = CLASSES.iter().filter(|(id, _)| s.cls & (1 << (id - 1)) != 0).map(|(_, name)| *name).collect();
        if !classes.is_empty() && classes.len() < CLASSES.len() {
            out.push(line(Tone::Plain, format!("Classes: {}", classes.join(", "))));
        }
        match s.side.as_deref() {
            Some("a") => out.push(line(Tone::Plain, "Alliance Only")),
            Some("h") => out.push(line(Tone::Plain, "Horde Only")),
            _ => {}
        }
        if s.rl > 1 {
            out.push(line(Tone::Plain, format!("Requires Level {}", s.rl)));
        }
        if let Some((_, rank, faction)) = &s.rep {
            out.push(line(Tone::Plain, format!("Requires {faction} - {}", RANKS.get(*rank).unwrap_or(&""))));
        }
        out.extend(secondary);
        for (trigger, _, text) in &s.sp {
            let Some(text) = text.as_deref().filter(|t| !t.is_empty()) else { continue };
            let when = match trigger {
                1 => "Equip",
                2 => "Chance on hit",
                _ => "Use",
            };
            out.push(line(Tone::Green, format!("{when}: {text}")));
        }
        if let Some(d) = &s.d {
            out.push(line(Tone::Gold, format!("\"{d}\"")));
        }
        out
    }

    /// A stat the game words as an Equip effect, rated stats as the percent they add.
    fn secondary(&self, stat: u16, v: i32, name: &str) -> Line {
        let text = match (self.ratings.get(&stat), rating(stat)) {
            (Some(per), Some(say)) => {
                let say = if v < 0 { say.replacen("Improves", "Decreases", 1).replacen("Increases", "Decreases", 1) } else { say.to_owned() };
                format!("Equip: {say} {}%.", percent(f64::from(v.abs()) / per))
            }
            _ => match stat {
                12 => format!("Equip: Increased Defense {}.", signed(v)),
                43 => format!("Equip: Restores {v} mana per 5 sec."),
                46 => format!("Equip: Restores {v} health per 5 sec."),
                _ => format!("Equip: {} {name}.", signed(v)),
            },
        };
        line(if v < 0 { Tone::Red } else { Tone::Green }, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATS: &str = r#"{"3":"Agility","4":"Strength","7":"Stamina","12":"Defense","31":"Hit","38":"Attack Power","43":"Mana per 5 sec"}"#;
    const META: &str = r#"{"classes":{"4":"Armor"},"sub":{"4.2":"Leather","2.7":"Sword","1.0":"Bag"}}"#;
    const RATINGS: &str = r#"{"31":{"per":10.0,"flat":true}}"#;

    fn sheets(items: &str) -> Sheets {
        parse(items, STATS, META, RATINGS).unwrap()
    }

    fn texts(lines: Vec<Line>) -> Vec<String> {
        lines.into_iter().map(|l| l.right.map_or_else(|| l.text.clone(), |r| format!("{} | {r}", l.text))).collect()
    }

    #[test]
    fn armor_reads_as_the_game_prints_it() {
        let s = sheets(r#"[{"id":18948,"n":"Barbaric Bracers","q":2,"il":32,"rl":27,"c":4,"sc":2,"inv":9,"b":2,"st":[[7,6],[4,4],[3,4]],"ar":47,"sell":1234}]"#);
        assert_eq!(texts(s.lines(18948)), ["Item Level 32", "Binds when equipped", "Wrist | Leather", "47 Armor", "+6 Stamina", "+4 Strength", "+4 Agility", "Requires Level 27"]);
        assert_eq!(s.lines(18948)[0].tone, Tone::Gold);
    }

    #[test]
    fn a_weapon_shows_its_damage_speed_and_effects_with_secondary_stats_after_the_requirements() {
        let s = sheets(
            r#"[{"id":1,"il":40,"rl":35,"c":2,"sc":7,"inv":13,"b":1,"u":1,"dmg":[40,75,0],"spd":2500,"st":[[31,10],[38,-14],[43,4]],"cls":9,
                 "rep":[529,5,"Argent Dawn"],"sp":[[2,99,"Wounds the target."],[0,7,null]],"d":"Sharp."}]"#,
        );
        let lines = s.lines(1);
        assert_eq!(
            texts(lines.clone()),
            [
                "Item Level 40",
                "Binds when picked up",
                "Unique",
                "One-Hand | Sword",
                "40 - 75 Damage | Speed 2.50",
                "(23.0 damage per second)",
                "Classes: Warrior, Rogue",
                "Requires Level 35",
                "Requires Argent Dawn - Honored",
                "Equip: Improves your chance to hit by 1%.",
                "Equip: \u{2212}14 Attack Power.",
                "Equip: Restores 4 mana per 5 sec.",
                "Chance on hit: Wounds the target.",
                "\"Sharp.\"",
            ]
        );
        assert_eq!(lines.iter().map(|l| l.tone).collect::<Vec<_>>()[9..], [Tone::Green, Tone::Red, Tone::Green, Tone::Green, Tone::Gold]);
    }

    #[test]
    fn a_bag_names_its_slots_and_an_unknown_item_has_no_lines() {
        let s = sheets(r#"[{"id":4,"il":25,"c":1,"sc":0,"inv":18,"slots":10}]"#);
        assert_eq!(texts(s.lines(4)), ["Item Level 25", "10 Slot Bag"]);
        assert_eq!(s.lines(5), vec![]);
    }

    #[test]
    fn a_negative_class_mask_names_its_classes_and_an_unreadable_item_leaves_the_rest() {
        let s = sheets(r#"[{"id":1,"inv":11,"cls":-1136},{"id":2,"inv":11,"cls":-1},{"id":3,"il":"sixty"}]"#);
        assert_eq!(texts(s.lines(1)), ["Finger", "Classes: Priest, Mage, Warlock"]);
        assert_eq!(texts(s.lines(2)), ["Finger"]);
        assert_eq!(s.lines(3), vec![]);
    }
}
