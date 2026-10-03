//! Parse grid datetime cells and format the hover card lines.

use time::format_description::well_known::Rfc3339;
use time::{Month, OffsetDateTime, UtcOffset};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DateTimeHover {
    pub timestamp: String,
    pub utc: String,
    pub local_label: String,
    pub local: String,
    pub relative: String,
}

impl DateTimeHover {
    pub fn rows(&self) -> [(String, String); 4] {
        [
            ("Timestamp".into(), self.timestamp.clone()),
            ("UTC".into(), self.utc.clone()),
            (self.local_label.clone(), self.local.clone()),
            ("Relative".into(), self.relative.clone()),
        ]
    }
}

pub fn hover_card(raw: &str) -> Option<DateTimeHover> {
    let dt = parse_datetime_cell(raw)?;
    Some(format_hover_card(
        dt,
        OffsetDateTime::now_utc(),
        local_utc_offset(),
        &local_tz_name(),
    ))
}

pub fn parse_datetime_cell(raw: &str) -> Option<OffsetDateTime> {
    let s = raw.trim();
    if s.is_empty() || is_non_instant(s) {
        return None;
    }
    if let Ok(dt) = OffsetDateTime::parse(s, &Rfc3339) {
        return Some(dt);
    }
    let normalized = normalize_rfc3339(s)?;
    OffsetDateTime::parse(&normalized, &Rfc3339).ok()
}

pub fn format_hover_card(
    dt: OffsetDateTime,
    now: OffsetDateTime,
    local_offset: UtcOffset,
    tz_name: &str,
) -> DateTimeHover {
    let utc = dt.to_offset(UtcOffset::UTC);
    let local = dt.to_offset(local_offset);
    DateTimeHover {
        timestamp: unix_millis(dt).to_string(),
        utc: format_wall(utc),
        local_label: tz_name.to_string(),
        local: format_wall(local),
        relative: format_relative(dt, now),
    }
}

fn unix_millis(dt: OffsetDateTime) -> i128 {
    dt.unix_timestamp_nanos() / 1_000_000
}

fn format_wall(dt: OffsetDateTime) -> String {
    format!(
        "{} {:02}, {} {:02}:{:02}:{:02}",
        month_short(dt.month()),
        dt.day(),
        dt.year(),
        dt.hour(),
        dt.minute(),
        dt.second()
    )
}

fn month_short(month: Month) -> &'static str {
    match month {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sep",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    }
}

pub fn format_relative(then: OffsetDateTime, now: OffsetDateTime) -> String {
    let secs = (now - then).whole_seconds();
    let past = secs >= 0;
    let abs = secs.unsigned_abs();
    if abs < 5 {
        return "just now".into();
    }
    let (n, unit) = if abs < 60 {
        (abs, "second")
    } else if abs < 3_600 {
        (abs / 60, "minute")
    } else if abs < 86_400 {
        (abs / 3_600, "hour")
    } else if abs < 86_400 * 30 {
        (abs / 86_400, "day")
    } else if abs < 86_400 * 365 {
        (abs / (86_400 * 30), "month")
    } else {
        (abs / (86_400 * 365), "year")
    };
    let unit = if n == 1 {
        unit.to_string()
    } else {
        format!("{unit}s")
    };
    if past {
        format!("{n} {unit} ago")
    } else {
        format!("in {n} {unit}")
    }
}

fn is_non_instant(s: &str) -> bool {
    s.eq_ignore_ascii_case("null")
        || s.eq_ignore_ascii_case("infinity")
        || s.eq_ignore_ascii_case("-infinity")
}

fn normalize_rfc3339(s: &str) -> Option<String> {
    let (date, rest) = s
        .split_once('T')
        .or_else(|| s.split_once('t'))
        .or_else(|| s.split_once(' '))?;
    if date.len() != 10 || rest.len() < 8 {
        return None;
    }
    let clock = &rest[..8];
    let mut tail = &rest[8..];
    let mut frac = String::new();
    if let Some(after_dot) = tail.strip_prefix('.') {
        let digits = after_dot.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        let mut raw_frac = after_dot[..digits].to_string();
        if raw_frac.len() > 9 {
            raw_frac.truncate(9);
        }
        frac = format!(".{raw_frac}");
        tail = &after_dot[digits..];
    }
    let offset = normalize_offset(tail)?;
    Some(format!("{date}T{clock}{frac}{offset}"))
}

