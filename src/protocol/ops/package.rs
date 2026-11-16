//! Packages: export, load, import preview and apply, saved conflicts, privacy
//! erasure, and provider imports.
//!
//! The JavaScript client keeps a loaded package or a preview between calls.
//! Each later call names the package by its path and fingerprint, or the
//! export by its path and import time, and the worker loads and previews it
//! again. A package or database that changed in between is refused, so a
//! preview never applies to data it did not describe.

use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::json::{self, Map, Value};
use crate::model::{CurrentRecord, ErasureReasonCode, RecordType};
use crate::package::conflicts::{self, ConflictChoice, ConflictSet};
use crate::package::erase::{ErasureAttachment, ErasurePreview, ErasureRecord};
use crate::package::{
    LoadedPackage, PackageImportConflict, PackageImportPreview, PackageImportResolution, PackageLoad,
};
use crate::protocol::definition::{Kind, Operation, TargetUse};
use crate::protocol::worker::{Call, Failure, Outcome};

use super::input;

/// The arrays of a package document that stream one element per line.
const STREAMED_ARRAYS: &[&str] =
    &["records", "record_revisions", "record_deletions", "record_import_keys", "record_erasure_markers"];

const PACKAGE_REFERENCE: &str = "{ source_path: string; source_fingerprint: string }";

pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "export.run",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ output_directory: string; exported_at?: unknown }",
        output: "Api.ExportDatabaseResult",
        items: None,
        callbacks: &[],
        run: export,
    },
    Operation {
        name: "package.load",
        kind: Kind::Async,
        target: TargetUse::None,
        input: "{ input_path: string }",
        output: "{ package: Record<string, unknown>; series_files: Array<{ reference: Model.AthleticStandardPackageT[\"measurement_series_files\"][number]; path: string }>; sample_record_id_examples: string[]; source_path: string; source_fingerprint: string; source_directory: string; attachment_files: Array<[string, string]> }",
        items: Some("{ key: string; value: unknown }"),
        callbacks: &[],
        run: load,
    },
    Operation {
        name: "package.preview",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: PACKAGE_REFERENCE,
        output: "Omit<Api.PackageImportPreview, \"loaded\">",
        items: None,
        callbacks: &[],
        run: preview,
    },
    Operation {
        name: "package.import",
        kind: Kind::Async,
        target: TargetUse::Write,
        input: "{ source_path: string; source_fingerprint: string; destination_database_id: unknown; destination_database_revision: unknown; resolutions: readonly unknown[] }",
        output: "{ database_revision: number; restored: boolean }",
        items: None,
        callbacks: &[],
        run: import,
    },
    Operation {
        name: "package.read_imported",
        kind: Kind::Async,
        target: TargetUse::None,
        input: "{ source_path: string; source_fingerprint: string; record_id: string }",
        output: "Model.CurrentRecordT | null",
        items: None,
        callbacks: &[],
        run: read_imported,
    },
    Operation {
        name: "conflicts.save",
        kind: Kind::Async,
        target: TargetUse::None,
        input: "{ database_path: string; preview: unknown }",
        output: "Api.PackageConflictSet | null",
        items: None,
        callbacks: &[],
        run: save_conflicts,
    },
    Operation {
        name: "conflicts.read",
        kind: Kind::Async,
        target: TargetUse::None,
        input: "{ database_path: string }",
        output: "Api.PackageConflictSet",
        items: None,
        callbacks: &[],
        run: read_conflicts,
    },
    Operation {
        name: "conflicts.clear",
        kind: Kind::Async,
        target: TargetUse::None,
        input: "{ database_path: string }",
        output: "null",
        items: None,
        callbacks: &[],
        run: clear_conflicts,
    },
    Operation {
        name: "conflicts.assert_current",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ state: unknown; preview: unknown }",
        output: "null",
        items: None,
        callbacks: &[],
        run: assert_current,
    },
    Operation {
        name: "conflicts.resolve",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ state: unknown; record_id: string; resolution: unknown }",
        output: "Api.PackageConflictSet",
        items: None,
        callbacks: &[],
        run: resolve_conflict,
    },
    Operation {
        name: "conflicts.path",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ database_path: string }",
        output: "string",
        items: None,
        callbacks: &[],
        run: conflict_path,
    },
    Operation {
        name: "conflicts.resolutions",
        kind: Kind::Sync,
        target: TargetUse::None,
        input: "{ state: unknown }",
        output: "Api.PackageImportResolution[]",
        items: None,
        callbacks: &[],
        run: resolutions,
    },
    Operation {
        name: "erase.preview",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ record_id: unknown; expected_revision: unknown }",
        output: "Api.ErasurePreview",
        items: None,
        callbacks: &[],
        run: erase_preview,
    },
    Operation {
        name: "erase.apply",
        kind: Kind::Async,
        target: TargetUse::Write,
        input: "{ preview: unknown; reason_code: unknown; erased_at?: unknown }",
        output: "{ erased_record_ids: string[]; erased_attachment_count: number; database_revision: number }",
        items: None,
        callbacks: &[],
        run: erase_apply,
    },
    Operation {
        name: "provider.preview",
        kind: Kind::Async,
        target: TargetUse::Read,
        input: "{ input_path: string; received_at?: unknown }",
        output: "Omit<Api.ProviderImportPreview, \"plan\" | \"changes\"> & { received_at: string; match_key_method: string; input_fingerprint: string }",
        items: Some("{ record_id: string; record_type: Model.RecordTypeT; action: \"new\" | \"matching\" | \"changed\"; match_key: string; data: unknown }"),
        callbacks: &[],
        run: provider_preview,
    },
    Operation {
        name: "provider.import",
        kind: Kind::Async,
        target: TargetUse::Write,
        input: "{ input_path: string; received_at: string; input_fingerprint: string; database_revision: unknown; new_record_count: number; matching_record_count: number; changed_record_count: number; conflict_count: number }",
        output: "{ database_revision: number; new_record_count: number; changed_record_count: number; matching_record_count: number }",
        items: None,
        callbacks: &[],
        run: provider_import,
    },
];

