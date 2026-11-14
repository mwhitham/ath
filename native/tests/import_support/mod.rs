//! Helpers shared by the import tests.
//!
//! `snapshot` and `Labels` follow `native/tools/reference/import-golden.ts`
//! exactly, so a native database can be compared with the 2.1.0 golden files.

#![allow(dead_code)]

pub mod baseline;

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use athleticstandard::json::{self, Map, Value};
use athleticstandard::store::{CreateOptions, Store};
use rusqlite::Connection;

pub const CREATED_AT: &str = "2026-10-13T11:00:00.000Z";

pub fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

pub fn whoop_fixture() -> PathBuf {
    repository_root().join("tests/fixtures/exports/whoop")
}

pub fn golden(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/import").join(name);
    json::parse(&std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))).unwrap()
}

/// A private temporary directory, removed when dropped.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new() -> TempDir {
        TempDir(athleticstandard::files::make_temporary_directory("ath-import-test-").unwrap())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn new_store(directory: &Path) -> Store {
    Store::create(
        directory.join("athlete.ath.sqlite"),
        CreateOptions { database_id: None, created_at: Some(CREATED_AT.into()) },
    )
    .unwrap()
}

/// The fixture files by name, sorted.
pub fn fixture_files() -> Vec<(String, String)> {
    let mut names: Vec<String> = std::fs::read_dir(whoop_fixture())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let text = std::fs::read_to_string(whoop_fixture().join(&name)).unwrap();
            (name, text)
        })
        .collect()
}

pub fn write_directory(path: &Path, files: &[(String, String)]) {
    for (name, text) in files {
        let target = path.join(name);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, text).unwrap();
    }
}

pub fn write_zip(path: &Path, files: &[(String, String)]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, text) in files {
        zip.start_file(name.as_str(), zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

/// Build a golden step's input under `directory` and return its path.
pub fn materialize(input: &Value, directory: &Path) -> PathBuf {
    let files = |value: &Value| -> Vec<(String, String)> {
        value
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, text)| (name.clone(), text.as_str().unwrap().to_string()))
            .collect()
    };
    match input["kind"].as_str().unwrap() {
        "file" => {
            let path = directory.join(input["name"].as_str().unwrap());
            std::fs::write(&path, input["text"].as_str().unwrap()).unwrap();
            path
        }
        "directory" => {
            let path = directory.join("input");
            write_directory(&path, &files(&input["files"]));
            path
        }
        "zip" => {
            let path = directory.join("input.zip");
            write_zip(&path, &files(&input["files"]));
            path
        }
        other => panic!("unknown input kind {other}"),
    }
}

/// Replace every lowercase UUID in text, as `text.replace(UUID, ...)` does.
pub fn replace_uuids(text: &str, mut replace: impl FnMut(&str) -> String) -> String {
    fn is_uuid(bytes: &[u8]) -> bool {
        bytes.len() == 36
            && bytes.iter().enumerate().all(|(index, byte)| match index {
                8 | 13 | 18 | 23 => *byte == b'-',
                _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(byte),
            })
    }
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut index = 0;
    while index < bytes.len() {
        if index + 36 <= bytes.len() && is_uuid(&bytes[index..index + 36]) {
            out.push_str(&replace(&text[index..index + 36]));
            index += 36;
        } else {
            let character = text[index..].chars().next().unwrap();
            out.push(character);
            index += character.len_utf8();
        }
    }
    out
}

/// Labels for record IDs, from each record's type and match key.
#[derive(Default)]
pub struct Labels {
    labels: HashMap<String, String>,
}

pub struct KeyFact {
    pub record_id: String,
    pub record_type: String,
    pub method: String,
    pub key: String,
}

