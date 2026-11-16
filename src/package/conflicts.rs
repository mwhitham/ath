//! The temporary conflict file of a package merge.
//!
//! A merge preview with conflicts is saved next to the database as
//! `<database>.import-conflicts.json`. `ath conflicts show` reads it with the
//! complete local and imported values. `ath conflicts resolve` records one
//! choice per record in that file; athlete data does not change. The import
//! then applies the choices, after checking that neither the package nor the
//! database changed since the preview. A saved choice for a stale preview is
//! refused.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::load::load_package;
use super::restore::{imported_record, preview_loaded, PackageImportPreview, PackageImportResolution};
use crate::error::{Error, Result};
use crate::files::{self, durable};
use crate::json::{self, Map, Value};
use crate::model::*;
use crate::store::{self, read, Store};

/// One recorded choice.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictChoice {
    /// Keep the local record. Nothing is written for it.
    Local,
    /// Write the imported value as a new local revision.
    Imported,
    /// Write this complete data object as a new local revision.
    Corrected { data: Value },
}

impl ConflictChoice {
    pub fn name(&self) -> &'static str {
        match self {
            ConflictChoice::Local => "local",
            ConflictChoice::Imported => "imported",
            ConflictChoice::Corrected { .. } => "corrected",
        }
    }

    /// `{"choice": ...}`, with `data` for a corrected value.
    pub fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("choice".into(), Value::String(self.name().into()));
        if let ConflictChoice::Corrected { data } = self {
            map.insert("data".into(), data.clone());
        }
        Value::Object(map)
    }
}

impl Serialize for ConflictChoice {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.to_value().serialize(serializer)
    }
}

/// One saved conflict.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SavedConflict {
    pub record_id: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<ConflictChoice>,
}

/// The temporary conflict file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConflictSet {
    pub conflict_set_version: u32,
    pub source_path: String,
    pub source_fingerprint: String,
    pub source_database_id: String,
    pub source_database_revision: u64,
    pub destination_database_id: String,
    pub destination_database_revision: u64,
    pub conflicts: Vec<SavedConflict>,
}

/// `<database>.import-conflicts.json`.
pub fn conflict_set_path(store: &Store) -> PathBuf {
    conflict_set_path_for(store.path())
}

/// `<database>.import-conflicts.json` for a database path. The conflict file
/// needs no open database, so the JavaScript client can read, save, and clear
/// it as 2.1.0 did.
pub fn conflict_set_path_for(database: &Path) -> PathBuf {
    files::sidecar(database, ".import-conflicts.json")
}

/// The stale-preview refusal.
const STALE: &str = "the package or database changed after the conflict preview; run ath import again";

/// Save the conflicts of a preview, without choices. A preview without
/// conflicts, or with a conflict that names no record, clears the file instead.
pub fn save_conflict_set(store: &Store, preview: &PackageImportPreview) -> Result<Option<ConflictSet>> {
    save_conflict_set_at(store.path(), preview)
}

/// [`save_conflict_set`] for a database path.
pub fn save_conflict_set_at(database: &Path, preview: &PackageImportPreview) -> Result<Option<ConflictSet>> {
    let conflicts: Vec<SavedConflict> = preview
        .conflicts
        .iter()
        .filter_map(|conflict| {
            conflict.record_id.as_ref().map(|record_id| SavedConflict {
                record_id: record_id.clone(),
                message: conflict.message.clone(),
                resolution: None,
            })
        })
        .collect();
    let Some(loaded) = preview.loaded.as_deref() else {
        clear_conflict_set_at(database)?;
        return Ok(None);
    };
    if conflicts.len() != preview.conflicts.len() || conflicts.is_empty() {
        clear_conflict_set_at(database)?;
        return Ok(None);
    }
    let state = ConflictSet {
        conflict_set_version: 1,
        source_path: loaded.source_path.clone(),
        source_fingerprint: loaded.source_fingerprint.clone(),
        source_database_id: preview.source_database_id.clone(),
        source_database_revision: preview.source_database_revision,
        destination_database_id: preview.destination_database_id.clone(),
        destination_database_revision: preview.destination_database_revision,
        conflicts,
    };
    write_conflict_set_at(database, &state)?;
    Ok(Some(state))
}

pub(crate) fn write_conflict_set(store: &Store, state: &ConflictSet) -> Result<()> {
    write_conflict_set_at(store.path(), state)
}

