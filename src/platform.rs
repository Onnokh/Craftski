//! What the desktop and the browser do differently below the loaders: the clock.

use std::time::{SystemTime, UNIX_EPOCH};

/// The wall clock. Every reading of the time goes through here.
pub fn now() -> SystemTime {
    SystemTime::now()
}

/// Seconds since the Unix epoch at `t`, 0 before it.
pub fn unix(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub fn unix_now() -> u64 {
    unix(now())
}
