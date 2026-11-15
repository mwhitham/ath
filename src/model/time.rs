//! Timestamp and calendar-date text.
//!
//! Stored instants use one exact form: `YYYY-MM-DDTHH:MM:SS.sssZ` in UTC with
//! three fractional digits. Calendar dates use `YYYY-MM-DD`. Both accept years
//! 0000 to 9999 and the Gregorian leap-year rule, as the published patterns do.
//! Text in these forms sorts in time order, so string comparison is safe.

/// The published pattern for a UTC timestamp with millisecond precision.
pub const UTC_TIMESTAMP_PATTERN: &str = r"^(?:(?:\d\d[2468][048]|\d\d[13579][26]|\d\d0[48]|[02468][048]00|[13579][26]00)-02-29|\d{4}-(?:(?:0[13578]|1[02])-(?:0[1-9]|[12]\d|3[01])|(?:0[469]|11)-(?:0[1-9]|[12]\d|30)|(?:02)-(?:0[1-9]|1\d|2[0-8])))T(?:(?:[01]\d|2[0-3]):[0-5]\d:[0-5]\d\.\d{3}(?:Z))$";

/// The published pattern for a calendar date.
pub const CALENDAR_DATE_PATTERN: &str = r"^(?:(?:\d\d[2468][048]|\d\d[13579][26]|\d\d0[48]|[02468][048]00|[13579][26]00)-02-29|\d{4}-(?:(?:0[13578]|1[02])-(?:0[1-9]|[12]\d|3[01])|(?:0[469]|11)-(?:0[1-9]|[12]\d|30)|(?:02)-(?:0[1-9]|1\d|2[0-8])))$";

fn digits(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(bytes.iter().fold(0, |total, digit| total * 10 + u32::from(digit - b'0')))
}

pub fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

pub fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Parse `YYYY-MM-DD` into its parts.
pub fn parse_calendar_date(text: &str) -> Option<(u32, u32, u32)> {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year = digits(&bytes[0..4])?;
    let month = digits(&bytes[5..7])?;
    let day = digits(&bytes[8..10])?;
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    Some((year, month, day))
}

pub fn is_calendar_date(text: &str) -> bool {
    parse_calendar_date(text).is_some()
}

/// Parse the stored UTC timestamp form into milliseconds since 1970.
pub fn parse_utc_timestamp(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() != 24
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'.'
        || bytes[23] != b'Z'
    {
        return None;
    }
    let (year, month, day) = parse_calendar_date(&text[0..10])?;
    let hour = digits(&bytes[11..13])?;
    let minute = digits(&bytes[14..16])?;
    let second = digits(&bytes[17..19])?;
    let millisecond = digits(&bytes[20..23])?;
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let days = days_from_civil(year as i64, month, day);
    Some(((days * 24 + hour as i64) * 60 + minute as i64) * 60_000 + second as i64 * 1000 + millisecond as i64)
}

pub fn is_utc_timestamp(text: &str) -> bool {
    parse_utc_timestamp(text).is_some()
}

/// Days from 1970-01-01 to a proleptic Gregorian date.
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = month as i64;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day as i64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The date for a day count from 1970-01-01.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 { month_index + 3 } else { month_index - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Format milliseconds since 1970 as the stored UTC timestamp form. This is
/// `Date.prototype.toISOString` for years 0000 to 9999.
pub fn format_utc_millis(millis: i64) -> String {
    let days = millis.div_euclid(86_400_000);
    let within = millis.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    let hour = within / 3_600_000;
    let minute = within / 60_000 % 60;
    let second = within / 1000 % 60;
    let millisecond = within % 1000;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millisecond:03}Z")
}

/// The current time in the stored UTC form.
pub fn now_utc() -> String {
    format_utc_millis(now_millis())
}

pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// A calendar date from its parts.
pub fn format_date(year: i64, month: u32, day: u32) -> String {
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_follow_the_published_pattern() {
        assert!(is_utc_timestamp("2026-01-02T03:04:05.678Z"));
        assert!(!is_utc_timestamp("2026-01-02T03:04:05Z"));
        assert!(!is_utc_timestamp("2026-01-02T03:04:05.678000Z"));
        assert!(!is_utc_timestamp("2026-01-02T03:04:05.678+01:00"));
        assert!(!is_utc_timestamp("2026-01-02T03:04:05.678z"));
        assert!(!is_utc_timestamp("2026-01-02T24:00:00.000Z"));
        assert!(!is_utc_timestamp("2026-01-02T23:59:60.000Z"));
        assert!(is_utc_timestamp("2024-02-29T00:00:00.000Z"));
        assert!(!is_utc_timestamp("2026-02-29T00:00:00.000Z"));
        assert!(!is_utc_timestamp("1900-02-29T00:00:00.000Z"));
        assert!(is_utc_timestamp("2000-02-29T00:00:00.000Z"));
        assert!(!is_calendar_date("2026-13-01"));
        assert!(!is_calendar_date("2026-04-31"));
        assert!(is_calendar_date("0000-02-29"));
    }

    #[test]
    fn millis_round_trip() {
        for text in [
            "1970-01-01T00:00:00.000Z",
            "2026-11-14T13:59:37.123Z",
            "0001-01-01T00:00:00.000Z",
            "1969-12-31T23:59:59.999Z",
        ] {
            let millis = parse_utc_timestamp(text).unwrap();
            assert_eq!(format_utc_millis(millis), text);
        }
        assert_eq!(parse_utc_timestamp("1970-01-01T00:00:01.000Z"), Some(1000));
    }
}
