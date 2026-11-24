//! Every record type through its whole life, as section 13.2 of
//! `build-history/v2.1.1/spec.md` requires: creation, a correction that
//! removes a child row, history, export, restore, merge, normal deletion, and
//! privacy erasure.
//!
//! The record types come from the model's own list, so a new type cannot be
//! left out. Each type starts from its record in the synthetic demo history.
//! The records it names are created first, in the same fresh database. Every
//! database the test makes must pass the library check at the end.

mod demo_support;
#[path = "temporary_root.rs"]
mod temporary_root;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use athleticstandard::files;
use athleticstandard::json::{self, Value};
use athleticstandard::model::*;
use athleticstandard::package::*;
use athleticstandard::store::check::check_database;
use athleticstandard::store::encode::child_rows;
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::write::StaleStyle;
use athleticstandard::store::{
    Changes, Correction, CreateOptions, Deletion, ExtensionsUpdate, NewRecord, OpenOptions, Store,
};
use rusqlite::{Connection, OpenFlags};
use serde_json::json;

const DATABASE_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000001";
const DATABASE_CREATED_AT: &str = "2026-08-09T12:00:00.000Z";
const ATTACHMENT_ID: &str = "01890f3e-e7d3-7cc0-88c8-00000000000e";
const SAMPLE_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000008";
const GRADE_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000012";
const POINT_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000007";
const CORRECTED_AT: &str = "2026-09-01T12:00:00.000Z";
const DELETED_AT: &str = "2026-09-02T12:00:00.000Z";
const ERASED_AT: &str = "2026-09-03T12:00:00.000Z";

/// Types whose data has no child collection, so their correction changes a
/// plain field only. Every other type's correction removes a child row.
const WITHOUT_CHILDREN: &[RecordType] = &[
    RecordType::AthleteProfile,
    RecordType::Source,
    RecordType::SourceDevice,
    RecordType::Attachment,
    RecordType::PredictionResultLink,
];

// ---- Demo records and the edits for each type ------------------------------

/// The demo's create entries, in the order the demo writes them.
fn demo_creates() -> Vec<Value> {
    let history = json::parse(include_str!("demo_support/history.json")).unwrap();
    history["changes"][0]["creates"].as_array().unwrap().clone()
}

fn record_id_of(create: &Value) -> &str {
    create["record_id"].as_str().unwrap()
}

/// Keep only the first item of the array at `path`.
fn keep_first(value: &mut Value, path: &[&str]) {
    let mut target = value;
    for key in path {
        target = &mut target[*key];
    }
    let items = target.as_array_mut().unwrap_or_else(|| panic!("{path:?} is an array"));
    assert!(items.len() >= 2, "{path:?} needs two items so one can be removed");
    items.truncate(1);
}

fn remove(value: &mut Value, key: &str) {
    assert!(value.as_object_mut().unwrap().remove(key).is_some(), "{key} must be present to be removed");
}

fn push(value: &mut Value, path: &[&str], item: Value) {
    let mut target = value;
    for key in path {
        target = &mut target[*key];
    }
    if target.is_null() {
        *target = json!([]);
    }
    target.as_array_mut().unwrap().push(item);
}

fn period(id: &str) -> Value {
    json!({ "period_type": "whoop_physiological_cycle", "provider_period_id": id })
}

fn route_point(at: &str, latitude: f64, longitude: f64) -> Value {
    json!({ "recorded_at": at, "latitude_deg": latitude, "longitude_deg": longitude })
}