/// Replace the conflict file for a database path.
pub(crate) fn write_conflict_set_at(database: &Path, state: &ConflictSet) -> Result<()> {
    let path = conflict_set_path_for(database);
    files::refuse_symbolic_link_components(&path, true)?;
    let temporary = files::sidecar(&path, &format!(".tmp-{}", std::process::id()));
    let text = format!("{}\n", json::to_pretty(&json::to_value(state)));
    let written = (|| -> Result<()> {
        let mut file = files::create_new_private(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        durable::remove_file_if_present(&path)?;
        fs::rename(&temporary, &path)?;
        Ok(())
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written
}

/// Read the conflict file.
pub fn read_conflict_set(store: &Store) -> Result<ConflictSet> {
    read_conflict_set_at(store.path())
}

/// [`read_conflict_set`] for a database path.
pub fn read_conflict_set_at(database: &Path) -> Result<ConflictSet> {
    let path = conflict_set_path_for(database);
    let missing = || Error::not_found("no temporary import conflicts; run ath import <package> to create a preview");
    match files::refuse_symbolic_link_components(&path, false) {
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => return Err(missing()),
        other => other?,
    }
    let text = match fs::read_to_string(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Err(missing()),
        other => other?,
    };
    parse_conflict_set(&text)
        .map_err(|detail| Error::refused(format!("temporary import conflict file is invalid: {detail}")))
}

pub(crate) fn parse_conflict_set(text: &str) -> std::result::Result<ConflictSet, String> {
    let value = json::parse(text).map_err(|error| error.message)?;
    let object = value.as_object().ok_or("expected an object")?;
    only_keys(
        object,
        &[
            "conflict_set_version",
            "source_path",
            "source_fingerprint",
            "source_database_id",
            "source_database_revision",
            "destination_database_id",
            "destination_database_revision",
            "conflicts",
        ],
        "",
    )?;
    if object.get("conflict_set_version").and_then(Value::as_f64) != Some(1.0) {
        return Err("conflict_set_version must be 1".into());
    }
    let fingerprint = nonempty(object, "source_fingerprint")?;
    if fingerprint.len() != 64
        || !fingerprint.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("source_fingerprint must be a lowercase SHA-256".into());
    }
    let conflicts = object.get("conflicts").and_then(Value::as_array).ok_or("conflicts must be an array")?;
    let mut saved = Vec::new();
    for (index, conflict) in conflicts.iter().enumerate() {
        let path = format!("conflicts.{index}");
        let item = conflict.as_object().ok_or(format!("{path} must be an object"))?;
        only_keys(item, &["record_id", "message", "resolution"], &path)?;
        let resolution = match item.get("resolution") {
            None => None,
            Some(resolution) => {
                Some(parse_choice(resolution).map_err(|detail| format!("{path}.resolution: {detail}"))?)
            }
        };
        saved.push(SavedConflict {
            record_id: nonempty(item, "record_id").map_err(|detail| format!("{path}.{detail}"))?,
            message: nonempty(item, "message").map_err(|detail| format!("{path}.{detail}"))?,
            resolution,
        });
    }
    Ok(ConflictSet {
        conflict_set_version: 1,
        source_path: nonempty(object, "source_path")?,
        source_fingerprint: fingerprint,
        source_database_id: nonempty(object, "source_database_id")?,
        source_database_revision: whole(object, "source_database_revision")?,
        destination_database_id: nonempty(object, "destination_database_id")?,
        destination_database_revision: whole(object, "destination_database_revision")?,
        conflicts: saved,
    })
}

fn parse_choice(value: &Value) -> std::result::Result<ConflictChoice, String> {
    let object = value.as_object().ok_or("expected an object")?;
    match object.get("choice").and_then(Value::as_str) {
        Some("local") => {
            only_keys(object, &["choice"], "")?;
            Ok(ConflictChoice::Local)
        }
        Some("imported") => {
            only_keys(object, &["choice"], "")?;
            Ok(ConflictChoice::Imported)
        }
        Some("corrected") => {
            only_keys(object, &["choice", "data"], "")?;
            Ok(ConflictChoice::Corrected { data: object.get("data").cloned().unwrap_or(Value::Null) })
        }
        _ => Err("choice must be local, imported, or corrected".into()),
    }
}

fn only_keys(object: &Map<String, Value>, allowed: &[&str], path: &str) -> std::result::Result<(), String> {
    match object.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => {
            Err(format!("unrecognized key {}", if path.is_empty() { key.clone() } else { format!("{path}.{key}") }))
        }
        None => Ok(()),
    }
}

fn nonempty(object: &Map<String, Value>, key: &str) -> std::result::Result<String, String> {
    match object.get(key).and_then(Value::as_str) {
        Some(text) if !text.is_empty() => Ok(text.to_string()),
        _ => Err(format!("{key} must be a nonempty string")),
    }
}

fn whole(object: &Map<String, Value>, key: &str) -> std::result::Result<u64, String> {
    match object.get(key).and_then(Value::as_f64) {
        Some(number) if number >= 0.0 && number.fract() == 0.0 && number <= crate::model::schema::MAX_SAFE_INTEGER => {
            Ok(number as u64)
        }
        _ => Err(format!("{key} must be a nonnegative whole number")),
    }
}

/// Remove the conflict file when it exists.
pub fn clear_conflict_set(store: &Store) -> Result<()> {
    clear_conflict_set_at(store.path())
}

/// [`clear_conflict_set`] for a database path.
pub fn clear_conflict_set_at(database: &Path) -> Result<()> {
    let path = conflict_set_path_for(database);
    files::refuse_symbolic_link_components(&path, true)?;
    durable::remove_file_if_present(&path)?;
    Ok(())
}

/// Refuse a saved conflict file that no longer describes this package and
/// this database revision.
pub fn assert_conflict_set_current(store: &Store, state: &ConflictSet, preview: &PackageImportPreview) -> Result<()> {
    let connection = store.connection();
    let metadata = store::read_metadata(connection)?;
    let revision = store::database_revision(connection)?;
    let key = |id: &str, message: &str| format!("{id}\u{0}{message}");
    let mut previewed: Vec<String> = preview
        .conflicts
        .iter()
        .filter_map(|conflict| conflict.record_id.as_ref().map(|id| key(id, &conflict.message)))
        .collect();
    let mut saved: Vec<String> =
        state.conflicts.iter().map(|conflict| key(&conflict.record_id, &conflict.message)).collect();
    previewed.sort_by(|left, right| json::compare_utf16(left, right));
    saved.sort_by(|left, right| json::compare_utf16(left, right));
    let loaded = preview.loaded.as_deref();
    if loaded.map(|loaded| loaded.source_path.as_str()) != Some(state.source_path.as_str())
        || loaded.map(|loaded| loaded.source_fingerprint.as_str()) != Some(state.source_fingerprint.as_str())
        || state.source_database_id != preview.source_database_id
        || state.source_database_revision != preview.source_database_revision
        || state.destination_database_id != metadata.database_id
        || state.destination_database_revision != revision
        || preview.conflicts.iter().any(|conflict| conflict.record_id.is_none())
        || previewed != saved
    {
        return Err(Error::stale(STALE));
    }
    Ok(())
}

/// The recorded choices, in conflict order.
pub fn conflict_resolutions(state: &ConflictSet) -> Vec<PackageImportResolution> {
    state
        .conflicts
        .iter()
        .filter_map(|conflict| {
            conflict
                .resolution
                .as_ref()
                .map(|choice| PackageImportResolution { record_id: conflict.record_id.clone(), choice: choice.clone() })
        })
        .collect()
}

/// Read the conflict file, load its package again, and rebuild the preview.
/// Refuses when either changed.
fn current_state(store: &Store) -> Result<(ConflictSet, PackageImportPreview)> {
    let state = read_conflict_set(store)?;
    let loaded = load_package(Path::new(&state.source_path))?;
    let preview = preview_loaded(store, loaded)?;
    assert_conflict_set_current(store, &state, &preview)?;
    Ok((state, preview))
}

/// One conflict with its complete local and imported values.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConflictDetail {
    pub record_id: String,
    pub message: String,
    pub local: Option<Value>,
    pub imported: Option<Value>,
    pub resolution: Option<ConflictChoice>,
}

