//! The player's choices that outlive a session, one small text file each under
//! `%LOCALAPPDATA%\tsm-crafts`: the profession shown, their rank in each profession and the
//! realm prices come from.

use std::path::PathBuf;

use crate::recipes::Profession;

/// The one rank saved before each profession had its own; it was always Leatherworking's.
const LEGACY_SKILL: &str = "skill.txt";
const REALM: &str = "realm.txt";
const PROFESSION: &str = "profession.txt";

fn path(file: &str) -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("tsm-crafts").join(file))
}

/// What `file` holds, trimmed, or blank when nothing is saved.
fn load(file: &str) -> String {
    path(file).and_then(|p| std::fs::read_to_string(p).ok()).map(|s| s.trim().to_string()).unwrap_or_default()
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
}
