//! `ath series`: a sample series read back as one row per calendar day and
//! source, or as the raw samples.
//!
//! Days use the requested timezone, else the athlete profile's. Sources are
//! never pooled: two devices measuring the same day give two rows.

use serde::Serialize;

use super::calendar::{calendar_date_in_zone, resolve_calendar_range, CalendarRangeRequest};
use crate::error::Result;
use crate::json;
use crate::model::grade::js_round;
use crate::model::{CurrentRecord, RecordData, SeriesQuantity};
use crate::store::query::{query_series, SeriesQuery, TimeRange};
use crate::store::Store;

pub const DEFAULT_SERIES_LIMIT: u32 = 10_000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeriesViewOptions {
    pub from: Option<String>,
    pub to: Option<String>,
    pub source_id: Option<String>,
    pub time_zone: Option<String>,
    pub raw: bool,
    pub limit: Option<u32>,
}

/// One calendar day of one source.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeriesDay {
    pub date: String,
    pub source_id: String,
    pub sample_count: usize,
    pub minimum: f64,
    pub maximum: f64,
    /// Rounded to one decimal place.
    pub mean: f64,
    pub first_at: String,
    pub last_at: String,
}

/// The `ath series --json` object.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SeriesView {
    pub quantity: &'static str,
    pub unit: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// True when the read stopped at the limit.
    pub truncated: bool,
    /// With `raw`: the current measurement-sample records.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "super::serialize_optional_records")]
    pub samples: Option<Vec<CurrentRecord>>,
    /// Without `raw`: one row per calendar day and source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days: Option<Vec<SeriesDay>>,
}

impl SeriesView {
    /// The number of rows the view holds.
    pub fn row_count(&self) -> usize {
        self.samples.as_ref().map(Vec::len).or_else(|| self.days.as_ref().map(Vec::len)).unwrap_or(0)
    }
}

struct DayGroup {
    date: String,
    source_id: String,
    values: Vec<f64>,
    times: Vec<String>,
}

/// Read a series view.
pub fn read_series_view(store: &Store, quantity: SeriesQuantity, options: &SeriesViewOptions) -> Result<SeriesView> {
    store.read_snapshot(|| {
        let connection = store.connection();
        let range = resolve_calendar_range(
            store,
            &CalendarRangeRequest {
                from: options.from.clone(),
                to: options.to.clone(),
                time_zone: options.time_zone.clone(),
                raw: options.raw,
            },
        )?;
        let limit = options.limit.unwrap_or(DEFAULT_SERIES_LIMIT);
        let mut query = SeriesQuery::new(quantity);
        query.range = TimeRange { from: range.from.clone(), to: range.to.clone() };
        query.source_id = options.source_id.clone();
        query.limit = Some(limit);
        let records = query_series(connection, &query)?;
        let truncated = records.len() == limit as usize;
        let mut view = SeriesView {
            quantity: quantity.as_str(),
            unit: quantity.unit(),
            time_zone: range.time_zone.clone(),
            from: range.from,
            to: range.to,
            source_id: options.source_id.clone(),
            truncated,
            samples: None,
            days: None,
        };
        if options.raw {
            view.samples = Some(records);
            return Ok(view);
        }
        let time_zone = range.time_zone.unwrap_or_default();
        let mut groups: Vec<DayGroup> = Vec::new();
        let mut index: std::collections::HashMap<(String, String), usize> = std::collections::HashMap::new();
        for record in &records {
            let RecordData::MeasurementSample(sample) = &record.data else {
                continue;
            };
            let date = calendar_date_in_zone(&sample.recorded_at, &time_zone)?;
            let key = (date.clone(), sample.source_id.clone());
            let position = *index.entry(key).or_insert_with(|| {
                groups.push(DayGroup {
                    date,
                    source_id: sample.source_id.clone(),
                    values: Vec::new(),
                    times: Vec::new(),
                });
                groups.len() - 1
            });
            groups[position].values.push(sample.value.standard_value);
            groups[position].times.push(sample.recorded_at.clone());
        }
        let mut days: Vec<SeriesDay> = groups
            .into_iter()
            .map(|group| {
                let sum = group.values.iter().fold(0.0, |total, value| total + value);
                SeriesDay {
                    date: group.date,
                    source_id: group.source_id,
                    sample_count: group.values.len(),
                    minimum: group.values.iter().copied().fold(f64::INFINITY, f64::min),
                    maximum: group.values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    mean: js_round(sum / group.values.len() as f64 * 10.0) / 10.0,
                    first_at: group.times.first().cloned().unwrap_or_default(),
                    last_at: group.times.last().cloned().unwrap_or_default(),
                }
            })
            .collect();
        days.sort_by(|left, right| left.date.cmp(&right.date).then_with(|| left.source_id.cmp(&right.source_id)));
        view.days = Some(days);
        Ok(view)
    })
}

