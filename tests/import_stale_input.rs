//! A provider import through the JavaScript client protocol refuses an export
//! that changed after its preview (spec section 8; deliberate-changes.md 66).
//!
//! `provider.preview` and `provider.import` are two worker processes, as the
//! npm client runs them. The import reads the export again. Here the export
//! changes between the two calls. Most changes keep every count the same, so
//! only the export's fingerprint can show that the plan is stale. Nothing may
//! be written. The `ath import` command needs no such check: it previews and
//! writes in one process from one reading of the export.

mod import_support;

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use import_support::{new_store, repository_root, TempDir};
use serde_json::{json, Value};

const RECEIVED_AT: &str = "2026-10-13T12:00:00.000Z";

/// Run one protocol operation in a new `ath __client` worker and return its
/// last event. Standard input stays open until that event arrives, as the
/// client keeps it open for an asynchronous operation.
fn call(op: &str, database: &Path, input: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ath"))
        .arg("__client")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let request = json!({
        "protocol": 1,
        "id": "1",
        "op": op,
        "input": input,
        "target": { "path": database.display().to_string() },
    });
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, "{request}").unwrap();
    stdin.flush().unwrap();
    let mut last = Value::Null;
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let event: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let done = matches!(event["type"].as_str(), Some("result") | Some("error"));
        last = event;
        if done {
            break;
        }
    }
    drop(stdin);
    child.wait().unwrap();
    last
}

fn preview(database: &Path, input: &Path) -> Value {
    let event = call(
        "provider.preview",
        database,
        json!({ "input_path": input.display().to_string(), "received_at": RECEIVED_AT }),
    );
    assert_eq!(event["type"], "result", "{event}");
    event["output"].clone()
}

/// The `provider.import` request the npm client sends for this preview.
fn import(database: &Path, input: &Path, preview: &Value) -> Value {
    let count = |key: &str| preview[key].as_array().map_or(0, Vec::len);
    assert!(preview["input_fingerprint"].is_string(), "{preview}");
    let request = json!({
        "input_path": input.display().to_string(),
        "received_at": RECEIVED_AT,
        "input_fingerprint": preview["input_fingerprint"],
        "database_revision": preview["database_revision"],
        "new_record_count": count("new_records"),
        "matching_record_count": count("matching_records"),
        "changed_record_count": count("changed_records"),
        "conflict_count": count("conflicts"),
    });
    call("provider.import", database, request)
}

/// The database revision and record count, read with an independent connection.
fn contents(database: &Path) -> (i64, i64) {
    let connection = rusqlite::Connection::open(database).unwrap();
    let revision = connection
        .query_row("SELECT current_revision FROM database_state WHERE singleton_id = 1", [], |row| row.get(0))
        .unwrap();
    let records = connection.query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0)).unwrap();
    (revision, records)
}

fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_folder(&entry.path(), &to.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

/// Replace the first `from` in a file with `to`.
fn edit(path: &Path, from: &str, to: &str) {
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains(from), "{} has no {from:?}", path.display());
    std::fs::write(path, text.replacen(from, to, 1)).unwrap();
}

struct Case {
    /// The input as previewed and imported.
    input: PathBuf,
    /// The file that changes, and the text that changes in it.
    changed: PathBuf,
    from: &'static str,
    to: &'static str,
}

fn whoop_folder(directory: &Path) -> Case {
    let input = directory.join("whoop");
    copy_folder(&repository_root().join("tests/fixtures/exports/whoop"), &input);
    // The first cycle's HRV: one value of one record.
    Case { changed: input.join("physiological_cycles.csv"), input, from: ",68.4,", to: ",69.4," }
}

fn oura_file(directory: &Path) -> Case {
    let input = directory.join("sleep.csv");
    std::fs::copy(repository_root().join("tests/fixtures/exports/oura-offset/sleep.csv"), &input).unwrap();
    // The first night's average HRV.
    Case { changed: input.clone(), input, from: ",45,14,52", to: ",46,14,52" }
}

/// A bare Apple XML whose workout reads a route file beside it. The route
/// file changes; the XML does not.
fn apple_route_beside_a_file(directory: &Path) -> Case {
    let folder = directory.join("apple");
    copy_folder(&repository_root().join("tests/fixtures/exports/apple-file-reference"), &folder);
    // The first route point's latitude.
    let route = folder.join("workout-routes/route_2026-06-14.gpx");
    Case { input: folder.join("export.xml"), changed: route, from: "lat=\"13.5\"", to: "lat=\"13.6\"" }
}

fn assert_refused_when_changed(case: impl Fn(&Path) -> Case) {
    let directory = TempDir::new();
    let database = directory.path().join("athlete.ath.sqlite");
    new_store(directory.path()).close().unwrap();
    let case = case(directory.path());
    let previewed = preview(&database, &case.input);
    assert!(!previewed["new_records"].as_array().unwrap().is_empty(), "{previewed}");

    edit(&case.changed, case.from, case.to);
    let again = preview(&database, &case.input);
    for key in ["new_records", "matching_records", "changed_records", "conflicts"] {
        assert_eq!(previewed[key].as_array().map(Vec::len), again[key].as_array().map(Vec::len), "{key}");
    }

    let refused = import(&database, &case.input, &previewed);
    assert_eq!(refused["type"], "error", "{refused}");
    assert_eq!(refused["error"]["code"], "stale", "{refused}");
    assert_eq!(contents(&database), (0, 0), "nothing may be written");

    // The preview made after the change imports.
    let imported = import(&database, &case.input, &again);
    assert_eq!(imported["type"], "result", "{imported}");
    assert_eq!(imported["output"]["database_revision"], 1);
}

#[test]
fn an_unchanged_export_imports_after_its_preview() {
    let directory = TempDir::new();
    let database = directory.path().join("athlete.ath.sqlite");
    new_store(directory.path()).close().unwrap();
    let case = whoop_folder(directory.path());
    let previewed = preview(&database, &case.input);
    let imported = import(&database, &case.input, &previewed);
    assert_eq!(imported["type"], "result", "{imported}");
    assert_eq!(imported["output"]["new_record_count"], previewed["new_records"].as_array().unwrap().len());
}

#[test]
fn a_folder_export_changed_after_its_preview_is_refused() {
    assert_refused_when_changed(whoop_folder);
}

#[test]
fn a_file_export_changed_after_its_preview_is_refused() {
    assert_refused_when_changed(oura_file);
}

#[test]
fn a_route_file_beside_an_apple_export_changed_after_its_preview_is_refused() {
    assert_refused_when_changed(apple_route_beside_a_file);
}

#[test]
fn an_export_whose_counts_changed_after_its_preview_is_refused() {
    let directory = TempDir::new();
    let database = directory.path().join("athlete.ath.sqlite");
    new_store(directory.path()).close().unwrap();
    let case = whoop_folder(directory.path());
    let previewed = preview(&database, &case.input);
    std::fs::remove_file(case.input.join("journal_entries.csv")).unwrap();
    let refused = import(&database, &case.input, &previewed);
    assert_eq!(refused["error"]["code"], "stale", "{refused}");
    assert_eq!(contents(&database), (0, 0));
}
