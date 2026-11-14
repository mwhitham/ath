//! Golden scenarios: open, replay, run each case, and compare.

use std::path::{Path, PathBuf};

use athleticstandard::json::{self, Map, Value};
use athleticstandard::store::{Changes, Correction, CreateOptions, Deletion, ExtensionsUpdate, NewRecord, Store};

use super::{golden, new_store, normalize, replay, scenario_ids, temporary_dir};

/// The deterministic 2.1.0 demo database.
pub fn demo_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/demo-athlete/athlete.ath.sqlite")
}

/// Replay the demo database's complete history into a new native database:
/// every revision in write order, then every deletion. The database revision
/// is then set to the demo's, so later writes report the same numbers.
pub fn replay_demo() -> (PathBuf, Store) {
    use rusqlite::{Connection, OpenFlags};
    let demo = Connection::open_with_flags(demo_path(), OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let metadata = athleticstandard::store::read_metadata(&demo).unwrap();
    let directory = temporary_dir("ath-demo-");
    let path = directory.join("athlete.ath.sqlite");
    let mut store = Store::create(
        &path,
        CreateOptions {
            database_id: Some(metadata.database_id.clone()),
            created_at: Some(metadata.created_at.clone()),
        },
    )
    .unwrap();
    let columns = athleticstandard::store::read::REVISION_COLUMNS
        .split(',')
        .map(|column| format!("record_revisions.{}", column.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut statement = demo
        .prepare(&format!(
            "SELECT records.created_at, {columns} FROM record_revisions JOIN records USING (record_id) ORDER BY record_revisions.rowid"
        ))
        .unwrap();
    let mut rows = statement.query([]).unwrap();
    let mut revisions = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        let created_at: String = row.get(0).unwrap();
        revisions.push((created_at, athleticstandard::store::read::revision_from_row(row).unwrap()));
    }
    for (created_at, revision) in revisions {
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
                stale_style: athleticstandard::store::write::StaleStyle::Revision,
            });
        }
        store.apply(changes).unwrap();
    }
    let mut deletions =
        demo.prepare("SELECT record_id, record_revision, deleted_at, deleted_by_json FROM record_deletions").unwrap();
    let deletions: Vec<(String, i64, String, String)> = deletions
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for (record_id, revision, deleted_at, deleted_by) in deletions {
        store
            .apply(Changes {
                deletions: vec![Deletion {
                    record_id,
                    expected_revision: revision as u64,
                    deleted_by: serde_json::from_str(&deleted_by).unwrap(),
                    deleted_at: Some(deleted_at),
                }],
                ..Changes::default()
            })
            .unwrap();
    }
    let revision: i64 = demo
        .query_row("SELECT current_revision FROM database_state WHERE singleton_id = 1", [], |row| row.get(0))
        .unwrap();
    store
        .transaction(|tx| {
            tx.execute("UPDATE database_state SET current_revision = ?1 WHERE singleton_id = 1", [revision])?;
            Ok(())
        })
        .unwrap();
    (directory, store)
}

/// Every record ID in a database.
pub fn database_ids(store: &Store) -> Vec<String> {
    let mut statement = store.connection().prepare("SELECT record_id FROM records").unwrap();
    statement.query_map([], |row| row.get::<_, String>(0)).unwrap().map(Result::unwrap).collect()
}

/// How a case's output is compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compare {
    /// The same JSON text after normalization.
    Exact,
    /// The same JSON text after normalization and sorting object keys.
    SortedKeys,
}

/// A golden scenario opened against a new native database.
pub struct Scenario {
    pub directory: PathBuf,
    pub store: Store,
    pub known_ids: Vec<String>,
    pub cases: Vec<Value>,
}

impl Scenario {
    pub fn open(relative: &str) -> Scenario {
        let file = golden(relative);
        let steps = file["steps"].as_array().cloned().unwrap_or_default();
        let (directory, mut store) = if file.get("base").and_then(Value::as_str) == Some("demo") {
            replay_demo()
        } else {
            new_store(file["created_at"].as_str().unwrap())
        };
        replay(&mut store, &steps);
        let cases = file["cases"].as_array().cloned().unwrap();
        let mut known_ids = scenario_ids(&steps);
        let inputs: Vec<Value> = cases.iter().map(|case| case.get("input").cloned().unwrap_or(Value::Null)).collect();
        known_ids.extend(scenario_ids(&inputs));
        let case_steps: Vec<Value> = cases.iter().filter_map(|case| case.get("steps").cloned()).collect();
        known_ids.extend(scenario_ids(&case_steps));
        known_ids.extend(database_ids(&store));
        Scenario { directory, store, known_ids, cases }
    }

    /// Run every case and compare with the golden output. `run` returns the
    /// output and how to compare it.
    pub fn check(mut self, mut run: impl FnMut(&mut Store, &str, &Value, &Path) -> (Value, Compare)) {
        let mut failures = Vec::new();
        let mut count = 0;
        for case in self.cases.clone() {
            if let Some(steps) = case.get("steps").and_then(Value::as_array) {
                replay(&mut self.store, steps);
                continue;
            }
            count += 1;
            let name = case["name"].as_str().unwrap();
            let call = case["call"].as_str().unwrap();
            let (actual, compare) = run(&mut self.store, call, &case["input"], &self.directory);
            let expected = &case["output"];
            let shape = |value: &Value| match compare {
                Compare::Exact => value.clone(),
                Compare::SortedKeys => sorted(value),
            };
            let expected_text = json::to_pretty(&normalize(&shape(expected), &self.known_ids));
            let actual_text = json::to_pretty(&normalize(&shape(&actual), &self.known_ids));
            if expected_text != actual_text {
                failures.push(format!("--- {name} ({call})\nexpected:\n{expected_text}\nactual:\n{actual_text}"));
            }
        }
        let _ = std::fs::remove_dir_all(&self.directory);
        assert!(count > 0, "the golden file has no cases");
        assert!(
            failures.is_empty(),
            "{} of {count} case(s) differ from the 2.1.0 reference:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

/// A copy of a value with object keys in sorted order.
pub fn sorted(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        Value::Object(map) => {
            let mut out = Map::new();
            for key in json::ordered_keys(map, true) {
                out.insert(key.clone(), sorted(&map[key.as_str()]));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// An object from key-value pairs.
pub fn object(pairs: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert(key.to_string(), value);
    }
    Value::Object(map)
}

pub fn opt_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

pub fn opt_u32(value: &Value, key: &str) -> Option<u32> {
    value.get(key).and_then(Value::as_u64).map(|number| number as u32)
}

pub fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}