/// The first revision and the corrected second revision of a type's data.
/// Each correction is a real change. A type with child collections starts with
/// at least one child, and its correction removes one.
fn revisions_for(record_type: RecordType, demo: &Value) -> (Value, Value) {
    use RecordType::*;
    let mut base = demo.clone();
    match record_type {
        PointMeasurement => {
            base["calculations"] = json!([{
                "from_quantity": "heart_rate",
                "method": "invented-test-method",
                "calculation_version": "1",
                "input_records": [{ "record_id": SAMPLE_ID, "revision": 1 }],
            }])
        }
        MeasurementSample => base["provider_reporting_periods"] = json!([period("cycle-2026-08-09")]),
        WorkoutSession => {
            push(&mut base, &["segments"], json!({ "label": "Cooldown", "source_reported_summaries": [] }));
            base["route"] = json!([
                route_point("2026-08-10T17:25:00.000Z", 10.0, 20.0),
                route_point("2026-08-10T17:30:00.000Z", 10.001, 20.001),
            ]);
        }
        BenchmarkResult => {
            base["segments"] = json!([
                { "label": "21", "score": { "duration_s": 90 } },
                { "label": "15", "score": { "duration_s": 110 } },
            ])
        }
        ProviderSummary | ProviderScore => push(&mut base, &["provider_reporting_periods"], period("cycle-2026-08-10")),
        AthleteReport => base["attachment_ids"] = json!([ATTACHMENT_ID]),
        NutritionEntry => push(
            &mut base,
            &["reported_items"],
            json!({ "food_description": "water", "amount": 500, "amount_unit": "ml" }),
        ),
        BenchmarkDefinition => base["authorship"]["evidence"] = json!([{ "record_id": POINT_ID, "revision": 1 }]),
        PredictionErrorAnalysis => push(
            &mut base,
            &["causes"],
            json!({
                "evidence": { "record_id": GRADE_ID, "revision": 1 },
                "explanation": "The grade shows the size of the miss.",
            }),
        ),
        PlannedSession => push(
            &mut base,
            &["blocks"],
            json!({ "block_type": "cooldown", "title": "Easy row", "planned_duration_s": 600 }),
        ),
        _ => {}
    }

    let mut corrected = base.clone();
    let value = |number: f64| json!(number);
    match record_type {
        AthleteProfile => {
            corrected["name"] = json!("Demo Athlete Corrected");
            corrected["display_units"] = json!("imperial");
        }
        Source => corrected["detail"] = json!("Corrected source detail"),
        SourceDevice => {
            corrected["model"] = json!("4.1");
            corrected["accepted_record_count"] = json!(6);
        }
        PointMeasurement => {
            corrected["value"]["standard_value"] = value(64.0);
            corrected["value"]["input_value"] = value(64.0);
            remove(&mut corrected, "calculations");
        }
        MeasurementSample => {
            corrected["value"]["standard_value"] = value(143.0);
            corrected["value"]["input_value"] = value(143.0);
            remove(&mut corrected, "provider_reporting_periods");
        }
        SleepSession => {
            corrected["asleep_seconds"] = value(25_800.0);
            remove(&mut corrected, "provider_reporting_periods");
        }
        WorkoutSession => {
            corrected["name"] = json!("Fran attempt");
            keep_first(&mut corrected, &["segments"]);
            keep_first(&mut corrected, &["route"]);
        }
        BenchmarkResult => {
            corrected["score"]["duration_s"] = value(295.0);
            keep_first(&mut corrected, &["segments"]);
            // A link change: the result no longer names its workout.
            remove(&mut corrected, "workout_session_id");
        }
        ProviderSummary => {
            corrected["value"]["standard_value"] = value(27_600.0);
            corrected["value"]["input_value"] = value(460.0);
            keep_first(&mut corrected, &["provider_reporting_periods"]);
        }
        ProviderScore => {
            corrected["value"]["standard_value"] = value(80.0);
            corrected["value"]["input_value"] = value(80.0);
            keep_first(&mut corrected, &["provider_reporting_periods"]);
        }
        Attachment => corrected["description"] = json!("Meal photo, corrected"),
        AthleteReport => {
            corrected["rating"] = value(3.0);
            remove(&mut corrected, "attachment_ids");
        }
        NutritionEntry => {
            corrected["athlete_description"] = json!("Chicken and rice");
            keep_first(&mut corrected, &["reported_items"]);
            remove(&mut corrected, "attachment_ids");
        }
        BenchmarkDefinition => {
            corrected["definition"] = json!("21-15-9 thrusters (43 kg) and pull-ups for time");
            corrected["authorship"]["evidence"] = json!([]);
        }
        Prediction => {
            corrected["reasoning"] = json!("Only the HRV evidence supported the prediction.");
            corrected["confidence"] = json!("low");
            keep_first(&mut corrected, &["authorship", "evidence"]);
        }
        PredictionResultLink => {
            corrected["status"] = json!("superseded");
            corrected["content_version"] = json!(2);
        }
        PredictionGrade => {
            corrected["authorship"]["status"] = json!("superseded");
            corrected["authorship"]["content_version"] = json!(2);
            corrected["authorship"]["evidence"] = json!([]);
        }
        PredictionErrorAnalysis => {
            corrected["lesson"] = json!("Weigh reported sleep quality more.");
            keep_first(&mut corrected, &["causes"]);
            keep_first(&mut corrected, &["authorship", "evidence"]);
        }
        TrainingPlan => {
            corrected["title"] = json!("Demo training week, revised");
            keep_first(&mut corrected, &["authorship", "evidence"]);
        }
        PlannedSession => {
            corrected["completion_status"] = json!("planned");
            // A link change: the session no longer names a completed workout.
            remove(&mut corrected, "completed_workout_session_id");
            keep_first(&mut corrected, &["blocks"]);
            keep_first(&mut corrected, &["authorship", "evidence"]);
        }
        CoachingNote => {
            corrected["note"] = json!("Hold the opening set to a steady pace.");
            keep_first(&mut corrected, &["authorship", "evidence"]);
        }
        SavedTrainingReview => {
            corrected["summary"] = json!("Training was consistent.");
            keep_first(&mut corrected, &["authorship", "evidence"]);
        }
        Goal => {
            corrected["title"] = json!("Fran under 4:25");
            corrected["target_score"]["duration_s"] = value(265.0);
            keep_first(&mut corrected, &["authorship", "evidence"]);
        }
        NutritionModelInterpretation => {
            corrected["confidence"] = value(0.6);
            keep_first(&mut corrected, &["authorship", "evidence"]);
        }
    }
    assert_ne!(base, corrected, "{record_type}: the correction must change the data");
    (base, corrected)
}

