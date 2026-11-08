//! foreverdb.net's data files, kept for a week in the cache.

use std::path::Path;
use std::time::Duration;

use crate::recipes;

/// foreverdb grows as players open merchants and loot items, so an older copy is fetched again.
const MAX_AGE: Duration = Duration::from_secs(7 * 86_400);

/// `https://foreverdb.net/data/{file}`, from the cache while it is under a week old. A failed
/// download falls back to an older copy.
pub fn fetch(file: &str) -> Result<String, String> {
    let url = format!("https://foreverdb.net/data/{file}");
    let path = recipes::cache_dir().map(|d| d.join(format!("foreverdb-{file}")));
    let fresh = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age < MAX_AGE);
    if let Some(p) = path.as_deref().filter(|p| fresh(p))
        && let Ok(text) = std::fs::read_to_string(p)
    {
        return Ok(text);
    }
    let fetched = ureq::get(&url)
        .call()
        .map_err(|e| format!("{url}: {e}"))
        .and_then(|mut r| r.body_mut().with_config().limit(64 << 20).read_to_string().map_err(|e| format!("{url}: {e}")));
    match (fetched, path) {
        (Ok(text), Some(p)) => {
            if let Err(e) = std::fs::create_dir_all(p.parent().expect("cache path has a parent")).and_then(|()| std::fs::write(&p, &text)) {
                eprintln!("{file} not cached at {}: {e}", p.display());
            }
            Ok(text)
        }
        (Ok(text), None) => Ok(text),
        (Err(e), Some(p)) => std::fs::read_to_string(&p).map_err(|_| e.clone()).inspect(|_| eprintln!("{e}; using the older copy at {}", p.display())),
        (Err(e), None) => Err(e),
    }
}
