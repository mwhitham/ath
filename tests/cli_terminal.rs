//! Prompts through a real terminal (spec section 13.2). The built `ath`
//! runs with its standard input, output, and error on a pseudoterminal, so
//! it sees a terminal the way it does for a person. The prompt texts are
//! command-behavior.md S15 and S18 to S20. Other tests use pipes or the
//! in-process terminal in `cli_runtime.rs`.
//!
//! While a prompt waits, no write lock may be held (spec section 8): another
//! connection writes during the wait, and the command then applies or
//! refuses its plan by the rules of that command.
//!
//! Every wait has a timeout, so a hang fails the test instead of blocking.

#![cfg(unix)]

mod common;

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::os::raw::{c_char, c_int};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use common::{fixture, private_folder, RESULT};

const TIMEOUT: Duration = Duration::from_secs(30);
const DEMO_DATABASE_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000001";
const WRITE_PROMPT: &str = "Write these records? [y] yes  [n] no ";
const MIGRATE_PROMPT: &str = "Create a verified backup and apply this migration? [y] yes  [n] no ";

extern "C" {
    fn posix_openpt(flags: c_int) -> c_int;
    fn grantpt(fd: c_int) -> c_int;
    fn unlockpt(fd: c_int) -> c_int;
    fn ptsname(fd: c_int) -> *mut c_char;
}

const O_RDWR: c_int = 2;
#[cfg(target_os = "linux")]
const O_NOCTTY: c_int = 0o400;
#[cfg(target_os = "macos")]
const O_NOCTTY: c_int = 0x20000;
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
const O_NOCTTY: c_int = 0;

/// `ptsname` returns a shared buffer, so one test at a time calls it.
static PTSNAME: Mutex<()> = Mutex::new(());

/// A new pseudoterminal: its controlling side and the path of its terminal side.
fn open_pseudoterminal() -> (File, PathBuf) {
    // SAFETY: each call is checked. The descriptor is owned by the returned
    // File from here on. `ptsname` is called under a lock and its string is
    // copied before the lock is released.
    unsafe {
        let fd = posix_openpt(O_RDWR | O_NOCTTY);
        assert!(fd >= 0, "posix_openpt: {}", std::io::Error::last_os_error());
        let master = File::from_raw_fd(fd);
        assert_eq!(grantpt(fd), 0, "grantpt: {}", std::io::Error::last_os_error());
        assert_eq!(unlockpt(fd), 0, "unlockpt: {}", std::io::Error::last_os_error());
        let _guard = PTSNAME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let name = ptsname(fd);
        assert!(!name.is_null(), "ptsname: {}", std::io::Error::last_os_error());
        let path = PathBuf::from(std::ffi::CStr::from_ptr(name).to_string_lossy().into_owned());
        (master, path)
    }
}

/// What the terminal has shown so far, and whether the program closed it.
#[derive(Default)]
struct Screen {
    bytes: Vec<u8>,
    closed: bool,
}

/// `ath` running on a pseudoterminal.
struct Running {
    child: Child,
    master: File,
    screen: Arc<(Mutex<Screen>, Condvar)>,
}

fn start(cwd: &Path, args: &[&str]) -> Running {
    let (master, slave_path) = open_pseudoterminal();
    let slave = std::fs::OpenOptions::new().read(true).write(true).custom_flags(O_NOCTTY).open(&slave_path).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_ath"));
    command
        .args(args)
        .current_dir(cwd)
        .env("TZ", "UTC")
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave));
    let child = command.spawn().unwrap();
    // Only the program holds the terminal side now, so the controlling side
    // reads an end when the program exits.
    drop(command);

    let screen = Arc::new((Mutex::new(Screen::default()), Condvar::new()));
    let shared = Arc::clone(&screen);
    let mut reader = master.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            // Linux reports EIO once the terminal side is closed.
            let read = reader.read(&mut buffer).unwrap_or(0);
            let (lock, changed) = &*shared;
            let mut screen = lock.lock().unwrap();
            if read == 0 {
                screen.closed = true;
                changed.notify_all();
                return;
            }
            screen.bytes.extend_from_slice(&buffer[..read]);
            changed.notify_all();
        }
    });
    Running { child, master, screen }
}

impl Running {
    /// The text shown so far, with the terminal's `\r\n` line ends as `\n`.
    fn text(&self) -> String {
        let screen = self.screen.0.lock().unwrap();
        String::from_utf8_lossy(&screen.bytes).replace("\r\n", "\n")
    }

    /// Wait until the screen shows `text`, or fail after the timeout.
    fn wait_for(&self, text: &str) {
        let (lock, changed) = &*self.screen;
        let deadline = Instant::now() + TIMEOUT;
        let mut screen = lock.lock().unwrap();
        loop {
            if String::from_utf8_lossy(&screen.bytes).replace("\r\n", "\n").contains(text) {
                return;
            }
            let now = Instant::now();
            assert!(
                !screen.closed && now < deadline,
                "the terminal never showed {text:?}; it showed:\n{}",
                String::from_utf8_lossy(&screen.bytes)
            );
            screen = changed.wait_timeout(screen, deadline - now).unwrap().0;
        }
    }