/// The `ath conflicts show` result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConflictsShow {
    pub source_path: String,
    pub source_database_revision: u64,
    pub destination_database_revision: u64,
    pub conflicts: Vec<ConflictDetail>,
}

/// Every saved conflict, or one, with complete local and imported values.
/// This reads only; it changes neither the database nor the conflict file.
pub fn show_conflicts(store: &Store, record_id: Option<&str>) -> Result<ConflictsShow> {
    let (state, preview) = current_state(store)?;
    let selected: Vec<&SavedConflict> =
        state.conflicts.iter().filter(|conflict| record_id.is_none_or(|id| conflict.record_id == id)).collect();
    if selected.is_empty() {
        return Err(Error::not_found(format!(
            "record is not in the temporary conflict set: {}",
            record_id.unwrap_or("undefined")
        )));
    }
    let loaded = preview.loaded.as_deref().expect("a loaded preview");
    let mut conflicts = Vec::new();
    for conflict in selected {
        conflicts.push(ConflictDetail {
            record_id: conflict.record_id.clone(),
            message: conflict.message.clone(),
            local: read::get_record(store.connection(), &conflict.record_id)?.map(|record| record.to_value()),
            imported: imported_record(loaded, &conflict.record_id)?.map(|record| record.to_value()),
            resolution: conflict.resolution.clone(),
        });
    }
    Ok(ConflictsShow {
        source_path: state.source_path,
        source_database_revision: state.source_database_revision,
        destination_database_revision: state.destination_database_revision,
        conflicts,
    })
}

