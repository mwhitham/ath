//! Package restore and merge.
//!
//! Restore fills an empty database (revision 0, no records). It keeps the
//! package's database ID, record IDs, complete history, deletions, erasure
//! markers, import keys, and database revision.
//!
//! Merge applies a package from the same database. A record new to this
//! database is added with its history. A record whose local history is a
//! prefix of the package history gains the later revisions. Any other
//! difference is a conflict that needs an explicit choice. A package from a
//! different database is a conflict as a whole: live databases are not merged.
//!
//! Both write through the shared record encoder. Apply runs under the write
//! lock and refuses a plan that is stale: the database or package changed
//! after the preview.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use rusqlite::{params, OptionalExtension, Transaction};
use serde::Serialize;

use super::attachments::{
    attachment_directory, stored_attachment_name, AttachmentFileBatch, AttachmentSource, NewAttachmentFile,
};
use super::conflicts::ConflictChoice;
use super::failpoint;
use super::load::{self, read_series_batch, LoadedPackage, PackageLoad, SeriesBatch};
use crate::error::{Error, Result};
use crate::files::{self, durable};
use crate::input_policy::MAX_PREVIEW_RECORD_IDS;
use crate::json::{self, Value};
use crate::model::*;
use crate::store::{self, encode, read, write, Layout, Store};

/// One conflict in a package preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackageImportConflict {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    pub message: String,
}

/// A choice for one conflicting record.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageImportResolution {
    pub record_id: String,
    pub choice: ConflictChoice,
}

/// What a package import would do. Serializes as `ath import <package> --json`.
#[derive(Debug, Clone, Serialize)]
pub struct PackageImportPreview {
    pub source_database_id: String,
    pub source_database_revision: u64,
    pub destination_database_id: String,
    pub destination_database_revision: u64,
    /// True when the destination is empty and adopts the package history.
    pub restore: bool,
    pub new_records: Vec<String>,
    pub new_record_count: u64,
    pub matching_records: Vec<String>,
    pub matching_record_count: u64,
    pub changed_records: Vec<String>,
    pub changed_record_count: u64,
    pub record_lists_truncated: bool,
    pub conflicts: Vec<PackageImportConflict>,
    pub unrepresentable_records: Vec<String>,
    pub unknown_fields: Vec<String>,
    /// Always false: a preview writes nothing.
    pub write_confirmed: bool,
    /// The loaded package. Absent when the package cannot be represented.
    #[serde(skip)]
    pub loaded: Option<Box<LoadedPackage>>,
}

impl PackageImportPreview {
    /// True when the package cannot be imported as it is.
    pub fn is_unrepresentable(&self) -> bool {
        !self.unrepresentable_records.is_empty() || !self.unknown_fields.is_empty()
    }
}

/// A completed package import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackageImportApplied {
    pub database_revision: u64,
    pub restored: bool,
}

/// Load a package and preview its import into `store`. Unknown fields and
/// unrepresentable records produce a preview with empty record lists that
/// cannot be applied.
pub fn preview_package_import(store: &Store, input: &Path) -> Result<PackageImportPreview> {
    match load::load_package_checked(input)? {
        PackageLoad::Loaded(loaded) => preview_loaded(store, *loaded),
        PackageLoad::Unrepresentable(representation) => {
            let destination = destination_state(store.connection())?;
            Ok(PackageImportPreview {
                source_database_id: representation.source_database_id.unwrap_or_else(|| "unavailable".into()),
                source_database_revision: representation.source_database_revision.unwrap_or(0),
                restore: destination.revision == 0 && destination.record_count == 0,
                destination_database_id: destination.database_id,
                destination_database_revision: destination.revision,
                new_records: Vec::new(),
                new_record_count: 0,
                matching_records: Vec::new(),
                matching_record_count: 0,
                changed_records: Vec::new(),
                changed_record_count: 0,
                record_lists_truncated: false,
                conflicts: Vec::new(),
                unrepresentable_records: representation.unrepresentable_records,
                unknown_fields: representation.unknown_fields,
                write_confirmed: false,
                loaded: None,
            })
        }
    }
}

