//! The published direct-SQL contract, exercised with independent SQLite
//! connections that do not use the library's validator.
//!
//! Writers follow the recipe in `spec/sql/README.md`. SQLite's own constraints
//! and triggers must still refuse the critical mistakes: a measured record
//! without a source, an athlete report that claims one, a broken revision
//! chain, a deletion flag without its deletion row, a record row for an erased
//! record, and a prediction link to a revision that does not exist. Valid
//! direct writes must read back through the library and pass `ath check`.

mod maintenance_support;

use std::collections::BTreeSet;
use std::path::PathBuf;

use athleticstandard::store::check::check_database;
use athleticstandard::store::contract::{published_contract, verify_contract_metadata, verify_live_schema};
use athleticstandard::store::encode;
use athleticstandard::store::migrate::{execute_confirmed_migration, MigrationOptions};
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::{CreateOptions, OpenOptions, Store, SQLITE_APPLICATION_ID, SQL_SCHEMA_VERSION};
use maintenance_support::*;
use rusqlite::{params, Connection};

const DATABASE_ID: &str = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1f01";
const CREATED_AT: &str = "2026-08-09T12:00:00.000Z";
const SOURCE_ID: &str = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e02";
const REPORT_ID: &str = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e04";
const SUMMARY_ID: &str = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e20";
const TOOL_ACTOR: &str = r#"{"author_type":"tool","tool":"ath","tool_version":"1.0.0"}"#;
const SOURCE_JSON: &str =
    r#"{"source_type":"wearable","data_writer":"WHOOP","data_route":"whoop_csv","vendor":"whoop"}"#;

fn report_json(extra: &str) -> String {
    format!(
        r#"{{"report_type":"soreness","reported_at":"{CREATED_AT}","input_reported_at":{{"timestamp_text":"2026-08-09T06:12:00-05:00","utc_offset_text":"-05:00","utc_offset_meaning":"provider_cycle_offset","timestamp_basis":"source_offset_timestamp"}},"reported_via":"text"{extra}}}"#
    )
}

/// The recipe's connection settings.
fn recipe_connection(path: &std::path::Path) -> Connection {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;").unwrap();
    connection
}

/// The recipe's write transaction start.
fn begin(connection: &Connection) {
    connection.execute_batch("BEGIN IMMEDIATE; PRAGMA foreign_keys = ON; PRAGMA defer_foreign_keys = ON;").unwrap();
}

/// A new database with one source, written by direct SQL.
fn seeded_database() -> PathBuf {
    let dir = temporary_dir("ath-direct-sql-");
    let path = dir.join("athlete.ath.sqlite");
    Store::create(
        &path,
        CreateOptions { database_id: Some(DATABASE_ID.to_string()), created_at: Some(CREATED_AT.to_string()) },
    )
    .unwrap()
    .close()
    .unwrap();
    let connection = recipe_connection(&path);
    begin(&connection);
    insert_source(&connection, SOURCE_ID, "WHOOP");
    connection.execute_batch("COMMIT").unwrap();
    path
}

fn insert_source(connection: &Connection, record_id: &str, writer: &str) {
    let source_json = SOURCE_JSON.replace("WHOOP", writer);
    connection
        .execute(
            "INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted) VALUES (?1, 'source', 1, ?2, 0)",
            params![record_id, CREATED_AT],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO record_revisions (record_id, revision, record_type, record_json, extensions_json, became_known_at, became_known_at_basis, changed_by_json, previous_revision)
             VALUES (?1, 1, 'source', ?2, NULL, ?3, 'local_write', ?4, NULL)",
            params![record_id, source_json, CREATED_AT, TOOL_ACTOR],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sources (record_id, record_type, source_type, data_writer, data_route, vendor, data_json)
             VALUES (?1, 'source', 'wearable', ?3, 'whoop_csv', 'whoop', ?2)",
            params![record_id, source_json, writer],
        )
        .unwrap();
    connection
        .execute("UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1", [])
        .unwrap();
}

fn refusal(connection: &Connection, sql: &str, args: impl rusqlite::Params) -> String {
    match connection.execute(sql, args) {
        Ok(_) => panic!("SQLite accepted: {sql}"),
        Err(error) => error.to_string(),
    }
}

fn insert_report_revision(connection: &Connection, record_json: &str, extensions_json: Option<&str>) -> String {
    connection
        .execute(
            "INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted) VALUES (?1, 'athlete_report', 1, ?2, 0)",
            params![REPORT_ID, CREATED_AT],
        )
        .unwrap();
    refusal(
        connection,
        "INSERT INTO record_revisions (record_id, revision, record_type, record_json, extensions_json, became_known_at, became_known_at_basis, changed_by_json, previous_revision)
         VALUES (?1, 1, 'athlete_report', ?2, ?3, ?4, 'local_write', ?5, NULL)",
        params![REPORT_ID, record_json, extensions_json, CREATED_AT, TOOL_ACTOR],
    )
}

