//! Where the app reads TSM's prices: the AppData.lua in use and why, or where it looked, with a
//! button that points it at the World of Warcraft folder.

use rune::{El, FileDialogOptions, FileDialogResult, Tx, col, text};

use super::widgets::{FOLDER_OPEN, button};
use super::{App, Load};
use crate::settings;
use crate::theme::{self, p};
use crate::wow::{self, AppData, Source};

/// The main area while there is no AppData.lua to read.
pub fn not_found(s: &App) -> El {
    let panel = col()
        .w_full()
        .max_w(640.0)
        .gap(12.0)
        .child(text("Point TSM Crafts at your World of Warcraft folder").font(theme::HEADING).color(p().gold).wrap())
        .child(text("It reads the prices the TSM desktop app saves for WoW Forever, in the TradeSkillMaster_AppHelper addon.").font(theme::BODY).color(p().muted).wrap())
        .child(block(s).mt(8.0));
    col().grow(1.0).h_full().min_w(0.0).center().padding(24.0).child(panel)
}

/// The AppData.lua in use and how the app came to it, or where it looked for one; why the last
/// pick found nothing; and the button that asks for the folder.
pub fn block(s: &App) -> El {
    let missing = matches!(s.load, Load::Unlocated);
    let mut out = col().key("appdata").w_full().gap(4.0);
    out = match &s.appdata {
        Some(a) => {
            let how = if missing { format!("AppData.lua, {}, is missing", a.source.words()) } else { format!("AppData.lua, {}", a.source.words()) };
            out.child(caption(&how, p().muted)).child(text(a.path.display().to_string()).font(theme::BODY).color(p().text).wrap())
        }
        None => {
            let looked = s.searched.iter().map(|dir| caption(&dir.display().to_string(), p().faint));
            out.child(text("AppData.lua not found").font(theme::BODY).color(p().text)).child(caption("Looked for _forever_ in:", p().muted)).children(looked)
        }
    };
    if let Some(note) = &s.folder_note {
        out = out.child(text(note).font(theme::CAPTION).color(p().loss).wrap().mt(4.0));
    }
    let choose = button("Choose folder...", "Choose your World of Warcraft folder", &FOLDER_OPEN, |s: &mut App, tx: &mut Tx| s.choose_folder(tx)).w(184.0);
    out.child(col().pt(12.0).child(choose))
}

fn caption(words: &str, color: rune::Color) -> El {
    text(words).font(theme::CAPTION).color(color).wrap()
}

impl App {
    fn choose_folder(&mut self, tx: &mut Tx) {
        let options = FileDialogOptions { title: "Choose your World of Warcraft folder".into(), ..Default::default() };
        tx.open_folder(options, |s: &mut App, result, _| s.picked_folder(result));
    }

    /// Reads prices from the AppData.lua under a picked folder, remembered for next time, or
    /// says why the folder has none. A cancelled dialog changes nothing.
    pub(super) fn picked_folder(&mut self, result: FileDialogResult) {
        let picked = match result {
            Ok(Some(picked)) => picked,
            Ok(None) => return,
            Err(e) => {
                self.folder_note = Some(format!("The folder dialog did not open: {e}"));
                return;
            }
        };
        match wow::appdata_in(&picked) {
            Some(path) => {
                settings::save_appdata(&path);
                self.folder_note = None;
                self.appdata = Some(AppData { path, source: Source::Chosen });
                self.load_market();
            }
            None => {
                let shown = picked.display();
                self.folder_note = Some(format!("No TSM AppHelper data under {shown}. Pick the folder that holds _forever_, and make sure the TSM desktop app has synced once."));
            }
        }
    }
}
