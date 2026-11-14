//! Replay the 2.1.0 helper's keyed-route scenarios: Garmin, Strava, the
//! Garmin pull, and mapped imports.
//!
//! The golden files hold what the 2.1.0 helper printed and stored. The native
//! result is compared with these field-specific normalizations, each a
//! deliberate change listed in `native/reference/deliberate-changes.md`:
//!
//! - Times: the helper read the clock, so the golden files name each time by
//!   the step that wrote it (`<step N>`). The native steps use fixed times,
//!   which are named the same way.
//! - Record JSON is compared parsed, with keys sorted. The helper wrote JSON
//!   with Rust's formatting and its own field order; the native store writes
//!   every record through one encoder.
//! - `changed_by.tool` is `ath`: the helper named itself `ath-core`.
//! - A revision the helper wrote is a provider import, not a local write
//!   (change 55 in `native/reference/deliberate-changes.md`).
//! - The database revision advances once per import that writes, not once per
//!   record, so only whether it advanced is compared.
//! - On the Garmin, Strava, and pull routes, 2.1.0 wrote a revision on every
//!   repeated import that differed from the stored record only in
//!   `received_at`. Those revisions are dropped from the 2.1.0 history before
//!   the comparison, and the rest are numbered again.
//! - A record that fails the 2.1.0 store's checks now fails the preview. When
//!   the 2.1.0 preview printed a result and its write failed with a message,
//!   the native preview must fail with that message and write nothing.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;

use athleticstandard::import::{self, mapping, ImportOptions, ProviderPreview, SharedImport};
use athleticstandard::json::{self, Value};
use athleticstandard::model::RecordActor;
use athleticstandard::runtime::garmin_pull;
use athleticstandard::store::{Changes, Deletion, Store};
use rusqlite::Connection;

use super::{golden, materialize};
use crate::import_support::{assert_same_json, new_store, replace_uuids, TempDir};

/// The fixed import time of a step, which the comparison names `<step N>`.
pub fn step_time(index: usize) -> String {
    format!("2026-10-{:02}T12:00:00.000Z", 13 + index)
}

/// Labels for record IDs: by import key, and a source by writer, route, and sensor.
#[derive(Default)]
pub struct KeyedLabels {
    labels: HashMap<String, String>,
}

impl KeyedLabels {
    pub fn learn(&mut self, connection: &Connection) {
        let mut statement = connection
            .prepare(
                "SELECT record_import_keys.record_id, records.record_type, match_key_method, match_key
                   FROM record_import_keys JOIN records USING (record_id)",
            )
            .unwrap();
        let mut pending: Vec<(String, String, String, String)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .filter(|(id, _, _, _)| !self.labels.contains_key(id))
            .collect();
        for _ in 0..3 {
            let mut index = pending.len();
            while index > 0 {
                index -= 1;
                let mut known = true;
                replace_uuids(&pending[index].3, |id| {
                    known &= self.labels.contains_key(id);
                    String::new()
                });
                if !known {
                    continue;
                }
                let (id, record_type, method, key) = pending.remove(index);
                let label = format!("{record_type} {method} {}", self.text(&key));
                self.labels.insert(id, label);
            }
        }
        let mut statement =
            connection.prepare("SELECT record_id, data_writer, data_route, sensor_type FROM sources").unwrap();
        let sources: Vec<(String, String, String, Option<String>)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for (id, writer, route, sensor) in sources {
            self.labels.entry(id).or_insert_with(|| format!("source {writer} {route} {}", sensor.unwrap_or_default()));
        }
    }

    pub fn text(&self, value: &str) -> String {
        replace_uuids(value, |id| match self.labels.get(id) {
            Some(label) => format!("<{label}>"),
            None => "<unlabeled>".to_string(),
        })
    }

    pub fn value(&self, value: &Value) -> Value {
        match value {
            Value::String(text) => Value::String(self.text(text)),
            Value::Array(items) => Value::Array(items.iter().map(|item| self.value(item)).collect()),
            Value::Object(map) => {
                Value::Object(map.iter().map(|(key, child)| (key.clone(), self.value(child))).collect())
            }
            other => other.clone(),
        }
    }
}

/// Native step times named as the golden files name them.
struct Times(Vec<String>);

impl Times {
    fn text(&self, value: &str) -> String {
        match self.0.iter().position(|time| time == value) {
            Some(step) => format!("<step {step}>"),
            None => value.to_string(),
        }
    }

