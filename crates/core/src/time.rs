//! Timestamps: stored as RFC 3339 UTC with milliseconds, shown in local time.

use chrono::{DateTime, Local, SecondsFormat, Utc};

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn now_millis() -> i64 {
    Utc::now().timestamp_millis()
}

fn parse(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value.trim()).ok().map(|d| d.with_timezone(&Utc))
}

/// `2026.11.11` in local time; 日期待补 when the value is not a timestamp.
pub fn date_text(value: &str) -> String {
    match parse(value) {
        Some(d) => d.with_timezone(&Local).format("%Y.%m.%d").to_string(),
        None => "日期待补".into(),
    }
}

/// `11-11 09:30` in local time.
pub fn short_time(value: &str) -> String {
    parse(value).map(|d| d.with_timezone(&Local).format("%m-%d %H:%M").to_string()).unwrap_or_default()
}

/// Milliseconds since the epoch, 0 when unparsable.
pub fn millis(value: &str) -> i64 {
    parse(value).map(|d| d.timestamp_millis()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let now = now_iso();
        assert!(millis(&now) > 0);
        assert_eq!(date_text("nope"), "日期待补");
        assert_eq!(date_text(&now).len(), 10);
    }
}
