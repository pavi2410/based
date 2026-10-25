//! Wall-clock stamps on messages and how they read.

use std::time::{SystemTime, UNIX_EPOCH};

/// Unix milliseconds now.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// "just now", "5m ago", "2h ago", "3d ago".
pub fn ago(now: u64, then: u64) -> String {
    let secs = now.saturating_sub(then) / 1000;
    match secs {
        0..10 => "just now".to_owned(),
        10..60 => format!("{secs}s ago"),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86_400 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86_400),
    }
}

/// "800ms", "12s", "1m 5s", "2h 3m".
pub fn took(ms: u64) -> String {
    let secs = ms / 1000;
    match secs {
        0 => format!("{ms}ms"),
        1..60 => format!("{secs}s"),
        60..3600 => format!("{}m {}s", secs / 60, secs % 60),
        _ => format!("{}h {}m", secs / 3600, secs % 3600 / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_naturally() {
        assert_eq!(ago(5_000, 1_000), "just now");
        assert_eq!(ago(31_000, 1_000), "30s ago");
        assert_eq!(ago(301_000, 1_000), "5m ago");
        assert_eq!(ago(7_201_000, 1_000), "2h ago");
        assert_eq!(ago(3 * 86_400_000, 0), "3d ago");
        assert_eq!(ago(0, 5), "just now");
        assert_eq!(took(800), "800ms");
        assert_eq!(took(12_400), "12s");
        assert_eq!(took(65_000), "1m 5s");
        assert_eq!(took(7_380_000), "2h 3m");
    }
}