    fn value(&self, value: &Value) -> Value {
        match value {
            Value::String(text) => Value::String(self.text(text)),
            Value::Array(items) => Value::Array(items.iter().map(|item| self.value(item)).collect()),
            Value::Object(map) => {
                Value::Object(map.iter().map(|(key, child)| (key.clone(), self.value(child))).collect())
            }
            other => other.clone(),
        }
    }
}

fn rows(connection: &Connection, sql: &str) -> Vec<Vec<rusqlite::types::Value>> {
    let mut statement = connection.prepare(sql).unwrap();
    let columns = statement.column_count();
    statement
        .query_map([], |row| (0..columns).map(|index| row.get(index)).collect::<rusqlite::Result<Vec<_>>>())
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn text(value: &rusqlite::types::Value) -> String {
    match value {
        rusqlite::types::Value::Text(text) => text.clone(),
        other => panic!("expected text, got {other:?}"),
    }
}

fn int(value: &rusqlite::types::Value) -> i64 {
    match value {
        rusqlite::types::Value::Integer(number) => *number,
        other => panic!("expected integer, got {other:?}"),
    }
}

/// The database as the keyed golden files describe it.
fn snapshot(connection: &Connection, labels: &mut KeyedLabels, times: &Times) -> Value {
    labels.learn(connection);
    let keys = rows(connection, "SELECT record_id, match_key_method, match_key, imported_at FROM record_import_keys");
    let revision = int(&rows(connection, "SELECT current_revision FROM database_state")[0][0]);
    let records =
        rows(connection, "SELECT record_id, record_type, current_revision, created_at, is_deleted FROM records");
    let revisions = rows(
        connection,
        "SELECT record_id, revision, previous_revision, became_known_at, became_known_at_basis, changed_by_json,
                record_json, extensions_json
           FROM record_revisions ORDER BY record_id, revision",
    );
    let deletions = rows(connection, "SELECT record_id, record_revision, deleted_at FROM record_deletions");
    let mut out: Vec<(String, Value)> = records
        .iter()
        .map(|row| {
            let id = text(&row[0]);
            let label = labels.text(&id);
            let revision_values: Vec<Value> = revisions
                .iter()
                .filter(|revision| text(&revision[0]) == id)
                .map(|revision| {
                    let mut changed_by = json::parse(&text(&revision[5])).unwrap();
                    if changed_by["author_type"] == "tool" {
                        changed_by["tool_version"] = Value::String("<ath version>".into());
                    }
                    let record = times.value(&json::parse(&labels.text(&text(&revision[6]))).unwrap());
                    serde_json::json!({
                        "revision": int(&revision[1]),
                        "previous_revision": match &revision[2] {
                            rusqlite::types::Value::Null => Value::Null,
                            value => Value::from(int(value)),
                        },
                        "became_known_at": times.text(&text(&revision[3])),
                        "became_known_at_basis": text(&revision[4]),
                        "changed_by": changed_by,
                        "record": record,
                        "extensions_json": match &revision[7] {
                            rusqlite::types::Value::Null => Value::Null,
                            value => Value::String(text(value)),
                        },
                    })
                })
                .collect();
            let key_values: Vec<Value> = keys
                .iter()
                .filter(|key| text(&key[0]) == id)
                .map(|key| {
                    serde_json::json!({
                        "method": text(&key[1]),
                        "key": labels.text(&text(&key[2])),
                        "imported_at": times.text(&text(&key[3])),
                    })
                })
                .collect();
            let deletion_values: Vec<Value> = deletions
                .iter()
                .filter(|deletion| text(&deletion[0]) == id)
                .map(|deletion| {
                    serde_json::json!({ "record_revision": int(&deletion[1]), "deleted_at": times.text(&text(&deletion[2])) })
                })
                .collect();
            let record = serde_json::json!({
                "label": label,
                "record_type": text(&row[1]),
                "created_at": times.text(&text(&row[3])),
                "current_revision": int(&row[2]),
                "is_deleted": int(&row[4]) == 1,
                "revisions": revision_values,
                "import_keys": key_values,
                "deletions": deletion_values,
            });
            (label, record)
        })
        .collect();
    out.sort_by(|left, right| json::compare_utf16(&left.0, &right.0));
    serde_json::json!({
        "database_revision": revision,
        "records": out.into_iter().map(|(_, record)| record).collect::<Vec<_>>(),
    })
}

/// The 2.1.0 history without revisions that changed only `received_at`.
fn without_clock_revisions(database: &Value) -> Value {
    let mut database = database.clone();
    for record in database["records"].as_array_mut().unwrap() {
        let mut kept: Vec<Value> = Vec::new();
        for revision in record["revisions"].as_array().unwrap() {
            let same_as_last = kept.last().is_some_and(|last: &Value| {
                let mut previous = last["record"].clone();
                previous["received_at"] = revision["record"]["received_at"].clone();
                json::to_sorted_string(&previous) == json::to_sorted_string(&revision["record"])
            });
            if !same_as_last {
                kept.push(revision.clone());
            }
        }
        for (index, revision) in kept.iter_mut().enumerate() {
            revision["revision"] = Value::from(index as u64 + 1);
            revision["previous_revision"] = if index == 0 { Value::Null } else { Value::from(index as u64) };
        }
        record["current_revision"] = Value::from(kept.len() as u64);
        record["revisions"] = Value::Array(kept);
    }
    database
}

/// The helper named itself `ath-core` in `changed_by`; the native program is `ath`.
fn native_tool_name(database: &Value) -> Value {
    let mut database = database.clone();
    for record in database["records"].as_array_mut().unwrap() {
        for revision in record["revisions"].as_array_mut().unwrap() {
            if revision["changed_by"]["tool"] == "ath-core" {
                revision["changed_by"]["tool"] = Value::String("ath".into());
                if revision["became_known_at_basis"] == "local_write" {
                    revision["became_known_at_basis"] = Value::String("provider_import".into());
                }
            }
        }
    }
    database.as_object_mut().unwrap().remove("database_revision");
    database
}

fn records_only(database: &Value) -> Value {
    let mut database = database.clone();
    database.as_object_mut().unwrap().remove("database_revision");
    database
}

/// Delete the first record whose label starts with `prefix`, at `deleted_at`.
fn delete_by_label(store: &mut Store, labels: &mut KeyedLabels, prefix: &str, deleted_at: &str) {
    labels.learn(store.connection());
    let found: Vec<(String, u64)> = rows(store.connection(), "SELECT record_id, current_revision FROM records")
        .iter()
        .map(|row| (text(&row[0]), int(&row[1]) as u64))
        .filter(|(id, _)| labels.text(id).starts_with(prefix))
        .collect();
    let (record_id, expected_revision) =
        found.first().cloned().unwrap_or_else(|| panic!("no record to delete: {prefix}"));
    store
        .apply(Changes {
            deletions: vec![Deletion {
                record_id,
                expected_revision,
                deleted_by: RecordActor::Athlete,
                deleted_at: Some(deleted_at.to_string()),
            }],
            ..Changes::default()
        })
        .unwrap();
}

/// A local server for the Garmin personal connection's three requests.
pub struct PersonalServer {
    pub base: String,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    address: std::net::SocketAddr,
}

impl PersonalServer {
    pub fn start(date: &str, activities: Value, hrv: Option<Value>) -> PersonalServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = stop.clone();
        let hrv_path = format!("/hrv-service/hrv/{date}");
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if flag.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let mut parts = request_line.split_whitespace();
                let method = parts.next().unwrap_or_default();
                let path = parts.next().unwrap_or_default();
                let (status, text) = if method == "POST" && path.ends_with("/personal-api-key/token") {
                    (200, "aaa.bbb.ccc".to_string())
                } else if path.starts_with("/activitylist-service/activities/search/activities") {
                    (200, json::to_string(&activities))
                } else if let (true, Some(hrv)) = (path == hrv_path, hrv.as_ref()) {
                    (200, json::to_string(hrv))
                } else {
                    (404, "not found".to_string())
                };
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                    text.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        PersonalServer { base: format!("http://{address}"), stop, address }
    }
}

