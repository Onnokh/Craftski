//! Item icon JPEGs from Wowhead's CDN, named through wago.tools CASC and cached on disk so a
//! later run needs no network.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::cache::{self, write_atomic};
use crate::recipes::{FileDataId, Recipe};

const WORKERS: usize = 6;

#[derive(Default)]
pub struct Report {
    pub icons: usize,
    pub cached: usize,
    pub downloaded: usize,
    pub bytes: u64,
    pub failed: Vec<(FileDataId, String)>,
}

/// Every icon the recipes draw: crafted items and reagents.
pub fn referenced(recipes: &[Recipe]) -> BTreeSet<FileDataId> {
    recipes.iter().flat_map(|r| std::iter::once(r.icon).chain(r.reagents.iter().map(|g| g.icon))).flatten().collect()
}

pub fn dir() -> Option<PathBuf> {
    cache::dir("icons")
}

pub fn image_path(dir: &Path, id: FileDataId) -> PathBuf {
    dir.join(format!("{}.jpg", id.0))
}

fn names_path(dir: &Path) -> PathBuf {
    dir.join("names.json")
}

/// The 56 px rendition: rows draw icons at 22 pt, which is 44 px at scale 2.
fn image_url(name: &str) -> String {
    format!("https://wow.zamimg.com/images/wow/icons/large/{name}.jpg")
}

/// The icon name in a CASC download's `Content-Disposition`, as in
/// `attachment; filename=inv_misc_leatherscrap_07.blp`.
pub fn name_from_disposition(header: &str) -> Option<String> {
    let file = header.split(';').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        key.trim().eq_ignore_ascii_case("filename").then(|| value.trim().trim_matches('"'))
    })?;
    let (stem, ext) = file.rsplit_once('.')?;
    let safe = !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    (ext.eq_ignore_ascii_case("blp") && safe).then(|| stem.to_ascii_lowercase())
}

