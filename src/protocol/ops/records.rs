//! Record writes and reads: create, correct, delete, read, list, history, links.
//!
//! Arguments are checked in the order 2.1.0 checked them, with the published
//! shapes, so the first refusal is the same one. The store then validates the
//! complete records and writes the change in one transaction.

use std::collections::HashMap;

use crate::error::Error;
use crate::json::{Map, Value};
use crate::model::time;
use crate::model::{BecameKnownAtBasis, CurrentRecord, RecordActor, RecordType};
use crate::protocol::definition::{Kind, Operation, TargetUse};
use crate::protocol::worker::{Call, Failure, Outcome};
use crate::store::read::ListOptions;
use crate::store::write::StaleStyle;
use crate::store::{Changes, Correction, Deletion, ExtensionsUpdate, NewRecord};

use super::input;

const WRITE_INPUT: &str = "unknown";

pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "records.write",
        kind: Kind::Async,
        target: TargetUse::Write,
        input: "{ inputs: readonly unknown[]; expected_current_revisions?: readonly unknown[]; attachments?: readonly AttachmentFile[] }",
        output: "{ records: Model.CurrentRecordT[]; database_revision: number }",
        items: None,
        callbacks: &[],
        run: write_records,
    },
    Operation {
        name: "records.write_changes",
        kind: Kind::Async,
        target: TargetUse::Write,
        input: "{ corrections?: readonly unknown[]; records?: readonly unknown[]; expected_current_revisions?: readonly unknown[]; attachments?: readonly AttachmentFile[] }",
        output: "{ records: Model.CurrentRecordT[]; database_revision: number }",
        items: None,
        callbacks: &[],
        run: write_changes,
    },
    Operation {
        name: "records.correct",
        kind: Kind::Async,
        target: TargetUse::Write,
        input: WRITE_INPUT,
        output: "{ record: Model.CurrentRecordT; database_revision: number }",
        items: None,
        callbacks: &[],
        run: correct,
    },
    Operation {
        name: "records.delete",
        kind: Kind::Async,
        target: TargetUse::Write,
        input: WRITE_INPUT,
        output: "{ record_id: string; record_revision: number; database_revision: number }",
        items: None,
        callbacks: &[],
        run: delete,
    },
    Operation {
        name: "records.read",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ record_id: unknown }",
        output: "Model.CurrentRecordT | null",
        items: None,
        callbacks: &[],
        run: read,
    },
    Operation {
        name: "records.list",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ record_type?: unknown; include_deleted?: boolean; limit?: unknown }",
        output: "null",
        items: Some("Model.CurrentRecordT"),
        callbacks: &[],
        run: list,
    },
    Operation {
        name: "records.revisions",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ record_id: unknown }",
        output: "null",
        items: Some("Model.RecordRevisionT"),
        callbacks: &[],
        run: revisions,
    },
    Operation {
        name: "records.links",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ record_id: string }",
        output: "Api.RecordLink[]",
        items: None,
        callbacks: &[],
        run: links,
    },
];

/// Apply a change. Attachment files the client prepared are written with it:
/// the files and the records that name them commit together or not at all.
fn apply_with_files(call: &mut Call<'_>, changes: Changes) -> Result<crate::store::Applied, Failure> {
    let files = input::array(&call.input, "attachments")?.to_vec();
    call.store()?;
    // Every input passed its checks; the transaction starts now.
    call.progress(
        serde_json::json!({ "phase": "write", "records": changes.creates.len() + changes.corrections.len() }),
    );
    if files.is_empty() {
        return Ok(call.store()?.apply(changes)?);
    }
    let store = call.store()?;
    let mut prepared = Vec::new();
    for file in &files {
        let record_id = input::checked_text("RecordId", file.get("record_id").unwrap_or(&Value::Null))?;
        let created_at = input::checked_text("UtcTimestamp", file.get("created_at").unwrap_or(&Value::Null))?;
        let changed_by: RecordActor = input::decode(file.get("changed_by").unwrap_or(&Value::Null))?;
        let data = input::decode(file.get("data").unwrap_or(&Value::Null))?;
        let bytes = file
            .get("bytes_base64")
            .and_then(Value::as_str)
            .and_then(super::prediction::base64_decode)
            .ok_or_else(|| Failure::Input("attachments[].bytes_base64 must be base64 text".into()))?;
        prepared.push(crate::package::attachments::PreparedAttachment::from_parts(
            store, record_id, created_at, changed_by, data, bytes,
        )?);
    }
    Ok(crate::package::attachments::apply_with_attachments(store, &prepared, changes)?)
}