impl Drop for PersonalServer {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = std::net::TcpStream::connect(self.address);
    }
}

pub const PERSONAL_KEY: &str = "invented-personal-key";

/// Run one golden step's import to a preview.
pub fn preview_step(
    store: &Store,
    step: &Value,
    directory: &Path,
    received_at: &str,
) -> athleticstandard::Result<ProviderPreview> {
    let options = ImportOptions { received_at: Some(received_at.to_string()) };
    match step["command"].as_str().unwrap() {
        "import-garmin" => {
            import::preview_shared(store, &materialize(&step["input"], directory), SharedImport::Garmin, &options)
        }
        "import-strava" => {
            import::preview_shared(store, &materialize(&step["input"], directory), SharedImport::Strava, &options)
        }
        "import-mapped" => {
            let mapping = mapping::load_mapping(step["mapping"]["text"].as_str().unwrap())?;
            import::preview_mapped(store, &materialize(&step["input"], directory), &mapping, &options)
        }
        "pull-garmin" => {
            let pull = &step["pull"];
            let date = pull["date"].as_str().unwrap();
            let hrv = (!pull["hrv"].is_null()).then(|| pull["hrv"].clone());
            let server = PersonalServer::start(date, pull["activities"].clone(), hrv);
            let day = garmin_pull::fetch_day(&server.base, PERSONAL_KEY, date)?;
            import::preview_pull(store, day, &options)
        }
        other => panic!("unknown command {other}"),
    }
}

