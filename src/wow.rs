//! Where this machine keeps TSM's AppData.lua for WoW Forever: the file the TSM desktop app
//! writes into the TradeSkillMaster_AppHelper addon of a Forever install.

use std::path::{Path, PathBuf};

use crate::settings;

/// The AppHelper file under a World of Warcraft folder, one folder per entry.
const UNDER_WOW: [&str; 5] = ["_forever_", "Interface", "AddOns", "TradeSkillMaster_AppHelper", "AppData.lua"];

/// Why the app reads AppData.lua from where it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// `--appdata PATH`.
    Flag,
    /// The `TSM_APPDATA` environment variable.
    Env,
    /// The folder the player chose, saved in settings.
    Chosen,
    /// A Forever install found without asking.
    Found,
}

impl Source {
    pub fn words(self) -> &'static str {
        match self {
            Source::Flag => "from --appdata",
            Source::Env => "from TSM_APPDATA",
            Source::Chosen => "chosen",
            Source::Found => "found automatically",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppData {
    pub path: PathBuf,
    pub source: Source,
}

/// The AppHelper file a picked folder leads to. Accepts the World of Warcraft folder, its
/// `_forever_` folder, `Interface`, `AddOns`, the `TradeSkillMaster_AppHelper` folder, or the
/// file itself; Some only when the file exists.
pub fn appdata_in(picked: &Path) -> Option<PathBuf> {
    if picked.is_file() {
        return (picked.file_name()? == UNDER_WOW[4]).then(|| picked.to_path_buf());
    }
    (0..UNDER_WOW.len()).map(|from| UNDER_WOW[from..].iter().fold(picked.to_path_buf(), |path, part| path.join(part))).find(|path| path.is_file())
}

/// A Forever install's AppData.lua found without asking, in the first of [`install_dirs`] that
/// has one.
pub fn find() -> Option<PathBuf> {
    install_dirs().iter().map(|wow| UNDER_WOW.iter().fold(wow.clone(), |path, part| path.join(part))).find(|path| path.is_file())
}

/// The World of Warcraft folders [`find`] looks in: the ones Blizzard's installer registered,
/// then `Program Files (x86)`, `Program Files`, the root and `Games` on each drive.
pub fn install_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for dir in registered().into_iter().chain(usual_dirs()) {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}

/// `--appdata`, else `TSM_APPDATA`, else the folder the player chose, else [`find`]. None when
/// nothing points at an existing file; the flag and the variable are taken as given even when
/// their file is missing.
pub fn locate() -> Option<AppData> {
    let args: Vec<String> = std::env::args().collect();
    let flag = args.iter().position(|a| a == "--appdata").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let given = flag.map(|path| AppData { path, source: Source::Flag }).or_else(|| std::env::var_os("TSM_APPDATA").map(|path| AppData { path: path.into(), source: Source::Env }));
    given
        .or_else(|| settings::load_appdata().filter(|path| path.is_file()).map(|path| AppData { path, source: Source::Chosen }))
        .or_else(|| find().map(|path| AppData { path, source: Source::Found }))
}

fn usual_dirs() -> impl Iterator<Item = PathBuf> {
    drives::local().into_iter().flat_map(|drive| [r"Program Files (x86)\World of Warcraft", r"Program Files\World of Warcraft", "World of Warcraft", r"Games\World of Warcraft"].map(|dir| drive.join(dir)))
}

/// The install folders in Blizzard's registry entries. Its `InstallPath` names a game folder
/// such as `_retail_` or `_forever_`, whose parent is the World of Warcraft folder.
fn registered() -> Vec<PathBuf> {
    const KEYS: [(&str, &str); 3] = [
        (r"SOFTWARE\WOW6432Node\Blizzard Entertainment\World of Warcraft", "InstallPath"),
        (r"SOFTWARE\Blizzard Entertainment\World of Warcraft", "InstallPath"),
        (r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\World of Warcraft", "InstallLocation"),
    ];
    KEYS.iter()
        .filter_map(|&(key, value)| registry::local_machine(key, value))
        .map(|path| {
            let path = PathBuf::from(path.trim_end_matches('\\'));
            let game = path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.len() > 2 && n.starts_with('_') && n.ends_with('_'));
            match path.parent() {
                Some(parent) if game => parent.to_path_buf(),
                _ => path,
            }
        })
        .collect()
}

#[cfg(windows)]
mod registry {
    use std::ffi::c_void;

    const HKEY_LOCAL_MACHINE: isize = 0x8000_0002_u32 as i32 as isize;
    const RRF_RT_REG_SZ: u32 = 0x2;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(key: isize, sub_key: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, bytes: *mut u32) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    /// A string value under HKEY_LOCAL_MACHINE, None when the key or value is missing.
    pub fn local_machine(key: &str, value: &str) -> Option<String> {
        let (key, value) = (wide(key), wide(value));
        let mut bytes = 0u32;
        // SAFETY: both names are NUL-terminated; a null data pointer asks only for the size.
        let sized = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut bytes) };
        if sized != 0 || bytes < 2 {
            return None;
        }
        let mut data = vec![0u16; bytes as usize / 2];
        // SAFETY: `data` holds `bytes` bytes, which RegGetValueW writes at most.
        let read = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), data.as_mut_ptr().cast(), &mut bytes) };
        if read != 0 {
            return None;
        }
        let text = String::from_utf16_lossy(&data[..bytes as usize / 2]);
        Some(text.trim_end_matches('\0').to_string()).filter(|t| !t.is_empty())
    }
}

