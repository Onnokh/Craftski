//! What the desktop and the browser do differently below the loaders: the clock, and on the web
//! the one way a request leaves the page.

use std::time::{SystemTime, UNIX_EPOCH};

/// The wall clock. Every reading of the time goes through here.
#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> SystemTime {
    SystemTime::now()
}

/// std's clock panics on wasm32-unknown-unknown; the browser's is `Date.now`.
#[cfg(target_arch = "wasm32")]
pub fn now() -> SystemTime {
    UNIX_EPOCH + std::time::Duration::from_millis(js_sys::Date::now() as u64)
}

/// Seconds since the Unix epoch at `t`, 0 before it.
pub fn unix(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub fn unix_now() -> u64 {
    unix(now())
}

/// A response the browser received, whatever its status.
#[cfg(target_arch = "wasm32")]
pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
    headers: web_sys::Headers,
}

#[cfg(target_arch = "wasm32")]
impl Reply {
    pub fn header(&self, name: &str) -> Option<String> {
        self.headers.get(name).ok().flatten()
    }

    pub fn text(self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// GETs `url` with the browser's `fetch`. Err only when no response came back.
#[cfg(target_arch = "wasm32")]
pub async fn get(url: &str) -> Result<Reply, String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    let why = |e: wasm_bindgen::JsValue| format!("{url}: {}", e.dyn_ref::<js_sys::Error>().map_or_else(|| format!("{e:?}"), |e| String::from(e.message())));
    let window = web_sys::window().ok_or_else(|| format!("{url}: no window"))?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str(url)).await.map_err(why)?.unchecked_into();
    let body = JsFuture::from(response.array_buffer().map_err(why)?).await.map_err(why)?;
    Ok(Reply { status: response.status(), body: js_sys::Uint8Array::new(&body).to_vec(), headers: response.headers() })
}