/// What a keyed scenario's runner does at one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// The same outputs and records as 2.1.0, after the normalizations above.
    Same,
    /// The native preview stops with the error the 2.1.0 write printed.
    PreviewRefusesTheWrite,
}

/// Replay a keyed scenario and require the 2.1.0 results.
pub fn run_keyed_scenario(name: &str) {
    let golden = golden(&format!("{name}.json"));
    let directory = TempDir::new();
    let mut store = new_store(directory.path());
    let mut labels = KeyedLabels::default();
    let steps = golden["steps"].as_array().unwrap();
    let times = Times((0..steps.len()).map(step_time).collect());
    let file_route = steps.iter().any(|step| step["command"] != "import-mapped");
    let mut native_revision = store.database_revision().unwrap();
    for (index, step) in steps.iter().enumerate() {
        let context = format!("{name} step {index}");
        let step_directory = directory.path().join(format!("step-{index}"));
        std::fs::create_dir(&step_directory).unwrap();
        let received_at = step_time(index);
        if let Value::Object(delete) = &step["delete_before"] {
            delete_by_label(&mut store, &mut labels, delete["label_prefix"].as_str().unwrap(), &received_at);
            native_revision += 1;
        }
        let before = records_only(&snapshot(store.connection(), &mut labels, &times));
        let expect = match (&step["preview"], &step["write"]) {
            (Value::Object(preview), Value::Object(write))
                if preview.contains_key("result") && write.contains_key("error") =>
            {
                Expect::PreviewRefusesTheWrite
            }
            _ => Expect::Same,
        };
        let preview = match preview_step(&store, step, &step_directory, &received_at) {
            Ok(preview) => preview,
            Err(error) => {
                let expected = match expect {
                    Expect::PreviewRefusesTheWrite => step["write"]["error"].as_str(),
                    Expect::Same => step["preview"]["error"].as_str().or(step["write"]["error"].as_str()),
                };
                assert_eq!(Some(error.to_string().as_str()), expected, "{context}: {error}");
                let after = records_only(&snapshot(store.connection(), &mut labels, &times));
                assert_same_json(&after, &before, &format!("{context}: a refused import writes nothing"));
                continue;
            }
        };
        assert_eq!(expect, Expect::Same, "{context}: the native preview should fail");
        if let Value::Object(golden_preview) = &step["preview"] {
            assert!(golden_preview.contains_key("result"), "{context}: 2.1.0 refused the preview: {}", step["preview"]);
            let output = labels.value(&preview.report_value(false).unwrap());
            assert_same_json(&output, &step["preview"]["result"], &format!("{context} preview"));
            let serialized = json::to_value(&preview);
            assert_eq!(serialized["write_confirmed"], Value::Bool(false), "{context}");
        }
        let result = preview.apply(&mut store).unwrap_or_else(|error| panic!("{context}: {error}"));
        labels.learn(store.connection());
        let written = labels.value(&preview.report_value(true).unwrap());
        assert_same_json(&written, &step["write"]["result"], &format!("{context} write"));

        let database = native_tool_name(&snapshot(store.connection(), &mut labels, &times));
        let mut expected = native_tool_name(&step["database"]);
        if file_route {
            expected = without_clock_revisions(&expected);
        }
        assert_same_json(&canonical(&database), &canonical(&expected), &format!("{context} database"));

        // The revision advances once when the import wrote something, as 2.1.0
        // wrote something once its clock-only revisions are set aside.
        let wrote = canonical(&expected) != canonical(&native_tool_name(&with_revision(&before)));
        assert_eq!(
            result.database_revision > native_revision,
            wrote,
            "{context}: the database revision advances when the import writes"
        );
        assert!(result.database_revision <= native_revision + 1, "{context}: one import advances the revision once");
        native_revision = result.database_revision;
    }
}

fn with_revision(records: &Value) -> Value {
    let mut database = records.clone();
    database["database_revision"] = Value::from(0);
    database
}

fn canonical(value: &Value) -> Value {
    json::parse(&json::to_sorted_string(value)).unwrap()
}
