//! The 2.1.3 additions (D254, D255, D261–D263): the coach author and its
//! rules, the version a package with a coach must declare, and the three
//! internal tables of migration step 7. The change log is checked against
//! the database revision each write commits.

mod demo_support;
// The maintenance helpers own this program's temporary folders.
mod maintenance_support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use athleticstandard::json::{self, Value};
use athleticstandard::model::package::ATHLETIC_STANDARD_VERSION;
use athleticstandard::model::{registry, ErasureReasonCode, RecordActor, RecordData, RecordType};
use athleticstandard::package::{
    apply_package_import, erase_record, export_package, preview_erasure, preview_package_import, ExportOptions,
    MANIFEST_DOCUMENT, PACKAGE_DOCUMENT,
};
use athleticstandard::store::check::check_database;
use athleticstandard::store::contract;
use athleticstandard::store::migrate::{execute_confirmed_migration, MigrationOptions};
use athleticstandard::store::write::StaleStyle;
use athleticstandard::store::{
    Changes, Correction, CreateOptions, Deletion, ExtensionsUpdate, NewRecord, OpenOptions, Store, SQL_SCHEMA_VERSION,
};
use rusqlite::Connection;
use serde_json::json;

const COACH_ID: &str = "6f1c2e4a-3b5d-4c7e-9f80-1a2b3c4d5e6f";
const NOTE: &str = "01890f3e-e7d3-7cc0-88c8-0000000000c1";
const OTHER_NOTE: &str = "01890f3e-e7d3-7cc0-88c8-0000000000c2";
const GOAL: &str = "01890f3e-e7d3-7cc0-88c8-0000000000c3";
const REPORT: &str = "01890f3e-e7d3-7cc0-88c8-0000000000c4";
/// The demo's workout, a measured record.
const WORKOUT: &str = "01890f3e-e7d3-7cc0-88c8-00000000000a";
const WRITTEN: &str = "2026-11-20T12:00:00.000Z";
const CORRECTED: &str = "2026-11-20T13:00:00.000Z";
const DELETED: &str = "2026-11-20T14:00:00.000Z";
const ERASED: &str = "2026-11-20T15:00:00.000Z";
const INTERNAL_TABLES: [&str; 3] = ["internal_change_log", "internal_write_receipts", "internal_access_state"];

fn coach_json() -> serde_json::Value {
    json!({ "author_type": "coach", "coach": "Example Coach", "coach_id": COACH_ID })
}

fn coach() -> RecordActor {
    serde_json::from_value(coach_json()).unwrap()
}

fn athlete() -> RecordActor {
    serde_json::from_value(json!({ "author_type": "athlete" })).unwrap()
}

fn value(data: serde_json::Value) -> Value {
    json::parse(&data.to_string()).unwrap()
}

fn issues(schema: &str, data: serde_json::Value) -> Vec<String> {
    registry::shape(schema).unwrap().issues(&value(data)).iter().map(|issue| issue.to_json().to_string()).collect()
}

fn authorship(author: serde_json::Value) -> serde_json::Value {
    json!({ "author": author, "authored_at": WRITTEN, "evidence": [], "content_version": 1, "status": "active" })
}

fn note_data(author: serde_json::Value, note: &str) -> serde_json::Value {
    json!({ "authorship": authorship(author), "title": "Deload week", "note": note })
}

fn goal_data(author: serde_json::Value) -> serde_json::Value {
    json!({ "authorship": authorship(author), "title": "Row 2000 m under eight minutes" })
}

fn record(record_type: &str, record_id: &str, data: serde_json::Value) -> serde_json::Value {
    json!({
        "record_id": record_id,
        "created_at": WRITTEN,
        "current_revision": 1,
        "is_deleted": false,
        "record_type": record_type,
        "data": data,
    })
}

fn report_data() -> serde_json::Value {
    json!({
        "report_type": "note",
        "reported_at": WRITTEN,
        "input_reported_at": { "timestamp_missing_reason": "not_applicable", "utc_offset_missing_reason": "not_applicable" },
        "reported_via": "text",
        "note": "Legs felt heavy.",
    })
}

