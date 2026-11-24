//! D268: a write recorded on one copy and replayed on another leaves both copies the
//! same. A working copy built from a database's rows exports like the database; every
//! kind of write replays to the same rows and the same export; a replay from an old
//! revision or an old access epoch is refused; and an erasure clears the replication log.

// The maintenance helpers own this program's temporary folders.
mod maintenance_support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use athleticstandard::json::{self, Value};
use athleticstandard::model::{BecameKnownAtBasis, ErasureReasonCode, RecordActor, RecordData, RecordType};
use athleticstandard::package::{
    apply_package_import, erase_record, export_package, preview_erasure, preview_package_import, ExportOptions,
};
use athleticstandard::store::replica::{schema_scripts, Changeset, TableRows, NOT_COPIED};
use athleticstandard::store::write::StaleStyle;
use athleticstandard::store::{Changes, Correction, Deletion, ExtensionsUpdate, NewRecord, OpenOptions, Store};
use serde_json::json;

const NOTE: &str = "01890f3e-e7d3-7cc0-88c8-0000000000d1";
const OTHER: &str = "01890f3e-e7d3-7cc0-88c8-0000000000d2";
const WRITTEN: &str = "2026-11-21T08:00:00.000Z";

/// One labelled write applied to a store.
type Step = (&'static str, Box<dyn Fn(&mut Store)>);

fn demo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/demo-athlete")
}

fn copy_folder(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_folder(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

/// A copy of the demo record in `directory/name`.
fn demo(directory: &Path, name: &str) -> Store {
    let folder = directory.join(name);
    copy_folder(&demo_root(), &folder);
    Store::open(folder.join("athlete.ath.sqlite"), OpenOptions::default()).unwrap()
}

fn all_rows(store: &Store) -> Vec<TableRows> {
    store
        .table_shapes()
        .unwrap()
        .into_iter()
        .filter(|shape| !NOT_COPIED.contains(&shape.name.as_str()))
        .map(|shape| store.table_rows(&shape.name).unwrap())
        .collect()
}

/// A working copy of `source` in `directory/name`, with its attachment files.
fn working_copy(source: &Store, directory: &Path, name: &str) -> Store {
    let folder = directory.join(name);
    fs::create_dir_all(&folder).unwrap();
    let attachments = source.path().parent().unwrap().join("attachments");
    if attachments.is_dir() {
        copy_folder(&attachments, &folder.join("attachments"));
    }
    Store::build_working_copy(folder.join("athlete.ath.sqlite"), &all_rows(source)).unwrap()
}

fn exported(store: &Store, directory: &Path, name: &str) -> BTreeMap<String, Vec<u8>> {
    let output = directory.join(name);
    export_package(store, &output, &ExportOptions::default()).unwrap();
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(&output).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            files.insert(path.file_name().unwrap().to_string_lossy().into_owned(), fs::read(&path).unwrap());
        }
    }
    files
}

fn assert_same(left: &Store, right: &Store, directory: &Path, label: &str) {
    assert_eq!(left.database_revision().unwrap(), right.database_revision().unwrap(), "{label}: revision");
    assert_eq!(left.metadata().database_id, right.metadata().database_id, "{label}: database ID");
    assert_eq!(all_rows(left), all_rows(right), "{label}: rows");
    assert_eq!(
        exported(left, directory, &format!("{label}-left")),
        exported(right, directory, &format!("{label}-right")),
        "{label}: export"
    );
}

fn athlete() -> RecordActor {
    serde_json::from_value(json!({ "author_type": "athlete" })).unwrap()
}

fn note_data(text: &str) -> Value {
    json::parse(
        &json!({
            "authorship": {
                "author": { "author_type": "coach", "coach": "Example Coach", "coach_id": "6f1c2e4a-3b5d-4c7e-9f80-1a2b3c4d5e6f" },
                "authored_at": WRITTEN,
                "evidence": [],
                "content_version": 1,
                "status": "active"
            },
            "title": "Deload week",
            "note": text
        })
        .to_string(),
    )
    .unwrap()
}

fn coach() -> RecordActor {
    serde_json::from_value(
        json!({ "author_type": "coach", "coach": "Example Coach", "coach_id": "6f1c2e4a-3b5d-4c7e-9f80-1a2b3c4d5e6f" }),
    )
    .unwrap()
}

fn create(record_id: &str, text: &str) -> Changes {
    Changes {
        creates: vec![NewRecord {
            record_id: Some(record_id.into()),
            created_at: Some(WRITTEN.into()),
            ..NewRecord::new(RecordData::decode(RecordType::CoachingNote, &note_data(text)).unwrap(), coach())
        }],
        ..Changes::default()
    }
}

fn correct(record_id: &str, revision: u64, text: &str) -> Changes {
    Changes {
        corrections: vec![Correction {
            record_id: record_id.into(),
            expected_revision: revision,
            data: RecordData::decode(RecordType::CoachingNote, &note_data(text)).unwrap(),
            extensions: ExtensionsUpdate::Keep,
            changed_by: coach(),
            became_known_at: Some(WRITTEN.into()),
            became_known_at_basis: BecameKnownAtBasis::LocalWrite,
            stale_style: StaleStyle::Revision,
        }],
        ..Changes::default()
    }
}