fn normalize_offset(s: &str) -> Option<String> {
    if s.is_empty() {
        return Some("Z".into());
    }
    if s.eq_ignore_ascii_case("z") {
        return Some("Z".into());
    }
    let (sign, rest) = match s.as_bytes().first().copied() {
        Some(b'+') => ('+', &s[1..]),
        Some(b'-') => ('-', &s[1..]),
        _ => return None,
    };
    let (h, m) = match rest.len() {
        2 if rest.bytes().all(|c| c.is_ascii_digit()) => (rest, "00"),
        4 if rest.bytes().all(|c| c.is_ascii_digit()) => (&rest[..2], &rest[2..]),
        5 if rest.as_bytes().get(2) == Some(&b':') => (&rest[..2], &rest[3..]),
        8 if rest.as_bytes().get(2) == Some(&b':') && rest.as_bytes().get(5) == Some(&b':') => {
            (&rest[..2], &rest[3..5])
        }
        _ => return None,
    };
    if !h.bytes().all(|c| c.is_ascii_digit()) || !m.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{sign}{h}:{m}"))
}

fn local_utc_offset() -> UtcOffset {
    UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC)
}

fn local_tz_name() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "Local".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::{Duration, Month};

    fn utc(year: i32, month: Month, day: u8, hour: u8, minute: u8, second: u8) -> OffsetDateTime {
        time::Date::from_calendar_date(year, month, day)
            .unwrap()
            .with_hms(hour, minute, second)
            .unwrap()
            .assume_utc()
    }

    #[test]
    fn hover_card_formats_timestamp_utc_local_relative() {
        let dt = utc(2026, Month::October, 3, 16, 36, 15);
        let now = dt + Duration::hours(2);
        let offset = UtcOffset::from_hms(5, 30, 0).unwrap();
        let card = format_hover_card(dt, now, offset, "Asia/Calcutta");

        assert_eq!(card.timestamp, "1791045375000");
        assert_eq!(card.utc, "Oct 03, 2026 16:36:15");
        assert_eq!(card.local_label, "Asia/Calcutta");
        assert_eq!(card.local, "Oct 03, 2026 22:06:15");
        assert_eq!(card.relative, "2 hours ago");
        assert_eq!(
            card.rows().map(|(k, _)| k),
            ["Timestamp", "UTC", "Asia/Calcutta", "Relative"]
        );
    }

    #[test]
    fn format_relative_uses_minutes() {
        let then = utc(2026, Month::October, 3, 16, 3, 15);
        let now = utc(2026, Month::October, 3, 16, 36, 15);
        assert_eq!(format_relative(then, now), "33 minutes ago");
    }

    #[test]
    fn parse_postgres_timestamp_and_timestamptz() {
        let naive = parse_datetime_cell("2020-01-15 12:30:00").unwrap();
        assert_eq!(
            (
                naive.year(),
                naive.month(),
                naive.day(),
                naive.hour(),
                naive.minute()
            ),
            (2020, Month::January, 15, 12, 30)
        );
        let zoned = parse_datetime_cell("2020-01-15 12:30:00+00").unwrap();
        assert_eq!(zoned, naive);
        let rfc = parse_datetime_cell("2020-01-15T12:30:00.123456Z").unwrap();
        assert_eq!(unix_millis(rfc) % 1_000, 123);
        assert_eq!(rfc.to_offset(UtcOffset::UTC).hour(), 12);
    }

    #[test]
    fn parse_rejects_null_and_infinity() {
        assert!(parse_datetime_cell("NULL").is_none());
        assert!(parse_datetime_cell("infinity").is_none());
        assert!(parse_datetime_cell("not a date").is_none());
    }
}