/// `value ?? fallback` for a field of a caller's input object.
fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key).filter(|value| !value.is_null())
}

fn expected_revisions(values: &[Value]) -> Result<Vec<(String, u64)>, Failure> {
    values
        .iter()
        .map(|expected| {
            let record_id = input::checked_text("RecordId", field(expected, "record_id").unwrap_or(&Value::Null))?;
            let revision = field(expected, "revision")
                .and_then(input::whole_number)
                .ok_or_else(|| Failure::Input("expected_current_revisions[].revision must be a whole number".into()))?;
            Ok((record_id, revision))
        })
        .collect()
}

fn basis(value: Option<&Value>) -> Result<BecameKnownAtBasis, Failure> {
    input::decode(value.unwrap_or(&Value::String("local_write".into())))
}

/// One `WriteRecordInput`, checked as 2.1.0's writeRecords checked it.
fn new_record(create: &Value, with_extensions: bool) -> Result<NewRecord, Failure> {
    let changed_by: RecordActor = input::decode(field(create, "changed_by").unwrap_or(&Value::Null))?;
    let created_at = input::checked_text_or("UtcTimestamp", field(create, "created_at"), time::now_utc)?;
    let became_known_at =
        input::checked_text_or("UtcTimestamp", field(create, "became_known_at"), || created_at.clone())?;
    let became_known_at_basis = basis(field(create, "became_known_at_basis"))?;
    let record_id = input::checked_text_or("RecordId", field(create, "record_id"), crate::ids::new_record_id)?;
    let record_type = input::checked_text("RecordType", create.get("record_type").unwrap_or(&Value::Null))?;
    let mut candidate = Map::new();
    candidate.insert("record_id".into(), Value::String(record_id.clone()));
    candidate.insert("record_type".into(), Value::String(record_type));
    candidate.insert("created_at".into(), Value::String(created_at.clone()));
    candidate.insert("current_revision".into(), Value::from(1));
    candidate.insert("is_deleted".into(), Value::Bool(false));
    if let Some(data) = create.get("data") {
        candidate.insert("data".into(), data.clone());
    }
    if with_extensions {
        if let Some(extensions) = create.get("extensions") {
            candidate.insert("extensions".into(), extensions.clone());
        }
    }
    let record = CurrentRecord::from_value(&Value::Object(candidate))?;
    Ok(NewRecord {
        data: record.data,
        extensions: record.extensions,
        record_id: Some(record_id),
        created_at: Some(created_at),
        became_known_at: Some(became_known_at),
        became_known_at_basis,
        changed_by,
    })
}

fn written(records: &[CurrentRecord]) -> Value {
    Value::Array(records.iter().map(CurrentRecord::to_value).collect())
}

/// `writeRecords(database, inputs, options)`: one change, one database revision.
fn write_records(call: &mut Call<'_>) -> Outcome {
    let inputs = input::array(&call.input, "inputs")?.to_vec();
    if inputs.is_empty() {
        return Err(Error::other("writeRecords needs at least one record").into());
    }
    let expected = expected_revisions(input::array(&call.input, "expected_current_revisions")?)?;
    let creates = inputs.iter().map(|create| new_record(create, true)).collect::<Result<Vec<_>, _>>()?;
    let order: Vec<String> = creates.iter().filter_map(|create| create.record_id.clone()).collect();
    let changes = Changes { expected_current_revisions: expected, creates, ..Changes::default() };
    let applied = apply_with_files(call, changes)?;
    // The store writes attachments first; the result keeps the caller's order.
    let mut by_id: HashMap<String, CurrentRecord> =
        applied.records.into_iter().map(|record| (record.record_id.clone(), record)).collect();
    let records: Vec<CurrentRecord> = order.iter().filter_map(|id| by_id.remove(id)).collect();
    Ok(serde_json::json!({ "records": written(&records), "database_revision": applied.database_revision }))
}