#[test]
fn a_working_copy_exports_like_its_source() {
    let directory = maintenance_support::temporary_dir("ath-replica-copy-");
    let source = demo(&directory, "source");
    let copy = working_copy(&source, &directory, "copy");
    assert_same(&source, &copy, &directory, "built");
}

#[test]
fn every_kind_of_write_replays_to_the_same_database() {
    let directory = maintenance_support::temporary_dir("ath-replica-writes-");
    let mut source = demo(&directory, "source");
    let mut copy = working_copy(&source, &directory, "copy");

    let steps: Vec<Step> = vec![
        (
            "create",
            Box::new(|store: &mut Store| {
                store.apply(create(NOTE, "Keep every session easy.")).unwrap();
            }),
        ),
        (
            "create two in one write",
            Box::new(|store: &mut Store| {
                let mut changes = create(OTHER, "Sleep eight hours.");
                changes.corrections = correct(NOTE, 1, "Keep every session easy, and walk daily.").corrections;
                store.apply(changes).unwrap();
            }),
        ),
        (
            "delete",
            Box::new(|store: &mut Store| {
                store
                    .apply(Changes {
                        deletions: vec![Deletion {
                            record_id: OTHER.into(),
                            expected_revision: 1,
                            deleted_by: athlete(),
                            deleted_at: Some(WRITTEN.into()),
                        }],
                        ..Changes::default()
                    })
                    .unwrap();
            }),
        ),
        (
            "erase",
            Box::new(|store: &mut Store| {
                let preview = preview_erasure(store, NOTE, 2).unwrap();
                erase_record(store, &preview, ErasureReasonCode::AthleteRequest, Some(WRITTEN)).unwrap();
            }),
        ),
    ];
    for (label, step) in steps {
        let captured = source.capture(|store| {
            step(store);
            Ok(())
        });
        let captured = captured.unwrap();
        assert!(!captured.changeset.is_empty(), "{label}: something was recorded");
        assert_eq!(captured.base_revision, copy.database_revision().unwrap());
        copy.replay(captured.base_revision, captured.access_epoch, &captured.changeset).unwrap();
        assert_same(&source, &copy, &directory, label);
    }
}

#[test]
fn a_replay_from_an_old_revision_or_epoch_is_refused() {
    let directory = maintenance_support::temporary_dir("ath-replica-guard-");
    let mut source = demo(&directory, "source");
    let mut copy = working_copy(&source, &directory, "copy");
    let captured = source.capture(|store| store.apply(create(NOTE, "First."))).unwrap();

    let stale = copy.replay(captured.base_revision + 1, captured.access_epoch, &captured.changeset).unwrap_err();
    assert_eq!(stale.code(), "stale", "{stale}");
    copy.raise_access_epoch().unwrap();
    let revoked = copy.replay(captured.base_revision, captured.access_epoch, &captured.changeset).unwrap_err();
    assert_eq!(revoked.code(), "stale", "{revoked}");
    assert_eq!(copy.database_revision().unwrap(), captured.base_revision, "nothing was applied");
}

#[test]
fn a_failed_operation_returns_its_error_and_records_nothing_later() {
    let directory = maintenance_support::temporary_dir("ath-replica-failed-");
    let mut source = demo(&directory, "source");
    source.apply(create(NOTE, "First.")).unwrap();
    let failed = source.capture(|store| store.apply(correct(NOTE, 7, "Stale.")));
    assert!(failed.is_err());
    // A later capture records only its own write.
    let captured = source.capture(|store| store.apply(correct(NOTE, 1, "Second."))).unwrap();
    let tables: Vec<&str> = captured.changeset.0.iter().map(|change| change.table.as_str()).collect();
    assert!(tables.contains(&"record_revisions") && tables.contains(&"database_state"), "{tables:?}");
    assert_eq!(tables.iter().filter(|table| **table == "record_revisions").count(), 1, "{tables:?}");
}

#[test]
fn a_changeset_survives_its_json_form() {
    let directory = maintenance_support::temporary_dir("ath-replica-json-");
    let mut source = demo(&directory, "source");
    let captured = source.capture(|store| store.apply(create(NOTE, "A note with \"quotes\" and ünïcode."))).unwrap();
    let text = captured.changeset.to_json();
    assert_eq!(Changeset::from_json(&text).unwrap(), captured.changeset);
}

#[test]
fn tables_come_after_the_tables_they_reference() {
    let directory = maintenance_support::temporary_dir("ath-replica-order-");
    let source = demo(&directory, "source");
    let order: Vec<String> = source.table_shapes().unwrap().into_iter().map(|shape| shape.name).collect();
    let position = |name: &str| order.iter().position(|table| table == name).unwrap();
    assert!(position("records") < position("record_revisions"));
    assert!(position("records") < position("workout_sessions"));
    assert!(position("record_revisions") < position("record_deletions"));
    for shape in source.table_shapes().unwrap() {
        assert!(!shape.primary_key.is_empty(), "{} has a primary key", shape.name);
    }
}