struct Destination {
    database_id: String,
    revision: u64,
    record_count: u64,
}

fn destination_state(connection: &rusqlite::Connection) -> Result<Destination> {
    let metadata = store::read_metadata(connection)?;
    let record_count: i64 = connection.query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))?;
    Ok(Destination {
        database_id: metadata.database_id,
        revision: store::database_revision(connection)?,
        record_count: record_count as u64,
    })
}

/// Preview a loaded package.
pub fn preview_loaded(store: &Store, loaded: LoadedPackage) -> Result<PackageImportPreview> {
    let connection = store.connection();
    let destination = destination_state(connection)?;
    let restore = destination.revision == 0 && destination.record_count == 0;
    let mut conflicts = Vec::new();
    if !restore && destination.database_id != loaded.database_id {
        conflicts.push(PackageImportConflict {
            record_id: None,
            message: format!(
                "destination database {} differs from package {}; live databases are not merged",
                destination.database_id, loaded.database_id
            ),
        });
    }
    let mut lists = Lists::default();
    if restore {
        lists.new_count = loaded.records.len() as u64 + loaded.sample_count();
        for record in &loaded.records {
            add_example(&mut lists.new, &record.record_id);
        }
        for id in &loaded.sample_record_id_examples {
            add_example(&mut lists.new, id);
        }
    } else if destination.database_id == loaded.database_id {
        for_each_batch(&loaded, |batch| {
            let classification = classify(connection, batch)?;
            conflicts.extend(classification.conflicts);
            for id in classification.new_ids {
                lists.new_count += 1;
                add_example(&mut lists.new, &id);
            }
            for id in classification.matching_ids {
                lists.matching_count += 1;
                add_example(&mut lists.matching, &id);
            }
            for id in classification.changed_ids {
                lists.changed_count += 1;
                add_example(&mut lists.changed, &id);
            }
            Ok(())
        })?;
    }
    let truncated = lists.new_count > lists.new.len() as u64
        || lists.matching_count > lists.matching.len() as u64
        || lists.changed_count > lists.changed.len() as u64;
    for list in [&mut lists.new, &mut lists.matching, &mut lists.changed] {
        list.sort_by(|left, right| json::compare_utf16(left, right));
    }
    Ok(PackageImportPreview {
        source_database_id: loaded.database_id.clone(),
        source_database_revision: loaded.database_revision,
        destination_database_id: destination.database_id,
        destination_database_revision: destination.revision,
        restore,
        new_records: lists.new,
        new_record_count: lists.new_count,
        matching_records: lists.matching,
        matching_record_count: lists.matching_count,
        changed_records: lists.changed,
        changed_record_count: lists.changed_count,
        record_lists_truncated: truncated,
        conflicts,
        unrepresentable_records: Vec::new(),
        unknown_fields: Vec::new(),
        write_confirmed: false,
        loaded: Some(Box::new(loaded)),
    })
}

#[derive(Default)]
struct Lists {
    new: Vec<String>,
    new_count: u64,
    matching: Vec<String>,
    matching_count: u64,
    changed: Vec<String>,
    changed_count: u64,
}

fn add_example(list: &mut Vec<String>, id: &str) {
    if list.len() < MAX_PREVIEW_RECORD_IDS {
        list.push(id.to_string());
    }
}

// ---- Batches and classification -------------------------------------------------

/// The records and history of one package file.
pub(crate) struct Batch<'a> {
    pub records: &'a [CurrentRecord],
    pub revisions: &'a [RecordRevision],
    pub deletions: &'a [RecordDeletion],
    pub import_keys: &'a [RecordImportKey],
}

impl<'a> Batch<'a> {
    fn of_series(series: &'a SeriesBatch) -> Self {
        Batch {
            records: &series.records,
            revisions: &series.revisions,
            deletions: &series.deletions,
            import_keys: &series.import_keys,
        }
    }
}