/// Every demo record that `data` names, directly or through another named
/// record, in demo order.
fn prerequisites(data: &Value, creates: &[Value], target_id: &str) -> Vec<Value> {
    fn named(value: &Value, ids: &BTreeSet<&str>, out: &mut BTreeSet<String>) {
        match value {
            Value::String(text) if ids.contains(text.as_str()) => {
                out.insert(text.clone());
            }
            Value::Array(items) => items.iter().for_each(|item| named(item, ids, out)),
            Value::Object(map) => map.values().for_each(|item| named(item, ids, out)),
            _ => {}
        }
    }
    let ids: BTreeSet<&str> = creates.iter().map(record_id_of).collect();
    let mut wanted = BTreeSet::new();
    named(data, &ids, &mut wanted);
    loop {
        let mut more = wanted.clone();
        for create in creates.iter().filter(|create| wanted.contains(record_id_of(create))) {
            named(&create["data"], &ids, &mut more);
        }
        if more == wanted {
            break;
        }
        wanted = more;
    }
    assert!(!wanted.contains(target_id), "a record cannot need itself");
    creates.iter().filter(|create| wanted.contains(record_id_of(create))).cloned().collect()
}

fn decode(record_type: RecordType, data: &Value) -> RecordData {
    RecordData::decode(record_type, data).unwrap_or_else(|error| panic!("{record_type}: {error:?}"))
}

fn new_record(create: &Value, data: &Value) -> NewRecord {
    let record_type = RecordType::parse(create["record_type"].as_str().unwrap()).unwrap();
    NewRecord {
        data: decode(record_type, data),
        extensions: None,
        record_id: Some(record_id_of(create).to_string()),
        created_at: Some(create["created_at"].as_str().unwrap().to_string()),
        became_known_at: create.get("became_known_at").and_then(Value::as_str).map(str::to_string),
        became_known_at_basis: create
            .get("became_known_at_basis")
            .and_then(Value::as_str)
            .map_or(BecameKnownAtBasis::LocalWrite, |basis| BecameKnownAtBasis::parse(basis).unwrap()),
        changed_by: serde_json::from_value(create["changed_by"].clone()).unwrap(),
    }
}

// ---- Databases, packages, and independent reads ---------------------------

fn open(path: &Path) -> Store {
    Store::open(path, OpenOptions::default()).unwrap()
}

fn open_read_only(path: &Path) -> Store {
    Store::open(path, OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap()
}

/// Remove a test folder. Windows cannot remove a folder while a database in
/// it is still open; the test program's root is emptied on its next run.
fn remove_dir(path: &Path) {
    if let Err(error) = fs::remove_dir_all(path) {
        if !cfg!(windows) {
            panic!("could not remove {}: {error}", path.display());
        }
    }
}

/// A new private folder `name` under `parent`, holding `athlete.ath.sqlite`.
fn database_path(parent: &Path, name: &str) -> PathBuf {
    let directory = parent.join(name);
    files::create_private_dir(&directory).unwrap();
    directory.join("athlete.ath.sqlite")
}

fn export(store: &Store, output: &Path) {
    export_package(store, output, &ExportOptions::default()).unwrap();
}

/// Restore a package into a new empty database.
fn restored(package: &Path, parent: &Path, name: &str) -> PathBuf {
    let path = database_path(parent, name);
    let mut store = Store::create(&path, CreateOptions::default()).unwrap();
    let preview = preview_package_import(&store, package).unwrap();
    assert!(preview.restore, "{name}: an empty database takes a restore");
    assert!(preview.conflicts.is_empty(), "{name}: {:?}", preview.conflicts);
    let applied = apply_package_import(&mut store, &preview, &[]).unwrap();
    assert!(applied.restored);
    store.close().unwrap();
    path
}

/// Merge a package that changes only `record_id` into an existing database.
fn merge_changed(path: &Path, package: &Path, record_id: &str, unchanged: usize) {
    let mut store = open(path);
    let preview = preview_package_import(&store, package).unwrap();
    assert!(!preview.restore);
    assert!(preview.conflicts.is_empty(), "{record_id}: {:?}", preview.conflicts);
    assert_eq!(preview.changed_records, [record_id]);
    assert_eq!(preview.new_record_count, 0);
    assert_eq!(preview.matching_record_count, unchanged as u64);
    let applied = apply_package_import(&mut store, &preview, &[]).unwrap();
    assert!(!applied.restored);
    store.close().unwrap();
}

fn inspect(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).unwrap()
}

