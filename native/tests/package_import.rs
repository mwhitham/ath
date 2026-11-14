//! Package restore, merge, conflicts, and input safety.
//! Ported from tests/v1-export-import.test.ts and tests/v1-package-safety.test.ts.

#[path = "package_support/mod.rs"]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use athleticstandard::json::{self, Value};
use athleticstandard::model::*;
use athleticstandard::package::*;
use athleticstandard::store::{self, read, Changes, Correction, ExtensionsUpdate, NewRecord, Store};
use athleticstandard::Error;
use common::*;
use serde_json::json;

const PROFILE_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000002";

/// `ath import <package> --yes` as the command composes it: preview, then
/// apply with the saved choices when there are conflicts.
fn import_confirmed(store: &mut Store, package: &Path) -> athleticstandard::Result<PackageImportApplied> {
    let preview = preview_package_import(store, package)?;
    let mut resolutions = Vec::new();
    if !preview.conflicts.is_empty() {
        let state = read_conflict_set(store)?;
        assert_conflict_set_current(store, &state, &preview)?;
        resolutions = conflict_resolutions(&state);
        if resolutions.len() != preview.conflicts.len() {
            return Err(Error::refused(format!(
                "import has {} unresolved conflict(s)",
                preview.conflicts.len() - resolutions.len()
            )));
        }
    }
    let applied = apply_package_import(store, &preview, &resolutions)?;
    clear_conflict_set(store)?;
    Ok(applied)
}

/// `ath import <package> --json`: preview and save or clear the conflict file.
fn import_preview(store: &Store, package: &Path) -> PackageImportPreview {
    let preview = preview_package_import(store, package).unwrap();
    if preview.conflicts.is_empty() {
        clear_conflict_set(store).unwrap();
    } else {
        save_conflict_set(store, &preview).unwrap();
    }
    preview
}

fn restored_copy(package: &Path, directory: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(directory.join(name)).unwrap();
    let path = empty_destination(&directory.join(name));
    let mut store = open(&path);
    import_confirmed(&mut store, package).unwrap();
    store.close().unwrap();
    path
}

fn rename_profile(path: &Path, name: &str) {
    let mut store = open(path);
    let current = store.get_record(PROFILE_ID).unwrap().unwrap();
    let mut data = current.data.to_value();
    data["name"] = Value::String(name.into());
    store
        .apply(Changes {
            corrections: vec![Correction {
                record_id: PROFILE_ID.into(),
                expected_revision: current.current_revision,
                data: RecordData::decode(RecordType::AthleteProfile, &data).unwrap(),
                extensions: ExtensionsUpdate::Keep,
                changed_by: RecordActor::Athlete,
                became_known_at: None,
                became_known_at_basis: BecameKnownAtBasis::LocalWrite,
                stale_style: store::write::StaleStyle::Revision,
            }],
            ..Changes::default()
        })
        .unwrap();
    store.close().unwrap();
}

fn log_report(path: &Path, note: &str) {
    let mut store = open(path);
    let data = RecordData::decode(
        RecordType::AthleteReport,
        &json!({
            "report_type": "note",
            "reported_at": time::now_utc(),
            "input_reported_at": { "timestamp_missing_reason": "not_applicable", "utc_offset_missing_reason": "not_applicable" },
            "reported_via": "text",
            "note": note,
        }),
    )
    .unwrap();
    store.apply(Changes { creates: vec![NewRecord::new(data, RecordActor::Athlete)], ..Changes::default() }).unwrap();
    store.close().unwrap();
}

fn export(path: &Path, output: &Path) {
    let store = open_read_only(path);
    export_package(&store, output, &ExportOptions::default()).unwrap();
}