/// Visit the main document, then each series file read again.
fn for_each_batch(loaded: &LoadedPackage, mut visit: impl FnMut(&Batch<'_>) -> Result<()>) -> Result<()> {
    visit(&Batch {
        records: &loaded.records,
        revisions: &loaded.revisions,
        deletions: &loaded.deletions,
        import_keys: &loaded.import_keys,
    })?;
    for file in &loaded.series_files {
        let series = read_series_batch(file)?;
        visit(&Batch::of_series(&series))?;
    }
    Ok(())
}

struct Classification {
    new_ids: Vec<String>,
    matching_ids: Vec<String>,
    changed_ids: Vec<String>,
    conflicts: Vec<PackageImportConflict>,
}

/// JSON text in published field order. Equal text means the same record
/// and the same key order inside supplied maps, as version 2.1.0 compared.
fn record_text(record: &CurrentRecord) -> String {
    json::to_string(&record.to_value())
}

fn revision_text(revision: &RecordRevision) -> String {
    json::to_string(&revision.to_value())
}

fn classify(connection: &rusqlite::Connection, batch: &Batch<'_>) -> Result<Classification> {
    let mut incoming: HashMap<&str, Vec<&RecordRevision>> = HashMap::new();
    for revision in batch.revisions {
        incoming.entry(revision.record_id.as_str()).or_default().push(revision);
    }
    let mut out = Classification {
        new_ids: Vec::new(),
        matching_ids: Vec::new(),
        changed_ids: Vec::new(),
        conflicts: Vec::new(),
    };
    let mut seen: HashSet<&str> = HashSet::new();
    for imported in batch.records {
        if !seen.insert(imported.record_id.as_str()) {
            continue;
        }
        let Some(local) = read::get_record(connection, &imported.record_id)? else {
            out.new_ids.push(imported.record_id.clone());
            continue;
        };
        let local_revisions = read::revisions(connection, &imported.record_id)?;
        let incoming_revisions = incoming.get(imported.record_id.as_str()).map(Vec::as_slice).unwrap_or_default();
        let overlap = local_revisions.len().min(incoming_revisions.len());
        let prefix_matches = (0..overlap)
            .all(|index| revision_text(&local_revisions[index]) == revision_text(incoming_revisions[index]));
        if !prefix_matches || incoming_revisions.len() < local_revisions.len() {
            out.conflicts.push(PackageImportConflict {
                record_id: Some(imported.record_id.clone()),
                message: "record revision history differs".into(),
            });
            continue;
        }
        if incoming_revisions.len() == local_revisions.len() && record_text(&local) == record_text(imported) {
            out.matching_ids.push(imported.record_id.clone());
            continue;
        }
        if incoming_revisions.len() > local_revisions.len()
            && incoming_revisions.len() as u64 == imported.current_revision
        {
            out.changed_ids.push(imported.record_id.clone());
            continue;
        }
        out.conflicts.push(PackageImportConflict {
            record_id: Some(imported.record_id.clone()),
            message: "current record does not match its imported revision history".into(),
        });
    }
    Ok(out)
}

/// The record a package holds for an ID, from the main document or a series file.
pub fn imported_record(loaded: &LoadedPackage, record_id: &str) -> Result<Option<CurrentRecord>> {
    if let Some(record) = loaded.records.iter().find(|record| record.record_id == record_id) {
        return Ok(Some(record.clone()));
    }
    for file in &loaded.series_files {
        let series = read_series_batch(file)?;
        if let Some(record) = series.records.into_iter().find(|record| record.record_id == record_id) {
            return Ok(Some(record));
        }
    }
    Ok(None)
}

// ---- Apply --------------------------------------------------------------------------

/// Apply a previewed package import. `resolutions` must answer every conflict
/// that names a record. Refuses when the database or package changed after
/// the preview. Writes everything in one transaction or nothing.
pub fn apply_package_import(
    store: &mut Store,
    preview: &PackageImportPreview,
    resolutions: &[PackageImportResolution],
) -> Result<PackageImportApplied> {
    let mut by_id: HashMap<&str, &PackageImportResolution> = HashMap::new();
    for resolution in resolutions {
        if by_id.insert(resolution.record_id.as_str(), resolution).is_some() {
            return Err(Error::refused(format!("conflict has more than one resolution: {}", resolution.record_id)));
        }
    }
    let unresolved =
        preview.conflicts.iter().any(|conflict| conflict.record_id.as_deref().is_none_or(|id| !by_id.contains_key(id)));
    let Some(loaded) = preview.loaded.as_deref().filter(|_| !unresolved && !preview.is_unrepresentable()) else {
        return Err(Error::refused("import has unresolved conflicts or unsupported records"));
    };
    for id in by_id.keys() {
        if !preview.conflicts.iter().any(|conflict| conflict.record_id.as_deref() == Some(*id)) {
            return Err(Error::refused(format!("resolution does not match an import conflict: {id}")));
        }
    }

    let files = attachment_files(store, loaded, &by_id)?;
    let mut batch = if files.is_empty() { None } else { Some(AttachmentFileBatch::begin(store, files)?) };
    let resolution_time = time::now_utc();
    let layout = store.layout();
    let result = store.transaction(|tx| {
        if store::database_revision(tx)? != preview.destination_database_revision
            || store::read_metadata(tx)?.database_id != preview.destination_database_id
        {
            return Err(Error::stale("destination database changed after preview"));
        }
        if preview.restore {
            restore_in(tx, layout, loaded)?;
        } else {
            merge_in(tx, layout, preview, loaded, &by_id, &resolution_time)?;
        }
        if let Some(batch) = &batch {
            batch.finish_in(tx)?;
        }
        failpoint!("restore.before_commit");
        store::database_revision(tx)
    });
    match result {
        Ok(database_revision) => {
            if let Some(batch) = batch.take() {
                batch.committed();
            }
            failpoint!("restore.after_commit");
            Ok(PackageImportApplied { database_revision, restored: preview.restore })
        }
        Err(error) => {
            if let Some(batch) = batch.as_mut() {
                batch.abort(store);
            }
            Err(error)
        }
    }
}

/// Attachment files the import must copy. A file already present with the
/// same hash is kept; one with another hash is a conflict.
fn attachment_files(
    store: &Store,
    loaded: &LoadedPackage,
    resolutions: &HashMap<&str, &PackageImportResolution>,
) -> Result<Vec<NewAttachmentFile>> {
    if loaded.attachment_files.is_empty() {
        return Ok(Vec::new());
    }
    let directory = attachment_directory(store.path());
    let mut files_to_copy = Vec::new();
    for record in &loaded.records {
        if resolutions
            .get(record.record_id.as_str())
            .is_some_and(|resolution| resolution.choice == ConflictChoice::Local)
        {
            continue;
        }
        let RecordData::Attachment(data) = &record.data else { continue };
        if record.is_deleted {
            continue;
        }
        let source = loaded
            .attachment_files
            .get(&record.record_id)
            .ok_or_else(|| Error::refused(format!("attachment bytes are missing: {}", record.record_id)))?;
        let name = stored_attachment_name(&record.record_id, data);
        let target = directory.join(&name);
        files::refuse_symbolic_link_components(&target, true)?;
        match fs::read(&target) {
            Ok(existing) => {
                if durable::sha256_hex(&existing) != data.sha256 {
                    return Err(Error::refused(format!("local attachment conflicts: {}", record.record_id)));
                }
                continue;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        files_to_copy.push(NewAttachmentFile {
            stored_name: name,
            source: AttachmentSource::File(source.clone()),
            byte_count: data.byte_count,
            sha256: data.sha256.clone(),
        });
    }
    Ok(files_to_copy)
}

// ---- Row writes ----------------------------------------------------------------------

fn insert_parent(tx: &Transaction<'_>, record: &CurrentRecord) -> Result<()> {
    tx.prepare_cached(
        "INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![
        record.record_id,
        record.record_type().as_str(),
        record.current_revision as i64,
        record.created_at,
        i64::from(record.is_deleted)
    ])?;
    Ok(())
}

fn insert_revision(tx: &Transaction<'_>, revision: &RecordRevision) -> Result<()> {
    tx.prepare_cached(
        "INSERT INTO record_revisions (
            record_id, revision, record_type, record_json, extensions_json,
            became_known_at, became_known_at_basis, changed_by_json, previous_revision
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?
    .execute(params![
        revision.record_id,
        revision.revision as i64,
        revision.record_type().as_str(),
        revision.data.to_json(),
        revision.extensions.as_ref().map(|map| json::to_string(&Value::Object(map.clone()))),
        revision.became_known_at,
        revision.became_known_at_basis.as_str(),
        json::encode(&revision.changed_by),
        revision.previous_revision.map(|number| number as i64),
    ])?;
    Ok(())
}

fn write_current(tx: &Transaction<'_>, layout: Layout, record: &CurrentRecord) -> Result<()> {
    encode::write_current_state(tx, layout, &record.record_id, &record.data, &record.data.to_json())
}

fn insert_deletion(tx: &Transaction<'_>, deletion: &RecordDeletion) -> Result<()> {
    tx.prepare_cached(
        "INSERT INTO record_deletions (record_id, record_revision, deleted_at, deleted_by_json) VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute(params![
        deletion.record_id,
        deletion.record_revision as i64,
        deletion.deleted_at,
        json::encode(&deletion.deleted_by)
    ])?;
    Ok(())
}

fn insert_import_key(tx: &Transaction<'_>, key: &RecordImportKey) -> Result<()> {
    tx.prepare_cached(
        "INSERT INTO record_import_keys (record_id, match_key_method, match_key, imported_at) VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute(params![key.record_id, key.match_key_method, key.match_key, key.imported_at])?;
    Ok(())
}

fn insert_erasure_marker(tx: &Transaction<'_>, marker: &RecordErasureMarker) -> Result<()> {
    tx.prepare_cached("INSERT INTO record_erasure_markers (record_id, erased_at, reason_code) VALUES (?1, ?2, ?3)")?
        .execute(params![marker.record_id, marker.erased_at, marker.reason_code.as_str()])?;
    Ok(())
}

/// Insert new samples with their history: parents, revisions, current state,
/// deletions, then import keys.
fn insert_new_samples(
    tx: &Transaction<'_>,
    layout: Layout,
    batch: &Batch<'_>,
    ids: Option<&HashSet<String>>,
) -> Result<()> {
    let wanted = |id: &str| ids.is_none_or(|ids| ids.contains(id));
    for record in batch.records.iter().filter(|record| wanted(&record.record_id)) {
        if record.record_type() != RecordType::MeasurementSample {
            return Err(Error::refused("measurement-series file contains a non-sample record"));
        }
        insert_parent(tx, record)?;
    }
    for revision in batch.revisions.iter().filter(|revision| wanted(&revision.record_id)) {
        insert_revision(tx, revision)?;
    }
    for record in batch.records.iter().filter(|record| wanted(&record.record_id)) {
        write_current(tx, layout, record)?;
    }
    for deletion in batch.deletions.iter().filter(|deletion| wanted(&deletion.record_id)) {
        insert_deletion(tx, deletion)?;
    }
    for key in batch.import_keys.iter().filter(|key| wanted(&key.record_id)) {
        insert_import_key(tx, key)?;
    }
    Ok(())
}

/// Restore into an empty database.
fn restore_in(tx: &Transaction<'_>, layout: Layout, loaded: &LoadedPackage) -> Result<()> {
    for record in &loaded.records {
        insert_parent(tx, record)?;
    }
    for revision in &loaded.revisions {
        insert_revision(tx, revision)?;
    }
    for record in &loaded.records {
        write_current(tx, layout, record)?;
    }
    for deletion in &loaded.deletions {
        insert_deletion(tx, deletion)?;
    }
    for key in &loaded.import_keys {
        insert_import_key(tx, key)?;
    }
    for file in &loaded.series_files {
        let series = read_series_batch(file)?;
        insert_new_samples(tx, layout, &Batch::of_series(&series), None)?;
        failpoint!("restore.after_series_file");
    }
    for record in &loaded.records {
        write::validate_relationships(tx, &record.data)?;
    }
    for marker in &loaded.erasure_markers {
        insert_erasure_marker(tx, marker)?;
    }
    tx.execute("UPDATE database_metadata SET database_id = ?1 WHERE singleton_id = 1", [&loaded.database_id])?;
    tx.execute(
        "UPDATE database_state SET current_revision = ?1 WHERE singleton_id = 1",
        [loaded.database_revision as i64],
    )?;
    Ok(())
}

/// Merge a package from the same database, in the order version 2.1.0 used.
fn merge_in(
    tx: &Transaction<'_>,
    layout: Layout,
    preview: &PackageImportPreview,
    loaded: &LoadedPackage,
    resolutions: &HashMap<&str, &PackageImportResolution>,
    resolution_time: &str,
) -> Result<()> {
    let mut imported_conflicts: HashMap<String, CurrentRecord> = HashMap::new();
    let mut to_validate: Vec<RecordData> = Vec::new();
    let (mut observed_new, mut observed_matching, mut observed_changed) = (0u64, 0u64, 0u64);
    for_each_batch(loaded, |batch| {
        let classification = classify(tx, batch)?;
        observed_new += classification.new_ids.len() as u64;
        observed_matching += classification.matching_ids.len() as u64;
        observed_changed += classification.changed_ids.len() as u64;
        for conflict in &classification.conflicts {
            let expected = conflict
                .record_id
                .as_ref()
                .and_then(|id| preview.conflicts.iter().find(|item| item.record_id.as_ref() == Some(id)));
            if expected.is_none_or(|expected| expected.message != conflict.message) {
                return Err(Error::stale("package conflict changed after preview"));
            }
            let id = conflict.record_id.clone().unwrap_or_default();
            if let Some(record) = batch.records.iter().find(|record| record.record_id == id) {
                imported_conflicts.insert(id, record.clone());
            }
        }
        let new_ids: HashSet<String> = classification.new_ids.iter().cloned().collect();
        let changed_ids: HashSet<String> = classification.changed_ids.iter().cloned().collect();
        let new_samples: HashSet<String> = batch
            .records
            .iter()
            .filter(|record| {
                record.record_type() == RecordType::MeasurementSample && new_ids.contains(&record.record_id)
            })
            .map(|record| record.record_id.clone())
            .collect();
        if !new_samples.is_empty() {
            insert_new_samples(tx, layout, batch, Some(&new_samples))?;
        }
        let generic_new: HashSet<&String> = new_ids.iter().filter(|id| !new_samples.contains(*id)).collect();
        for record in batch.records.iter().filter(|record| generic_new.contains(&record.record_id)) {
            insert_parent(tx, record)?;
        }
        for revision in batch.revisions {
            if generic_new.contains(&revision.record_id) {
                insert_revision(tx, revision)?;
            } else if changed_ids.contains(&revision.record_id) {
                let present = tx
                    .prepare_cached("SELECT 1 FROM record_revisions WHERE record_id = ?1 AND revision = ?2")?
                    .exists(params![revision.record_id, revision.revision as i64])?;
                if !present {
                    insert_revision(tx, revision)?;
                }
            }
        }
        for record in batch.records {
            if generic_new.contains(&record.record_id) || changed_ids.contains(&record.record_id) {
                write_current(tx, layout, record)?;
                tx.prepare_cached("UPDATE records SET current_revision = ?1, is_deleted = ?2 WHERE record_id = ?3")?
                    .execute(params![record.current_revision as i64, i64::from(record.is_deleted), record.record_id])?;
                to_validate.push(record.data.clone());
            }
        }
        for deletion in batch.deletions {
            if new_samples.contains(&deletion.record_id) || resolutions.contains_key(deletion.record_id.as_str()) {
                continue;
            }
            let present = tx
                .prepare_cached("SELECT 1 FROM record_deletions WHERE record_id = ?1")?
                .exists([&deletion.record_id])?;
            if !present {
                insert_deletion(tx, deletion)?;
            }
        }
        for key in batch.import_keys {
            let local_choice = resolutions
                .get(key.record_id.as_str())
                .is_some_and(|resolution| resolution.choice == ConflictChoice::Local);
            if new_samples.contains(&key.record_id) || local_choice {
                continue;
            }
            let owner: Option<String> = tx
                .prepare_cached(
                    "SELECT record_id FROM record_import_keys WHERE match_key_method = ?1 AND match_key = ?2",
                )?
                .query_row(params![key.match_key_method, key.match_key], |row| row.get(0))
                .optional()?;
            match owner {
                None => insert_import_key(tx, key)?,
                Some(owner) if owner != key.record_id => {
                    return Err(Error::stale("provider import key changed after preview"));
                }
                Some(_) => {}
            }
        }
        Ok(())
    })?;
    if observed_new != preview.new_record_count
        || observed_matching != preview.matching_record_count
        || observed_changed != preview.changed_record_count
    {
        return Err(Error::stale("package records changed after preview"));
    }

    let mut resolved: Vec<RecordData> = Vec::new();
    for conflict in &preview.conflicts {
        let Some(id) = conflict.record_id.as_deref() else { continue };
        let Some(resolution) = resolutions.get(id) else { continue };
        if resolution.choice == ConflictChoice::Local {
            continue;
        }
        resolved.push(resolve_in(tx, layout, resolution, imported_conflicts.get(id), resolution_time)?);
    }
    for data in to_validate.iter().chain(&resolved) {
        write::validate_relationships(tx, data)?;
    }
    for marker in &loaded.erasure_markers {
        let existing: Option<(String, String)> = tx
            .prepare_cached("SELECT erased_at, reason_code FROM record_erasure_markers WHERE record_id = ?1")?
            .query_row([&marker.record_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional()?;
        match existing {
            None => insert_erasure_marker(tx, marker)?,
            Some((erased_at, reason)) if erased_at != marker.erased_at || reason != marker.reason_code.as_str() => {
                return Err(Error::stale("erasure marker changed after preview"));
            }
            Some(_) => {}
        }
    }
    tx.execute("UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1", [])?;
    Ok(())
}

/// Write the chosen value of one conflict as a new local revision.
fn resolve_in(
    tx: &Transaction<'_>,
    layout: Layout,
    resolution: &PackageImportResolution,
    imported: Option<&CurrentRecord>,
    resolution_time: &str,
) -> Result<RecordData> {
    let id = resolution.record_id.as_str();
    let imported = imported.ok_or_else(|| Error::stale(format!("imported conflict record is missing: {id}")))?;
    let current: Option<(String, i64, String, i64, String)> = tx
        .query_row(
            "SELECT records.record_type, records.current_revision, records.created_at, records.is_deleted,
                    record_revisions.became_known_at
             FROM records
             JOIN record_revisions
               ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             WHERE records.record_id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let (record_type, current_revision, created_at, is_deleted, became_known_at) =
        current.ok_or_else(|| Error::stale(format!("local conflict record is missing: {id}")))?;
    if record_type != imported.record_type().as_str() || is_deleted != 0 || imported.is_deleted {
        return Err(Error::refused(format!("conflict cannot change record type or deleted state: {id}")));
    }
    if time::parse_utc_timestamp(resolution_time) < time::parse_utc_timestamp(&became_known_at) {
        return Err(Error::refused(format!("conflict resolution time precedes the current revision: {id}")));
    }
    let data_value = match &resolution.choice {
        ConflictChoice::Corrected { data } => data.clone(),
        _ => imported.data.to_value(),
    };
    let next = current_revision as u64 + 1;
    let mut record_value = json::Map::new();
    record_value.insert("record_id".into(), Value::String(id.into()));
    record_value.insert("record_type".into(), Value::String(record_type));
    record_value.insert("created_at".into(), Value::String(created_at));
    record_value.insert("current_revision".into(), Value::from(next));
    record_value.insert("is_deleted".into(), Value::Bool(false));
    record_value.insert("data".into(), data_value);
    let record = CurrentRecord::from_value(&Value::Object(record_value))?;
    let revision = RecordRevision {
        record_id: id.into(),
        revision: next,
        previous_revision: Some(current_revision as u64),
        became_known_at: resolution_time.into(),
        became_known_at_basis: BecameKnownAtBasis::PackageMerge,
        changed_by: RecordActor::Athlete,
        data: record.data.clone(),
        extensions: None,
    };
    RecordRevision::from_value(&revision.to_value())?;
    insert_revision(tx, &revision)?;
    write_current(tx, layout, &record)?;
    tx.execute("UPDATE records SET current_revision = ?1 WHERE record_id = ?2", params![next as i64, id])?;
    Ok(record.data)
}
