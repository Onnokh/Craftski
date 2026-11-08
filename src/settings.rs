//! The player's choices that outlive a session, one small text file each under
//! `%LOCALAPPDATA%\tsm-crafts`: the profession shown, their rank in each profession and the
//! realm prices come from, the AppData.lua they chose, and the character who buys from merchants.

use std::path::{Path, PathBuf};

use crate::flips::{Bartering, Rules};
use crate::merchants::Side;
use crate::recipes::Profession;

/// The one rank saved before each profession had its own; it was always Leatherworking's.
const LEGACY_SKILL: &str = "skill.txt";
const REALM: &str = "realm.txt";
const PROFESSION: &str = "profession.txt";
const APPDATA: &str = "appdata.txt";
const FACTION: &str = "faction.txt";
/// One faction name per line.
const HONORED: &str = "honored.txt";
/// Bartering's percent.
const BARTERING: &str = "bartering.txt";

fn path(file: &str) -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("tsm-crafts").join(file))
}

/// What `file` holds, trimmed, or None when nothing is saved.
fn saved(file: &str) -> Option<String> {
    path(file).and_then(|p| std::fs::read_to_string(p).ok()).map(|s| s.trim().to_string())
}

/// What `file` holds, trimmed, or blank when nothing is saved.
fn load(file: &str) -> String {
    saved(file).unwrap_or_default()
}

fn save(file: &str, text: &str) {
    let Some(path) = path(file) else { return };
    let saved = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&path, text.trim()));
    if let Err(e) = saved {
        eprintln!("not saved to {}: {e}", path.display());
    }
}

fn skill_file(profession: Profession) -> String {
    format!("skill-{}.txt", profession.slug())
}

/// The rank saved for `profession` as the player typed it, or blank when none is saved.
pub fn load_skill(profession: Profession) -> String {
    let file = skill_file(profession);
    let own = path(&file).is_some_and(|p| p.exists());
    load(if !own && profession == Profession::Leatherworking { LEGACY_SKILL } else { file.as_str() })
}

pub fn save_skill(profession: Profession, text: &str) {
    save(&skill_file(profession), text);
}

/// The profession last shown, Leatherworking when none is saved or the file names none.
pub fn load_profession() -> Profession {
    Profession::parse(&load(PROFESSION)).unwrap_or(Profession::Leatherworking)
}

pub fn save_profession(profession: Profession) {
    save(PROFESSION, profession.slug());
}

/// `--profession NAME`, else the profession last shown. Err when the flag names no profession.
pub fn start_profession() -> Result<Profession, String> {
    let args: Vec<String> = std::env::args().collect();
    let Some(at) = args.iter().position(|a| a == "--profession") else { return Ok(load_profession()) };
    let named = args.get(at + 1).map_or("", String::as_str);
    Profession::parse(named).ok_or_else(|| {
        let slugs: Vec<&str> = Profession::ALL.iter().map(|p| p.slug()).collect();
        format!("--profession {named:?} names no profession; use one of {}", slugs.join(", "))
    })
}

/// The realm last chosen, or blank when none is saved; prices read from the first realm in
/// AppData.lua when it names no realm by this name.
pub fn load_realm() -> String {
    load(REALM)
}

pub fn save_realm(realm: &str) {
    save(REALM, realm);
}

/// The AppData.lua found under the folder the player last chose, None when they never chose one.
pub fn load_appdata() -> Option<PathBuf> {
    Some(PathBuf::from(load(APPDATA))).filter(|p| !p.as_os_str().is_empty())
}

pub fn save_appdata(path: &Path) {
    save(APPDATA, &path.to_string_lossy());
}

/// The character last described, the default's choice for anything never saved.
pub fn load_rules() -> Rules {
    rules_from(saved)
}

pub fn save_rules(rules: &Rules) {
    for (file, text) in rules_files(rules) {
        save(file, &text);
    }
}

/// An honored file saved empty means Honored with no one, unlike a missing one.
fn rules_from(saved: impl Fn(&str) -> Option<String>) -> Rules {
    let default = Rules::default();
    Rules {
        side: saved(FACTION).and_then(|s| Side::named(&s)).unwrap_or(default.side),
        honored: saved(HONORED).map_or(default.honored, |s| s.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()),
        bartering: saved(BARTERING).and_then(|s| Bartering::ALL.into_iter().find(|b| b.percent().to_string() == s)).unwrap_or(default.bartering),
        listing_hours: default.listing_hours,
    }
}

fn rules_files(rules: &Rules) -> [(&'static str, String); 3] {
    [(FACTION, rules.side.name().to_string()), (HONORED, rules.honored.join("\n")), (BARTERING, rules.bartering.percent().to_string())]
}

/// The rank a typed value names; anything but a whole number means no rank.
pub fn parse_skill(text: &str) -> Option<u16> {
    text.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_whole_number_sets_a_rank() {
        assert_eq!((parse_skill(" 225 "), parse_skill(""), parse_skill("22a"), parse_skill("-5")), (Some(225), None, None, None));
    }

    #[test]
    fn each_profession_keeps_its_rank_in_its_own_file() {
        assert_eq!([Profession::FirstAid, Profession::Leatherworking].map(skill_file), ["skill-first-aid.txt", "skill-leatherworking.txt"]);
    }

    fn saved_as(files: &[(&'static str, &str)]) -> impl Fn(&str) -> Option<String> {
        let files: Vec<(&str, String)> = files.iter().map(|&(f, t)| (f, t.to_string())).collect();
        move |file| files.iter().find(|(f, _)| *f == file).map(|(_, t)| t.clone())
    }

    #[test]
    fn the_character_reads_back_as_saved() {
        let rules = Rules { side: Side::Alliance, honored: vec!["Darnassus".into(), "Stormwind".into()], bartering: Bartering::Five, listing_hours: 8 };
        let files = rules_files(&rules);
        assert_eq!(files, [("faction.txt", "Alliance".to_string()), ("honored.txt", "Darnassus\nStormwind".to_string()), ("bartering.txt", "5".to_string())]);
        let pairs: Vec<(&str, &str)> = files.iter().map(|(f, t)| (*f, t.as_str())).collect();
        assert_eq!(rules_from(saved_as(&pairs)), rules);
    }

    #[test]
    fn nothing_saved_is_the_default_character_and_an_empty_honored_file_is_no_one() {
        assert_eq!(rules_from(saved_as(&[])), Rules { side: Side::Horde, honored: vec!["Thunder Bluff".into()], bartering: Bartering::None, listing_hours: 8 });
        let unknown = rules_from(saved_as(&[("faction.txt", "Scourge"), ("honored.txt", ""), ("bartering.txt", "7")]));
        assert_eq!(unknown, Rules { side: Side::Horde, honored: vec![], bartering: Bartering::None, listing_hours: 8 });
    }
}
