//! What the app downloads once and keeps under `%LOCALAPPDATA%\Craftski`.

use std::path::{Path, PathBuf};

/// The folder the app keeps its downloads and settings in.
pub fn root() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Craftski"))
}

pub fn dir(name: &str) -> Option<PathBuf> {
    Some(root()?.join(name))
}

/// Moves the folder of the app's old name, TSM Crafts, to [`root`], so the player keeps their
/// settings and downloads. Does nothing once [`root`] exists.
pub fn adopt_old_folder() {
    if let Some(new) = root() {
        adopt(&new.with_file_name("tsm-crafts"), &new);
    }
}

fn adopt(old: &Path, new: &Path) {
    if !new.exists()
        && old.is_dir()
        && let Err(e) = std::fs::rename(old, new)
    {
        eprintln!("{} not moved to {}: {e}", old.display(), new.display());
    }
}

/// Writes beside `path` and renames over it, so a reader never sees half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_folder_moves_to_the_new_name_only_while_that_is_free() {
        let base = std::env::temp_dir().join(format!("craftski-adopt-{}", std::process::id()));
        let (old, new) = (base.join("tsm-crafts"), base.join("Craftski"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("faction.txt"), "Horde").unwrap();
        adopt(&old, &new);
        assert_eq!((old.exists(), std::fs::read_to_string(new.join("faction.txt")).unwrap()), (false, "Horde".to_string()));

        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("faction.txt"), "Alliance").unwrap();
        adopt(&old, &new);
        assert_eq!((old.exists(), std::fs::read_to_string(new.join("faction.txt")).unwrap()), (true, "Horde".to_string()));
        std::fs::remove_dir_all(base).unwrap();
    }
}
