//! Replay the deterministic demo database through the Rust writer and require
//! byte-identical athlete rows in every table. The demo database was written
//! by the 2.1.0 implementation and covers all 24 record types.

use std::path::PathBuf;

use athleticstandard::json;
use athleticstandard::model::*;
use athleticstandard::store::{self, Changes, Correction, CreateOptions, Deletion, ExtensionsUpdate, NewRecord, Store};
use rusqlite::{Connection, OpenFlags};

fn demo_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/version-2-1-0/athlete.ath.sqlite")
}

fn temporary_dir() -> PathBuf {
    athleticstandard::files::make_temporary_directory("ath-replay-").unwrap()
}

/// Every athlete-data table, compared row by row.
const TABLES: &[&str] = &[
    "records",
    "record_revisions",
    "record_deletions",
    "record_erasure_markers",
    "record_import_keys",
    "athlete_profile",
    "sources",
    "source_devices",
    "point_measurements",
    "point_measurement_calculations",
    "point_measurement_calculation_inputs",
    "measurement_samples",
    "sleep_sessions",
    "workout_sessions",
    "workout_session_segments",
    "workout_route_points",
    "benchmark_results",
    "benchmark_result_segments",
    "provider_scores",
    "provider_summaries",
    "provider_reporting_periods",
    "athlete_reports",
    "nutrition_entries",
    "reported_nutrition_items",
    "attachments",
    "record_attachment_links",
    "benchmark_definitions",
    "predictions",
    "prediction_result_links",
    "prediction_grades",
    "prediction_error_analyses",
    "prediction_error_causes",
    "training_plans",
    "planned_sessions",
    "planned_session_blocks",
    "coaching_notes",
    "saved_training_reviews",
    "goals",
    "nutrition_model_interpretations",
    "authored_record_evidence",
];

/// The 2.1.0 columns of a table. A column added later (migration 6 adds
/// `athlete_reports.workout_session_id`) is compared only for being empty.
fn dump(connection: &Connection, table: &str) -> Vec<String> {
    let mut names = connection.prepare(&format!("SELECT name FROM pragma_table_info('{table}')")).unwrap();
    let names: Vec<String> = names.query_map([], |row| row.get(0)).unwrap().map(Result::unwrap).collect();
    let mut kept = Vec::new();
    for name in names {
        if table == "athlete_reports" && name == "workout_session_id" {
            let filled: i64 = connection
                .query_row(&format!("SELECT count(*) FROM {table} WHERE {name} IS NOT NULL"), [], |row| row.get(0))
                .unwrap();
            assert_eq!(filled, 0, "{table}.{name} is empty in the demo");
        } else {
            kept.push(format!("\"{name}\""));
        }
    }
    let mut statement = connection.prepare(&format!("SELECT {} FROM {table}", kept.join(", "))).unwrap();
    let columns = statement.column_count();
    let mut rows: Vec<String> = statement
        .query_map([], |row| {
            let mut parts = Vec::new();
            for index in 0..columns {
                let value: rusqlite::types::Value = row.get(index)?;
                parts.push(format!("{value:?}"));
            }
            Ok(parts.join(" | "))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    rows.sort();
    rows
}

#[test]
fn replaying_the_demo_history_reproduces_every_row() {
    let demo = Connection::open_with_flags(demo_path(), OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let (database_id, created_at): (String, String) = demo
        .query_row("SELECT database_id, created_at FROM database_metadata WHERE singleton_id = 1", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    let directory = temporary_dir();
    let path = directory.join("replay.ath.sqlite");
    let mut replay =
        Store::create(&path, CreateOptions { database_id: Some(database_id), created_at: Some(created_at) }).unwrap();

    let mut statement = demo
        .prepare(&format!(
            "SELECT record_revisions.rowid, records.created_at, {} FROM record_revisions JOIN records USING (record_id) ORDER BY record_revisions.rowid",
            store::read::REVISION_COLUMNS
                .split(',')
                .map(|column| format!("record_revisions.{}", column.trim()))
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .unwrap();
    let mut rows = statement.query([]).unwrap();
    let mut changes_in_order = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        let created_at: String = row.get(1).unwrap();
        let revision = store::read::revision_from_row(row).unwrap();
        changes_in_order.push((created_at, revision));
    }
    for (created_at, revision) in changes_in_order {
        let mut changes = Changes::default();
        if revision.revision == 1 {
            changes.creates.push(NewRecord {
                data: revision.data.clone(),
                extensions: revision.extensions.clone(),
                record_id: Some(revision.record_id.clone()),
                created_at: Some(created_at),
                became_known_at: Some(revision.became_known_at.clone()),
                became_known_at_basis: revision.became_known_at_basis,
                changed_by: revision.changed_by.clone(),
            });
        } else {
            changes.corrections.push(Correction {
                record_id: revision.record_id.clone(),
                expected_revision: revision.revision - 1,
                data: revision.data.clone(),
                extensions: ExtensionsUpdate::Set(revision.extensions.clone()),
                changed_by: revision.changed_by.clone(),
                became_known_at: Some(revision.became_known_at.clone()),
                became_known_at_basis: revision.became_known_at_basis,
                stale_style: store::write::StaleStyle::Revision,
            });
        }
        replay.apply(changes).unwrap();
    }
    let mut deletions =
        demo.prepare("SELECT record_id, record_revision, deleted_at, deleted_by_json FROM record_deletions").unwrap();
    let deletions: Vec<(String, i64, String, String)> = deletions
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for (record_id, revision, deleted_at, deleted_by) in deletions {
        let deleted_by: RecordActor = serde_json::from_str(&deleted_by).unwrap();
        replay
            .apply(Changes {
                deletions: vec![Deletion {
                    record_id,
                    expected_revision: revision as u64,
                    deleted_by,
                    deleted_at: Some(deleted_at),
                }],
                ..Changes::default()
            })
            .unwrap();
    }

    let mut differences = Vec::new();
    let replayed = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    for table in TABLES {
        let expected = dump(&demo, table);
        let actual = dump(&replayed, table);
        if expected != actual {
            differences.push(format!("{table}:\n  expected {expected:#?}\n  actual {actual:#?}"));
        }
    }
    drop(replayed);
    replay.close().unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn every_demo_record_round_trips_through_the_model() {
    let demo = Connection::open_with_flags(demo_path(), OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut statement = demo.prepare("SELECT record_type, record_json FROM record_revisions").unwrap();
    let rows: Vec<(String, String)> =
        statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().map(Result::unwrap).collect();
    assert!(rows.len() >= 24);
    for (record_type, text) in rows {
        let record_type = RecordType::parse(&record_type).unwrap();
        let data = RecordData::decode(record_type, &json::parse(&text).unwrap()).unwrap();
        assert_eq!(data.to_json(), text, "{record_type}");
    }
}
