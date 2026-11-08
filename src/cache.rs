//! What the app downloads once and keeps under `%LOCALAPPDATA%\tsm-crafts`.

use std::path::{Path, PathBuf};

pub fn dir(name: &str) -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("tsm-crafts").join(name))
}

/// Writes beside `path` and renames over it, so a reader never sees half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)
}
