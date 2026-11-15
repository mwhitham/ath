//! Pulse Band: an invented provider, used only to show what a new reader
//! needs. It is not a real export format and is not shipped.
//!
//! The export is a folder with `pulseband_daily.csv`. Each row has `time`
//! (an instant with its UTC offset), `resting_heart_rate` in beats per
//! minute, and `hrv_rmssd` in milliseconds. Each value becomes a point
//! measurement from one Pulse Band source.

use super::candidate::{offset_timestamp_receipt, source_value, Candidate, MatchKey, Section, SourceInput};
use super::csv::{cell, number, parse_csv};
use super::detect::{read_csv_bundle, DetectedExport};
use super::instant::normalized_utc;
use super::provider::{CandidateSink, ProviderReader};
use crate::error::Result;
use crate::model::*;

/// The file that marks a Pulse Band export.
pub const PULSEBAND_MARKER: &str = "pulseband_daily.csv";

pub struct PulseBandReader;

impl ProviderReader for PulseBandReader {
    fn read(&self, export: &DetectedExport, sink: &mut CandidateSink<'_>) -> Result<()> {
        let bundle = read_csv_bundle(export)?;
        let Some((_, text)) = bundle.iter().find(|(name, _)| name == PULSEBAND_MARKER) else {
            return Ok(());
        };
        let source_id = sink.export_source("Pulse Band", "Pulse Band", Some("optical"));
        for row in parse_csv(text) {
            let Some(time) = cell(&row, &["time"]) else {
                sink.skip("row without a time", "");
                continue;
            };
            let Ok(recorded_at) = normalized_utc(&time) else {
                sink.skip("time without an absolute instant", &time);
                continue;
            };
            for (column, quantity) in [
                ("resting_heart_rate", PointMeasurementType::RestingHeartRate),
                ("hrv_rmssd", PointMeasurementType::HrvRmssd),
            ] {
                let Some(value) = number(&row, &[column]) else { continue };
                let unit = quantity.unit();
                let data = PointMeasurementData {
                    source_id: source_id.clone(),
                    provider_record_id: None,
                    provider_reporting_periods: None,
                    received_at: sink.received_at().to_string(),
                    received_at_basis: ReceivedAtBasis::ImportTime,
                    supplied_fields: None,
                    field_basis: None,
                    quantity,
                    recorded_at: Some(recorded_at.clone()),
                    input_timestamp: offset_timestamp_receipt(&time, UtcOffsetMeaning::DocumentedRecordingOffset),
                    value: source_value(value, unit, Some(&SourceInput::written(value, unit))),
                    calculations: None,
                };
                sink.add(Candidate::new(
                    RecordData::PointMeasurement(data),
                    MatchKey::measured(RecordType::PointMeasurement, quantity.as_str(), &recorded_at),
                    Section::Record,
                ))?;
            }
        }
        Ok(())
    }
}