/// The `ath conflicts resolve --json` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConflictResolveResult {
    pub record_id: String,
    pub choice: String,
    pub unresolved_conflicts: usize,
    /// Always false: a choice is recorded in the conflict file only.
    pub athlete_data_changed: bool,
    /// The package path to import again after every choice is recorded.
    #[serde(skip)]
    pub source_path: String,
}

/// Record one choice in the conflict file. Athlete data does not change. A
/// corrected value must be a complete data object for the record's type.
pub fn resolve_conflict(store: &Store, record_id: &str, choice: ConflictChoice) -> Result<ConflictResolveResult> {
    let (mut state, _) = current_state(store)?;
    let Some(index) = state.conflicts.iter().position(|conflict| conflict.record_id == record_id) else {
        return Err(Error::not_found(format!("record is not in the temporary conflict set: {record_id}")));
    };
    let Some(local) = read::get_record(store.connection(), record_id)? else {
        return Err(Error::not_found(format!("local conflict record no longer exists: {record_id}")));
    };
    if let ConflictChoice::Corrected { data } = &choice {
        let mut value = local.to_value();
        if let Value::Object(map) = &mut value {
            map.insert("current_revision".into(), Value::from(local.current_revision + 1));
            map.insert("is_deleted".into(), Value::Bool(false));
            map.insert("data".into(), data.clone());
        }
        CurrentRecord::from_value(&value)?;
    }
    let name = choice.name().to_string();
    state.conflicts[index].resolution = Some(choice);
    write_conflict_set(store, &state)?;
    Ok(ConflictResolveResult {
        record_id: record_id.to_string(),
        choice: name,
        unresolved_conflicts: state.conflicts.iter().filter(|conflict| conflict.resolution.is_none()).count(),
        athlete_data_changed: false,
        source_path: state.source_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_conflict_file_is_strict() {
        let good = r#"{"conflict_set_version":1,"source_path":"/p","source_fingerprint":"0000000000000000000000000000000000000000000000000000000000000000","source_database_id":"a","source_database_revision":1,"destination_database_id":"a","destination_database_revision":2,"conflicts":[{"record_id":"r","message":"m","resolution":{"choice":"corrected","data":{"x":1}}}]}"#;
        let state = parse_conflict_set(good).unwrap();
        assert_eq!(
            state.conflicts[0].resolution,
            Some(ConflictChoice::Corrected { data: json::parse(r#"{"x":1}"#).unwrap() })
        );
        assert_eq!(format!("{}\n", json::to_pretty(&json::to_value(&state))), {
            let value = json::parse(good).unwrap();
            format!("{}\n", json::to_pretty(&value))
        });
        for bad in [
            good.replace("\"conflict_set_version\":1", "\"conflict_set_version\":2"),
            good.replace("\"source_path\":\"/p\"", "\"source_path\":\"\""),
            good.replace("0000000000000000000000000000000000000000000000000000000000000000", "abc"),
            good.replace("\"message\":\"m\"", "\"message\":\"m\",\"extra\":1"),
            good.replace("\"choice\":\"corrected\"", "\"choice\":\"other\""),
            good.replace("{\"choice\":\"corrected\",\"data\":{\"x\":1}}", "{\"choice\":\"local\",\"data\":1}"),
        ] {
            assert!(parse_conflict_set(&bad).is_err(), "{bad}");
        }
    }
}