// ---- Export ---------------------------------------------------------------

/// `exportRecords(database, outputDirectory, options)`.
fn export(call: &mut Call<'_>) -> Outcome {
    let directory = input::required_string(&call.input, "output_directory")?;
    let exported_at = match input::get(&call.input, "exported_at") {
        Some(value) => Some(input::checked_text("UtcTimestamp", value)?),
        None => None,
    };
    let options = crate::package::ExportOptions { exported_at };
    let result = crate::package::export_package(call.store()?, Path::new(&directory), &options)?;
    Ok(json::to_value(&result))
}

// ---- Load -----------------------------------------------------------------

fn representation_failure(representation: crate::package::PackageRepresentation) -> Failure {
    let mut details = Map::new();
    if let Some(id) = representation.source_database_id {
        details.insert("source_database_id".into(), Value::String(id));
    }
    if let Some(revision) = representation.source_database_revision {
        details.insert("source_database_revision".into(), Value::from(revision));
    }
    details.insert(
        "unrepresentable_records".into(),
        Value::Array(representation.unrepresentable_records.into_iter().map(Value::String).collect()),
    );
    details.insert(
        "unknown_fields".into(),
        Value::Array(representation.unknown_fields.into_iter().map(Value::String).collect()),
    );
    Failure::PackageRepresentation(Value::Object(details))
}

fn load_checked(path: &str) -> Result<LoadedPackage, Failure> {
    match crate::package::load_package_checked(Path::new(path))? {
        PackageLoad::Loaded(loaded) => Ok(*loaded),
        PackageLoad::Unrepresentable(representation) => Err(representation_failure(representation)),
    }
}

/// Load the package a client names again and refuse one that changed.
fn reload(source_path: &str, fingerprint: &str) -> Result<LoadedPackage, Failure> {
    let loaded = load_checked(source_path)?;
    if loaded.source_fingerprint != fingerprint {
        return Err(
            Error::stale(format!("the package changed after it was loaded; load it again: {source_path}")).into()
        );
    }
    Ok(loaded)
}