    /// Type a line, as a person would after reading the prompt.
    fn answer(&mut self, line: &str) {
        self.master.write_all(format!("{line}\n").as_bytes()).unwrap();
        self.master.flush().unwrap();
    }

    /// Wait for the program to exit and the terminal to close. Returns the
    /// exit status and everything the terminal showed.
    fn finish(mut self) -> (i32, String) {
        let deadline = Instant::now() + TIMEOUT;
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                panic!("ath did not exit; the terminal showed:\n{}", self.text());
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let (lock, changed) = &*self.screen;
        let mut screen = lock.lock().unwrap();
        while !screen.closed {
            let now = Instant::now();
            assert!(now < deadline, "the terminal stayed open after ath exited");
            screen = changed.wait_timeout(screen, deadline - now).unwrap().0;
        }
        drop(screen);
        (status.code().unwrap_or(-1), self.text())
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        // A failed test must not leave a program waiting at a prompt.
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
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

/// Take the write lock with an independent connection, write, and commit.
/// A command that held the write lock would make this fail after one second.
fn write_from_another_connection(database: &Path) {
    let connection = rusqlite::Connection::open(database).unwrap();
    connection.busy_timeout(Duration::from_secs(1)).unwrap();
    connection
        .execute_batch(
            "BEGIN IMMEDIATE;
             UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1;
             COMMIT;",
        )
        .expect("another connection could not write while ath waited at a prompt");
}

/// A private copy of the 2.1.0 demo database, at SQL schema step 4.
fn copy_demo(folder: &Path) -> PathBuf {
    let path = folder.join("athlete.ath.sqlite");
    std::fs::copy(common::demo_database(), &path).unwrap();
    athleticstandard::files::set_owner_only(&path).unwrap();
    path
}

fn schema_step(database: &Path) -> u32 {
    let connection = rusqlite::Connection::open(database).unwrap();
    connection.query_row("SELECT sql_schema_version FROM database_metadata", [], |row| row.get(0)).unwrap()
}

fn folder_names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_write_confirmed_with_y_is_written_after_another_writer_commits() {
    let folder = private_folder();
    let database = fixture(folder.path()).path;
    let (revision, records) = contents(&database);
    let file = database.display().to_string();
    let mut running = start(folder.path(), &["log", "--file", &file, "Easy", "aerobic", "session."]);
    running.wait_for(WRITE_PROMPT);
    // No write lock is held while the prompt waits. A log only adds new
    // records, so another writer's change does not make it stale; 2.1.0
    // wrote it too.
    write_from_another_connection(&database);
    running.answer("y");
    let (status, text) = running.finish();
    assert_eq!(status, 0, "{text}");
    assert!(text.contains(&format!("wrote 1 record at database revision {}\n", revision + 2)), "{text}");
    assert_eq!(contents(&database), (revision + 2, records + 1));
}

#[test]
fn a_write_refused_with_n_writes_nothing() {
    let folder = private_folder();
    let database = fixture(folder.path()).path;
    let before = contents(&database);
    let file = database.display().to_string();
    let mut running = start(folder.path(), &["log", "--file", &file, "Easy", "aerobic", "session."]);
    running.wait_for(WRITE_PROMPT);
    running.answer("n");
    let (status, text) = running.finish();
    assert_eq!(status, 0, "{text}");
    assert!(text.ends_with(&format!("{WRITE_PROMPT}n\nnothing written\n")), "{text}");
    assert_eq!(contents(&database), before);
}

#[test]
fn a_stale_delete_is_refused_after_the_record_changed_during_the_prompt() {
    let folder = private_folder();
    let database = fixture(folder.path()).path;
    let file = database.display().to_string();
    let mut running = start(folder.path(), &["delete", RESULT, "--revision", "1", "--file", &file]);
    running.wait_for("Delete this record from current views? [y] yes  [n] no ");
    write_from_another_connection(&database);

    // A second process deletes the same record while the first one waits.
    let mut other = Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(["delete", RESULT, "--revision", "1", "--yes", "--file", &file])
        .current_dir(folder.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + TIMEOUT;
    while other.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "the second delete waited for a lock");
        std::thread::sleep(Duration::from_millis(20));
    }
    let other = other.wait_with_output().unwrap();
    assert_eq!(other.status.code(), Some(0), "{}", String::from_utf8_lossy(&other.stderr));
    let after_other = contents(&database);

    running.answer("y");
    let (status, text) = running.finish();
    assert_eq!(status, 1, "{text}");
    // The delete checks the record at its expected revision when it writes
    // (semantic-rules.md S11), so the plan made before the wait is refused.
    assert!(text.ends_with(&format!("ath: Record is already deleted: {RESULT}\n")), "{text}");
    assert_eq!(contents(&database), after_other, "the refused delete wrote nothing");
}

#[test]
fn migrate_asks_for_the_database_id_and_refuses_a_wrong_one() {
    let folder = private_folder();
    let database = copy_demo(folder.path());
    let before = std::fs::read(&database).unwrap();
    let file = database.display().to_string();
    let question = format!("Type the database ID {DEMO_DATABASE_ID} to confirm migration: ");

    let mut wrong = start(folder.path(), &["migrate", "--yes", "--file", &file]);
    wrong.wait_for(&question);
    wrong.answer("01890f3e-e7d3-7cc0-88c8-000000000002");
    let (status, text) = wrong.finish();
    assert_eq!(status, 1, "{text}");
    assert!(
        text.ends_with("ath: second confirmation did not exactly match the database ID; nothing was migrated\n"),
        "{text}"
    );
    assert_eq!(std::fs::read(&database).unwrap(), before, "nothing was migrated");
    // The preview reads the file read-only, which keeps WAL sidecars (spec
    // section 7), so only a backup would show a change here.
    let names = folder_names(folder.path());
    assert!(names.iter().all(|name| !name.contains("backup")), "a refused migration made a backup: {names:?}");

    let mut typed = start(folder.path(), &["migrate", "--yes", "--file", &file]);
    typed.wait_for(&question);
    typed.answer(DEMO_DATABASE_ID);
    let (status, text) = typed.finish();
    assert_eq!(status, 0, "{text}");
    assert!(
        text.contains(&format!(
            "\nmigrated SQL schema 4 to {} with verified backup at ",
            athleticstandard::store::SQL_SCHEMA_VERSION
        )),
        "{text}"
    );
    assert_eq!(schema_step(&database), athleticstandard::store::SQL_SCHEMA_VERSION);
}

#[test]
fn a_migration_on_open_needs_both_answers() {
    let folder = private_folder();
    let database = copy_demo(folder.path());
    let before = std::fs::read(&database).unwrap();
    let names = folder_names(folder.path());
    let file = database.display().to_string();
    let question = format!("Type the database ID {DEMO_DATABASE_ID} to confirm migration: ");

    let mut wrong = start(folder.path(), &["log", "--file", &file, "Easy", "aerobic", "session."]);
    wrong.wait_for(MIGRATE_PROMPT);
    wrong.answer("y");
    wrong.wait_for(&question);
    wrong.answer("not the database ID");
    let (status, text) = wrong.finish();
    assert_eq!(status, 1, "{text}");
    assert!(
        text.ends_with("ath: second confirmation did not exactly match the database ID; nothing was migrated\n"),
        "{text}"
    );
    assert!(!text.contains(WRITE_PROMPT), "{text}");
    assert_eq!(std::fs::read(&database).unwrap(), before, "nothing was migrated");
    assert_eq!(folder_names(folder.path()), names, "no backup or other file was made");

    let mut confirmed = start(folder.path(), &["log", "--file", &file, "Easy", "aerobic", "session."]);
    confirmed.wait_for(MIGRATE_PROMPT);
    confirmed.answer("yes");
    confirmed.wait_for(&question);
    confirmed.answer(DEMO_DATABASE_ID);
    confirmed.wait_for(&format!(
        "migrated SQL schema 4 to {} with verified backup at ",
        athleticstandard::store::SQL_SCHEMA_VERSION
    ));
    confirmed.wait_for(WRITE_PROMPT);
    confirmed.answer("y");
    let (status, text) = confirmed.finish();
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("wrote 1 record at database revision 4\n"), "{text}");
    assert_eq!(schema_step(&database), athleticstandard::store::SQL_SCHEMA_VERSION);
}

