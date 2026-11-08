//! The player's choices that outlive a session: for now only their profession rank.

use std::path::PathBuf;

fn skill_path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("tsm-crafts").join("skill.txt"))
}

/// The saved rank as the player typed it, or blank when none is saved.
pub fn load_skill() -> String {
    skill_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|s| s.trim().to_string()).unwrap_or_default()
}

pub fn save_skill(text: &str) {
    let Some(path) = skill_path() else { return };
    let saved = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&path, text.trim()));
    if let Err(e) = saved {
        eprintln!("skill not saved to {}: {e}", path.display());
    }
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