#[test]
fn the_contract_file_agrees_with_the_code() {
    let contract = published_contract().unwrap();
    assert_eq!(contract.sql_schema_version, SQL_SCHEMA_VERSION);
    assert_eq!(verify_contract_metadata(&contract), Vec::<String>::new());
    assert!(contract.internal_tables.contains(&"internal_pending_file_operations".to_string()));
    assert!(!contract.stable_writable_tables.contains(&"internal_pending_file_operations".to_string()));
}

#[test]
fn the_contract_lists_exactly_the_child_tables_the_encoder_replaces() {
    // The demo holds one record of every type.
    let path = copy_fixture(&demo_fixture(), "ath-direct-encoder-", false);
    let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: true }).unwrap();
    let records =
        store.list_records(&ListOptions { include_deleted: true, limit: 1000, ..ListOptions::default() }).unwrap();
    let types: BTreeSet<&str> = records.iter().map(|record| record.record_type().as_str()).collect();
    assert_eq!(types.len(), 24);
    let mut encoded = BTreeSet::new();
    for record in &records {
        for child in encode::child_rows(&record.data) {
            encoded.insert((child.table.to_string(), child.owner_column.to_string()));
        }
    }
    let contract = published_contract().unwrap();
    let listed: BTreeSet<(String, String)> = contract
        .collection_replacement
        .tables
        .iter()
        .map(|table| (table.table.clone(), table.parent_column.clone()))
        .collect();
    assert_eq!(encoded, listed);
}

#[test]
fn a_new_database_matches_the_contract() {
    let path = seeded_database();
    let connection = recipe_connection(&path);
    assert_eq!(verify_live_schema(&connection, &published_contract().unwrap()).unwrap(), Vec::<String>::new());
    let application_id: i64 = connection.pragma_query_value(None, "application_id", |row| row.get(0)).unwrap();
    assert_eq!(application_id, i64::from(SQLITE_APPLICATION_ID));
    let user_version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
    assert_eq!(user_version, SQL_SCHEMA_VERSION);
    // The stable views read without the library.
    let active: i64 = connection.query_row("SELECT COUNT(*) FROM sql_v1_active_records", [], |row| row.get(0)).unwrap();
    assert_eq!(active, 1);
    let revision: i64 =
        connection.query_row("SELECT database_revision FROM sql_v1_database_info", [], |row| row.get(0)).unwrap();
    assert_eq!(revision, 1);
}

#[test]
fn defer_foreign_keys_lasts_only_one_transaction() {
    let path = seeded_database();
    let connection = recipe_connection(&path);
    let outside: i64 = connection.pragma_query_value(None, "defer_foreign_keys", |row| row.get(0)).unwrap();
    assert_eq!(outside, 0);
    begin(&connection);
    let inside: i64 = connection.pragma_query_value(None, "defer_foreign_keys", |row| row.get(0)).unwrap();
    assert_eq!(inside, 1);
    insert_source(&connection, "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e21", "WHOOP strap");
    connection.execute_batch("COMMIT").unwrap();
    let after: i64 = connection.pragma_query_value(None, "defer_foreign_keys", |row| row.get(0)).unwrap();
    assert_eq!(after, 0);
    drop(connection);

    // The library reads the direct write and the check passes.
    let store = Store::open(&path, OpenOptions::default()).unwrap();
    let record = store.get_record("01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e21").unwrap().unwrap();
    assert_eq!(record.current_revision, 1);
    let result = check_database(&store).unwrap();
    assert!(result.valid && result.issues.is_empty(), "{:?}", result.issues);
}

