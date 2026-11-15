//! `ath link`, `ath correct`, and `ath delete`: previewed changes to one
//! existing record, written through `Store::apply` after confirmation. A
//! correction with attachment files is written through
//! `package::apply_with_attachments`.
//!
//! Each prints a preview, stops for `--json` or `--dry-run`, asks unless
//! `--yes` is given (S18), and writes one change with the expected revision
//! from `--revision`. A stale revision is refused.

use std::path::PathBuf;

use super::shared::{confirm_flags, object, proceed, required_argument, with_store, Open};
use crate::cli::parse::{Args, PositiveInteger};
use crate::cli::runtime::{refuse, CommandResult, Context, Failure};
use crate::error::Error;
use crate::json::{self, Value};
use crate::model::{
    record_id as record_id_schema, time, BecameKnownAtBasis, CurrentRecord, PathSegment, RecordActor, RecordData,
    RecordType,
};
use crate::package::{apply_with_attachments, prepare_attachments};
use crate::store::write::StaleStyle;
use crate::store::{Changes, Correction, Deletion, ExtensionsUpdate, Store};

fn expected_revision(args: &Args) -> PositiveInteger {
    args.positive_integer("revision").expect("the parser requires --revision")
}

/// `readRecord`: the record ID must be a lowercase UUIDv7, as 2.1.0 checked.
fn read_record(store: &Store, record_id: &str) -> Result<Option<CurrentRecord>, Failure> {
    record_id_schema().validate(&Value::String(record_id.to_string())).map_err(Error::Invalid)?;
    Ok(store.get_record(record_id)?)
}

/// The checks `correctRecord` made before validating the new data: the
/// record exists, is at the expected revision, and is not deleted.
fn check_correctable(store: &Store, record_id: &str, expected: PositiveInteger) -> CommandResult {
    match crate::store::read::current_revision(store.connection(), record_id)? {
        None => Err(refuse(format!("Record not found: {record_id}"))),
        Some((current, _)) if current != expected.get() => {
            Err(refuse(format!("Record {record_id} is at revision {current}, not expected revision {expected}")))
        }
        Some((_, true)) => Err(refuse(format!("Deleted record cannot be corrected: {record_id}"))),
        Some(_) => Ok(()),
    }
}

/// A correction of one record with the expected revision.
fn correction(record_id: &str, expected: PositiveInteger, data: RecordData, changed_by: RecordActor) -> Changes {
    Changes {
        corrections: vec![Correction {
            record_id: record_id.to_string(),
            expected_revision: expected.get(),
            data,
            extensions: ExtensionsUpdate::Keep,
            changed_by,
            became_known_at: None,
            became_known_at_basis: BecameKnownAtBasis::LocalWrite,
            stale_style: StaleStyle::Revision,
        }],
        ..Changes::default()
    }
}

/// `ath link <result-id> <session-id> --revision <n>`.
pub fn link(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let result_id = required_argument(args, "result-id").to_string();
    let session_id = required_argument(args, "session-id").to_string();
    let expected = expected_revision(args);
    let flags = confirm_flags(args);
    with_store(context, args, Open::Write, |context, store, path| {
        let result = read_record(store, &result_id)?
            .filter(|record| record.record_type() == RecordType::BenchmarkResult && !record.is_deleted)
            .ok_or_else(|| refuse(format!("{result_id} is not a current benchmark result")))?;
        let session = read_record(store, &session_id)?
            .filter(|record| record.record_type() == RecordType::WorkoutSession && !record.is_deleted)
            .ok_or_else(|| refuse(format!("{session_id} is not a current workout session")))?;
        let RecordData::BenchmarkResult(data) = &result.data else { unreachable!("the record type was checked") };
        let old_session = data.workout_session_id.clone();
        if flags.json {
            return context.say_json(&object(vec![
                ("file", Value::String(path.display().to_string())),
                ("record_id", Value::String(result.record_id.clone())),
                ("expected_revision", expected.to_json()),
                ("current_revision", Value::from(result.current_revision)),
                ("old_workout_session_id", old_session.clone().map_or(Value::Null, Value::String)),
                ("new_workout_session_id", Value::String(session.record_id.clone())),
                ("write_confirmed", Value::Bool(false)),
            ]));
        }
        context.say(&format!(
            "result {} revision {}\n  workout session: {} → {}",
            result.record_id,
            result.current_revision,
            old_session.as_deref().unwrap_or("none"),
            session.record_id
        ))?;
        if !proceed(context, flags, "Write this link as a correction?")? {
            return Ok(());
        }
        check_correctable(store, &result.record_id, expected)?;
        let mut linked = data.clone();
        linked.workout_session_id = Some(session.record_id.clone());
        let applied = store.apply(correction(
            &result.record_id,
            expected,
            RecordData::BenchmarkResult(linked),
            RecordActor::Athlete,
        ))?;
        let revision = written_revision(&applied.records, &result.record_id)?;
        context.say(&format!("linked {} to {} at revision {revision}", result.record_id, session.record_id))
    })
}

