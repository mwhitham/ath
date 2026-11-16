//! Calendar days, timezones, and the instants that bound them.
//!
//! A calendar day starts at local midnight in a named timezone. The start is
//! found by asking what the wall clock reads at a candidate instant and moving
//! the candidate by the difference, as the 2.1.0 program did with
//! `Intl.DateTimeFormat`. The same search gives the same answer on days when a
//! clock change moves midnight, and the same refusal when it cannot settle.

use chrono::{FixedOffset, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{Connection, OptionalExtension};

use crate::error::{Error, Result};
use crate::model::time;
pub use crate::store::query::parse_utc_timestamp;
use crate::store::Store;

/// A timezone accepted for calendar views: an IANA name in any letter case,
/// or a fixed UTC offset such as `+05:30`, which `Intl.DateTimeFormat` also
/// accepts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Zone {
    Named(Tz),
    Fixed(FixedOffset),
}

impl Zone {
    /// Read a timezone name. `None` when the name is not a timezone.
    pub fn parse(name: &str) -> Option<Zone> {
        if let Some(offset) = parse_offset(name) {
            return Some(Zone::Fixed(offset));
        }
        crate::model::resolve_time_zone(name).map(Zone::Named)
    }

    /// Read a timezone name, or refuse with the published message.
    pub fn require(name: &str) -> Result<Zone> {
        Zone::parse(name).ok_or_else(|| not_a_time_zone(name))
    }

    /// The wall-clock time at an instant, to the second.
    pub fn wall_time(&self, millis: i64) -> NaiveDateTime {
        let instant = Utc.timestamp_millis_opt(millis).single().unwrap_or_default();
        match self {
            Zone::Named(zone) => instant.with_timezone(zone).naive_local(),
            Zone::Fixed(offset) => instant.with_timezone(offset).naive_local(),
        }
    }
}

/// The refusal for a name that is not a timezone.
pub fn not_a_time_zone(name: &str) -> Error {
    Error::other(format!("'{name}' is not an IANA timezone such as America/Chicago"))
}

/// `±HH:MM`, `±HHMM`, or `±HH`.
fn parse_offset(text: &str) -> Option<FixedOffset> {
    let sign = match text.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let rest = &text[1..];
    let digits = |part: &str| {
        (part.len() == 2 && part.bytes().all(|b| b.is_ascii_digit())).then(|| part.parse::<i32>().ok()).flatten()
    };
    let (hours, minutes) = match rest.len() {
        2 => (digits(rest)?, 0),
        4 => (digits(&rest[0..2])?, digits(&rest[2..4])?),
        5 if rest.as_bytes()[2] == b':' => (digits(&rest[0..2])?, digits(&rest[3..5])?),
        _ => return None,
    };
    if hours > 23 || minutes > 59 {
        return None;
    }
    FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60))
}

/// True for `YYYY-MM-DD` with ASCII digits. The date may still be invalid.
pub fn looks_like_calendar_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes.iter().enumerate().all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

/// The parts of a calendar date, or the published refusal. Years below 100
/// are refused because `Date.UTC` reads them as 19xx.
pub fn calendar_date_parts(text: &str) -> Result<(u32, u32, u32)> {
    if !looks_like_calendar_date(text) {
        return Err(Error::other(format!("expected YYYY-MM-DD, not '{text}'")));
    }
    match time::parse_calendar_date(text) {
        Some((year, month, day)) if year >= 100 => Ok((year, month, day)),
        _ => Err(Error::other(format!("invalid calendar date '{text}'"))),
    }
}

/// The day after a calendar date.
pub fn next_calendar_date(text: &str) -> Result<String> {
    let (year, month, day) = calendar_date_parts(text)?;
    let days = time::days_from_civil(i64::from(year), month, day) + 1;
    let (year, month, day) = time::civil_from_days(days);
    Ok(time::format_date(year, month, day))
}

fn wall_millis(wall: NaiveDateTime) -> i64 {
    wall.and_utc().timestamp_millis()
}

/// The first instant of a calendar date in a timezone, in the stored UTC form.
pub fn start_of_date_in_zone(date: &str, zone_name: &str) -> Result<String> {
    let (year, month, day) = calendar_date_parts(date)?;
    let zone = Zone::require(zone_name)?;
    let target = time::days_from_civil(i64::from(year), month, day) * 86_400_000;
    let mut candidate = target;
    for _ in 0..6 {
        let seen = wall_millis(zone.wall_time(candidate));
        let difference = target - seen;
        if difference == 0 {
            return Ok(time::format_utc_millis(candidate));
        }
        candidate += difference;
    }
    Err(Error::other(format!("could not find the start of {date} in {zone_name}")))
}

/// The local calendar date of a timestamp in a timezone.
pub fn calendar_date_in_zone(timestamp: &str, zone_name: &str) -> Result<String> {
    let zone = Zone::require(zone_name)?;
    let millis = crate::model::js_date_parse(timestamp)
        .ok_or_else(|| Error::other(format!("Invalid time value: {timestamp}")))?;
    Ok(zone.wall_time(millis).format("%Y-%m-%d").to_string())
}

/// The athlete profile's default timezone, when one is saved.
pub(crate) fn saved_time_zone(connection: &Connection) -> Result<Option<String>> {
    let value: Option<Option<String>> = connection
        .query_row(
            "SELECT athlete_profile.default_time_zone
             FROM athlete_profile
             JOIN records ON records.record_id = athlete_profile.record_id
             WHERE records.is_deleted = 0",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value.flatten())
}

/// A requested time window: calendar days or exact UTC instants.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CalendarRangeRequest {
    pub from: Option<String>,
    pub to: Option<String>,
    pub time_zone: Option<String>,
    /// Raw sample reads need a timezone only for calendar-day bounds.
    pub raw: bool,
}