const LIMIT_NOTE: &str = "result reached --limit; narrow the time range or raise the limit";

/// The human text `ath series` printed.
pub fn render_series_view(view: &SeriesView) -> String {
    if let Some(samples) = &view.samples {
        if samples.is_empty() {
            return format!("no {} samples matched", view.quantity);
        }
        let mut lines = vec![format!("{} ({}) — raw samples", view.quantity, view.unit)];
        for sample in samples {
            let RecordData::MeasurementSample(data) = &sample.data else {
                continue;
            };
            lines.push(format!(
                "{}  {}{}  {}  {}",
                data.recorded_at,
                json::format_number(data.value.standard_value),
                view.unit,
                data.source_id,
                sample.record_id
            ));
        }
        if view.truncated {
            lines.push(LIMIT_NOTE.to_string());
        }
        return lines.join("\n");
    }
    let days = view.days.as_deref().unwrap_or_default();
    if view.days.as_ref().is_some_and(Vec::is_empty) {
        return format!("no {} samples matched", view.quantity);
    }
    let mut lines = vec![
        format!(
            "{} ({}) by calendar day in {}",
            view.quantity,
            view.unit,
            view.time_zone.as_deref().unwrap_or("undefined")
        ),
        "date        source                                n       mean      min       max".to_string(),
    ];
    for day in days {
        lines.push(format!(
            "{}  {}  {}  {}  {}  {}",
            day.date,
            day.source_id,
            super::pad_start(&day.sample_count.to_string(), 6),
            super::pad_start(&json::format_number(day.mean), 8),
            super::pad_start(&json::format_number(day.minimum), 8),
            super::pad_start(&json::format_number(day.maximum), 8),
        ));
    }
    if view.truncated {
        lines.push(LIMIT_NOTE.to_string());
    }
    lines.join("\n")
}

/// Write an instant with the UTC offset of a reference timestamp, as the 2.1.0
/// sidecar reader did. Milliseconds appear only when the instant has some.
pub fn at_offset_of(reference: &str, instant_millis: f64) -> String {
    let rounded = js_round(instant_millis) as i64;
    let iso = crate::model::time::format_utc_millis(rounded);
    let offset = reference.len().checked_sub(6).and_then(|start| reference.get(start..)).filter(|tail| {
        let bytes = tail.as_bytes();
        (bytes[0] == b'+' || bytes[0] == b'-')
            && bytes[1].is_ascii_digit()
            && bytes[2].is_ascii_digit()
            && bytes[3] == b':'
            && bytes[4].is_ascii_digit()
            && bytes[5].is_ascii_digit()
    });
    let whole_second = rounded.rem_euclid(1000) == 0;
    let Some(offset) = offset else {
        return if whole_second { format!("{}Z", &iso[..19]) } else { iso };
    };
    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let hours: i64 = offset[1..3].parse().unwrap_or(0);
    let minutes: i64 = offset[4..6].parse().unwrap_or(0);
    let shifted = crate::model::time::format_utc_millis(rounded + sign * (hours * 60 + minutes) * 60_000);
    let clock = if whole_second { &shifted[..19] } else { &shifted[..23] };
    format!("{clock}{offset}")
}

#[cfg(test)]
mod tests {
    use super::at_offset_of;

    #[test]
    fn offsets_follow_the_reference() {
        let millis = 1_791_331_200_000.0; // 2026-10-07T00:00:00.000Z
        assert_eq!(at_offset_of("2026-10-06T19:00:00-05:00", millis), "2026-10-06T19:00:00-05:00");
        assert_eq!(at_offset_of("2026-10-06T19:00:00Z", millis), "2026-10-07T00:00:00Z");
        assert_eq!(at_offset_of("2026-10-06T19:00:00+05:30", millis + 250.0), "2026-10-07T05:30:00.250+05:30");
        assert_eq!(at_offset_of("2026-10-06T19:00:00Z", millis + 1.0), "2026-10-07T00:00:00.001Z");
    }
}