#[test]
fn a_server_keeps_a_contiguous_log_and_an_erasure_clears_it() {
    let directory = maintenance_support::temporary_dir("ath-replica-log-");
    let mut working = demo(&directory, "working");
    let mut server = working_copy(&working, &directory, "server");
    let start = server.database_revision().unwrap();

    for (record_id, text) in [(NOTE, "One."), (OTHER, "Two.")] {
        let captured = working.capture(|store| store.apply(create(record_id, text))).unwrap();
        server
            .commit_replicated(
                captured.base_revision,
                captured.new_revision,
                captured.access_epoch,
                &captured.changeset,
                WRITTEN,
            )
            .unwrap();
    }
    let entries = server.replication_log_after(start).unwrap().expect("the log is contiguous");
    assert_eq!(entries.len(), 2);

    // A second working copy at the start catches up from the log.
    let mut late = demo(&directory, "late");
    for (base, _, changeset) in &entries {
        late.replay(*base, late.access_epoch().unwrap(), changeset).unwrap();
    }
    assert_same(&server, &late, &directory, "caught up");

    // A wrong new revision is refused and leaves the server unchanged.
    let captured = working.capture(|store| store.apply(correct(NOTE, 1, "Three."))).unwrap();
    let refused = server
        .commit_replicated(
            captured.base_revision,
            captured.new_revision + 1,
            captured.access_epoch,
            &captured.changeset,
            WRITTEN,
        )
        .unwrap_err();
    assert!(refused.to_string().contains("ended at revision"), "{refused}");
    server
        .commit_replicated(
            captured.base_revision,
            captured.new_revision,
            captured.access_epoch,
            &captured.changeset,
            WRITTEN,
        )
        .unwrap();

    let captured = working
        .capture(|store| {
            let preview = preview_erasure(store, NOTE, 2)?;
            erase_record(store, &preview, ErasureReasonCode::AthleteRequest, Some(WRITTEN)).map(|_| ())
        })
        .unwrap();
    server
        .commit_replicated(
            captured.base_revision,
            captured.new_revision,
            captured.access_epoch,
            &captured.changeset,
            WRITTEN,
        )
        .unwrap();
    assert!(
        server.replication_log_after(start).unwrap().is_none(),
        "the erasure cleared the log, so a copy at the start must be rebuilt"
    );
    assert_eq!(server.replication_log_after(server.database_revision().unwrap()).unwrap(), Some(Vec::new()));
    assert_same(&working, &server, &directory, "erased");
}

#[test]
fn the_schema_scripts_build_the_same_schema_as_a_new_file() {
    let directory = maintenance_support::temporary_dir("ath-replica-schema-");
    let made = Store::create(directory.join("made.ath.sqlite"), Default::default()).unwrap();
    let scripted = directory.join("scripted.ath.sqlite");
    let connection = rusqlite::Connection::open(&scripted).unwrap();
    for script in schema_scripts(&made.metadata().database_id, &made.metadata().created_at) {
        connection.execute_batch(&script).unwrap();
    }
    let read = |connection: &rusqlite::Connection, sql: &str| -> String {
        connection.query_row(sql, [], |row| row.get(0)).unwrap()
    };
    let made_connection = rusqlite::Connection::open(made.path()).unwrap();
    for sql in [
        "SELECT json_group_array(json_array(name, sql)) FROM (SELECT name, coalesce(sql, '') AS sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY name)",
        "SELECT json_array(application_marker, sql_schema_version, athleticstandard_version, database_id, created_at) FROM database_metadata",
        "SELECT json_array(current_revision) FROM database_state",
        "SELECT json_array(access_epoch) FROM internal_access_state",
    ] {
        assert_eq!(read(&made_connection, sql), read(&connection, sql), "{sql}");
    }
}

#[test]
fn a_package_restore_into_an_empty_record_replays() {
    let directory = maintenance_support::temporary_dir("ath-replica-restore-");
    let source = demo(&directory, "source");
    let package = directory.join("package");
    export_package(&source, &package, &ExportOptions::default()).unwrap();

    let mut empty = Store::create(directory.join("empty").join("athlete.ath.sqlite"), Default::default()).unwrap();
    let mut copy = working_copy(&empty, &directory, "copy");
    let captured = empty
        .capture(|store| {
            let preview = preview_package_import(store, &package)?;
            apply_package_import(store, &preview, &[]).map(|_| ())
        })
        .unwrap();
    copy_folder(&directory.join("empty").join("attachments"), &directory.join("copy").join("attachments"));
    copy.replay(captured.base_revision, copy.access_epoch().unwrap(), &captured.changeset).unwrap();
    assert_same(&empty, &copy, &directory, "restored");
}