#[test]
fn json_at_a_terminal_never_asks() {
    let folder = private_folder();
    let database = fixture(folder.path()).path;
    let before = contents(&database);
    let file = database.display().to_string();
    // Nothing is typed. A prompt would wait until the timeout fails the test.
    let (status, text) =
        start(folder.path(), &["log", "--json", "--file", &file, "Easy", "aerobic", "session."]).finish();
    assert_eq!(status, 0, "{text}");
    assert!(text.starts_with('{') && !text.contains("[y] yes"), "{text}");
    assert_eq!(contents(&database), before);

    let older = private_folder();
    let demo = copy_demo(older.path());
    let demo_file = demo.display().to_string();
    let original = std::fs::read(&demo).unwrap();
    let (status, text) =
        start(older.path(), &["delete", RESULT, "--revision", "1", "--json", "--file", &demo_file]).finish();
    assert_eq!(status, 1, "{text}");
    assert_eq!(
        text,
        format!(
            "ath: Database SQL schema 4 must be upgraded to {} before other commands can run. Run ath migrate.\n",
            athleticstandard::store::SQL_SCHEMA_VERSION
        )
    );
    let (status, text) = start(older.path(), &["migrate", "--json", "--yes", "--file", &demo_file]).finish();
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("\"write_confirmed\": false") && !text.contains("Type the database ID"), "{text}");
    assert_eq!(std::fs::read(&demo).unwrap(), original, "nothing was migrated");
}