impl Labels {
    pub fn learn(&mut self, entries: Vec<KeyFact>) {
        let mut pending: Vec<KeyFact> =
            entries.into_iter().filter(|entry| !self.labels.contains_key(&entry.record_id)).collect();
        for _ in 0..3 {
            if pending.is_empty() {
                break;
            }
            let mut index = pending.len();
            while index > 0 {
                index -= 1;
                let mut known = true;
                replace_uuids(&pending[index].key, |id| {
                    known &= self.labels.contains_key(id);
                    String::new()
                });
                if !known {
                    continue;
                }
                let entry = pending.remove(index);
                let label = format!("{} {} {}", entry.record_type, entry.method, self.text(&entry.key));
                self.labels.insert(entry.record_id, label);
            }
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

fn text_column(connection: &Connection, sql: &str) -> Vec<Vec<rusqlite::types::Value>> {
    let mut statement = connection.prepare(sql).unwrap();
    let columns = statement.column_count();
    statement
        .query_map([], |row| (0..columns).map(|index| row.get(index)).collect::<rusqlite::Result<Vec<_>>>())
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn as_text(value: &rusqlite::types::Value) -> String {
    match value {
        rusqlite::types::Value::Text(text) => text.clone(),
        other => panic!("expected text, got {other:?}"),
    }
}

fn as_int(value: &rusqlite::types::Value) -> i64 {
    match value {
        rusqlite::types::Value::Integer(number) => *number,
        other => panic!("expected integer, got {other:?}"),
    }
}

/// The database as the golden files describe it.
pub fn snapshot(connection: &Connection, labels: &mut Labels) -> Value {
    let keys = text_column(
        connection,
        "SELECT record_import_keys.record_id, records.record_type, match_key_method, match_key, imported_at
           FROM record_import_keys JOIN records USING (record_id)",
    );
    labels.learn(
        keys.iter()
            .map(|row| KeyFact {
                record_id: as_text(&row[0]),
                record_type: as_text(&row[1]),
                method: as_text(&row[2]),
                key: as_text(&row[3]),
            })
            .collect(),
    );
    let revision = as_int(&text_column(connection, "SELECT current_revision FROM database_state")[0][0]);
    let records =
        text_column(connection, "SELECT record_id, record_type, current_revision, created_at, is_deleted FROM records");
    let revisions = text_column(
        connection,
        "SELECT record_id, revision, previous_revision, became_known_at, became_known_at_basis, changed_by_json,
                record_json, extensions_json
           FROM record_revisions ORDER BY record_id, revision",
    );
    let deletions = text_column(connection, "SELECT record_id, record_revision, deleted_at FROM record_deletions");
    let mut out: Vec<(String, Value)> = records
        .iter()
        .map(|row| {
            let id = as_text(&row[0]);
            let label = labels.text(&id);
            let mut record = Map::new();
            record.insert("label".into(), Value::String(label.clone()));
            record.insert("record_type".into(), Value::String(as_text(&row[1])));
            record.insert("created_at".into(), Value::String(as_text(&row[3])));
            record.insert("current_revision".into(), Value::from(as_int(&row[2])));
            record.insert("is_deleted".into(), Value::Bool(as_int(&row[4]) == 1));
            let revision_values: Vec<Value> = revisions
                .iter()
                .filter(|revision| as_text(&revision[0]) == id)
                .map(|revision| {
                    let mut changed_by = json::parse(&as_text(&revision[5])).unwrap();
                    if changed_by["author_type"] == "tool" {
                        changed_by["tool_version"] = Value::String("<ath version>".into());
                    }
                    serde_json::json!({
                        "revision": as_int(&revision[1]),
                        "previous_revision": match &revision[2] {
                            rusqlite::types::Value::Null => Value::Null,
                            value => Value::from(as_int(value)),
                        },
                        "became_known_at": as_text(&revision[3]),
                        "became_known_at_basis": as_text(&revision[4]),
                        "changed_by": changed_by,
                        "record_json": labels.text(&as_text(&revision[6])),
                        "extensions_json": match &revision[7] {
                            rusqlite::types::Value::Null => Value::Null,
                            value => Value::String(as_text(value)),
                        },
                    })
                })
                .collect();
            record.insert("revisions".into(), Value::Array(revision_values));
            let key_values: Vec<Value> = keys
                .iter()
                .filter(|key| as_text(&key[0]) == id)
                .map(|key| {
                    serde_json::json!({
                        "method": as_text(&key[2]),
                        "key": labels.text(&as_text(&key[3])),
                        "imported_at": as_text(&key[4]),
                    })
                })
                .collect();
            record.insert("import_keys".into(), Value::Array(key_values));
            let deletion_values: Vec<Value> = deletions
                .iter()
                .filter(|deletion| as_text(&deletion[0]) == id)
                .map(|deletion| {
                    serde_json::json!({ "record_revision": as_int(&deletion[1]), "deleted_at": as_text(&deletion[2]) })
                })
                .collect();
            record.insert("deletions".into(), Value::Array(deletion_values));
            (label, Value::Object(record))
        })
        .collect();
    out.sort_by(|left, right| json::compare_utf16(&left.0, &right.0));
    serde_json::json!({
        "database_revision": revision,
        "records": out.into_iter().map(|(_, record)| record).collect::<Vec<_>>(),
    })
}

/// Compare two JSON values as the golden files print them, with a readable diff.
pub fn assert_same_json(actual: &Value, expected: &Value, context: &str) {
    let actual = json::to_deterministic_pretty(actual);
    let expected = json::to_deterministic_pretty(expected);
    if actual != expected {
        let line = actual
            .lines()
            .zip(expected.lines())
            .position(|(left, right)| left != right)
            .unwrap_or_else(|| actual.lines().count().min(expected.lines().count()));
        let around = |text: &str| text.lines().skip(line.saturating_sub(3)).take(8).collect::<Vec<_>>().join("\n");
        panic!(
            "{context}: native output differs from 2.1.0 at line {}\n--- native\n{}\n--- 2.1.0\n{}",
            line + 1,
            around(&actual),
            around(&expected)
        );
    }
}