#[cfg(windows)]
mod drives {
    use std::path::PathBuf;

    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLogicalDrives() -> u32;
        fn GetDriveTypeW(root: *const u16) -> u32;
    }

    /// The roots of this machine's own disks. Network drives are left out because a
    /// disconnected one stalls any look inside it, and this runs before the window opens.
    pub fn local() -> Vec<PathBuf> {
        // SAFETY: takes no arguments and returns a bitmask.
        let mask = unsafe { GetLogicalDrives() };
        (0..26u8)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| format!("{}:\\", char::from(b'A' + i)))
            .filter(|root| {
                let wide: Vec<u16> = root.encode_utf16().chain([0]).collect();
                // SAFETY: `wide` is a NUL-terminated root path.
                matches!(unsafe { GetDriveTypeW(wide.as_ptr()) }, DRIVE_REMOVABLE | DRIVE_FIXED)
            })
            .map(PathBuf::from)
            .collect()
    }
}

#[cfg(not(windows))]
mod drives {
    pub fn local() -> Vec<std::path::PathBuf> {
        Vec::new()
    }
}

#[cfg(not(windows))]
mod registry {
    pub fn local_machine(_: &str, _: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temp World of Warcraft folder with a Forever install whose AppHelper has synced, and a
    /// `_retail_` folder beside it with nothing in it.
    fn wow(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("tsm-crafts-wow-{name}-{}", std::process::id())).join("World of Warcraft");
        let helper = root.join(r"_forever_\Interface\AddOns\TradeSkillMaster_AppHelper");
        std::fs::create_dir_all(&helper).unwrap();
        std::fs::create_dir_all(root.join("_retail_")).unwrap();
        std::fs::write(helper.join("AppData.lua"), "").unwrap();
        root
    }

    #[test]
    fn any_folder_from_world_of_warcraft_down_to_the_apphelper_leads_to_its_file() {
        let root = wow("levels");
        let file = root.join(r"_forever_\Interface\AddOns\TradeSkillMaster_AppHelper\AppData.lua");
        for picked in ["", "_forever_", r"_forever_\Interface", r"_forever_\Interface\AddOns", r"_forever_\Interface\AddOns\TradeSkillMaster_AppHelper", r"_forever_\Interface\AddOns\TradeSkillMaster_AppHelper\AppData.lua"] {
            assert_eq!(appdata_in(&root.join(picked)), Some(file.clone()), "{picked}");
        }
        std::fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_folder_without_the_file_leads_nowhere() {
        let root = wow("empty");
        let other = root.join(r"_retail_\notes.txt");
        std::fs::write(&other, "").unwrap();
        assert_eq!((appdata_in(&root.join("_retail_")), appdata_in(&other), appdata_in(&root.join("missing"))), (None, None, None));
        std::fs::remove_file(root.join(r"_forever_\Interface\AddOns\TradeSkillMaster_AppHelper\AppData.lua")).unwrap();
        assert_eq!(appdata_in(&root), None);
        std::fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }
}