/// Puts every icon in `ids` on disk, calling `on_ready` from a worker thread as each one is
/// there, cached or downloaded. Blocks until every icon has landed or failed.
pub fn prefetch(ids: &BTreeSet<FileDataId>, on_ready: impl Fn(FileDataId, PathBuf) + Sync) -> Result<Report, String> {
    let dir = dir().ok_or("LOCALAPPDATA is not set")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut names: BTreeMap<u32, String> = std::fs::read(names_path(&dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let ids: Vec<FileDataId> = ids.iter().copied().collect();
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build().into();
    let next = AtomicUsize::new(0);
    let jobs: Vec<Job> = std::thread::scope(|scope| {
        let (ids, next, agent, dir, known, on_ready) = (&ids, &next, &agent, &dir, &names, &on_ready);
        let workers: Vec<_> = (0..WORKERS)
            .map(|_| {
                scope.spawn(move || {
                    let mut done = Vec::new();
                    while let Some(&id) = ids.get(next.fetch_add(1, Ordering::Relaxed)) {
                        let job = fetch_one(agent, dir, known, id);
                        if job.result.is_ok() {
                            on_ready(id, image_path(dir, id));
                        }
                        done.push(job);
                    }
                    done
                })
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect()
    });

    let mut report = Report { icons: ids.len(), ..Report::default() };
    let known = names.len();
    for job in jobs {
        if let Some(name) = job.resolved {
            names.insert(job.id.0, name);
        }
        match job.result {
            Ok(Got::Cached) => report.cached += 1,
            Ok(Got::Downloaded(n)) => {
                report.downloaded += 1;
                report.bytes += n;
            }
            Err(e) => report.failed.push((job.id, e)),
        }
    }
    if names.len() > known
        && let Err(e) = serde_json::to_vec(&names).map_err(std::io::Error::other).and_then(|b| write_atomic(&names_path(&dir), &b))
    {
        eprintln!("icon names not written to {}: {e}", dir.display());
    }
    report.failed.sort_by_key(|(id, _)| *id);
    Ok(report)
}

struct Job {
    id: FileDataId,
    /// A name this run looked up, to remember for the next.
    resolved: Option<String>,
    result: Result<Got, String>,
}

enum Got {
    Cached,
    Downloaded(u64),
}

fn fetch_one(agent: &ureq::Agent, dir: &Path, known: &BTreeMap<u32, String>, id: FileDataId) -> Job {
    let path = image_path(dir, id);
    if path.exists() {
        return Job { id, resolved: None, result: Ok(Got::Cached) };
    }
    let (name, resolved) = match known.get(&id.0) {
        Some(name) => (name.clone(), None),
        None => match resolve_name(agent, id) {
            Ok(name) => (name.clone(), Some(name)),
            Err(e) => return Job { id, resolved: None, result: Err(e) },
        },
    };
    let result = download(agent, &name).and_then(|bytes| {
        write_atomic(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Got::Downloaded(bytes.len() as u64))
    });
    Job { id, resolved, result }
}

fn resolve_name(agent: &ureq::Agent, id: FileDataId) -> Result<String, String> {
    let url = format!("https://wago.tools/api/casc/{}?download", id.0);
    let response = agent.head(&url).call().map_err(|e| format!("{url}: {e}"))?;
    let header = response.headers().get("content-disposition").and_then(|v| v.to_str().ok()).ok_or_else(|| format!("{url}: no content-disposition"))?;
    name_from_disposition(header).ok_or_else(|| format!("{url}: no .blp file name in {header:?}"))
}

fn download(agent: &ureq::Agent, name: &str) -> Result<Vec<u8>, String> {
    let url = image_url(name);
    let mut response = agent.get(&url).call().map_err(|e| format!("{url}: {e}"))?;
    let bytes = response.body_mut().with_config().limit(1 << 20).read_to_vec().map_err(|e| format!("{url}: {e}"))?;
    if !bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Err(format!("{url}: not a JPEG"));
    }
    Ok(bytes)
}

/// Icons the game names directly, such as the rail's `trade_*` profession icons, cached as
/// `<name>.jpg` beside the numbered ones. Fetched one after another: there are only a few.
pub fn fetch_named(names: &[&str]) -> Vec<(String, Result<PathBuf, String>)> {
    let Some(dir) = dir() else { return names.iter().map(|n| (n.to_string(), Err("LOCALAPPDATA is not set".into()))).collect() };
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build().into();
    names
        .iter()
        .map(|&name| {
            let path = named_path(&dir, name);
            let result = if path.exists() {
                Ok(path)
            } else {
                std::fs::create_dir_all(&dir)
                    .map_err(|e| format!("{}: {e}", dir.display()))
                    .and_then(|()| download(&agent, name))
                    .and_then(|bytes| write_atomic(&path, &bytes).map_err(|e| format!("{}: {e}", path.display())))
                    .map(|()| path)
            };
            (name.to_string(), result)
        })
        .collect()
}

fn named_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.jpg"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::{Quality, Reagent};

    #[test]
    fn names_an_icon_from_its_content_disposition() {
        assert_eq!(name_from_disposition("attachment; filename=inv_misc_leatherscrap_07.blp"), Some("inv_misc_leatherscrap_07".into()));
        assert_eq!(name_from_disposition("attachment; FileName=\"INV_Boots_05.BLP\""), Some("inv_boots_05".into()));
        assert_eq!(name_from_disposition("attachment"), None);
        assert_eq!(name_from_disposition("attachment; filename=inv_boots_05.png"), None);
        assert_eq!(name_from_disposition("attachment; filename=../inv_boots_05.blp"), None);
        assert_eq!(name_from_disposition("attachment; filename=.blp"), None);
    }

    #[test]
    fn caches_each_icon_as_its_file_data_id() {
        assert_eq!(image_path(Path::new("cache/icons"), FileDataId(134256)), Path::new("cache/icons/134256.jpg"));
        assert_eq!(names_path(Path::new("cache/icons")), Path::new("cache/icons/names.json"));
        assert_eq!(named_path(Path::new("cache/icons"), "trade_tailoring"), Path::new("cache/icons/trade_tailoring.jpg"));
        assert_eq!(image_url("inv_misc_leatherscrap_07"), "https://wow.zamimg.com/images/wow/icons/large/inv_misc_leatherscrap_07.jpg");
    }

    #[test]
    fn references_crafted_and_reagent_icons_once_each() {
        let reagent = |item_id, icon: Option<u32>| Reagent { item_id, item_name: String::new(), quality: Quality::Common, icon: icon.map(FileDataId), count: 1 };
        let recipe = |spell_id, icon: Option<u32>, reagents| Recipe {
            spell_id,
            item_id: spell_id,
            item_name: String::new(),
            quality: Quality::Common,
            slot: None,
            icon: icon.map(FileDataId),
            quantity: 1.0,
            skill: None,
            reagents,
        };
        let recipes = [recipe(1, Some(134256), vec![reagent(2319, Some(134254)), reagent(9, None)]), recipe(2, None, vec![reagent(2319, Some(134254)), reagent(4304, Some(134257))])];
        assert_eq!(referenced(&recipes), BTreeSet::from([FileDataId(134254), FileDataId(134256), FileDataId(134257)]));
    }
}
