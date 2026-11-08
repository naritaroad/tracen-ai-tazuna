//! In-memory rate limits for the MCP server (§6.6). Counters live only in memory: no IP address
//! is ever written to the database, and a restart clears them.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Limits {
    windows: Mutex<HashMap<String, (Instant, u32)>>,
}

impl Limits {
    /// Count one use of `key` in a window of `period`. `false` once `max` uses were already counted.
    pub fn allow(&self, key: &str, period: Duration, max: u32) -> bool {
        let now = Instant::now();
        let Ok(mut windows) = self.windows.lock() else { return false };
        if windows.len() > 50_000 {
            windows.retain(|_, (start, _)| now.duration_since(*start) < Duration::from_secs(86_400));
        }
        let entry = windows.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) >= period {
            *entry = (now, 0);
        }
        if entry.1 >= max {
            return false;
        }
        entry.1 += 1;
        true
    }
}

pub const MINUTE: Duration = Duration::from_secs(60);
pub const DAY: Duration = Duration::from_secs(86_400);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_at_the_limit_per_key() {
        let l = Limits::default();
        assert!(l.allow("ip:a", MINUTE, 2));
        assert!(l.allow("ip:a", MINUTE, 2));
        assert!(!l.allow("ip:a", MINUTE, 2));
        assert!(l.allow("ip:b", MINUTE, 2), "other keys keep their own count");
    }

    #[test]
    fn a_new_window_starts_over() {
        let l = Limits::default();
        assert!(l.allow("k", Duration::from_millis(1), 1));
        std::thread::sleep(Duration::from_millis(5));
        assert!(l.allow("k", Duration::from_millis(1), 1));
    }
}