#[test]
fn a_direct_correction_replaces_child_collections() {
    use athleticstandard::model::*;
    let path = seeded_database();
    let period_a = r#"{"period_type":"oura_activity_day","provider_date":"2026-08-08"}"#;
    let period_b = r#"{"period_type":"oura_activity_day","provider_date":"2026-08-07"}"#;
    let summary = |value: u32, periods: &str| {
        format!(
            r#"{{"source_id":"{SOURCE_ID}","received_at":"{CREATED_AT}","received_at_basis":"import_time","summary_type":"activity","provider_reporting_periods":[{periods}],"value":{{"standard_value":{value},"standard_unit":"score","input_value":{value},"input_unit":"score"}},"input_timestamp":{{"timestamp_missing_reason":"not_provided_by_source","utc_offset_missing_reason":"not_provided_by_source"}}}}"#
        )
    };
    // Create through the library.
    let mut store = Store::open(&path, OpenOptions::default()).unwrap();
    let data = RecordData::decode(
        RecordType::ProviderSummary,
        &athleticstandard::json::parse(&summary(80, &format!("{period_a},{period_b}"))).unwrap(),
    )
    .unwrap();
    let mut create = athleticstandard::store::NewRecord::new(
        data,
        RecordActor::Tool { tool: "ath".into(), tool_version: "1.0.0".into() },
    );
    create.record_id = Some(SUMMARY_ID.to_string());
    create.created_at = Some(CREATED_AT.to_string());
    store.apply(athleticstandard::store::Changes { creates: vec![create], ..Default::default() }).unwrap();
    store.close().unwrap();

    // Correct it directly, following the recipe.
    let corrected = summary(81, period_a);
    let connection = recipe_connection(&path);
    begin(&connection);
    connection
        .execute(
            "INSERT INTO record_revisions (record_id, revision, record_type, record_json, extensions_json, became_known_at, became_known_at_basis, changed_by_json, previous_revision)
             VALUES (?1, 2, 'provider_summary', ?2, NULL, '2026-08-09T13:00:00.000Z', 'local_write', ?3, 1)",
            params![SUMMARY_ID, corrected, TOOL_ACTOR],
        )
        .unwrap();
    connection.execute("UPDATE records SET current_revision = 2 WHERE record_id = ?1", [SUMMARY_ID]).unwrap();
    connection
        .execute(
            "INSERT INTO provider_summaries (record_id, record_type, source_id, summary_type, recorded_at, standard_value, standard_unit, provider_record_id, received_at, data_json)
             VALUES (?1, 'provider_summary', ?2, 'activity', NULL, 81, 'score', NULL, ?3, ?4)
             ON CONFLICT(record_id) DO UPDATE SET
               source_id = excluded.source_id, summary_type = excluded.summary_type, recorded_at = excluded.recorded_at,
               standard_value = excluded.standard_value, standard_unit = excluded.standard_unit,
               provider_record_id = excluded.provider_record_id, received_at = excluded.received_at, data_json = excluded.data_json",
            params![SUMMARY_ID, SOURCE_ID, CREATED_AT, corrected],
        )
        .unwrap();
    connection.execute("DELETE FROM provider_reporting_periods WHERE owner_record_id = ?1", [SUMMARY_ID]).unwrap();
    connection
        .execute(
            "INSERT INTO provider_reporting_periods (owner_record_id, period_order, period_type, provider_period_id, provider_date, start_at, end_at, period_json)
             VALUES (?1, 0, 'oura_activity_day', NULL, '2026-08-08', NULL, NULL, ?2)",
            params![SUMMARY_ID, period_a],
        )
        .unwrap();
    connection
        .execute("UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1", [])
        .unwrap();
    connection.execute_batch("COMMIT").unwrap();
    let periods: i64 = connection
        .query_row("SELECT COUNT(*) FROM provider_reporting_periods WHERE owner_record_id = ?1", [SUMMARY_ID], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(periods, 1);
    drop(connection);

    let mut store = Store::open(&path, OpenOptions::default()).unwrap();
    let record = store.get_record(SUMMARY_ID).unwrap().unwrap();
    assert_eq!(record.current_revision, 2);
    let RecordData::ProviderSummary(data) = &record.data else { panic!("expected a provider summary") };
    assert_eq!(data.value.standard_value, 81.0);
    assert_eq!(data.provider_reporting_periods.as_ref().map(Vec::len), Some(1));
    let result = check_database(&store).unwrap();
    assert!(result.valid && result.issues.is_empty(), "{:?}", result.issues);
    // A library correction continues the chain.
    let applied = store
        .apply(athleticstandard::store::Changes {
            corrections: vec![athleticstandard::store::Correction {
                record_id: SUMMARY_ID.to_string(),
                expected_revision: 2,
                data: record.data.clone(),
                extensions: athleticstandard::store::ExtensionsUpdate::Keep,
                changed_by: RecordActor::Tool { tool: "ath".into(), tool_version: "1.0.0".into() },
                became_known_at: Some("2026-08-09T14:00:00.000Z".to_string()),
                became_known_at_basis: BecameKnownAtBasis::LocalWrite,
                stale_style: athleticstandard::store::write::StaleStyle::Revision,
            }],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(applied.records[0].current_revision, 3);
}

#[test]
fn an_athlete_report_cannot_claim_a_source() {
    let path = seeded_database();
    let connection = recipe_connection(&path);
    begin(&connection);
    let error = insert_report_revision(&connection, &report_json(&format!(r#","source_id":"{SOURCE_ID}""#)), None);
    assert!(error.contains("source_id"), "{error}");
    connection.execute_batch("ROLLBACK").unwrap();

    begin(&connection);
    let error = insert_report_revision(
        &connection,
        &report_json(""),
        Some(&format!(r#"{{"ath.fit/demo-widget":{{"source_id":"{SOURCE_ID}"}}}}"#)),
    );
    assert!(error.contains("source_id"), "{error}");
    connection.execute_batch("ROLLBACK").unwrap();

    // Changing a stored report to claim a source is refused too.
    begin(&connection);
    connection
        .execute(
            "INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted) VALUES (?1, 'athlete_report', 1, ?2, 0)",
            params![REPORT_ID, CREATED_AT],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO record_revisions (record_id, revision, record_type, record_json, extensions_json, became_known_at, became_known_at_basis, changed_by_json, previous_revision)
             VALUES (?1, 1, 'athlete_report', ?2, NULL, ?3, 'local_write', ?4, NULL)",
            params![REPORT_ID, report_json(""), CREATED_AT, TOOL_ACTOR],
        )
        .unwrap();
    let error = refusal(
        &connection,
        "UPDATE record_revisions SET record_json = json_set(record_json, '$.source_id', ?1) WHERE record_id = ?2",
        params![SOURCE_ID, REPORT_ID],
    );
    assert!(error.contains("athlete-reported record data cannot contain source_id"), "{error}");
    connection.execute_batch("ROLLBACK").unwrap();

    // Triggers do not depend on the connection's foreign-key setting.
    connection.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
    let error = refusal(
        &connection,
        "INSERT INTO record_revisions (record_id, revision, record_type, record_json, extensions_json, became_known_at, became_known_at_basis, changed_by_json, previous_revision)
         VALUES ('01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e99', 1, 'athlete_report', ?1, NULL, ?2, 'local_write', ?3, NULL)",
        params![report_json(&format!(r#","source_id":"{SOURCE_ID}""#)), CREATED_AT, TOOL_ACTOR],
    );
    assert!(error.contains("source_id"), "{error}");
}

#[test]
fn a_measured_record_requires_its_source() {
    let path = seeded_database();
    let connection = recipe_connection(&path);
    connection.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
    let error = refusal(
        &connection,
        "INSERT INTO point_measurements (record_id, record_type, source_id, quantity, recorded_at, standard_value, standard_unit, provider_record_id, received_at, data_json)
         VALUES ('01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e30', 'point_measurement', NULL, 'hrv_rmssd', ?1, 50, 'ms', NULL, ?1, '{}')",
        [CREATED_AT],
    );
    assert!(error.contains("measured records require source_id"), "{error}");

    // A source that does not exist fails the deferred foreign key at commit.
    connection.execute_batch("PRAGMA foreign_keys = ON").unwrap();
    begin(&connection);
    connection
        .execute(
            "INSERT INTO point_measurements (record_id, record_type, source_id, quantity, recorded_at, standard_value, standard_unit, provider_record_id, received_at, data_json)
             VALUES ('01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e30', 'point_measurement', '01890f3e-e7d3-7cc0-88c8-5d9c6b3a1eff', 'hrv_rmssd', ?1, 50, 'ms', NULL, ?1, '{}')",
            [CREATED_AT],
        )
        .unwrap();
    let error = connection.execute_batch("COMMIT").unwrap_err().to_string();
    assert!(error.contains("FOREIGN KEY"), "{error}");
    connection.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn revision_chains_and_record_types_are_enforced() {
    let path = seeded_database();
    let connection = recipe_connection(&path);
    // A revision whose type disagrees with its record.
    let error = refusal(
        &connection,
        "INSERT INTO record_revisions (record_id, revision, record_type, record_json, extensions_json, became_known_at, became_known_at_basis, changed_by_json, previous_revision)
         VALUES (?1, 2, 'athlete_report', ?2, NULL, ?3, 'local_write', ?4, 1)",
        params![SOURCE_ID, report_json(""), CREATED_AT, TOOL_ACTOR],
    );
    assert!(error.contains("record_type"), "{error}");
    // A revision that skips a number.
    let error = refusal(
        &connection,
        "INSERT INTO record_revisions (record_id, revision, record_type, record_json, extensions_json, became_known_at, became_known_at_basis, changed_by_json, previous_revision)
         VALUES (?1, 3, 'source', ?2, NULL, ?3, 'local_write', ?4, 1)",
        params![SOURCE_ID, SOURCE_JSON, CREATED_AT, TOOL_ACTOR],
    );
    assert!(error.contains("CHECK"), "{error}");
    // A current revision that does not exist.
    let error = refusal(&connection, "UPDATE records SET current_revision = 5 WHERE record_id = ?1", [SOURCE_ID]);
    assert!(error.contains("records.current_revision must reference an existing revision"), "{error}");
    // A typed row of the wrong type.
    let error = refusal(
        &connection,
        "INSERT INTO sources (record_id, record_type, source_type, data_writer, data_route, data_json)
         VALUES ('01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e31', 'point_measurement', 'manual', 'x', 'y', '{}')",
        [],
    );
    assert!(error.contains("source"), "{error}");
}

#[test]
fn deletion_and_erasure_rules_are_enforced() {
    let path = seeded_database();
    let connection = recipe_connection(&path);
    // A deletion flag needs its deletion row.
    let error = refusal(&connection, "UPDATE records SET is_deleted = 1 WHERE record_id = ?1", [SOURCE_ID]);
    assert!(error.contains("records.is_deleted must match record_deletions"), "{error}");

    // The soft-delete recipe works, and the library hides the record.
    begin(&connection);
    connection
        .execute(
            "INSERT INTO record_deletions (record_id, record_revision, deleted_at, deleted_by_json) VALUES (?1, 1, '2026-08-09T15:00:00.000Z', ?2)",
            params![SOURCE_ID, TOOL_ACTOR],
        )
        .unwrap();
    connection.execute("UPDATE records SET is_deleted = 1 WHERE record_id = ?1", [SOURCE_ID]).unwrap();
    connection
        .execute("UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1", [])
        .unwrap();
    connection.execute_batch("COMMIT").unwrap();
    {
        let store = Store::open(&path, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
        assert!(store.list_records(&ListOptions::default()).unwrap().is_empty());
        assert!(store.get_record(SOURCE_ID).unwrap().unwrap().is_deleted);
        let result = check_database(&store).unwrap();
        assert!(result.valid && result.issues.is_empty(), "{:?}", result.issues);
    }

    // An erasure marker blocks a records row for that ID.
    let erased = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e40";
    connection
        .execute(
            "INSERT INTO record_erasure_markers (record_id, erased_at, reason_code) VALUES (?1, '2026-08-09T16:00:00.000Z', 'athlete_request')",
            [erased],
        )
        .unwrap();
    let error = refusal(
        &connection,
        "INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted) VALUES (?1, 'source', 1, ?2, 0)",
        params![erased, CREATED_AT],
    );
    assert!(error.contains("erased records cannot have a current records row"), "{error}");
    // And blocks changes to a records row that still has one.
    connection
        .execute(
            "INSERT INTO record_erasure_markers (record_id, erased_at, reason_code) VALUES (?1, '2026-08-09T16:00:00.000Z', 'other')",
            [SOURCE_ID],
        )
        .unwrap();
    let error = refusal(&connection, "UPDATE records SET created_at = created_at WHERE record_id = ?1", [SOURCE_ID]);
    assert!(error.contains("erased records cannot have a current records row"), "{error}");
}

#[test]
fn a_prediction_link_must_name_existing_revisions() {
    let path = copy_fixture(&demo_fixture(), "ath-direct-link-", true);
    let mut store = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
    execute_confirmed_migration(&mut store, &MigrationOptions::default()).unwrap();
    store.close().unwrap();
    let connection = recipe_connection(&path);
    let (prediction, result): (String, String) = connection
        .query_row("SELECT prediction_id, benchmark_result_id FROM prediction_result_links", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    let error = refusal(
        &connection,
        "INSERT INTO prediction_result_links (record_id, record_type, prediction_id, prediction_revision, benchmark_result_id, benchmark_result_revision, linked_at, created_by_type, content_version, status, data_json)
         VALUES ('01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e50', 'prediction_result_link', ?1, 99, ?2, 1, ?3, 'athlete', 1, 'active', '{}')",
        params![prediction, result, CREATED_AT],
    );
    assert!(error.contains("prediction_result_links must reference existing revisions"), "{error}");
}

#[test]
fn the_internal_table_is_not_a_stable_writable_table() {
    let contract = published_contract().unwrap();
    let path = seeded_database();
    let connection = recipe_connection(&path);
    let columns: Vec<String> = connection
        .prepare("SELECT name FROM pragma_table_info('internal_pending_file_operations')")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(columns, ["operation_id", "operation_kind", "relative_path", "created_at"]);
    assert!(contract.internal_tables.iter().any(|table| table == "internal_pending_file_operations"));
    // It holds no record values: no column refers to records or revisions.
    let foreign: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_list('internal_pending_file_operations')", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(foreign, 0);
}

// ---- Further rules from the published constraints and triggers ----------

const NUTRITION_ID: &str = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e60";
const ATTACHMENT_ID: &str = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e61";
const PROFILE_ID: &str = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e62";
const SECOND_ID: &str = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e63";
const THIRD_ID: &str = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e64";
const ATTACHMENT_JSON: &str = r#"{"file_name":"meal-photo.png","media_type":"image/png","byte_count":68,"sha256":"d3fe8e599fb1deea9821063c744434802875c26cceeb9f2d55e8566b15a012f4","description":"Meal photo"}"#;
const PROFILE_JSON: &str = r#"{"display_units":"metric","default_time_zone":"America/Chicago"}"#;
const PLAIN_EXTENSIONS: &str = r#"{"ath.fit/demo-widget":{"note":"kept"}}"#;

fn nutrition_json(extra: &str) -> String {
    format!(
        r#"{{"reported_at":"{CREATED_AT}","input_reported_at":{{"timestamp_text":"2026-08-09T07:00:00-05:00","utc_offset_text":"-05:00","utc_offset_meaning":"documented_recording_offset","timestamp_basis":"source_offset_timestamp"}},"reported_via":"text","athlete_description":"Rice and beans","reported_items":[]{extra}}}"#
    )
}

fn benchmark_json(key: &str) -> String {
    format!(
        r#"{{"authorship":{{"author":{{"author_type":"athlete"}},"authored_at":"{CREATED_AT}","evidence":[],"content_version":1,"status":"active"}},"benchmark_key":"{key}","benchmark_type":"named_wod","score_type":"time","definition":"For time"}}"#
    )
}

/// The recipe's first rows for a new record: its records row and its first
/// revision. Returns SQLite's refusal of the revision, if any.
fn insert_first_revision(
    connection: &Connection,
    record_id: &str,
    record_type: &str,
    record_json: &str,
    extensions_json: Option<&str>,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO records (record_id, record_type, current_revision, created_at, is_deleted) VALUES (?1, ?2, 1, ?3, 0)",
            params![record_id, record_type, CREATED_AT],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO record_revisions (record_id, revision, record_type, record_json, extensions_json, became_known_at, became_known_at_basis, changed_by_json, previous_revision)
             VALUES (?1, 1, ?2, ?3, ?4, ?5, 'local_write', ?6, NULL)",
            params![record_id, record_type, record_json, extensions_json, CREATED_AT, TOOL_ACTOR],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn next_database_revision(connection: &Connection) {
    connection
        .execute("UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1", [])
        .unwrap();
}

/// The library reads the file and `ath check` finds nothing wrong.
fn assert_checked(path: &std::path::Path) {
    let store = Store::open(path, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
    let result = check_database(&store).unwrap();
    assert!(result.valid && result.issues.is_empty(), "{:?}", result.issues);
}

#[test]
fn nutrition_entries_attachments_and_profiles_cannot_claim_a_source() {
    let path = seeded_database();
    let connection = recipe_connection(&path);
    let claim = format!(r#"{{"ath.fit/demo-widget":{{"nested":[{{"source_id":"{SOURCE_ID}"}}]}}}}"#);
    let nutrition = nutrition_json("");
    let cases = [
        (NUTRITION_ID, "nutrition_entry", nutrition.as_str()),
        (ATTACHMENT_ID, "attachment", ATTACHMENT_JSON),
        (PROFILE_ID, "athlete_profile", PROFILE_JSON),
    ];
    for (record_id, record_type, record_json) in cases {
        // Extensions that name a source at any depth are refused.
        begin(&connection);
        let error = insert_first_revision(&connection, record_id, record_type, record_json, Some(&claim)).unwrap_err();
        assert!(
            error.contains("athlete-reported record extensions cannot contain source_id"),
            "{record_type}: {error}"
        );
        connection.execute_batch("ROLLBACK").unwrap();

        // The same revision without the claim is accepted. Changing its
        // extensions to add the claim is refused.
        begin(&connection);
        insert_first_revision(&connection, record_id, record_type, record_json, Some(PLAIN_EXTENSIONS)).unwrap();
        let error = refusal(
            &connection,
            "UPDATE record_revisions SET extensions_json = ?1 WHERE record_id = ?2",
            params![claim, record_id],
        );
        assert!(
            error.contains("athlete-reported record extensions cannot contain source_id"),
            "{record_type}: {error}"
        );
        connection.execute_batch("ROLLBACK").unwrap();
    }

    // A nutrition entry's own data cannot name a source either.
    begin(&connection);
    let claimed = nutrition_json(&format!(r#","source_id":"{SOURCE_ID}""#));
    let error = insert_first_revision(&connection, NUTRITION_ID, "nutrition_entry", &claimed, None).unwrap_err();
    assert!(error.contains("athlete-reported record data cannot contain source_id"), "{error}");
    connection.execute_batch("ROLLBACK").unwrap();
    begin(&connection);
    insert_first_revision(&connection, NUTRITION_ID, "nutrition_entry", &nutrition, None).unwrap();
    let error = refusal(
        &connection,
        "UPDATE record_revisions SET record_json = json_set(record_json, '$.source_id', ?1) WHERE record_id = ?2",
        params![SOURCE_ID, NUTRITION_ID],
    );
    assert!(error.contains("athlete-reported record data cannot contain source_id"), "{error}");
    connection.execute_batch("ROLLBACK").unwrap();

    // Complete direct writes without a claim commit and pass the check.
    begin(&connection);
    insert_first_revision(&connection, NUTRITION_ID, "nutrition_entry", &nutrition, Some(PLAIN_EXTENSIONS)).unwrap();
    connection
        .execute(
            "INSERT INTO nutrition_entries (record_id, record_type, reported_at, athlete_description, data_json)
             VALUES (?1, 'nutrition_entry', ?2, 'Rice and beans', ?3)",
            params![NUTRITION_ID, CREATED_AT, nutrition],
        )
        .unwrap();
    insert_first_revision(&connection, PROFILE_ID, "athlete_profile", PROFILE_JSON, Some(PLAIN_EXTENSIONS)).unwrap();
    connection
        .execute(
            "INSERT INTO athlete_profile (record_id, record_type, default_time_zone, data_json)
             VALUES (?1, 'athlete_profile', 'America/Chicago', ?2)",
            params![PROFILE_ID, PROFILE_JSON],
        )
        .unwrap();
    next_database_revision(&connection);
    connection.execute_batch("COMMIT").unwrap();
    drop(connection);
    assert_checked(&path);
}

#[test]
fn one_profile_one_source_per_route_one_benchmark_per_key_and_one_record_per_import_key() {
    let path = seeded_database();
    let connection = recipe_connection(&path);
    let insert_profile = "INSERT INTO athlete_profile (record_id, record_type, default_time_zone, data_json)
                          VALUES (?1, 'athlete_profile', 'America/Chicago', ?2)";
    let insert_benchmark = "INSERT INTO benchmark_definitions (record_id, record_type, benchmark_key, benchmark_type, score_type, definition, data_json)
                            VALUES (?1, 'benchmark_definition', ?2, 'named_wod', 'time', 'For time', ?3)";
    let insert_import_key =
        "INSERT INTO record_import_keys (record_id, match_key_method, match_key, imported_at) VALUES (?1, ?2, ?3, ?4)";
    let source_key = r#"["source","WHOOP","whoop_csv",""]"#;

    // The first profile, the first benchmark with its key, and the source's
    // import key commit.
    begin(&connection);
    insert_first_revision(&connection, PROFILE_ID, "athlete_profile", PROFILE_JSON, None).unwrap();
    connection.execute(insert_profile, params![PROFILE_ID, PROFILE_JSON]).unwrap();
    insert_first_revision(&connection, THIRD_ID, "benchmark_definition", &benchmark_json("fran"), None).unwrap();
    connection.execute(insert_benchmark, params![THIRD_ID, "fran", benchmark_json("fran")]).unwrap();
    connection.execute(insert_import_key, params![SOURCE_ID, "whoop_csv/v1", source_key, CREATED_AT]).unwrap();
    next_database_revision(&connection);
    connection.execute_batch("COMMIT").unwrap();

    // A second profile row is refused.
    begin(&connection);
    insert_first_revision(&connection, SECOND_ID, "athlete_profile", PROFILE_JSON, None).unwrap();
    let error = refusal(&connection, insert_profile, params![SECOND_ID, PROFILE_JSON]);
    assert!(error.contains("UNIQUE constraint failed: index 'athlete_profile_single_record_idx'"), "{error}");
    connection.execute_batch("ROLLBACK").unwrap();

    // A second source with the same writer, route, and no sensor type is
    // refused. The same writer and route with a sensor type is another source.
    begin(&connection);
    insert_first_revision(&connection, SECOND_ID, "source", SOURCE_JSON, None).unwrap();
    let error = refusal(
        &connection,
        "INSERT INTO sources (record_id, record_type, source_type, data_writer, data_route, vendor, sensor_type, data_json)
         VALUES (?1, 'source', 'wearable', 'WHOOP', 'whoop_csv', 'whoop', NULL, ?2)",
        params![SECOND_ID, SOURCE_JSON],
    );
    assert!(error.contains("UNIQUE constraint failed: index 'sources_writer_route_sensor_idx'"), "{error}");
    connection.execute_batch("ROLLBACK").unwrap();
    let sensor_json = r#"{"source_type":"wearable","data_writer":"WHOOP","data_route":"whoop_csv","vendor":"whoop","sensor_type":"ppg"}"#;
    begin(&connection);
    insert_first_revision(&connection, SECOND_ID, "source", sensor_json, None).unwrap();
    connection
        .execute(
            "INSERT INTO sources (record_id, record_type, source_type, data_writer, data_route, vendor, sensor_type, data_json)
             VALUES (?1, 'source', 'wearable', 'WHOOP', 'whoop_csv', 'whoop', 'ppg', ?2)",
            params![SECOND_ID, sensor_json],
        )
        .unwrap();
    next_database_revision(&connection);
    connection.execute_batch("COMMIT").unwrap();

    // A second benchmark with the same key is refused; another key is not.
    let grace_id = "01890f3e-e7d3-7cc0-88c8-5d9c6b3a1e65";
    begin(&connection);
    insert_first_revision(&connection, grace_id, "benchmark_definition", &benchmark_json("fran"), None).unwrap();
    let error = refusal(&connection, insert_benchmark, params![grace_id, "fran", benchmark_json("fran")]);
    assert!(error.contains("UNIQUE constraint failed: benchmark_definitions.benchmark_key"), "{error}");
    connection.execute_batch("ROLLBACK").unwrap();
    begin(&connection);
    insert_first_revision(&connection, grace_id, "benchmark_definition", &benchmark_json("grace"), None).unwrap();
    connection.execute(insert_benchmark, params![grace_id, "grace", benchmark_json("grace")]).unwrap();
    next_database_revision(&connection);
    connection.execute_batch("COMMIT").unwrap();

    // One import key names one record. The same key under another method,
    // and another key for the same record, are accepted.
    let error = refusal(&connection, insert_import_key, params![SECOND_ID, "whoop_csv/v1", source_key, CREATED_AT]);
    assert!(
        error.contains("UNIQUE constraint failed: record_import_keys.match_key_method, record_import_keys.match_key"),
        "{error}"
    );
    begin(&connection);
    connection.execute(insert_import_key, params![SECOND_ID, "oura_csv/v1", source_key, CREATED_AT]).unwrap();
    let sensor_key = r#"["source","WHOOP","whoop_csv","ppg"]"#;
    connection.execute(insert_import_key, params![SECOND_ID, "whoop_csv/v1", sensor_key, CREATED_AT]).unwrap();
    connection.execute_batch("COMMIT").unwrap();
    drop(connection);
    assert_checked(&path);
}

#[test]
fn sleep_and_workout_sessions_cannot_end_before_they_start() {
    let path = seeded_database();
    let connection = recipe_connection(&path);
    let start = "2026-08-09T03:30:00.000Z";
    let before_start = "2026-08-09T03:29:59.999Z";
    let sleep = "INSERT INTO sleep_sessions (record_id, record_type, source_id, start_at, end_at, provider_record_id, received_at, data_json)
                 VALUES (?1, 'sleep_session', ?2, ?3, ?4, NULL, ?5, '{}')";
    let workout = "INSERT INTO workout_sessions (record_id, record_type, source_id, start_at, end_at, activity_type, provider_record_id, received_at, data_json)
                   VALUES (?1, 'workout_session', ?2, ?3, ?4, 'running', NULL, ?5, '{}')";
    // Foreign keys wait for the commit, and these transactions roll back, so
    // only the typed rows' own checks run.
    for (table, sql) in [("sleep_sessions", sleep), ("workout_sessions", workout)] {
        begin(&connection);
        let error = refusal(&connection, sql, params![SECOND_ID, SOURCE_ID, start, before_start, CREATED_AT]);
        assert!(error.contains("CHECK constraint failed: end_at >= start_at"), "{table}: {error}");
        // A session that ends when it starts, and one that ends later, are accepted.
        connection.execute(sql, params![SECOND_ID, SOURCE_ID, start, start, CREATED_AT]).unwrap();
        connection.execute(sql, params![THIRD_ID, SOURCE_ID, start, "2026-08-09T11:00:00.000Z", CREATED_AT]).unwrap();
        connection.execute_batch("ROLLBACK").unwrap();
    }
}

#[test]
fn revision_extensions_are_limited_to_65536_characters() {
    let path = seeded_database();
    let connection = recipe_connection(&path);
    let extensions = |length: usize| {
        let prefix = r#"{"ath.fit/demo-widget":{"note":""#;
        let suffix = r#""}}"#;
        let text = format!("{prefix}{}{suffix}", "x".repeat(length - prefix.len() - suffix.len()));
        assert_eq!(text.len(), length);
        text
    };
    let insert = "INSERT INTO record_revisions (record_id, revision, record_type, record_json, extensions_json, became_known_at, became_known_at_basis, changed_by_json, previous_revision)
                  VALUES (?1, 2, 'source', ?2, ?3, ?4, 'local_write', ?5, 1)";
    begin(&connection);
    let error =
        refusal(&connection, insert, params![SOURCE_ID, SOURCE_JSON, extensions(65_537), CREATED_AT, TOOL_ACTOR]);
    assert!(error.contains("CHECK constraint failed") && error.contains("length(extensions_json) <= 65536"), "{error}");
    connection.execute(insert, params![SOURCE_ID, SOURCE_JSON, extensions(65_536), CREATED_AT, TOOL_ACTOR]).unwrap();
    connection.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn an_owner_links_an_attachment_once() {
    let path = seeded_database();
    let connection = recipe_connection(&path);
    let link = "INSERT INTO record_attachment_links (owner_record_id, attachment_order, attachment_record_id) VALUES (?1, ?2, ?3)";
    // Foreign keys wait for the commit, and this transaction rolls back, so
    // only the link table's own keys run.
    begin(&connection);
    connection.execute(link, params![NUTRITION_ID, 0, ATTACHMENT_ID]).unwrap();
    let error = refusal(&connection, link, params![NUTRITION_ID, 1, ATTACHMENT_ID]);
    assert!(
        error.contains(
            "UNIQUE constraint failed: record_attachment_links.owner_record_id, record_attachment_links.attachment_record_id"
        ),
        "{error}"
    );
    // One position holds one attachment.
    let error = refusal(&connection, link, params![NUTRITION_ID, 0, SECOND_ID]);
    assert!(
        error.contains(
            "UNIQUE constraint failed: record_attachment_links.owner_record_id, record_attachment_links.attachment_order"
        ),
        "{error}"
    );
    // A second attachment in the next position is accepted.
    connection.execute(link, params![NUTRITION_ID, 1, SECOND_ID]).unwrap();
    connection.execute_batch("ROLLBACK").unwrap();
}