fn new_record(record_type: RecordType, record_id: &str, data: serde_json::Value, actor: RecordActor) -> NewRecord {
    NewRecord {
        record_id: Some(record_id.to_string()),
        created_at: Some(WRITTEN.to_string()),
        ..NewRecord::new(RecordData::decode(record_type, &value(data)).unwrap(), actor)
    }
}

fn note(record_id: &str, text: &str) -> NewRecord {
    new_record(RecordType::CoachingNote, record_id, note_data(coach_json(), text), coach())
}

fn new_store(directory: &Path) -> (PathBuf, Store) {
    let path = directory.join("athlete.ath.sqlite");
    let store = Store::create(&path, CreateOptions::default()).unwrap();
    (path, store)
}

fn inspect(path: &Path) -> Connection {
    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

/// Change-log rows as (database revision, record ID, record revision, kind).
fn change_log(path: &Path) -> Vec<(u64, String, u64, String)> {
    let connection = inspect(path);
    let mut statement = connection
        .prepare(
            "SELECT database_revision, record_id, record_revision, change_kind FROM internal_change_log
             ORDER BY database_revision, record_id, change_kind, record_revision",
        )
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get::<_, i64>(0)? as u64, row.get(1)?, row.get::<_, i64>(2)? as u64, row.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn row(database_revision: u64, record_id: &str, record_revision: u64, kind: &str) -> (u64, String, u64, String) {
    (database_revision, record_id.to_string(), record_revision, kind.to_string())
}

fn count(path: &Path, table: &str) -> i64 {
    inspect(path).query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0)).unwrap()
}

fn access_rows(path: &Path) -> Vec<(i64, i64)> {
    let connection = inspect(path);
    let mut statement = connection.prepare("SELECT singleton_id, access_epoch FROM internal_access_state").unwrap();
    statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().map(Result::unwrap).collect()
}

/// Every file below `directory`, by relative path with `/` separators.
fn files_below(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, directory: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(directory, directory, &mut out);
    out
}