/// `loadAthleticStandardPackage(inputPath)`: the checked package document,
/// its series files, and its attachment files. Records, revisions,
/// deletions, import keys, and erasure markers stream one per line.
fn load(call: &mut Call<'_>) -> Outcome {
    let input_path = input::required_string(&call.input, "input_path")?;
    let loaded = load_checked(&input_path)?;
    let (document_path, _) = crate::package::load::package_document_path(Path::new(&loaded.source_path))?;
    let (_, document) = crate::package::load::safe_read_json(&document_path)?;
    let mut checked = crate::model::package::package_schema().output(&document);
    if let Value::Object(map) = &mut checked {
        for key in STREAMED_ARRAYS {
            if let Some(Value::Array(items)) = map.get_mut(*key) {
                for value in std::mem::take(items) {
                    call.item(serde_json::json!({ "key": key, "value": value }));
                }
            }
        }
    }
    let series_files: Vec<Value> = loaded
        .series_files
        .iter()
        .map(|file| serde_json::json!({ "reference": json::to_value(&file.reference), "path": file.path.display().to_string() }))
        .collect();
    let attachment_files: Vec<Value> =
        loaded.attachment_files.iter().map(|(id, path)| serde_json::json!([id, path.display().to_string()])).collect();
    Ok(serde_json::json!({
        "package": checked,
        "series_files": series_files,
        "sample_record_id_examples": loaded.sample_record_id_examples,
        "source_path": loaded.source_path,
        "source_fingerprint": loaded.source_fingerprint,
        "source_directory": loaded.source_directory.display().to_string(),
        "attachment_files": attachment_files,
    }))
}

// ---- Preview and import ----------------------------------------------------

fn preview_json(preview: &PackageImportPreview) -> Value {
    let mut value = json::to_value(preview);
    if let Value::Object(map) = &mut value {
        map.remove("write_confirmed");
    }
    value
}

fn reference(input: &Value) -> Result<(String, String), Failure> {
    Ok((input::required_string(input, "source_path")?, input::required_string(input, "source_fingerprint")?))
}

/// `previewPackageImport(database, loaded)`.
fn preview(call: &mut Call<'_>) -> Outcome {
    let (path, fingerprint) = reference(&call.input)?;
    let loaded = reload(&path, &fingerprint)?;
    let preview = crate::package::preview_loaded(call.store()?, loaded)?;
    Ok(preview_json(&preview))
}

fn choice(resolution: &Value) -> Result<ConflictChoice, Failure> {
    match resolution.get("choice").and_then(Value::as_str) {
        Some("local") => Ok(ConflictChoice::Local),
        Some("imported") => Ok(ConflictChoice::Imported),
        Some("corrected") => {
            Ok(ConflictChoice::Corrected { data: resolution.get("data").cloned().unwrap_or(Value::Null) })
        }
        _ => Err(Error::refused("resolution choice must be local, imported, or corrected").into()),
    }
}