/// A resolved window in exact UTC instants, with the timezone used.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct InstantRange {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
}

/// Turn calendar-day bounds into UTC instants. A calendar day uses the given
/// timezone, else the profile's. Exact instants must use the stored UTC form.
pub fn resolve_calendar_range(store: &Store, request: &CalendarRangeRequest) -> Result<InstantRange> {
    resolve_calendar_range_in(store.connection(), request)
}

/// `resolve_calendar_range` on the connection a calculation already reads.
pub(crate) fn resolve_calendar_range_in(
    connection: &Connection,
    request: &CalendarRangeRequest,
) -> Result<InstantRange> {
    let is_date = |value: &Option<String>| value.as_deref().is_some_and(looks_like_calendar_date);
    let needs_time_zone = is_date(&request.from) || is_date(&request.to) || !request.raw;
    let time_zone = match &request.time_zone {
        Some(zone) => Some(zone.clone()),
        None if needs_time_zone => saved_time_zone(connection)?,
        None => None,
    };
    if needs_time_zone && time_zone.is_none() {
        return Err(Error::other(
            "a calendar-day series view needs --timezone <IANA name> or a default timezone in the athlete profile",
        ));
    }
    if let Some(zone) = &time_zone {
        Zone::require(zone)?;
    }
    let from = match &request.from {
        Some(from) if looks_like_calendar_date(from) => {
            Some(start_of_date_in_zone(from, time_zone.as_deref().unwrap_or_default())?)
        }
        Some(from) => Some(parse_utc_timestamp(from)?),
        None => None,
    };
    let to = match &request.to {
        Some(to) if looks_like_calendar_date(to) => {
            let next = start_of_date_in_zone(&next_calendar_date(to)?, time_zone.as_deref().unwrap_or_default())?;
            Some(time::format_utc_millis(time::parse_utc_timestamp(&next).unwrap_or_default() - 1))
        }
        Some(to) => Some(parse_utc_timestamp(to)?),
        None => None,
    };
    if let (Some(from), Some(to)) = (&from, &to) {
        if from > to {
            return Err(Error::other("--from must not be after --to"));
        }
    }
    Ok(InstantRange { from, to, time_zone })
}

/// The cut-off for a historical read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AsOfTime {
    pub as_of: String,
    /// The timezone used when the request named a calendar day.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
}

/// Resolve `--as-of`: now, an exact UTC instant, or the last millisecond of a
/// calendar day in the given or saved timezone.
pub fn resolve_as_of_time(
    store: &Store,
    value: Option<&str>,
    explicit_time_zone: Option<&str>,
    now: impl FnOnce() -> String,
) -> Result<AsOfTime> {
    let Some(value) = value else {
        return Ok(AsOfTime { as_of: now(), time_zone: None });
    };
    if !looks_like_calendar_date(value) {
        return Ok(AsOfTime { as_of: parse_utc_timestamp(value)?, time_zone: None });
    }
    let time_zone = match explicit_time_zone {
        Some(zone) => Some(zone.to_string()),
        None => saved_time_zone(store.connection())?,
    };
    let Some(time_zone) = time_zone else {
        return Err(Error::other(
            "a calendar --as-of date needs --timezone <IANA name> or a default timezone in the athlete profile",
        ));
    };
    let next = start_of_date_in_zone(&next_calendar_date(value)?, &time_zone)?;
    let as_of = time::format_utc_millis(time::parse_utc_timestamp(&next).unwrap_or_default() - 1);
    Ok(AsOfTime { as_of, time_zone: Some(time_zone) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_bounds_follow_clock_changes() {
        assert_eq!(start_of_date_in_zone("2026-03-08", "America/Chicago").unwrap(), "2026-03-08T06:00:00.000Z");
        assert_eq!(start_of_date_in_zone("2026-03-09", "America/Chicago").unwrap(), "2026-03-09T05:00:00.000Z");
        assert_eq!(start_of_date_in_zone("2026-11-01", "America/Chicago").unwrap(), "2026-11-01T05:00:00.000Z");
        assert_eq!(start_of_date_in_zone("2026-11-02", "america/chicago").unwrap(), "2026-11-02T06:00:00.000Z");
        assert_eq!(start_of_date_in_zone("2026-01-01", "+05:30").unwrap(), "2025-12-31T18:30:00.000Z");
        assert_eq!(start_of_date_in_zone("2026-01-01", "Asia/Kolkata").unwrap(), "2025-12-31T18:30:00.000Z");
    }

    #[test]
    fn midnight_that_does_not_exist_is_refused_like_the_old_search() {
        // Santiago moves its clocks at midnight, so 2026-09-06 00:00 never occurs.
        let error = start_of_date_in_zone("2026-09-06", "America/Santiago");
        assert_eq!(
            error.map_err(|error| error.to_string()),
            Err("could not find the start of 2026-09-06 in America/Santiago".to_string())
        );
    }

    #[test]
    fn dates_are_checked_like_the_old_parser() {
        assert_eq!(calendar_date_parts("2026-2-01").unwrap_err().to_string(), "expected YYYY-MM-DD, not '2026-2-01'");
        assert_eq!(calendar_date_parts("2026-02-30").unwrap_err().to_string(), "invalid calendar date '2026-02-30'");
        assert_eq!(calendar_date_parts("0050-01-01").unwrap_err().to_string(), "invalid calendar date '0050-01-01'");
        assert_eq!(next_calendar_date("2026-12-31").unwrap(), "2027-01-01");
        assert_eq!(calendar_date_in_zone("2026-10-02T03:00:00.000Z", "America/Chicago").unwrap(), "2026-10-01");
        assert!(Zone::parse("Z").is_none());
        assert!(Zone::parse("+24:00").is_none());
    }
}