#[test]
fn a_coach_note_survives_export_and_restore_byte_for_byte() {
    let directory = maintenance_support::temporary_dir("ath-record-2-1-3-round-trip-");
    let (_, mut store) = new_store(&directory);
    store
        .apply(Changes { creates: vec![note(NOTE, "Keep every session easy this week.")], ..Changes::default() })
        .unwrap();
    let corrected = RecordData::decode(
        RecordType::CoachingNote,
        &value(note_data(coach_json(), "Keep every session easy, and sleep eight hours.")),
    )
    .unwrap();
    store
        .apply(Changes {
            corrections: vec![Correction {
                record_id: NOTE.into(),
                expected_revision: 1,
                data: corrected,
                extensions: ExtensionsUpdate::Keep,
                changed_by: coach(),
                became_known_at: Some(CORRECTED.into()),
                became_known_at_basis: athleticstandard::model::BecameKnownAtBasis::LocalWrite,
                stale_style: StaleStyle::Revision,
            }],
            ..Changes::default()
        })
        .unwrap();

    let first = directory.join("first");
    export_package(&store, &first, &ExportOptions::default()).unwrap();
    let document = json::parse(&fs::read_to_string(first.join(PACKAGE_DOCUMENT)).unwrap()).unwrap();
    assert_eq!(document["athleticstandard_version"], ATHLETIC_STANDARD_VERSION);
    assert_eq!(document["records"][0]["data"]["authorship"]["author"], value(coach_json()));
    for revision in document["record_revisions"].as_array().unwrap() {
        assert_eq!(revision["changed_by"], value(coach_json()));
    }

    // The package holds the record, its history, and nothing internal.
    let package = files_below(&first);
    assert_eq!(package.keys().collect::<Vec<_>>(), [PACKAGE_DOCUMENT, MANIFEST_DOCUMENT]);
    let keys: Vec<&String> = document.as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        [
            "athleticstandard_version",
            "database_id",
            "database_revision",
            "measurement_series_files",
            "record_deletions",
            "record_erasure_markers",
            "record_import_keys",
            "record_revisions",
            "records"
        ]
    );
    for (file, bytes) in &package {
        let text = String::from_utf8_lossy(bytes);
        for word in INTERNAL_TABLES.iter().copied().chain(["access_epoch", "request_digest", "change_kind"]) {
            assert!(!text.contains(word), "{file} names {word}");
        }
    }

    // Restore into an empty database, then export again.
    let restored_path = directory.join("restored.ath.sqlite");
    let mut restored = Store::create(&restored_path, CreateOptions::default()).unwrap();
    let preview = preview_package_import(&restored, &first).unwrap();
    assert!(preview.restore && !preview.is_unrepresentable(), "{preview:?}");
    apply_package_import(&mut restored, &preview, &[]).unwrap();
    let second = directory.join("second");
    export_package(&restored, &second, &ExportOptions::default()).unwrap();
    assert_eq!(files_below(&second), package, "the second export differs");
    let kept = restored.revisions(NOTE).unwrap();
    assert_eq!(kept.iter().map(|revision| revision.changed_by.clone()).collect::<Vec<_>>(), [coach(), coach()]);
    drop(restored);

    // A restore starts a new change log and keeps the other internal tables
    // as a new database has them.
    assert!(change_log(&restored_path).is_empty());
    assert_eq!(count(&restored_path, "internal_write_receipts"), 0);
    assert_eq!(access_rows(&restored_path), [(1, 0)]);
    drop(store);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_coach_writes_only_notes_plans_and_planned_sessions() {
    // The shapes: a coach author on the three types passes.
    assert_eq!(issues("CoachingNoteRecord", record("coaching_note", NOTE, note_data(coach_json(), "Easy."))), [""; 0]);
    let plan = json!({ "authorship": authorship(coach_json()), "title": "Base block" });
    assert_eq!(issues("TrainingPlanRecord", record("training_plan", NOTE, plan)), [""; 0]);

    // On any other authored type it is refused.
    let goal = issues("GoalRecord", record("goal", GOAL, goal_data(coach_json())));
    assert_eq!(goal.len(), 1, "{goal:?}");
    assert!(goal[0].contains(r#""path":["data","authorship","author"]"#), "{goal:?}");
    assert!(
        goal[0].contains("A coach may write only coaching_note, training_plan, and planned_session records, not goal"),
        "{goal:?}"
    );
    let review = json!({
        "authorship": authorship(coach_json()),
        "period_start": "2026-11-01",
        "period_end": "2026-11-14",
        "summary": "Two steady weeks.",
    });
    let review = issues("SavedTrainingReviewRecord", record("saved_training_review", GOAL, review));
    assert!(review.len() == 1 && review[0].contains("not saved_training_review"), "{review:?}");

    // A coach cannot be the actor of an athlete report's revision.
    let revision = json!({
        "record_id": REPORT,
        "revision": 1,
        "previous_revision": null,
        "became_known_at": WRITTEN,
        "became_known_at_basis": "local_write",
        "changed_by": coach_json(),
        "record_type": "athlete_report",
        "data": report_data(),
    });
    let refused = issues("AthleteReportRevision", revision.clone());
    assert!(refused.len() == 1 && refused[0].contains(r#""path":["changed_by"]"#), "{refused:?}");
    assert!(refused[0].contains("not athlete_report"), "{refused:?}");
    let mut by_athlete = revision;
    by_athlete["changed_by"] = json!({ "author_type": "athlete" });
    assert_eq!(issues("AthleteReportRevision", by_athlete), [""; 0]);

    // The store refuses the same writes and writes nothing.
    let directory = maintenance_support::temporary_dir("ath-record-2-1-3-wall-");
    let path = demo_support::write(&directory);
    let mut store = Store::open(&path, OpenOptions::default()).unwrap();
    let before = store.database_revision().unwrap();
    let refuse = |store: &mut Store, changes: Changes| store.apply(changes).unwrap_err().to_string();

    let report = new_record(RecordType::AthleteReport, REPORT, report_data(), coach());
    assert_eq!(
        refuse(&mut store, Changes { creates: vec![report], ..Changes::default() }),
        "changed_by: A coach may write only coaching_note, training_plan, and planned_session records, not athlete_report"
    );
    let goal = new_record(RecordType::Goal, GOAL, goal_data(coach_json()), athlete());
    assert_eq!(
        refuse(&mut store, Changes { creates: vec![goal], ..Changes::default() }),
        "data.authorship.author: A coach may write only coaching_note, training_plan, and planned_session records, not goal"
    );
    let workout = store.get_record(WORKOUT).unwrap().unwrap();
    let correction = Correction {
        record_id: WORKOUT.into(),
        expected_revision: workout.current_revision,
        data: workout.data.clone(),
        extensions: ExtensionsUpdate::Keep,
        changed_by: coach(),
        became_known_at: Some(WRITTEN.into()),
        became_known_at_basis: athleticstandard::model::BecameKnownAtBasis::LocalWrite,
        stale_style: StaleStyle::Revision,
    };
    assert_eq!(
        refuse(&mut store, Changes { corrections: vec![correction], ..Changes::default() }),
        "changed_by: A coach may write only coaching_note, training_plan, and planned_session records, not workout_session"
    );
    assert_eq!(store.database_revision().unwrap(), before, "a refused write writes nothing");
    assert!(store.get_record(REPORT).unwrap().is_none() && store.get_record(GOAL).unwrap().is_none());
    drop(store);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_coach_needs_a_name_and_a_lowercase_uuid() {
    for schema in ["RecordActor", "AuthoredRecordAuthor"] {
        assert_eq!(issues(schema, coach_json()), [""; 0], "{schema}");
        // Any UUID version is accepted: the host chooses the ID.
        let mut version_seven = coach_json();
        version_seven["coach_id"] = json!(NOTE);
        assert_eq!(issues(schema, version_seven), [""; 0], "{schema}");
        for (field, bad) in [
            ("coach_id", json!("6F1C2E4A-3B5D-4C7E-9F80-1A2B3C4D5E6F")),
            ("coach_id", json!("6f1c2e4a3b5d4c7e9f801a2b3c4d5e6f")),
            ("coach_id", json!("coach@example.com")),
            ("coach_id", json!("")),
            ("coach", json!("")),
            ("coach", json!(7)),
        ] {
            let mut actor = coach_json();
            actor[field] = bad.clone();
            let found = issues(schema, actor);
            assert_eq!(found.len(), 1, "{schema} {field} {bad}: {found:?}");
            assert!(found[0].contains(&format!(r#""path":["{field}"]"#)), "{found:?}");
        }
        let mut missing = coach_json();
        missing.as_object_mut().unwrap().remove("coach_id");
        assert!(!issues(schema, missing).is_empty(), "{schema}");
        let mut extra = coach_json();
        extra["email"] = json!("coach@example.com");
        assert!(issues(schema, extra)[0].contains("Unrecognized key"), "{schema}");
    }
}

#[test]
fn a_coach_cannot_delete_a_record() {
    let deletion =
        json!({ "record_id": NOTE, "record_revision": 1, "deleted_at": DELETED, "deleted_by": coach_json() });
    let found = issues("RecordDeletion", deletion);
    assert!(found.len() == 1 && found[0].contains("A coach cannot delete a record"), "{found:?}");

    let directory = maintenance_support::temporary_dir("ath-record-2-1-3-delete-");
    let (_, mut store) = new_store(&directory);
    store.apply(Changes { creates: vec![note(NOTE, "Easy week.")], ..Changes::default() }).unwrap();
    let before = store.database_revision().unwrap();
    let error = store
        .apply(Changes {
            deletions: vec![Deletion {
                record_id: NOTE.into(),
                expected_revision: 1,
                deleted_by: coach(),
                deleted_at: Some(DELETED.into()),
            }],
            ..Changes::default()
        })
        .unwrap_err()
        .to_string();
    assert_eq!(error, "deleted_by: A coach cannot delete a record");
    assert_eq!(store.database_revision().unwrap(), before);
    assert!(!store.get_record(NOTE).unwrap().unwrap().is_deleted);
    drop(store);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_package_with_a_coach_declares_2_1_3_or_later() {
    let directory = maintenance_support::temporary_dir("ath-record-2-1-3-version-");
    let (_, mut store) = new_store(&directory);
    store.apply(Changes { creates: vec![note(NOTE, "Easy week.")], ..Changes::default() }).unwrap();
    let package = directory.join("package");
    export_package(&store, &package, &ExportOptions::default()).unwrap();
    drop(store);

    let text = fs::read_to_string(package.join(PACKAGE_DOCUMENT)).unwrap();
    let document: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(issues("AthleticStandardPackage", document.clone()), [""; 0]);
    for older in ["2.1.2", "2.1.0", "1.0.0"] {
        let mut declared = document.clone();
        declared["athleticstandard_version"] = json!(older);
        let found = issues("AthleticStandardPackage", declared);
        let message = format!("A coach author needs Athletic Standard 2.1.3 or later; this package declares {older}");
        // The note's author, and its revision's actor and author.
        assert_eq!(found.len(), 3, "{older}: {found:?}");
        assert!(found[0].contains(r#""path":["records",0,"data","authorship","author"]"#), "{found:?}");
        assert!(found[1].contains(r#""path":["record_revisions",0,"changed_by"]"#), "{found:?}");
        assert!(found[2].contains(r#""path":["record_revisions",0,"data","authorship","author"]"#), "{found:?}");
        assert!(found.iter().all(|issue| issue.contains(&message)), "{found:?}");
    }

    // `ath import` refuses the older declaration and names the reason.
    let older = text.replacen(
        &format!("\"athleticstandard_version\": \"{ATHLETIC_STANDARD_VERSION}\""),
        "\"athleticstandard_version\": \"2.1.2\"",
        1,
    );
    assert_ne!(older, text);
    fs::write(package.join(PACKAGE_DOCUMENT), older).unwrap();
    fs::remove_file(package.join(MANIFEST_DOCUMENT)).unwrap();
    let destination = Store::create(directory.join("destination.ath.sqlite"), CreateOptions::default()).unwrap();
    let preview = preview_package_import(&destination, &package).unwrap();
    assert!(preview.is_unrepresentable());
    assert!(
        preview.unrepresentable_records.iter().any(|line| line
            == "records.0.data.authorship.author: A coach author needs Athletic Standard 2.1.3 or later; this package declares 2.1.2"),
        "{:?}",
        preview.unrepresentable_records
    );
    drop(destination);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_new_database_has_the_three_internal_tables() {
    let directory = maintenance_support::temporary_dir("ath-record-2-1-3-new-");
    let (path, store) = new_store(&directory);
    assert_eq!(store.metadata().sql_schema_version, SQL_SCHEMA_VERSION);
    assert_eq!(SQL_SCHEMA_VERSION, 7);
    drop(store);
    assert!(change_log(&path).is_empty());
    assert_eq!(count(&path, "internal_write_receipts"), 0);
    assert_eq!(access_rows(&path), [(1, 0)]);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_2_1_2_database_migrates_to_step_7_with_empty_internal_tables() {
    // A 2.1.2 program runs the published steps 5 and 6 on the 2.1.0 demo.
    let path =
        maintenance_support::copy_fixture(&maintenance_support::demo_fixture(), "ath-record-2-1-3-step-6-", true);
    let migrations = maintenance_support::repository().join("spec/sql/migrations");
    let direct = maintenance_support::direct(&path);
    direct.execute_batch("PRAGMA journal_mode = DELETE; BEGIN IMMEDIATE;").unwrap();
    for file in ["005-version-2-1-1.sql", "006-version-2-1-2.sql"] {
        direct.execute_batch(&fs::read_to_string(migrations.join(file)).unwrap()).unwrap();
    }
    direct.execute_batch("PRAGMA user_version = 6; COMMIT;").unwrap();
    drop(direct);
    assert_eq!(maintenance_support::stored_versions(&path), ("2.1.2".to_string(), 6));
    let revision = maintenance_support::database_revision(&path);
    let has_table = |name: &str| -> bool {
        inspect(&path)
            .query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [name], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap()
            == 1
    };
    assert!(INTERNAL_TABLES.iter().all(|table| !has_table(table)));

    let mut store = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
    let result = execute_confirmed_migration(&mut store, &MigrationOptions::default()).unwrap();
    assert!(result.migrated);
    let steps: Vec<u32> = result.preview.planned_steps.iter().map(|step| step.target_sql_schema_version).collect();
    assert_eq!(steps, [7]);
    assert_eq!(store.metadata().sql_schema_version, 7);
    assert_eq!(store.metadata().athleticstandard_version, "2.1.3");
    let report = check_database(&store).unwrap();
    assert!(report.valid && report.issues.is_empty(), "{:?}", report.issues);
    let published = contract::published_contract().unwrap();
    assert_eq!(contract::verify_live_schema(&store, &published).unwrap(), Vec::<String>::new());
    drop(store);

    assert!(INTERNAL_TABLES.iter().all(|table| has_table(table)));
    assert!(change_log(&path).is_empty(), "the migration logs no change");
    assert_eq!(count(&path, "internal_write_receipts"), 0);
    assert_eq!(access_rows(&path), [(1, 0)]);
    assert_eq!(maintenance_support::database_revision(&path), revision, "the migration changes no record");
}

#[test]
fn the_change_log_names_the_revision_each_write_committed() {
    let directory = maintenance_support::temporary_dir("ath-record-2-1-3-log-");
    let (path, mut store) = new_store(&directory);

    let created = store
        .apply(Changes {
            creates: vec![note(NOTE, "Easy week."), note(OTHER_NOTE, "Rest on Sunday.")],
            ..Changes::default()
        })
        .unwrap()
        .database_revision;
    assert_eq!(created, 1);
    assert_eq!(change_log(&path), [row(1, NOTE, 1, "revision"), row(1, OTHER_NOTE, 1, "revision")]);

    let corrected =
        RecordData::decode(RecordType::CoachingNote, &value(note_data(coach_json(), "Easy week, then test."))).unwrap();
    let corrected = store
        .apply(Changes {
            corrections: vec![Correction {
                record_id: NOTE.into(),
                expected_revision: 1,
                data: corrected,
                extensions: ExtensionsUpdate::Keep,
                changed_by: athlete(),
                became_known_at: Some(CORRECTED.into()),
                became_known_at_basis: athleticstandard::model::BecameKnownAtBasis::LocalWrite,
                stale_style: StaleStyle::Revision,
            }],
            ..Changes::default()
        })
        .unwrap()
        .database_revision;
    assert_eq!(corrected, 2);

    let deleted = store
        .apply(Changes {
            deletions: vec![Deletion {
                record_id: NOTE.into(),
                expected_revision: 2,
                deleted_by: athlete(),
                deleted_at: Some(DELETED.into()),
            }],
            ..Changes::default()
        })
        .unwrap()
        .database_revision;
    assert_eq!(deleted, 3);
    assert_eq!(store.database_revision().unwrap(), 3);
    assert_eq!(
        change_log(&path),
        [
            row(1, NOTE, 1, "revision"),
            row(1, OTHER_NOTE, 1, "revision"),
            row(2, NOTE, 2, "revision"),
            row(3, NOTE, 2, "deletion"),
        ]
    );

    // An erasure leaves one value-free row, as it leaves one marker. The
    // erased record's earlier rows go with its history.
    let preview = preview_erasure(&store, NOTE, 2).unwrap();
    let erased = erase_record(&mut store, &preview, ErasureReasonCode::AthleteRequest, Some(ERASED)).unwrap();
    assert_eq!(erased.database_revision, 4);
    assert_eq!(change_log(&path), [row(1, OTHER_NOTE, 1, "revision"), row(4, NOTE, 0, "erasure")]);
    drop(store);

    // A direct writer that follows the published soft-delete recipe is
    // logged under the revision its transaction commits.
    let direct = maintenance_support::direct(&path);
    direct
        .execute_batch(&format!(
            "BEGIN IMMEDIATE;
             PRAGMA defer_foreign_keys = ON;
             INSERT INTO record_deletions (record_id, record_revision, deleted_at, deleted_by_json)
               VALUES ('{OTHER_NOTE}', 1, '{DELETED}', '{{\"author_type\":\"athlete\"}}');
             UPDATE records SET is_deleted = 1 WHERE record_id = '{OTHER_NOTE}';
             UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1;
             COMMIT;"
        ))
        .unwrap();
    drop(direct);
    assert_eq!(maintenance_support::database_revision(&path), 5);
    assert_eq!(
        change_log(&path),
        [row(1, OTHER_NOTE, 1, "revision"), row(4, NOTE, 0, "erasure"), row(5, OTHER_NOTE, 1, "deletion")]
    );

    // The log holds IDs and numbers only.
    let connection = inspect(&path);
    let columns: Vec<String> = connection
        .prepare("SELECT name FROM pragma_table_info('internal_change_log') ORDER BY cid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(columns, ["database_revision", "record_id", "record_revision", "change_kind"]);
    drop(connection);
    fs::remove_dir_all(directory).unwrap();
}