/// `ath correct <record-id> --revision <n> --data <json>`.
pub fn correct(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let record_id = required_argument(args, "record-id").to_string();
    let expected = expected_revision(args);
    let data_text = args.value("data").expect("the parser requires --data").to_string();
    let agent = args.value("agent").map(ToString::to_string);
    let attachments = args.values("attachment");
    let flags = confirm_flags(args);
    with_store(context, args, Open::Write, |context, store, path| {
        let current = read_record(store, &record_id)?
            .filter(|record| !record.is_deleted)
            .ok_or_else(|| refuse(format!("current record not found: {record_id}")))?;
        let data = json::parse(&data_text).map_err(|error| {
            let reason = error.message.strip_prefix("Invalid JSON: ").unwrap_or(&error.message).to_string();
            refuse(format!("--data is not valid JSON: {reason}"))
        })?;
        let actor = match agent {
            None => RecordActor::Athlete,
            Some(agent) => RecordActor::Agent { agent, agent_version: None },
        };
        let paths: Vec<PathBuf> = attachments.iter().map(|path| context.resolve(path)).collect();
        let prepared = prepare_attachments(store, &paths, &actor, &time::now_utc())?;
        let mut data = data;
        if !prepared.is_empty() {
            if !matches!(current.record_type(), RecordType::AthleteReport | RecordType::NutritionEntry) {
                return Err(refuse("--attachment can correct only an athlete_report or nutrition_entry"));
            }
            let Value::Object(fields) = &mut data else {
                return Err(refuse("--data must be a complete record data object"));
            };
            // The new attachment IDs follow any the data already names.
            let mut ids = match fields.get("attachment_ids") {
                Some(Value::Array(ids)) => ids.clone(),
                _ => Vec::new(),
            };
            ids.extend(prepared.iter().map(|file| Value::String(file.record_id.clone())));
            fields.insert("attachment_ids".into(), Value::Array(ids));
        }
        let attachment_preview = prepared
            .iter()
            .map(|file| {
                object(vec![
                    ("record_id", Value::String(file.record_id.clone())),
                    ("data", RecordData::Attachment(file.data.clone()).to_value()),
                ])
            })
            .collect();
        let preview = object(vec![
            ("file", Value::String(path.display().to_string())),
            ("record_id", Value::String(current.record_id.clone())),
            ("record_type", Value::String(current.record_type().as_str().to_string())),
            ("expected_revision", expected.to_json()),
            ("current_revision", Value::from(current.current_revision)),
            ("before", current.data.to_value()),
            ("after", data.clone()),
            ("attachments", Value::Array(attachment_preview)),
            ("write_confirmed", Value::Bool(false)),
        ]);
        context.say_json(&preview)?;
        if flags.json || !proceed(context, flags, "Write this correction?")? {
            return Ok(());
        }
        check_correctable(store, &current.record_id, expected)?;
        let decoded = RecordData::decode(current.record_type(), &data)
            .map_err(|error| Error::Invalid(error.under(&[PathSegment::Key("data".to_string())])))?;
        let mut changes = correction(&current.record_id, expected, decoded, actor);
        changes.creates = prepared.iter().map(|file| file.new_record()).collect();
        let applied = apply_with_attachments(store, &prepared, changes)?;
        let revision = written_revision(&applied.records, &current.record_id)?;
        context.say(&format!(
            "corrected {} to revision {revision} (database revision {})",
            current.record_id, applied.database_revision
        ))
    })
}

/// `ath delete <record-id> --revision <n>`.
pub fn delete(context: &mut Context<'_>, args: &Args) -> CommandResult {
    let record_id = required_argument(args, "record-id").to_string();
    let expected = expected_revision(args);
    let flags = confirm_flags(args);
    with_store(context, args, Open::Write, |context, store, path| {
        let current = read_record(store, &record_id)?
            .filter(|record| !record.is_deleted)
            .ok_or_else(|| refuse(format!("current record not found: {record_id}")))?;
        let preview = object(vec![
            ("file", Value::String(path.display().to_string())),
            ("record_id", Value::String(current.record_id.clone())),
            ("record_type", Value::String(current.record_type().as_str().to_string())),
            ("expected_revision", expected.to_json()),
            ("current_revision", Value::from(current.current_revision)),
            ("retained_history", Value::Bool(true)),
            ("write_confirmed", Value::Bool(false)),
        ]);
        context.say_json(&preview)?;
        if flags.json || !proceed(context, flags, "Delete this record from current views?")? {
            return Ok(());
        }
        let applied = store.apply(Changes {
            deletions: vec![Deletion {
                record_id: current.record_id.clone(),
                expected_revision: expected.get(),
                deleted_by: RecordActor::Athlete,
                deleted_at: None,
            }],
            ..Changes::default()
        })?;
        context.say(&format!(
            "deleted {} at database revision {}; history retained",
            current.record_id, applied.database_revision
        ))
    })
}

/// The new revision of a corrected record.
fn written_revision(records: &[CurrentRecord], record_id: &str) -> Result<u64, Failure> {
    records
        .iter()
        .find(|record| record.record_id == record_id)
        .map(|record| record.current_revision)
        .ok_or_else(|| refuse("the correction did not return its record"))
}
