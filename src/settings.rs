//! The player's choices that outlive a session, one small text file each under
//! `%LOCALAPPDATA%\tsm-crafts`: their profession rank and the realm prices come from.

use std::path::PathBuf;

const SKILL: &str = "skill.txt";
const REALM: &str = "realm.txt";

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

/// The saved rank as the player typed it, or blank when none is saved.
pub fn load_skill() -> String {
    load(SKILL)
}

pub fn save_skill(text: &str) {
    save(SKILL, text);
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
    use super::parse_skill;

    #[test]
    fn only_a_whole_number_sets_a_rank() {
        assert_eq!((parse_skill(" 225 "), parse_skill(""), parse_skill("22a"), parse_skill("-5")), (Some(225), None, None, None));
    }
}
