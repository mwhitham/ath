//! Invented records in the `writeRecords` input shape, for scenario steps.

use athleticstandard::json::Value;
use serde_json::json;

pub fn id(n: u32) -> String {
    format!("01890f3e-e7d3-7cc0-88c8-{n:012x}")
}

pub fn tool() -> Value {
    json!({ "author_type": "tool", "tool": "test", "tool_version": "1.0.0" })
}

pub fn athlete() -> Value {
    json!({ "author_type": "athlete" })
}

pub fn receipt(at: &str) -> Value {
    json!({
        "timestamp_text": at,
        "utc_offset_text": "Z",
        "utc_offset_meaning": "documented_recording_offset",
        "timestamp_basis": "source_instant",
    })
}

pub fn value(standard: f64, unit: &str) -> Value {
    json!({ "standard_value": standard, "standard_unit": unit, "input_value": standard, "input_unit": unit })
}

pub fn source(record_id: &str, writer: &str, source_type: &str) -> Value {
    json!({
        "record_type": "source",
        "record_id": record_id,
        "created_at": "2026-01-01T00:00:00.000Z",
        "changed_by": tool(),
        "data": { "source_type": source_type, "data_writer": writer, "data_route": "test" },
    })
}

/// A record input first learned at `known_at` instead of its creation time.
pub fn known(mut input: Value, known_at: &str) -> Value {
    input["became_known_at"] = Value::String(known_at.into());
    input
}

pub fn point(record_id: &str, source_id: &str, quantity: &str, at: &str, standard: f64, unit: &str) -> Value {
    json!({
        "record_type": "point_measurement",
        "record_id": record_id,
        "created_at": at,
        "changed_by": tool(),
        "data": {
            "source_id": source_id,
            "received_at": at,
            "received_at_basis": "import_time",
            "quantity": quantity,
            "recorded_at": at,
            "input_timestamp": receipt(at),
            "value": value(standard, unit),
        },
    })
}

/// A point measurement with only a provider day, written at `written_at`.
pub fn period_point(record_id: &str, source_id: &str, date: &str, standard: f64, written_at: &str) -> Value {
    json!({
        "record_type": "point_measurement",
        "record_id": record_id,
        "created_at": written_at,
        "changed_by": tool(),
        "data": {
            "source_id": source_id,
            "received_at": written_at,
            "received_at_basis": "import_time",
            "provider_reporting_periods": [{ "period_type": "oura_sleep_day", "provider_date": date }],
            "quantity": "hrv_rmssd",
            "input_timestamp": {
                "timestamp_missing_reason": "not_provided_by_source",
                "utc_offset_missing_reason": "not_provided_by_source",
            },
            "value": value(standard, "ms"),
        },
    })
}

pub fn sleep(record_id: &str, source_id: &str, start: &str, end: &str) -> Value {
    json!({
        "record_type": "sleep_session",
        "record_id": record_id,
        "created_at": end,
        "changed_by": tool(),
        "data": {
            "source_id": source_id,
            "received_at": end,
            "received_at_basis": "import_time",
            "start_at": start,
            "end_at": end,
            "input_start_timestamp": receipt(start),
            "input_end_timestamp": receipt(end),
            "source_reported_summaries": [],
            "asleep_seconds": 25_000,
        },
    })
}

pub fn definition(record_id: &str, key: &str, text: &str) -> Value {
    json!({
        "record_type": "benchmark_definition",
        "record_id": record_id,
        "created_at": "2026-01-01T00:00:00.000Z",
        "changed_by": tool(),
        "data": {
            "authorship": {
                "author": { "author_type": "athlete" },
                "authored_at": "2026-01-01T00:00:00.000Z",
                "evidence": [],
                "content_version": 1,
                "status": "active",
            },
            "benchmark_key": key,
            "benchmark_type": "named_wod",
            "score_type": "time",
            "definition": text,
        },
    })
}

pub fn result(record_id: &str, source_id: &str, benchmark_id: &str, at: &str, seconds: f64) -> Value {
    json!({
        "record_type": "benchmark_result",
        "record_id": record_id,
        "created_at": at,
        "changed_by": athlete(),
        "data": {
            "source_id": source_id,
            "received_at": at,
            "received_at_basis": "local_write_time",
            "benchmark_definition_id": benchmark_id,
            "recorded_at": at,
            "input_timestamp": receipt(at),
            "score": { "duration_s": seconds },
        },
    })
}

pub fn prediction(record_id: &str, benchmark_id: &str, at: &str, seconds: f64) -> Value {
    json!({
        "record_type": "prediction",
        "record_id": record_id,
        "created_at": at,
        "changed_by": { "author_type": "agent", "agent": "test-agent" },
        "data": {
            "authorship": {
                "author": { "author_type": "agent", "agent": "test-agent", "model": "test-model" },
                "authored_at": at,
                "evidence": [],
                "content_version": 1,
                "status": "active",
            },
            "benchmark_definition_id": benchmark_id,
            "predicted_score": { "duration_s": seconds },
            "predicted_range": { "low": { "duration_s": seconds - 10.0 }, "high": { "duration_s": seconds + 10.0 } },
            "confidence": "moderate",
            "reasoning": "Invented test prediction",
        },
    })
}

/// A scenario step that writes these records in one transaction.
pub fn write(records: Vec<Value>) -> Value {
    json!({ "write": records })
}

/// A scenario step that corrects a record.
pub fn correct(record_id: &str, expected_revision: u64, known_at: &str, data: &Value) -> Value {
    json!({ "correct": {
        "record_id": record_id,
        "expected_revision": expected_revision,
        "changed_by": tool(),
        "became_known_at": known_at,
        "data": data,
    }})
}

/// A scenario step that deletes a record.
pub fn delete(record_id: &str, expected_revision: u64, deleted_at: &str) -> Value {
    json!({ "delete": {
        "record_id": record_id,
        "expected_revision": expected_revision,
        "deleted_by": athlete(),
        "deleted_at": deleted_at,
    }})
}