/// The record a correction would make, checked before the write.
fn corrected(
    call: &mut Call<'_>,
    record_id: &str,
    data: Option<&Value>,
    extensions: Option<&Value>,
    keep_extensions: bool,
) -> Result<(CurrentRecord, u64), Failure> {
    let current =
        call.store()?.get_record(record_id)?.ok_or_else(|| Error::other(format!("Record not found: {record_id}")))?;
    let mut candidate = Map::new();
    candidate.insert("record_id".into(), Value::String(record_id.to_string()));
    candidate.insert("record_type".into(), Value::String(current.record_type().as_str().into()));
    candidate.insert("created_at".into(), Value::String(current.created_at.clone()));
    candidate.insert("current_revision".into(), Value::from(current.current_revision + 1));
    candidate.insert("is_deleted".into(), Value::Bool(false));
    if let Some(data) = data {
        candidate.insert("data".into(), data.clone());
    }
    match (extensions, keep_extensions) {
        (Some(extensions), _) => {
            candidate.insert("extensions".into(), extensions.clone());
        }
        (None, true) => {
            if let Some(existing) = &current.extensions {
                candidate.insert("extensions".into(), Value::Object(existing.clone()));
            }
        }
        (None, false) => {}
    }
    Ok((CurrentRecord::from_value(&Value::Object(candidate))?, current.current_revision))
}

/// `writeRecordChanges(database, input)`: corrections and new records in one
/// change. A correction keeps the record's extensions.
fn write_changes(call: &mut Call<'_>) -> Outcome {
    let corrections_in = input::array(&call.input, "corrections")?.to_vec();
    let creates_in = input::array(&call.input, "records")?.to_vec();
    if corrections_in.is_empty() && creates_in.is_empty() {
        return Err(Error::other("writeRecordChanges needs at least one change").into());
    }
    let expected = expected_revisions(input::array(&call.input, "expected_current_revisions")?)?;
    let mut creates = Vec::new();
    for create in creates_in.iter().filter(|create| create.get("record_type") == Some(&Value::from("attachment"))) {
        creates.push(new_record(create, false)?);
    }
    let mut corrections = Vec::new();
    for correction in &corrections_in {
        let record_id = input::checked_text("RecordId", field(correction, "record_id").unwrap_or(&Value::Null))?;
        let changed_by: RecordActor = input::decode(field(correction, "changed_by").unwrap_or(&Value::Null))?;
        let became_known_at =
            input::checked_text_or("UtcTimestamp", field(correction, "became_known_at"), time::now_utc)?;
        let became_known_at_basis = basis(field(correction, "became_known_at_basis"))?;
        let expected_revision = field(correction, "expected_revision")
            .and_then(input::whole_number)
            .ok_or_else(|| Error::stale(format!("Record {record_id} changed after preview; preview it again")))?;
        let (current_revision, deleted) = crate::store::read::current_revision(call.store()?.connection(), &record_id)?
            .ok_or_else(|| Error::other(format!("Record not found: {record_id}")))?;
        if current_revision != expected_revision || deleted {
            return Err(Error::stale(format!("Record {record_id} changed after preview; preview it again")).into());
        }
        let (record, _) = corrected(call, &record_id, correction.get("data"), None, false)?;
        corrections.push(Correction {
            record_id,
            expected_revision,
            data: record.data,
            extensions: ExtensionsUpdate::Keep,
            changed_by,
            became_known_at: Some(became_known_at),
            became_known_at_basis,
            stale_style: StaleStyle::Preview,
        });
    }
    for create in creates_in.iter().filter(|create| create.get("record_type") != Some(&Value::from("attachment"))) {
        creates.push(new_record(create, false)?);
    }
    let changes = Changes { expected_current_revisions: expected, creates, corrections, ..Changes::default() };
    let applied = apply_with_files(call, changes)?;
    Ok(serde_json::json!({ "records": written(&applied.records), "database_revision": applied.database_revision }))
}

