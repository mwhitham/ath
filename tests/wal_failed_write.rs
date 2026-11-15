//! The 2.1.0 `tests/v1-wal-failed-write.test.ts` scenario, end to end. An
//! outside process holds a read snapshot on the database while `ath log`
//! commits, a later stale `ath delete` fails, and the outside process is
//! killed. The committed record stays visible before and after, the stale
//! delete changes nothing, `ath check` passes, and the committed data reaches
//! the database file once the last connection checkpoints it.
//!
//! The outside reader is this test program run again as a child process, with
//! a plain SQLite connection, so the test needs no `sqlite3` program and runs
//! on every system.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use common::private_folder;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

const READER_DATABASE: &str = "ATH_TEST_WAL_READER_DATABASE";
const READY: &str = "outside reader holds a snapshot";
const POINT: &str = "01890f3e-e7d3-7cc0-88c8-000000000007";

/// The outside reader: open the database named by the environment, start a
/// read transaction, say so, and wait to be killed.
#[test]
#[ignore = "run as the outside reader by keeps_committed_wal_data_when_a_later_write_fails"]
fn outside_reader() {
    let Ok(path) = std::env::var(READER_DATABASE) else { return };
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("BEGIN DEFERRED;").unwrap();
    let count: i64 = connection.query_row("SELECT count(*) FROM records", [], |row| row.get(0)).unwrap();
    println!("{READY} of {count} records");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

fn start_reader(path: &Path) -> Child {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["outside_reader", "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env(READER_DATABASE, path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    loop {
        let line = lines.next().expect("the reader printed nothing").unwrap();
        if line.contains(READY) {
            break;
        }
    }
    child
}

fn ath(cwd: &Path, argv: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(argv)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// A copy of the 2.1.1 demo database and its attachment.
fn demo_copy(folder: &Path) -> PathBuf {
    let demo = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/demo-athlete");
    let path = folder.join("athlete.ath.sqlite");
    std::fs::copy(demo.join("athlete.ath.sqlite"), &path).unwrap();
    athleticstandard::files::set_owner_only(&path).unwrap();
    athleticstandard::files::create_private_dir(&folder.join("attachments")).unwrap();
    for entry in std::fs::read_dir(demo.join("attachments")).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), folder.join("attachments").join(entry.file_name())).unwrap();
    }
    path
}

fn show(folder: &Path, record_id: &str) -> Value {
    let (status, stdout, stderr) = ath(folder, &["records", "show", record_id, "--json"]);
    assert_eq!(status, 0, "{stderr}");
    serde_json::from_str(&stdout).unwrap()
}

fn database_revision(folder: &Path) -> i64 {
    let (status, stdout, stderr) = ath(folder, &["check", "--json"]);
    assert_eq!(status, 0, "{stderr}");
    serde_json::from_str::<Value>(&stdout).unwrap()["metadata"]["database_revision"].as_i64().unwrap_or(-1)
}

#[test]
fn keeps_committed_wal_data_when_a_later_write_fails() {
    let folder = private_folder();
    let path = demo_copy(folder.path());
    let wal = folder.path().join("athlete.ath.sqlite-wal");
    let mut reader = start_reader(&path);

    let (status, stdout, stderr) = ath(folder.path(), &["log", "note two", "-y"]);
    assert_eq!(status, 0, "{stdout}{stderr}");
    assert!(std::fs::metadata(&wal).map(|file| file.len() > 0).unwrap_or(false), "the commit waits in the WAL");

    let (status, stdout, stderr) = ath(folder.path(), &["records", "list", "--json"]);
    assert_eq!(status, 0, "{stderr}");
    let listed: Value = serde_json::from_str(&stdout).unwrap();
    let reports: Vec<&Value> = listed["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|record| record["record_type"] == "athlete_report")
        .collect();
    assert_eq!(reports.len(), 1, "the demo's own report is deleted; the new note is the only current one");
    let report = reports[0]["record_id"].as_str().unwrap().to_string();

    let revision_before = database_revision(folder.path());
    let (status, _, stderr) = ath(folder.path(), &["delete", POINT, "--revision", "1", "-y"]);
    assert_eq!(
        (status, stderr),
        (1, format!("ath: Record {POINT} is at revision 2, not expected revision 1\n")),
        "a stale delete fails"
    );
    assert_eq!(database_revision(folder.path()), revision_before, "the failed delete changed nothing");
    let point = show(folder.path(), POINT);
    assert_eq!(
        (point["record"]["current_revision"].as_i64(), point["record"]["is_deleted"].as_bool()),
        (Some(2), Some(false))
    );

    assert_eq!(show(folder.path(), &report)["record"]["data"]["note"], "note two");

    reader.kill().unwrap();
    reader.wait().unwrap();

    assert_eq!(show(folder.path(), &report)["record"]["data"]["note"], "note two");
    let (status, stdout, stderr) = ath(folder.path(), &["check"]);
    assert_eq!(status, 0, "{stdout}{stderr}");
    assert!(stdout.contains("valid Athletic Standard 2.1.1 database"), "{stdout}");

    // The file is still a WAL database: header bytes 18 and 19 are 2.
    let header = std::fs::read(&path).unwrap();
    assert_eq!((header[18], header[19]), (2, 2));
    // The last connection to close checkpoints the WAL into the file and
    // removes it.
    let last = Connection::open(&path).unwrap();
    let _: i64 = last.query_row("SELECT count(*) FROM records", [], |row| row.get(0)).unwrap();
    last.close().unwrap();
    assert!(!wal.exists(), "the WAL was checkpointed and removed");
    let immutable = Connection::open_with_flags(
        format!("file:{}?immutable=1", path.display()),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .unwrap();
    let note: String = immutable
        .query_row(
            "SELECT json_extract(record_json, '$.note') FROM record_revisions WHERE record_id = ?1",
            [&report],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(note, "note two");
}
