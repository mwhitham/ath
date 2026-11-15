//! Read-only views for people and agents: calendar ranges, sample series by
//! day, and records with their history and links.
//!
//! Views return typed values whose JSON matches the 2.1.0 `--json` output.
//! The `render_*` functions return the human text the 2.1.0 commands printed.
//! Nothing here writes athlete data or prints.

pub mod calendar;
pub mod records;
pub mod series;

use serde::ser::{Serialize, SerializeSeq, Serializer};

use crate::model::{CurrentRecord, RecordRevision};

/// Serialize a current record as its published JSON object.
pub fn serialize_record<S: Serializer>(record: &CurrentRecord, serializer: S) -> Result<S::Ok, S::Error> {
    record.to_value().serialize(serializer)
}

/// Serialize records as published JSON objects.
pub fn serialize_records<S: Serializer>(records: &[CurrentRecord], serializer: S) -> Result<S::Ok, S::Error> {
    let mut sequence = serializer.serialize_seq(Some(records.len()))?;
    for record in records {
        sequence.serialize_element(&record.to_value())?;
    }
    sequence.end()
}

/// Serialize optional records as published JSON objects.
pub fn serialize_optional_records<S: Serializer>(
    records: &Option<Vec<CurrentRecord>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match records {
        Some(records) => serialize_records(records, serializer),
        None => serializer.serialize_none(),
    }
}

/// Serialize an optional revision as its published JSON object.
pub fn serialize_optional_revision<S: Serializer>(
    revision: &Option<RecordRevision>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match revision {
        Some(revision) => revision.to_value().serialize(serializer),
        None => serializer.serialize_none(),
    }
}

/// Serialize optional revisions as published JSON objects.
pub fn serialize_optional_revisions<S: Serializer>(
    revisions: &Option<Vec<RecordRevision>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match revisions {
        Some(revisions) => {
            let mut sequence = serializer.serialize_seq(Some(revisions.len()))?;
            for revision in revisions {
                sequence.serialize_element(&revision.to_value())?;
            }
            sequence.end()
        }
        None => serializer.serialize_none(),
    }
}

/// `String.prototype.padStart` with spaces, counted in UTF-16 units.
pub fn pad_start(text: &str, width: usize) -> String {
    let length = crate::json::utf16_len(text);
    if length >= width {
        text.to_string()
    } else {
        format!("{}{text}", " ".repeat(width - length))
    }
}

/// `String.prototype.padEnd` with spaces, counted in UTF-16 units.
pub fn pad_end(text: &str, width: usize) -> String {
    let length = crate::json::utf16_len(text);
    if length >= width {
        text.to_string()
    } else {
        format!("{text}{}", " ".repeat(width - length))
    }
}
