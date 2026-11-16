//! Provider timestamps read as absolute instants.
//!
//! The 2.1.0 importer turned provider text into a stored UTC instant with
//! JavaScript's `Date.parse` and `toISOString`. This module reads the same
//! text the same way for the forms provider readers produce: a date, a time,
//! and an offset or `Z`.
//!
//! Accepted, as V8 accepts them:
//!
//! - `YYYY-MM-DD`, then `T`, `t`, or one space, then `HH:MM`, optional `:SS`,
//!   and an optional fraction of one or more digits after `.`. Digits after
//!   the third fractional digit are dropped, not rounded.
//! - `Z`, `z`, `+HH:MM`, or `+HHMM` (or `-`) directly after the time. After a
//!   space separator, one space may also come before a `+HHMM` offset.
//! - A day up to 31 in any month. A day past the end of the month carries into
//!   the next month, so `2026-02-30` is 2 March.
//! - Hour 24 only as `24:00:00` with no fraction, meaning the next midnight.
//!
//! Text without an offset is refused. JavaScript reads it in the computer's
//! own timezone, which would make a stored instant depend on the machine.

use crate::error::{Error, Result};
use crate::model::time;

fn digits(bytes: &[u8]) -> Option<i64> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(bytes.iter().fold(0, |total, digit| total * 10 + i64::from(digit - b'0')))
}

/// Milliseconds since 1970 for provider text with an explicit offset.
pub fn parse_instant(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() < 16 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year = digits(&bytes[0..4])?;
    let month = digits(&bytes[5..7])?;
    let day = digits(&bytes[8..10])?;
    let space_separated = match bytes[10] {
        b'T' | b't' => false,
        b' ' => true,
        _ => return None,
    };
    if bytes[13] != b':' {
        return None;
    }
    let hour = digits(&bytes[11..13])?;
    let minute = digits(&bytes[14..16])?;
    let mut index = 16;
    let mut second = 0;
    let mut millisecond = 0;
    if bytes.get(index) == Some(&b':') {
        second = digits(bytes.get(index + 1..index + 3)?)?;
        index += 3;
        if bytes.get(index) == Some(&b'.') {
            let start = index + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end == start {
                return None;
            }
            let fraction = &bytes[start..end.min(start + 3)];
            millisecond = digits(fraction)? * 10_i64.pow(3 - fraction.len() as u32);
            index = end;
        }
    }
    if space_separated && bytes.get(index) == Some(&b' ') {
        index += 1;
        if !matches!(bytes.get(index), Some(b'+' | b'-')) || bytes.len() != index + 5 {
            return None;
        }
    }
    let offset_minutes = match &bytes[index..] {
        [b'Z' | b'z'] => 0,
        [sign @ (b'+' | b'-'), rest @ ..] => {
            let (hours, minutes) = match rest {
                [h1, h2, b':', m1, m2] | [h1, h2, m1, m2] => (digits(&[*h1, *h2])?, digits(&[*m1, *m2])?),
                _ => return None,
            };
            if hours > 23 || minutes > 59 {
                return None;
            }
            let total = hours * 60 + minutes;
            if *sign == b'-' {
                -total
            } else {
                total
            }
        }
        _ => return None,
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || minute > 59 || second > 59 {
        return None;
    }
    if hour > 24 || (hour == 24 && (minute != 0 || second != 0 || millisecond != 0)) {
        return None;
    }
    let days = time::days_from_civil(year, month as u32, 1) + day - 1;
    let local = ((days * 24 + hour) * 60 + minute) * 60_000 + second * 1000 + millisecond;
    Some(local - offset_minutes * 60_000)
}

/// The stored UTC form of provider text, as `new Date(Date.parse(text)).toISOString()`.
pub fn normalized_utc(text: &str) -> Result<String> {
    let millis = parse_instant(text)
        .filter(|millis| (-62_167_219_200_000..=253_402_300_799_999).contains(millis))
        .ok_or_else(|| Error::other(format!("Provider timestamp is not an absolute instant: {text}")))?;
    Ok(time::format_utc_millis(millis))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(text: &str) -> Option<String> {
        normalized_utc(text).ok()
    }

    #[test]
    fn reads_offset_timestamps_as_v8_does() {
        assert_eq!(utc("2026-08-09T06:20:00-07:00").as_deref(), Some("2026-08-09T13:20:00.000Z"));
        assert_eq!(utc("2026-08-09t06:20:00z").as_deref(), Some("2026-08-09T06:20:00.000Z"));
        assert_eq!(utc("2026-08-09T06:20-07:00").as_deref(), Some("2026-08-09T13:20:00.000Z"));
        assert_eq!(utc("2026-08-09T06:20:00.9999Z").as_deref(), Some("2026-08-09T06:20:00.999Z"));
        assert_eq!(utc("2026-08-09T06:20:00.5Z").as_deref(), Some("2026-08-09T06:20:00.500Z"));
        assert_eq!(utc("2026-02-30T00:00:00Z").as_deref(), Some("2026-03-02T00:00:00.000Z"));
        assert_eq!(utc("2026-08-09T24:00:00Z").as_deref(), Some("2026-08-10T00:00:00.000Z"));
        assert_eq!(utc("2026-08-09T06:20:00-0700").as_deref(), Some("2026-08-09T13:20:00.000Z"));
        assert_eq!(utc("2026-08-09 06:20:00 +0700").as_deref(), Some("2026-08-08T23:20:00.000Z"));
        assert_eq!(utc("2026-08-09T00:00:00-23:59").as_deref(), Some("2026-08-09T23:59:00.000Z"));
    }

    #[test]
    fn refuses_what_v8_refuses() {
        for text in [
            "2026-13-01T00:00:00-07:00",
            "2026-08-09T24:00:01Z",
            "2026-08-09T06:20:60Z",
            "2026-08-09T06:20:00T-0700",
            "2026-08-09T06:20:00+ab:cd",
            "2026-08-09T06:20:00-07000",
            "2026-08-09T06:20:00+24:00",
            "2026-08-09T06:20:00.Z",
            "2026-08-09T06:20:00 -07:00",
            "2026-08-00T00:00:00Z",
            "2026-08-09T06:20:00",
            "2026/08/09T06:20:00Z",
        ] {
            assert_eq!(utc(text), None, "{text}");
        }
    }
}