fn table_names(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap();
    statement.query_map([], |row| row.get(0)).unwrap().map(Result::unwrap).collect()
}

/// Every row of every table except the database's own identity and revision,
/// read with an independent connection.
fn stored_rows(path: &Path) -> BTreeMap<String, Vec<String>> {
    let connection = inspect(path);
    let mut out = BTreeMap::new();
    for table in table_names(&connection) {
        // The change log records how each copy was written, as the database
        // revision does; a restore starts a new one.
        if table == "database_metadata" || table == "database_state" || table == "internal_change_log" {
            continue;
        }
        let mut statement = connection.prepare(&format!("SELECT * FROM {table}")).unwrap();
        let columns = statement.column_count();
        let mut rows: Vec<String> = statement
            .query_map([], |row| {
                let mut cells = Vec::with_capacity(columns);
                for index in 0..columns {
                    cells.push(format!("{:?}", row.get::<_, rusqlite::types::Value>(index)?));
                }
                Ok(cells.join(" | "))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        rows.sort();
        out.insert(table, rows);
    }
    out
}

/// Tables, other than the erasure markers, with a text cell that names `id`.
/// The change log's erasure row is value-free like the marker, so only its
/// other rows count.
fn tables_naming(path: &Path, id: &str) -> Vec<String> {
    let connection = inspect(path);
    let mut found = Vec::new();
    for table in table_names(&connection) {
        if table == "record_erasure_markers" {
            continue;
        }
        let query = if table == "internal_change_log" {
            "SELECT * FROM internal_change_log WHERE change_kind <> 'erasure'".to_string()
        } else {
            format!("SELECT * FROM {table}")
        };
        let mut statement = connection.prepare(&query).unwrap();
        let columns = statement.column_count();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let names = (0..columns).any(|index| match row.get::<_, rusqlite::types::Value>(index).unwrap() {
                rusqlite::types::Value::Text(text) => text.contains(id),
                _ => false,
            });
            if names {
                found.push(table.clone());
                break;
            }
        }
    }
    found
}

/// Child rows by table, as the record encoder would write them for `data`.
fn expected_children(data: &RecordData) -> BTreeMap<&'static str, usize> {
    child_rows(data).into_iter().map(|child| (child.table, child.rows.len())).collect()
}

/// Child rows by table, as stored for `record_id`.
fn stored_children(path: &Path, data: &RecordData, record_id: &str) -> BTreeMap<&'static str, usize> {
    let connection = inspect(path);
    child_rows(data)
        .into_iter()
        .map(|child| {
            let count: i64 = connection
                .query_row(
                    &format!("SELECT COUNT(*) FROM {} WHERE {} = ?1", child.table, child.owner_column),
                    [record_id],
                    |row| row.get(0),
                )
                .unwrap();
            (child.table, count as usize)
        })
        .collect()
}

/// Every record with its complete history, through the library.
fn records_and_history(path: &Path) -> BTreeMap<String, (CurrentRecord, Vec<RecordRevision>)> {
    let store = open_read_only(path);
    let records = store.list_records(&ListOptions { record_type: None, include_deleted: true, limit: 10_000 }).unwrap();
    records
        .into_iter()
        .map(|record| {
            let revisions = store.revisions(&record.record_id).unwrap();
            (record.record_id.clone(), (record, revisions))
        })
        .collect()
}

