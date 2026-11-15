//! Shared helpers for command-line tests: an in-process runner with chosen
//! terminal facts and answers, and a small database built from the committed
//! synthetic demo records.
#![cfg_attr(not(unix), allow(unused_imports, dead_code))]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use athleticstandard::cli::{self, Io, Services, Terminal};
use athleticstandard::json;
use athleticstandard::model::{RecordActor, RecordData, RecordType};
use athleticstandard::store::{Changes, CreateOptions, NewRecord, Store};

/// What one run printed and its exit status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// How a command runs: its folder, terminal facts, input, and services.
#[derive(Clone)]
pub struct Session {
    pub cwd: PathBuf,
    pub terminal: Terminal,
    pub stdin: Vec<u8>,
    /// Lines the controlling terminal answers with, or `None` when there is
    /// no controlling terminal.
    pub controlling_terminal: Option<Vec<u8>>,
    pub services: Services,
    /// The environment the command sees.
    pub env: BTreeMap<String, String>,
}

impl Session {
    /// A piped run: no terminal anywhere, empty input.
    pub fn piped(cwd: &Path) -> Session {
        Session {
            cwd: cwd.to_path_buf(),
            terminal: Terminal::default(),
            stdin: Vec::new(),
            controlling_terminal: None,
            services: Services::default(),
            env: BTreeMap::new(),
        }
    }

    /// A run in a terminal that answers with `answers`.
    pub fn terminal(cwd: &Path, answers: &str) -> Session {
        Session {
            terminal: Terminal { stdin: true, stdout: true, stderr: true, stdout_columns: None, stderr_columns: None },
            stdin: answers.as_bytes().to_vec(),
            ..Session::piped(cwd)
        }
    }

    pub fn run(&self, argv: &[&str]) -> Output {
        let argv: Vec<String> = argv.iter().map(ToString::to_string).collect();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut stdin: &[u8] = &self.stdin;
        let controlling = self.controlling_terminal.clone();
        let mut open_terminal =
            move || controlling.clone().map(|bytes| Box::new(std::io::Cursor::new(bytes)) as Box<dyn BufRead>);
        let io = Io {
            stdout: &mut stdout,
            stderr: &mut stderr,
            stdin: &mut stdin,
            terminal: self.terminal,
            open_terminal: &mut open_terminal,
            cwd: self.cwd.clone(),
            env: self.env.clone(),
        };
        let status = cli::run(&argv, io, self.services.clone());
        Output { status, stdout: String::from_utf8(stdout).unwrap(), stderr: String::from_utf8(stderr).unwrap() }
    }
}

pub fn demo_database() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/demo-athlete/athlete.ath.sqlite")
}

/// A private temporary folder, named by its canonical path so no component
/// is a symbolic link (the system temporary folder can be one).
pub struct Folder {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

impl Folder {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// A private temporary folder that the permission checks accept.
pub fn private_folder() -> Folder {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let path = dir.path().canonicalize().unwrap();
    Folder { _dir: dir, path }
}

/// Record IDs in the fixture database.
pub struct Fixture {
    pub path: PathBuf,
    pub benchmark_definition: String,
    pub benchmark_result: String,
    /// The session the result names.
    pub session: String,
    /// Another session the result can be linked to.
    pub other_session: String,
    pub database_revision: u64,
}

pub const MANUAL_SOURCE: &str = "01890f3e-e7d3-7cc0-88c8-000000000004";
pub const WEARABLE_SOURCE: &str = "01890f3e-e7d3-7cc0-88c8-000000000003";
pub const DEFINITION: &str = "01890f3e-e7d3-7cc0-88c8-000000000006";
pub const SESSION: &str = "01890f3e-e7d3-7cc0-88c8-00000000000a";
pub const RESULT: &str = "01890f3e-e7d3-7cc0-88c8-00000000000b";

/// The current data of a demo record.
fn demo_data(record_id: &str) -> (RecordType, json::Value) {
    // `immutable` reads the committed file without creating sidecar files.
    let uri = format!("file:{}?immutable=1", demo_database().canonicalize().unwrap().display());
    let connection = rusqlite::Connection::open_with_flags(
        uri,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .unwrap();
    let (record_type, record_json): (String, String) = connection
        .query_row(
            "SELECT records.record_type, record_revisions.record_json FROM records
             JOIN record_revisions ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             WHERE records.record_id = ?1",
            [record_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    (RecordType::parse(&record_type).unwrap(), json::parse(&record_json).unwrap())
}

/// A new `athlete.ath.sqlite` in `folder` with two sources, a benchmark
/// definition, two workout sessions, and a benchmark result.
pub fn fixture(folder: &Path) -> Fixture {
    let path = folder.join("athlete.ath.sqlite");
    let mut store = Store::create(&path, CreateOptions::default()).unwrap();
    let other_session = athleticstandard::ids::new_record_id();
    let mut creates = Vec::new();
    for (record_id, source_id) in [
        (MANUAL_SOURCE, MANUAL_SOURCE),
        (WEARABLE_SOURCE, WEARABLE_SOURCE),
        (DEFINITION, DEFINITION),
        (SESSION, SESSION),
        (other_session.as_str(), SESSION),
        (RESULT, RESULT),
    ] {
        let (record_type, value) = demo_data(source_id);
        let data = RecordData::decode(record_type, &value).unwrap();
        creates
            .push(NewRecord { record_id: Some(record_id.to_string()), ..NewRecord::new(data, RecordActor::Athlete) });
    }
    let applied = store.apply(Changes { creates, ..Changes::default() }).unwrap();
    store.close().unwrap();
    Fixture {
        path,
        benchmark_definition: DEFINITION.to_string(),
        benchmark_result: RESULT.to_string(),
        session: SESSION.to_string(),
        other_session,
        database_revision: applied.database_revision,
    }
}

/// The current revision and deletion state of a record.
pub fn record_state(path: &Path, record_id: &str) -> (u64, bool) {
    let store =
        Store::open(path, athleticstandard::store::OpenOptions { read_only: true, allow_outdated_schema: true })
            .unwrap();
    let record = store.get_record(record_id).unwrap().unwrap();
    (record.current_revision, record.is_deleted)
}

pub fn database_revision(path: &Path) -> u64 {
    let store =
        Store::open(path, athleticstandard::store::OpenOptions { read_only: true, allow_outdated_schema: true })
            .unwrap();
    store.database_revision().unwrap()
}