#[test]
fn a_restore_preview_writes_nothing_then_the_restore_keeps_everything() {
    let directory = temporary("ath-restore-");
    let package = materialize_golden_package("demo", &directory);
    let destination = empty_destination(&directory);
    let mut store = open(&destination);
    let preview = preview_package_import(&store, &package).unwrap();
    assert!(preview.restore);
    assert_eq!(preview.new_record_count, 25);
    assert_eq!(preview.matching_record_count, 0);
    assert_eq!(preview.changed_record_count, 0);
    assert!(!preview.record_lists_truncated);
    assert!(preview.conflicts.is_empty());
    assert!(!preview.write_confirmed);
    assert!(store.list_records(&read::ListOptions { include_deleted: true, ..Default::default() }).unwrap().is_empty());
    assert_eq!(store.database_revision().unwrap(), 0);

    let applied = apply_package_import(&mut store, &preview, &[]).unwrap();
    assert_eq!(applied, PackageImportApplied { database_revision: 3, restored: true });
    let records =
        store.list_records(&read::ListOptions { include_deleted: true, limit: 100, ..Default::default() }).unwrap();
    assert_eq!(records.len(), 25);
    assert_eq!(store::read_metadata(store.connection()).unwrap().database_id, "01890f3e-e7d3-7cc0-88c8-000000000001");
    assert_eq!(store.revisions("01890f3e-e7d3-7cc0-88c8-000000000007").unwrap().len(), 2);
    assert!(directory.join("attachments").join(DEMO_ATTACHMENT).is_file());
    assert!(pending_file_operations(store.connection()).unwrap().is_empty());

    // The same package again is a merge in which every record matches.
    let again = preview_package_import(&store, &package).unwrap();
    assert!(!again.restore);
    assert_eq!(again.matching_record_count, 25);
    assert_eq!(again.new_record_count, 0);
    assert!(again.conflicts.is_empty());
    let applied = apply_package_import(&mut store, &again, &[]).unwrap();
    assert_eq!(applied, PackageImportApplied { database_revision: 4, restored: false });
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_merge_adds_later_revisions_and_new_records() {
    let directory = temporary("ath-merge-");
    let base = materialize_golden_package("demo", &directory);
    let local = restored_copy(&base, &directory, "local");
    let remote = restored_copy(&base, &directory, "remote");
    rename_profile(&remote, "Remote name");
    log_report(&remote, "Remote note");
    let incoming = directory.join("incoming");
    export(&remote, &incoming);

    let mut store = open(&local);
    let preview = preview_package_import(&store, &incoming).unwrap();
    assert_eq!(preview.changed_records, [PROFILE_ID]);
    assert_eq!(preview.new_record_count, 1);
    assert_eq!(preview.matching_record_count, 24);
    assert!(preview.conflicts.is_empty());
    let applied = apply_package_import(&mut store, &preview, &[]).unwrap();
    assert!(!applied.restored);
    let profile = store.get_record(PROFILE_ID).unwrap().unwrap();
    assert_eq!(profile.current_revision, 2);
    assert_eq!(profile.data.to_value()["name"], "Remote name");
    let remote_store = open_read_only(&remote);
    assert_eq!(store.revisions(PROFILE_ID).unwrap(), remote_store.revisions(PROFILE_ID).unwrap());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn package_conflicts_are_shown_resolved_and_refused_when_stale() {
    let directory = temporary("ath-conflict-");
    let base = materialize_golden_package("demo", &directory);
    let local = restored_copy(&base, &directory, "local");
    let imported = restored_copy(&base, &directory, "imported");
    rename_profile(&local, "Local name");
    rename_profile(&imported, "Imported name");
    let incoming = directory.join("incoming");
    export(&imported, &incoming);

    let mut store = open(&local);
    let preview = import_preview(&store, &incoming);
    assert_eq!(
        json::to_value(&preview.conflicts),
        json!([{ "record_id": PROFILE_ID, "message": "record revision history differs" }])
    );
    assert!(conflict_set_path(&store).is_file());

    let shown = show_conflicts(&store, Some(PROFILE_ID)).unwrap();
    assert_eq!(shown.conflicts.len(), 1);
    assert_eq!(shown.conflicts[0].local.as_ref().unwrap()["data"]["name"], "Local name");
    assert_eq!(shown.conflicts[0].imported.as_ref().unwrap()["data"]["name"], "Imported name");
    assert_eq!(shown.conflicts[0].resolution, None);
    let shown_json = json::to_value(&shown);
    assert_eq!(shown_json["conflicts"][0]["resolution"], Value::Null);
    let keys: Vec<&String> = shown_json.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["source_path", "source_database_revision", "destination_database_revision", "conflicts"]);
    let missing = show_conflicts(&store, Some("01890f3e-e7d3-7cc0-88c8-0000000000aa")).unwrap_err();
    assert_eq!(
        missing.to_string(),
        "record is not in the temporary conflict set: 01890f3e-e7d3-7cc0-88c8-0000000000aa"
    );

    let unresolved = import_confirmed(&mut store, &incoming).unwrap_err();
    assert!(unresolved.to_string().contains("1 unresolved conflict"), "{unresolved}");

    let recorded = resolve_conflict(&store, PROFILE_ID, ConflictChoice::Imported).unwrap();
    assert_eq!(
        json::to_value(&recorded),
        json!({ "record_id": PROFILE_ID, "choice": "imported", "unresolved_conflicts": 0, "athlete_data_changed": false })
    );
    store.close().unwrap();

    // A change after the choices makes them stale.
    log_report(&local, "Database changed after conflict choices");
    let mut store = open(&local);
    let stale = import_confirmed(&mut store, &incoming).unwrap_err();
    assert!(stale.to_string().contains("changed after the conflict preview"), "{stale}");
    assert!(resolve_conflict(&store, PROFILE_ID, ConflictChoice::Imported)
        .unwrap_err()
        .to_string()
        .contains("changed after the conflict preview"));

    // A new preview resets the choices; a recorded choice then applies.
    import_preview(&store, &incoming);
    assert_eq!(read_conflict_set(&store).unwrap().conflicts[0].resolution, None);
    resolve_conflict(&store, PROFILE_ID, ConflictChoice::Imported).unwrap();
    import_confirmed(&mut store, &incoming).unwrap();
    let profile = store.get_record(PROFILE_ID).unwrap().unwrap();
    assert_eq!(profile.current_revision, 3);
    assert_eq!(profile.data.to_value()["name"], "Imported name");
    let history = store.revisions(PROFILE_ID).unwrap();
    assert_eq!(history[2].became_known_at_basis, BecameKnownAtBasis::PackageMerge);
    assert_eq!(history[2].changed_by, RecordActor::Athlete);
    assert!(!conflict_set_path(&store).exists());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn local_and_corrected_choices_apply() {
    let directory = temporary("ath-conflict-choices-");
    let base = materialize_golden_package("demo", &directory);
    let local = restored_copy(&base, &directory, "local");
    let imported = restored_copy(&base, &directory, "imported");
    rename_profile(&local, "Local name");
    rename_profile(&imported, "Imported name");
    let incoming = directory.join("incoming");
    export(&imported, &incoming);

    // Keep local: the record does not change, the database revision advances.
    let copy = directory.join("copy");
    fs::create_dir(&copy).unwrap();
    let kept = copy.join("destination.ath.sqlite");
    fs::copy(&local, &kept).unwrap();
    let mut store = open(&kept);
    import_preview(&store, &incoming);
    resolve_conflict(&store, PROFILE_ID, ConflictChoice::Local).unwrap();
    let before = store.database_revision().unwrap();
    import_confirmed(&mut store, &incoming).unwrap();
    assert_eq!(store.database_revision().unwrap(), before + 1);
    assert_eq!(store.get_record(PROFILE_ID).unwrap().unwrap().current_revision, 2);
    store.close().unwrap();

    // A corrected value must be complete and valid for the record type.
    let mut store = open(&local);
    import_preview(&store, &incoming);
    let invalid = resolve_conflict(&store, PROFILE_ID, ConflictChoice::Corrected { data: json!({ "name": 5 }) });
    assert!(matches!(invalid, Err(Error::Invalid(_))), "{invalid:?}");
    let mut corrected = store.get_record(PROFILE_ID).unwrap().unwrap().data.to_value();
    corrected["name"] = Value::String("Agreed name".into());
    let result = resolve_conflict(&store, PROFILE_ID, ConflictChoice::Corrected { data: corrected }).unwrap();
    assert_eq!(result.choice, "corrected");
    let saved = fs::read_to_string(conflict_set_path(&store)).unwrap();
    assert!(saved.contains("\"choice\": \"corrected\""), "{saved}");
    import_confirmed(&mut store, &incoming).unwrap();
    let profile = store.get_record(PROFILE_ID).unwrap().unwrap();
    assert_eq!(profile.current_revision, 3);
    assert_eq!(profile.data.to_value()["name"], "Agreed name");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_missing_conflict_file_says_how_to_create_one() {
    let directory = temporary("ath-conflict-missing-");
    let database = empty_destination(&directory);
    let store = open(&database);
    let error = show_conflicts(&store, None).unwrap_err();
    assert_eq!(error.to_string(), "no temporary import conflicts; run ath import <package> to create a preview");
    fs::write(conflict_set_path(&store), "{\"conflict_set_version\":2}").unwrap();
    assert!(read_conflict_set(&store)
        .unwrap_err()
        .to_string()
        .starts_with("temporary import conflict file is invalid"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn another_nonempty_database_conflicts_and_prerelease_json_is_refused() {
    let directory = temporary("ath-restore-other-");
    let package = materialize_golden_package("demo", &directory);
    let destination = empty_destination(&directory);
    log_report(&destination, "Something already here");
    let store = open(&destination);
    let preview = preview_package_import(&store, &package).unwrap();
    assert!(!preview.restore);
    assert_eq!(preview.conflicts.len(), 1);
    assert_eq!(preview.conflicts[0].record_id, None);
    assert!(preview.conflicts[0].message.contains("live databases are not merged"));
    assert_eq!(preview.new_record_count, 0);
    assert!(save_conflict_set(&store, &preview).unwrap().is_none());

    let prerelease = repository().join("examples/demo-athlete/athlete.ath.json");
    let error = preview_package_import(&store, &prerelease).unwrap_err();
    assert!(error.to_string().contains("prerelease JSON format"), "{error}");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn malformed_deep_oversized_and_linked_documents_are_refused() {
    let directory = temporary("ath-package-safety-");
    let malformed = directory.join("malformed.json");
    fs::write(&malformed, "{").unwrap();
    assert!(load_package(&malformed).unwrap_err().to_string().starts_with("package JSON is invalid"));

    let nested = directory.join("nested.json");
    fs::write(&nested, format!("{}null{}", "{\"child\":".repeat(129), "}".repeat(129))).unwrap();
    assert!(load_package(&nested).unwrap_err().to_string().starts_with("package JSON exceeds nesting limit 128"));

    let oversized = directory.join("oversized.json");
    fs::write(&oversized, "{}").unwrap();
    fs::OpenOptions::new().write(true).open(&oversized).unwrap().set_len(512 * 1024 * 1024 + 1).unwrap();
    assert!(load_package(&oversized).unwrap_err().to_string().starts_with("package JSON exceeds"));

    #[cfg(unix)]
    {
        let linked = directory.join("linked.json");
        std::os::unix::fs::symlink(&malformed, &linked).unwrap();
        assert!(load_package(&linked).unwrap_err().to_string().contains("symbolic link"));
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_series_file_outside_the_package_is_refused() {
    let directory = temporary("ath-package-traversal-");
    let package = materialize_golden_package("demo", &directory);
    fs::remove_file(package.join("export-manifest.json")).unwrap();
    let path = package.join("athlete.ath.json");
    let mut document = json::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    document["measurement_series_files"][0]["file"] = Value::String("../outside.json".into());
    fs::write(&path, format!("{}\n", json::to_string(&document))).unwrap();
    let error = match load_package_checked(&package).unwrap() {
        PackageLoad::Unrepresentable(representation) => representation.message(),
        PackageLoad::Loaded(_) => panic!("a traversing path loaded"),
    };
    assert!(error.contains("Package path must be relative") || error.contains("package file escapes"), "{error}");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_destination_changed_after_preview_is_refused_and_copied_files_are_removed() {
    let directory = temporary("ath-package-cleanup-");
    let package = materialize_golden_package("demo", &directory);
    let destination = empty_destination(&directory);
    let store = open(&destination);
    let preview = preview_package_import(&store, &package).unwrap();
    assert!(preview.restore);
    store.close().unwrap();
    log_report(&destination, "Changed after preview");
    let mut store = open(&destination);
    let error = apply_package_import(&mut store, &preview, &[]).unwrap_err();
    assert_eq!(error.to_string(), "destination database changed after preview");
    let attachments = directory.join("attachments");
    let left: Vec<_> = fs::read_dir(&attachments).map(|entries| entries.collect()).unwrap_or_default();
    assert!(left.is_empty(), "{left:?}");
    assert!(pending_file_operations(store.connection()).unwrap().is_empty());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_series_file_changed_after_preview_rolls_back_the_restore() {
    let directory = temporary("ath-package-changed-series-");
    let package = materialize_golden_package("demo", &directory);
    let destination = empty_destination(&directory);
    let mut store = open(&destination);
    let preview = preview_package_import(&store, &package).unwrap();
    let series = preview.loaded.as_ref().unwrap().series_files[0].path.clone();
    let text = fs::read_to_string(&series).unwrap();
    fs::write(&series, format!("{text}\n")).unwrap();
    let error = apply_package_import(&mut store, &preview, &[]).unwrap_err();
    assert!(error.to_string().starts_with("series hash changed after package load"), "{error}");
    assert!(store.list_records(&read::ListOptions { include_deleted: true, ..Default::default() }).unwrap().is_empty());
    assert_eq!(store.database_revision().unwrap(), 0);
    assert!(!directory.join("attachments").join(DEMO_ATTACHMENT).exists());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_manifest_mismatch_is_refused() {
    let directory = temporary("ath-package-manifest-");
    let package = materialize_golden_package("demo", &directory);
    let attachment = package.join("attachments").join(DEMO_ATTACHMENT);
    let mut bytes = fs::read(&attachment).unwrap();
    bytes.push(0);
    fs::write(&attachment, bytes).unwrap();
    let error = load_package(&package).unwrap_err();
    assert_eq!(error.to_string(), format!("package file does not match its manifest: attachments/{DEMO_ATTACHMENT}"));
    fs::remove_dir_all(directory).unwrap();
}

/// Build a tiny package from source records: (record ID, route, deleted, supplied fields).
fn source_package(directory: &Path, name: &str, sources: &[(&str, &str, bool, Value)]) -> PathBuf {
    let path = directory.join(name);
    fs::create_dir_all(&path).unwrap();
    let mut records = Vec::new();
    let mut revisions = Vec::new();
    let mut deletions = Vec::new();
    for (id, route, deleted, supplied) in sources {
        let mut data = json!({ "source_type": "wearable", "data_writer": "S", "data_route": route });
        if !supplied.is_null() {
            data["supplied_fields"] = supplied.clone();
        }
        records.push(json!({ "record_id": id, "created_at": "2026-01-01T00:00:00.000Z", "current_revision": 1,
            "is_deleted": deleted, "record_type": "source", "data": data }));
        revisions.push(json!({ "record_id": id, "revision": 1, "previous_revision": null,
            "became_known_at": "2026-01-01T00:00:00.000Z", "became_known_at_basis": "local_write",
            "changed_by": { "author_type": "athlete" }, "record_type": "source", "data": data }));
        if *deleted {
            deletions.push(json!({ "record_id": id, "record_revision": 1, "deleted_at": "2026-01-02T00:00:00.000Z",
                "deleted_by": { "author_type": "athlete" } }));
        }
    }
    let document = json!({ "athleticstandard_version": "2.1.0", "database_id": "01890f3e-0000-7000-8000-000000000001",
        "database_revision": 5, "records": records, "record_revisions": revisions, "record_deletions": deletions,
        "record_erasure_markers": [], "record_import_keys": [], "measurement_series_files": [] });
    fs::write(path.join("athlete.ath.json"), json::to_string(&document)).unwrap();
    path
}

const SOURCE_A: &str = "01890f3e-0000-7000-8000-00000000000a";
const SOURCE_B: &str = "01890f3e-0000-7000-8000-00000000000b";

/// 2.1.0 compares records as JSON text, so supplied fields in another key
/// order are a conflict even when the values are the same. Kept as 2.1.0
/// behaves; see the report for the open question.
#[test]
fn reordered_supplied_fields_conflict_as_in_2_1_0() {
    let directory = temporary("ath-merge-order-");
    let base = source_package(&directory, "base", &[(SOURCE_A, "r-a", false, json!({ "zeta": 1, "alpha": 2 }))]);
    let reordered =
        source_package(&directory, "reordered", &[(SOURCE_A, "r-a", false, json!({ "alpha": 2, "zeta": 1 }))]);
    let database = empty_destination(&directory);
    let mut store = open(&database);
    import_confirmed(&mut store, &base).unwrap();
    let same = preview_package_import(&store, &base).unwrap();
    assert_eq!(same.matching_record_count, 1);
    let preview = preview_package_import(&store, &reordered).unwrap();
    assert_eq!(preview.matching_record_count, 0);
    assert_eq!(
        json::to_value(&preview.conflicts),
        json!([{ "record_id": SOURCE_A, "message": "record revision history differs" }])
    );
    fs::remove_dir_all(directory).unwrap();
}

/// 2.1.0 sets a new record's deleted flag before inserting its deletion, so
/// the database refuses a merge that adds a deleted record. Kept as 2.1.0
/// behaves; see the report for the open question. Nothing is written.
#[test]
fn a_merge_that_adds_a_deleted_record_is_refused_as_in_2_1_0() {
    let directory = temporary("ath-merge-deleted-");
    let base = source_package(&directory, "base", &[(SOURCE_A, "r-a", false, Value::Null)]);
    let more = source_package(
        &directory,
        "more",
        &[(SOURCE_A, "r-a", false, Value::Null), (SOURCE_B, "r-b", true, Value::Null)],
    );
    let database = empty_destination(&directory);
    let mut store = open(&database);
    import_confirmed(&mut store, &base).unwrap();
    let preview = preview_package_import(&store, &more).unwrap();
    assert_eq!(preview.new_record_count, 1);
    let error = apply_package_import(&mut store, &preview, &[]).unwrap_err();
    assert!(error.to_string().contains("records.is_deleted must match record_deletions"), "{error}");
    assert!(store.get_record(SOURCE_B).unwrap().is_none());
    assert_eq!(store.database_revision().unwrap(), 5);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn every_supported_package_version_is_accepted_and_others_are_reported() {
    let directory = temporary("ath-package-versions-");
    for version in ["2.0.0", "2.1.0", "2.1.1"] {
        let package = source_package(&directory, &format!("v{version}"), &[(SOURCE_A, "r-a", false, Value::Null)]);
        let path = package.join("athlete.ath.json");
        let text = fs::read_to_string(&path).unwrap().replace("\"2.1.0\"", &format!("\"{version}\""));
        fs::write(&path, text).unwrap();
        assert!(matches!(load_package_checked(&package).unwrap(), PackageLoad::Loaded(_)), "{version}");
    }
    let package = source_package(&directory, "v9", &[(SOURCE_A, "r-a", false, Value::Null)]);
    let path = package.join("athlete.ath.json");
    let text = fs::read_to_string(&path).unwrap().replace("\"2.1.0\"", "\"9.0.0\"");
    fs::write(&path, text).unwrap();
    match load_package_checked(&package).unwrap() {
        PackageLoad::Unrepresentable(representation) => {
            assert_eq!(representation.unknown_fields, ["athleticstandard_version: 9.0.0 is not supported by 2.1.1"]);
            assert_eq!(representation.source_database_id.as_deref(), Some("01890f3e-0000-7000-8000-000000000001"));
        }
        PackageLoad::Loaded(_) => panic!("an unknown version loaded"),
    }
    fs::remove_dir_all(directory).unwrap();
}