/// Two databases hold the same records, history, deletions, erasure markers,
/// and typed rows, through the library and through SQL.
fn assert_same_records(label: &str, left: &Path, right: &Path) {
    assert_eq!(records_and_history(left), records_and_history(right), "{label}: records or history differ");
    assert_eq!(stored_rows(left), stored_rows(right), "{label}: stored rows differ");
}

/// A restore also keeps the database revision. A merge advances it once for
/// the whole package, so a merged copy can differ there.
fn assert_same_restore(label: &str, source: &Path, restored: &Path) {
    assert_same_records(label, source, restored);
    let revision = |path: &Path| open_read_only(path).database_revision().unwrap();
    assert_eq!(revision(source), revision(restored), "{label}: database revision");
}

fn assert_checks_clean(label: &str, path: &Path) {
    let store = open_read_only(path);
    let report = check_database(&store).unwrap();
    assert!(report.valid, "{label}: {:?}", report.issues);
    assert!(report.issues.is_empty(), "{label}: {:?}", report.issues);
}

/// Erase `record_id` at `revision` and check that only value-free markers
/// remain for every erased record. Returns the erased IDs.
fn erase_and_check(path: &Path, record_id: &str, revision: u64) -> Vec<String> {
    let mut store = open(path);
    let preview = preview_erasure(&store, record_id, revision).unwrap();
    let erased: Vec<String> = preview.records.iter().map(|record| record.record_id.clone()).collect();
    assert!(erased.iter().any(|id| id == record_id));
    let result = erase_record(&mut store, &preview, ErasureReasonCode::AthleteRequest, Some(ERASED_AT)).unwrap();
    assert_eq!(result.erased_record_ids, erased);
    for id in &erased {
        assert!(store.get_record(id).unwrap().is_none(), "{id} kept its current record");
        assert!(store.revisions(id).unwrap().is_empty(), "{id} kept history");
    }
    assert_eq!(
        preview_erasure(&store, record_id, revision).unwrap_err().to_string(),
        format!("Record was already erased: {record_id}")
    );
    store.close().unwrap();

    let connection = inspect(path);
    let columns: Vec<String> = connection
        .prepare("SELECT name FROM pragma_table_info('record_erasure_markers') ORDER BY cid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(columns, ["record_id", "erased_at", "reason_code"], "a marker holds no values");
    for id in &erased {
        let marker: (String, String) = connection
            .query_row("SELECT erased_at, reason_code FROM record_erasure_markers WHERE record_id = ?1", [id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(marker, (ERASED_AT.to_string(), "athlete_request".to_string()), "{id}");
        assert_eq!(tables_naming(path, id), Vec::<String>::new(), "{id}: only its marker may name it");
    }
    if erased.iter().any(|id| id == ATTACHMENT_ID) {
        let (name, _) = demo_support::attachment();
        assert!(!path.parent().unwrap().join("attachments").join(name).exists(), "the attachment file stayed");
    }
    erased
}

/// Each erased ID appears once in a package: in its marker, which holds only
/// the ID, the time, and the reason.
fn assert_package_holds_only_markers(package: &Path, erased: &[String]) {
    let document = json::parse(&fs::read_to_string(package.join(PACKAGE_DOCUMENT)).unwrap()).unwrap();
    let markers = document["record_erasure_markers"].as_array().unwrap();
    let mut texts = Vec::new();
    for entry in walk(package) {
        texts.push(String::from_utf8_lossy(&fs::read(entry).unwrap()).into_owned());
    }
    for id in erased {
        let marker = markers.iter().find(|marker| marker["record_id"] == id.as_str()).expect("a marker");
        assert_eq!(marker.as_object().unwrap().keys().collect::<Vec<_>>(), ["erased_at", "reason_code", "record_id"]);
        let mentions: usize = texts.iter().map(|text| text.matches(id.as_str()).count()).sum();
        assert_eq!(mentions, 1, "{id}: the package names an erased record outside its marker");
    }
}

fn walk(directory: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}

// ---- The lifecycle ------------------------------------------------------------

fn lifecycle(record_type: RecordType, creates: &[Value]) {
    let label = record_type.as_str();
    let target = creates
        .iter()
        .find(|create| create["record_type"] == label)
        .unwrap_or_else(|| panic!("the demo has no {label}"))
        .clone();
    let id = record_id_of(&target).to_string();
    let (base, corrected) = revisions_for(record_type, &target["data"]);
    let needed = prerequisites(&base, creates, &id);
    let base_data = decode(record_type, &base);
    let corrected_data = decode(record_type, &corrected);

    let directory = temporary_root::new_dir(&format!("ath-lifecycle-{label}-"));
    let source = database_path(&directory, "source");
    if id == ATTACHMENT_ID || needed.iter().any(|create| record_id_of(create) == ATTACHMENT_ID) {
        let attachments = source.parent().unwrap().join("attachments");
        files::create_private_dir(&attachments).unwrap();
        let (name, bytes) = demo_support::attachment();
        fs::write(attachments.join(name), bytes).unwrap();
    }

    // Create: the named records first, then the record itself.
    let mut store = Store::create(
        &source,
        CreateOptions { database_id: Some(DATABASE_ID.into()), created_at: Some(DATABASE_CREATED_AT.into()) },
    )
    .unwrap();
    if !needed.is_empty() {
        let named = needed.iter().map(|create| new_record(create, &create["data"])).collect();
        store.apply(Changes { creates: named, ..Changes::default() }).unwrap();
    }
    store.apply(Changes { creates: vec![new_record(&target, &base)], ..Changes::default() }).unwrap();
    let created = store.get_record(&id).unwrap().expect("the record exists");
    assert_eq!((created.current_revision, created.is_deleted), (1, false), "{label}");
    assert_eq!(created.data, base_data, "{label}");
    assert_eq!(stored_children(&source, &base_data, &id), expected_children(&base_data), "{label}");
    let first_package = directory.join("first");
    export(&store, &first_package);

    // Correct: a second revision, and the removed child rows are gone.
    store
        .apply(Changes {
            corrections: vec![Correction {
                record_id: id.clone(),
                expected_revision: 1,
                data: corrected_data.clone(),
                extensions: ExtensionsUpdate::Keep,
                changed_by: RecordActor::Athlete,
                became_known_at: Some(CORRECTED_AT.into()),
                became_known_at_basis: BecameKnownAtBasis::LocalWrite,
                stale_style: StaleStyle::Revision,
            }],
            ..Changes::default()
        })
        .unwrap();
    let current = store.get_record(&id).unwrap().unwrap();
    assert_eq!((current.current_revision, current.is_deleted), (2, false), "{label}");
    assert_eq!(current.data, corrected_data, "{label}");
    let history = store.revisions(&id).unwrap();
    assert_eq!(history.len(), 2, "{label}");
    assert_eq!((history[0].revision, history[0].previous_revision), (1, None), "{label}");
    assert_eq!((history[1].revision, history[1].previous_revision), (2, Some(1)), "{label}");
    assert_eq!(history[0].data, base_data, "{label}: the first revision changed");
    assert_eq!(history[1].data, corrected_data, "{label}");
    assert_eq!(history[1].became_known_at, CORRECTED_AT, "{label}");

    let before = expected_children(&base_data);
    let after = expected_children(&corrected_data);
    assert_eq!(stored_children(&source, &corrected_data, &id), after, "{label}: child rows after the correction");
    assert_eq!(expected_children(&history[0].data), before, "{label}: the first revision keeps its children");
    if WITHOUT_CHILDREN.contains(&record_type) {
        assert!(before.values().all(|count| *count == 0), "{label} has child rows: {before:?}");
    } else {
        assert!(
            before.iter().any(|(table, count)| after[table] < *count),
            "{label}: the correction removes no child row ({before:?} to {after:?})"
        );
    }

    // Export and restore the corrected database.
    let corrected_package = directory.join("corrected");
    export(&store, &corrected_package);
    let restored_corrected = restored(&corrected_package, &directory, "restored-corrected");
    assert_same_restore(&format!("{label} restore"), &source, &restored_corrected);

    // Merge the corrected package into a database that has only revision 1.
    let merged = restored(&first_package, &directory, "merged");
    assert_eq!(open_read_only(&merged).revisions(&id).unwrap().len(), 1, "{label}");
    merge_changed(&merged, &corrected_package, &id, needed.len());
    assert_same_records(&format!("{label} merge"), &source, &merged);

    // Delete: the record is hidden, its history stays, and it cannot change.
    store
        .apply(Changes {
            deletions: vec![Deletion {
                record_id: id.clone(),
                expected_revision: 2,
                deleted_by: RecordActor::Athlete,
                deleted_at: Some(DELETED_AT.into()),
            }],
            ..Changes::default()
        })
        .unwrap();
    let deleted = store.get_record(&id).unwrap().unwrap();
    assert_eq!((deleted.current_revision, deleted.is_deleted), (2, true), "{label}");
    assert_eq!(store.revisions(&id).unwrap(), history, "{label}: deletion changed history");
    let visible =
        store.list_records(&ListOptions { record_type: Some(record_type), ..ListOptions::default() }).unwrap();
    assert!(visible.iter().all(|record| record.record_id != id), "{label}: a deleted record is listed");
    let refused = store.apply(Changes {
        corrections: vec![Correction {
            record_id: id.clone(),
            expected_revision: 2,
            data: corrected_data.clone(),
            extensions: ExtensionsUpdate::Keep,
            changed_by: RecordActor::Athlete,
            became_known_at: Some(DELETED_AT.into()),
            became_known_at_basis: BecameKnownAtBasis::LocalWrite,
            stale_style: StaleStyle::Revision,
        }],
        ..Changes::default()
    });
    assert_eq!(refused.unwrap_err().to_string(), format!("Deleted record cannot be corrected: {id}"));
    let again = store.apply(Changes {
        deletions: vec![Deletion {
            record_id: id.clone(),
            expected_revision: 2,
            deleted_by: RecordActor::Athlete,
            deleted_at: Some(DELETED_AT.into()),
        }],
        ..Changes::default()
    });
    assert_eq!(again.unwrap_err().to_string(), format!("Record is already deleted: {id}"));

    // The deletion survives another export and restore, and merges into a
    // database that has only revision 1.
    let deleted_package = directory.join("deleted");
    export(&store, &deleted_package);
    store.close().unwrap();
    let restored_deleted = restored(&deleted_package, &directory, "restored-deleted");
    assert!(open_read_only(&restored_deleted).get_record(&id).unwrap().unwrap().is_deleted, "{label}");
    assert_same_restore(&format!("{label} deleted restore"), &source, &restored_deleted);
    let merged_deleted = restored(&first_package, &directory, "merged-deleted");
    merge_changed(&merged_deleted, &deleted_package, &id, needed.len());
    assert_same_records(&format!("{label} deleted merge"), &source, &merged_deleted);

    // Erase: the deleted record here, and the live corrected record in the
    // merged copy. Both leave the same value-free markers.
    let erased = erase_and_check(&source, &id, 2);
    let erased_live = erase_and_check(&merged, &id, 2);
    assert_eq!(erased, erased_live, "{label}");
    assert_same_records(&format!("{label} erasure"), &source, &merged);
    let erased_package = directory.join("erased");
    export(&open_read_only(&source), &erased_package);
    assert_package_holds_only_markers(&erased_package, &erased);
    let restored_erased = restored(&erased_package, &directory, "restored-erased");
    assert_same_restore(&format!("{label} erased restore"), &source, &restored_erased);

    for (name, path) in [
        ("source", &source),
        ("restored corrected", &restored_corrected),
        ("merged", &merged),
        ("restored deleted", &restored_deleted),
        ("merged deleted", &merged_deleted),
        ("restored erased", &restored_erased),
    ] {
        assert_checks_clean(&format!("{label} {name}"), path);
    }
    remove_dir(&directory);
}

#[test]
fn every_record_type_lives_through_correction_export_restore_merge_deletion_and_erasure() {
    assert_eq!(RecordType::ALL.len(), 24, "the model lists 24 record types");
    let creates = demo_creates();
    let demo_types: BTreeSet<&str> = creates.iter().map(|create| create["record_type"].as_str().unwrap()).collect();
    let model_types: BTreeSet<&str> = RecordType::ALL.iter().map(|record_type| record_type.as_str()).collect();
    assert_eq!(demo_types, model_types, "the demo history must hold every record type");
    for record_type in WITHOUT_CHILDREN {
        assert!(RecordType::ALL.contains(record_type));
    }
    for record_type in RecordType::ALL {
        lifecycle(*record_type, &creates);
    }
}

// ---- History that other records cite ------------------------------------------

const RESULT_ID: &str = "01890f3e-e7d3-7cc0-88c8-00000000000b";
const LINK_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000011";
const PREDICTION_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000010";
const ANALYSIS_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000013";

fn correction(store: &Store, record_id: &str, edit: impl FnOnce(&mut Value)) -> Correction {
    let current = store.get_record(record_id).unwrap().unwrap();
    let mut data = current.data.to_value();
    edit(&mut data);
    Correction {
        record_id: record_id.into(),
        expected_revision: current.current_revision,
        data: decode(current.record_type(), &data),
        extensions: ExtensionsUpdate::Keep,
        changed_by: RecordActor::Athlete,
        became_known_at: Some(CORRECTED_AT.into()),
        became_known_at_basis: BecameKnownAtBasis::LocalWrite,
        stale_style: StaleStyle::Revision,
    }
}

fn reference(data: &RecordData, key: &str) -> (String, u64) {
    let value = data.to_value();
    (value[key]["record_id"].as_str().unwrap().to_string(), value[key]["revision"].as_u64().unwrap())
}

/// A grade names the exact link revision it graded, and a link names exact
/// prediction and result revisions (D99). After the result, the link, and the
/// grade are corrected, the first grade revision still names the first link
/// revision, which still names the first result revision, and each of them is
/// retained. Evidence that cites an earlier revision keeps resolving.
#[test]
fn grades_and_evidence_keep_the_revisions_they_name_after_later_corrections() {
    let directory = temporary_root::new_dir("ath-lifecycle-history-");
    let path = demo_support::write(&directory);
    let mut store = open(&path);

    // The result is corrected and the link moves to its second revision.
    let result = correction(&store, RESULT_ID, |data| data["score"]["duration_s"] = json!(310));
    let link = correction(&store, LINK_ID, |data| {
        data["benchmark_result"]["revision"] = json!(2);
        data["content_version"] = json!(2);
    });
    store.apply(Changes { corrections: vec![result, link], ..Changes::default() }).unwrap();

    // A grade that names the new link but keeps the old arithmetic is refused.
    let stale = correction(&store, GRADE_ID, |data| data["prediction_result_link"]["revision"] = json!(2));
    let refused = store.apply(Changes { corrections: vec![stale], ..Changes::default() }).unwrap_err();
    assert_eq!(refused.to_string(), "Saved grade actual score does not match the linked result revision");

    // The grade's second revision grades the new link: 280 s predicted, 310 s actual.
    let regrade = correction(&store, GRADE_ID, |data| {
        data["prediction_result_link"]["revision"] = json!(2);
        data["actual_score"] = json!({ "duration_s": 310 });
        data["signed_error"] = json!(-30);
        data["absolute_error_percentage"] = json!(9.7);
        data["authorship"]["content_version"] = json!(2);
        data["authorship"]["evidence"] = json!([{ "record_id": LINK_ID, "revision": 2 }]);
    });
    store.apply(Changes { corrections: vec![regrade], ..Changes::default() }).unwrap();

    let check_history = |store: &Store, label: &str| {
        let grade = store.revisions(GRADE_ID).unwrap();
        assert_eq!(grade.len(), 2, "{label}");
        assert_eq!(reference(&grade[0].data, "prediction_result_link"), (LINK_ID.into(), 1), "{label}");
        assert_eq!(reference(&grade[1].data, "prediction_result_link"), (LINK_ID.into(), 2), "{label}");
        assert_eq!(grade[0].data.to_value()["actual_score"]["duration_s"].as_f64(), Some(300.0), "{label}");
        let first_link = store.revision(LINK_ID, 1).unwrap().expect("link revision 1 is retained");
        assert_eq!(reference(&first_link.data, "benchmark_result"), (RESULT_ID.into(), 1), "{label}");
        assert_eq!(reference(&first_link.data, "prediction"), (PREDICTION_ID.into(), 1), "{label}");
        let first_result = store.revision(RESULT_ID, 1).unwrap().expect("result revision 1 is retained");
        assert_eq!(first_result.data.to_value()["score"]["duration_s"].as_f64(), Some(300.0), "{label}");
        // The miss analysis still cites the first grade revision.
        let analysis = store.get_record(ANALYSIS_ID).unwrap().unwrap();
        assert_eq!(reference(&analysis.data, "prediction_grade"), (GRADE_ID.into(), 1), "{label}");
        // The prediction cites the point measurement's first revision. The
        // demo corrected that measurement from 62 ms to 64 ms.
        let prediction = store.get_record(PREDICTION_ID).unwrap().unwrap();
        let evidence = &prediction.data.to_value()["authorship"]["evidence"][0];
        assert_eq!(evidence, &json!({ "record_id": POINT_ID, "revision": 1 }), "{label}");
        assert_eq!(store.get_record(POINT_ID).unwrap().unwrap().current_revision, 2, "{label}");
        let cited = store.revision(POINT_ID, 1).unwrap().expect("point revision 1 is retained");
        assert_eq!(cited.data.to_value()["value"]["standard_value"].as_f64(), Some(62.0), "{label}");
    };
    check_history(&store, "source");
    let package = directory.join("package");
    export(&store, &package);
    store.close().unwrap();
    assert_checks_clean("source", &path);

    let restored_path = restored(&package, &directory, "restored");
    assert_same_restore("restore", &path, &restored_path);
    check_history(&open_read_only(&restored_path), "restored");
    assert_checks_clean("restored", &restored_path);
    remove_dir(&directory);
}