/// `correctRecord(database, input)`: a new revision with the same ID.
fn correct(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let record_id = input::checked_text("RecordId", field(&request, "record_id").unwrap_or(&Value::Null))?;
    let changed_by: RecordActor = input::decode(field(&request, "changed_by").unwrap_or(&Value::Null))?;
    let became_known_at = input::checked_text_or("UtcTimestamp", field(&request, "became_known_at"), time::now_utc)?;
    let became_known_at_basis = basis(field(&request, "became_known_at_basis"))?;
    let expected_revision = field(&request, "expected_revision").and_then(input::whole_number);
    let (current_revision, deleted) = crate::store::read::current_revision(call.store()?.connection(), &record_id)?
        .ok_or_else(|| Error::other(format!("Record not found: {record_id}")))?;
    let Some(expected_revision) = expected_revision.filter(|expected| *expected == current_revision) else {
        let shown =
            field(&request, "expected_revision").map(crate::json::to_string).unwrap_or_else(|| "undefined".into());
        return Err(Error::stale(format!(
            "Record {record_id} is at revision {current_revision}, not expected revision {shown}"
        ))
        .into());
    };
    if deleted {
        return Err(Error::other(format!("Deleted record cannot be corrected: {record_id}")).into());
    }
    let extensions = field(&request, "extensions");
    let (record, _) = corrected(call, &record_id, request.get("data"), extensions, true)?;
    let update = match extensions {
        None => ExtensionsUpdate::Keep,
        Some(_) => ExtensionsUpdate::Set(record.extensions.clone()),
    };
    let applied = call.store()?.apply(Changes {
        corrections: vec![Correction {
            record_id,
            expected_revision,
            data: record.data,
            extensions: update,
            changed_by,
            became_known_at: Some(became_known_at),
            became_known_at_basis,
            stale_style: StaleStyle::Revision,
        }],
        ..Changes::default()
    })?;
    let record = applied.records.into_iter().next().ok_or_else(|| Error::other("the correction wrote no record"))?;
    Ok(serde_json::json!({ "record": record.to_value(), "database_revision": applied.database_revision }))
}

/// `deleteRecord(database, input)`: a normal deletion; history stays.
fn delete(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let record_id = input::checked_text("RecordId", field(&request, "record_id").unwrap_or(&Value::Null))?;
    let deleted_by: RecordActor = input::decode(field(&request, "deleted_by").unwrap_or(&Value::Null))?;
    let deleted_at = input::checked_text_or("UtcTimestamp", field(&request, "deleted_at"), time::now_utc)?;
    let expected_revision = field(&request, "expected_revision")
        .and_then(input::whole_number)
        .ok_or_else(|| Failure::Input("expected_revision must be a whole number".into()))?;
    let applied = call.store()?.apply(Changes {
        deletions: vec![Deletion {
            record_id: record_id.clone(),
            expected_revision,
            deleted_by,
            deleted_at: Some(deleted_at),
        }],
        ..Changes::default()
    })?;
    Ok(serde_json::json!({
        "record_id": record_id,
        "record_revision": expected_revision,
        "database_revision": applied.database_revision,
    }))
}

/// `readRecord(database, recordId)`, also exported as `getRecord`.
fn read(call: &mut Call<'_>) -> Outcome {
    let record_id = input::checked_text("RecordId", call.input.get("record_id").unwrap_or(&Value::Null))?;
    Ok(call.store()?.get_record(&record_id)?.map(|record| record.to_value()).unwrap_or(Value::Null))
}

/// `listRecords(database, options)`: current records in creation order.
fn list(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let record_type = match field(&request, "record_type") {
        Some(value) => Some(input::decode::<RecordType>(value)?),
        None => None,
    };
    let limit = match field(&request, "limit") {
        None => 100,
        Some(value) => match input::whole_number(value) {
            Some(limit) if (1..=u64::from(crate::store::read::MAX_LIST_LIMIT)).contains(&limit) => limit as u32,
            _ => return Err(Error::other("Record list limit must be between 1 and 10000").into()),
        },
    };
    let options = ListOptions { record_type, include_deleted: input::flag(&request, "include_deleted")?, limit };
    let records = call.store()?.list_records(&options)?;
    for record in &records {
        call.item(record.to_value());
    }
    Ok(Value::Null)
}

/// `readRecordRevisions(database, recordId)`: every retained revision, oldest first.
fn revisions(call: &mut Call<'_>) -> Outcome {
    let record_id = input::checked_text("RecordId", call.input.get("record_id").unwrap_or(&Value::Null))?;
    let revisions = call.store()?.revisions(&record_id)?;
    for revision in &revisions {
        call.item(revision.to_value());
    }
    Ok(Value::Null)
}

/// `readRecordLinks(database, recordId)`.
fn links(call: &mut Call<'_>) -> Outcome {
    let record_id = input::required_string(&call.input, "record_id")?;
    let links = crate::view::records::read_record_links(call.store()?, &record_id)?;
    Ok(crate::json::to_value(&links))
}