/// `importRecords(database, preview, resolutions)`: the package is loaded and
/// previewed again, and the write refuses a database that changed after the
/// caller's preview.
fn import(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let (path, fingerprint) = reference(&request)?;
    let loaded = reload(&path, &fingerprint)?;
    let mut preview = crate::package::preview_loaded(call.store()?, loaded)?;
    // The write checks the database against the revision the caller saw.
    preview.destination_database_id =
        request.get("destination_database_id").and_then(Value::as_str).unwrap_or_default().to_string();
    preview.destination_database_revision =
        request.get("destination_database_revision").and_then(input::whole_number).unwrap_or(u64::MAX);
    let resolutions = input::array(&request, "resolutions")?
        .iter()
        .map(|resolution| {
            Ok(PackageImportResolution {
                record_id: resolution.get("record_id").and_then(Value::as_str).unwrap_or_default().to_string(),
                choice: choice(resolution)?,
            })
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let applied = crate::package::apply_package_import(call.store()?, &preview, &resolutions)?;
    Ok(json::to_value(&applied))
}

/// `readImportedPackageRecord(loaded, recordId)` for a record in a series file.
fn read_imported(call: &mut Call<'_>) -> Outcome {
    let (path, fingerprint) = reference(&call.input)?;
    let record_id = input::required_string(&call.input, "record_id")?;
    let loaded = reload(&path, &fingerprint)?;
    Ok(crate::package::imported_record(&loaded, &record_id)?.map(|record| record.to_value()).unwrap_or(Value::Null))
}

// ---- Saved conflicts -------------------------------------------------------

/// A preview summary from the client. Only the fields the conflict file
/// compares are read; the package is named by path and fingerprint.
fn preview_summary(value: &Value) -> Result<PackageImportPreview, Failure> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let number = |key: &str| value.get(key).and_then(input::whole_number).unwrap_or(u64::MAX);
    let loaded_value = value.get("loaded").cloned().unwrap_or(Value::Null);
    let loaded = LoadedPackage {
        athleticstandard_version: String::new(),
        database_id: String::new(),
        database_revision: 0,
        records: Vec::new(),
        revisions: Vec::new(),
        deletions: Vec::new(),
        erasure_markers: Vec::new(),
        import_keys: Vec::new(),
        series_files: Vec::new(),
        sample_record_id_examples: Vec::new(),
        source_path: loaded_value.get("source_path").and_then(Value::as_str).unwrap_or_default().to_string(),
        source_fingerprint: loaded_value
            .get("source_fingerprint")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        source_directory: PathBuf::new(),
        attachment_files: Default::default(),
    };
    let conflicts = value
        .get("conflicts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|conflict| PackageImportConflict {
                    record_id: conflict.get("record_id").and_then(Value::as_str).map(str::to_string),
                    message: conflict.get("message").and_then(Value::as_str).unwrap_or_default().to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(PackageImportPreview {
        source_database_id: text("source_database_id"),
        source_database_revision: number("source_database_revision"),
        destination_database_id: text("destination_database_id"),
        destination_database_revision: number("destination_database_revision"),
        restore: value.get("restore") == Some(&Value::Bool(true)),
        new_records: Vec::new(),
        new_record_count: 0,
        matching_records: Vec::new(),
        matching_record_count: 0,
        changed_records: Vec::new(),
        changed_record_count: 0,
        record_lists_truncated: false,
        conflicts,
        unrepresentable_records: Vec::new(),
        unknown_fields: Vec::new(),
        write_confirmed: false,
        loaded: Some(Box::new(loaded)),
    })
}

fn state_from(value: &Value) -> Result<ConflictSet, Failure> {
    conflicts::parse_conflict_set(&json::to_string(value))
        .map_err(|detail| Error::refused(format!("temporary import conflict file is invalid: {detail}")).into())
}

/// `savePackageConflictSet(database, preview)`.
fn save_conflicts(call: &mut Call<'_>) -> Outcome {
    let preview = preview_summary(input::required(&call.input, "preview")?)?;
    let database = input::required_string(&call.input, "database_path")?;
    let saved = conflicts::save_conflict_set_at(Path::new(&database), &preview)?;
    Ok(saved.map(|state| json::to_value(&state)).unwrap_or(Value::Null))
}

/// `readPackageConflictSet(database)`.
fn read_conflicts(call: &mut Call<'_>) -> Outcome {
    let database = input::required_string(&call.input, "database_path")?;
    Ok(json::to_value(&conflicts::read_conflict_set_at(Path::new(&database))?))
}

/// `clearPackageConflictSet(database)`.
fn clear_conflicts(call: &mut Call<'_>) -> Outcome {
    let database = input::required_string(&call.input, "database_path")?;
    conflicts::clear_conflict_set_at(Path::new(&database))?;
    Ok(Value::Null)
}

/// `assertPackageConflictSetCurrent(database, state, preview)`.
fn assert_current(call: &mut Call<'_>) -> Outcome {
    let state = state_from(input::required(&call.input, "state")?)?;
    let preview = preview_summary(input::required(&call.input, "preview")?)?;
    conflicts::assert_conflict_set_current(call.store()?, &state, &preview)?;
    Ok(Value::Null)
}

/// `resolvePackageConflict(database, state, recordId, resolution)`: record one
/// choice in the conflict file. Athlete data does not change.
fn resolve_conflict(call: &mut Call<'_>) -> Outcome {
    let mut state = state_from(input::required(&call.input, "state")?)?;
    let record_id = input::required_string(&call.input, "record_id")?;
    let resolution = input::required(&call.input, "resolution")?.clone();
    let Some(index) = state.conflicts.iter().position(|conflict| conflict.record_id == record_id) else {
        return Err(Error::other(format!("record is not in the temporary conflict set: {record_id}")).into());
    };
    let store = call.store()?;
    let Some(local) = store.get_record(&record_id)? else {
        return Err(Error::other(format!("local conflict record no longer exists: {record_id}")).into());
    };
    let choice = choice(&resolution)?;
    if let ConflictChoice::Corrected { data } = &choice {
        let mut value = local.to_value();
        if let Value::Object(map) = &mut value {
            map.insert("current_revision".into(), Value::from(local.current_revision + 1));
            map.insert("is_deleted".into(), Value::Bool(false));
            map.insert("data".into(), data.clone());
        }
        CurrentRecord::from_value(&value)?;
    }
    state.conflicts[index].resolution = Some(choice);
    conflicts::write_conflict_set_at(store.path(), &state)?;
    Ok(json::to_value(&state))
}

/// `packageConflictSetPath(database)`: `<database>.import-conflicts.json`.
fn conflict_path(call: &mut Call<'_>) -> Outcome {
    let path = input::required_string(&call.input, "database_path")?;
    Ok(Value::String(conflicts::conflict_set_path_for(Path::new(&path)).display().to_string()))
}

/// `conflictResolutions(state)`: the recorded choices, in conflict order.
fn resolutions(call: &mut Call<'_>) -> Outcome {
    let state = state_from(input::required(&call.input, "state")?)?;
    let out: Vec<Value> = conflicts::conflict_resolutions(&state)
        .into_iter()
        .map(|resolution| {
            let mut map = Map::new();
            map.insert("record_id".into(), Value::String(resolution.record_id));
            if let Value::Object(choice) = resolution.choice.to_value() {
                map.extend(choice);
            }
            Value::Object(map)
        })
        .collect();
    Ok(Value::Array(out))
}

// ---- Erasure ---------------------------------------------------------------

/// `previewRecordErasure(database, recordId, expectedRevision)`.
fn erase_preview(call: &mut Call<'_>) -> Outcome {
    let record_id = input::checked_text("RecordId", call.input.get("record_id").unwrap_or(&Value::Null))?;
    let expected = call
        .input
        .get("expected_revision")
        .and_then(input::whole_number)
        .ok_or_else(|| Error::other("expected revision must be a positive integer"))?;
    let preview = crate::package::preview_erasure(call.store()?, &record_id, expected)?;
    Ok(json::to_value(&preview))
}

fn erasure_preview_from(value: &Value) -> Result<ErasurePreview, Failure> {
    let text = |value: &Value, key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let number = |value: &Value, key: &str| value.get(key).and_then(input::whole_number).unwrap_or(u64::MAX);
    let records = value
        .get("records")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|record| {
            Ok(ErasureRecord {
                record_id: text(record, "record_id"),
                record_type: input::decode::<RecordType>(record.get("record_type").unwrap_or(&Value::Null))?,
                current_revision: number(record, "current_revision"),
            })
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let attachments = value
        .get("attachments")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|attachment| ErasureAttachment {
            record_id: text(attachment, "record_id"),
            path: text(attachment, "path"),
            present: attachment.get("present") == Some(&Value::Bool(true)),
        })
        .collect();
    Ok(ErasurePreview {
        root_record_id: text(value, "root_record_id"),
        root_expected_revision: number(value, "root_expected_revision"),
        database_revision: number(value, "database_revision"),
        records,
        attachments,
    })
}

/// `eraseRecord(database, input)`: erase what the preview lists; the
/// database must not have changed since the preview.
fn erase_apply(call: &mut Call<'_>) -> Outcome {
    let preview = erasure_preview_from(input::required(&call.input, "preview")?)?;
    let reason: ErasureReasonCode = input::decode(call.input.get("reason_code").unwrap_or(&Value::Null))?;
    let erased_at = match input::get(&call.input, "erased_at") {
        Some(Value::String(text)) => Some(text.clone()),
        Some(other) => Some(json::to_string(other)),
        None => None,
    };
    let result = crate::package::erase_record(call.store()?, &preview, reason, erased_at.as_deref())?;
    Ok(json::to_value(&result))
}

// ---- Provider imports ----------------------------------------------------

/// `previewProviderImport(database, inputPath, options)`. Planned records
/// stream one per line.
fn provider_preview(call: &mut Call<'_>) -> Outcome {
    let input_path = input::required_string(&call.input, "input_path")?;
    let received_at = match input::get(&call.input, "received_at") {
        Some(value) => Some(input::checked_text("UtcTimestamp", value)?),
        None => Some(crate::model::time::now_utc()),
    };
    let options = crate::import::ImportOptions { received_at };
    // Taken before the export is read, so a change during the read shows
    // at import time.
    let input_fingerprint = crate::import::detect::export_fingerprint(Path::new(&input_path))?;
    let preview = crate::import::provider::preview(call.store()?, Path::new(&input_path), &options)?;
    for planned in preview.planned()? {
        let data = json::parse(&planned.data_json).map_err(crate::Error::from)?;
        call.item(serde_json::json!({
            "record_id": planned.record_id,
            "record_type": planned.record_type,
            "action": planned.action.as_str(),
            "match_key": planned.match_key,
            "data": data,
        }));
    }
    let skipped: Map<String, Value> =
        preview.skipped.iter().map(|(reason, skipped)| (reason.clone(), json::to_value(skipped))).collect();
    Ok(serde_json::json!({
        "provider": preview.provider,
        "label": preview.imported,
        "database_revision": preview.destination_database_revision,
        "new_records": json::to_value(&preview.new_records()?),
        "matching_records": json::to_value(&preview.matching_records()?),
        "changed_records": json::to_value(&preview.changed_records()?),
        "conflicts": json::to_value(&preview.conflicts),
        "skipped": Value::Object(skipped),
        "received_at": preview.received_at(),
        "match_key_method": preview.match_key_method(),
        "input_fingerprint": input_fingerprint,
    }))
}

/// `importProviderRecords(database, preview)`: the export is read and
/// previewed again at the caller's import time. A database or export that
/// changed after the caller's preview is refused. The export's fingerprint
/// must match the preview's before and after it is read again.
fn provider_import(call: &mut Call<'_>) -> Outcome {
    let request = call.input.clone();
    let input_path = input::required_string(&request, "input_path")?;
    let received_at = input::required_string(&request, "received_at")?;
    let expected_fingerprint = input::required_string(&request, "input_fingerprint")?;
    let export_changed = || Error::stale("The provider export changed after its import preview; preview it again");
    let unchanged = || {
        crate::import::detect::export_fingerprint(Path::new(&input_path))
            .map(|fingerprint| fingerprint == expected_fingerprint)
    };
    if !unchanged()? {
        return Err(export_changed().into());
    }
    let options = crate::import::ImportOptions { received_at: Some(received_at) };
    let store = call.store()?;
    let preview = crate::import::provider::preview(store, Path::new(&input_path), &options)?;
    if !unchanged()? {
        return Err(export_changed().into());
    }
    let count = |key: &str| request.get(key).and_then(input::whole_number);
    if count("database_revision") != Some(preview.destination_database_revision) {
        return Err(Error::stale("Database changed after provider import preview; preview it again").into());
    }
    if count("new_record_count") != Some(preview.new_record_count())
        || count("matching_record_count") != Some(preview.matching_record_count())
        || count("changed_record_count") != Some(preview.changed_record_count())
        || count("conflict_count") != Some(preview.conflicts.len() as u64)
    {
        return Err(export_changed().into());
    }
    let result = preview.apply(store)?;
    Ok(json::to_value(&result))
}
