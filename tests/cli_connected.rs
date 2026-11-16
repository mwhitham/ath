//! Connected commands run in-process where a terminal, a password store, or
//! a package registry is needed: second confirmations typed at a terminal,
//! `ath key set` with hidden input, and `ath --update` with an injected
//! registry answer. The executable-level comparison with 2.1.0 is
//! `cli_golden.rs`; these paths cannot run there without a real terminal or
//! a real service.
#![cfg_attr(not(unix), allow(unused))]

mod common;

use std::path::Path;
use std::rc::Rc;

use athleticstandard::runtime::keyring::{KeyRefusal, MemorySecretStore};
use common::{database_revision, fixture, private_folder, record_state, Session};

const VERSION_2_0_0_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000001";

fn copy_version_2_0_0(folder: &Path) -> std::path::PathBuf {
    let path = folder.join("athlete.ath.sqlite");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/version-2-0-0/database.ath.sqlite"),
        &path,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    path
}

#[test]
fn migrate_asks_for_the_database_id_at_a_terminal() {
    let folder = private_folder();
    copy_version_2_0_0(folder.path());
    let wrong = Session::terminal(folder.path(), "not-the-id\n").run(&["migrate", "--yes"]);
    assert_eq!(wrong.status, 1);
    assert!(wrong.stdout.ends_with(&format!("Type the database ID {VERSION_2_0_0_ID} to confirm migration: ")));
    assert_eq!(wrong.stderr, "ath: second confirmation did not exactly match the database ID; nothing was migrated\n");

    let typed = Session::terminal(folder.path(), &format!("{VERSION_2_0_0_ID}\n")).run(&["migrate", "--yes"]);
    assert_eq!(typed.status, 0, "{}", typed.stderr);
    assert!(
        typed.stdout.contains("to confirm migration: migrated SQL schema 3 to 5 with verified backup at "),
        "{}",
        typed.stdout
    );
    let again = Session::piped(folder.path()).run(&["migrate"]);
    assert!(again.stdout.ends_with("no migration is required\n"));
}

#[test]
fn erase_asks_twice_at_a_terminal() {
    let folder = private_folder();
    let fixture = fixture(folder.path());
    let id = fixture.benchmark_result.as_str();
    let declined = Session::terminal(folder.path(), "n\n").run(&["erase", id, "--revision", "1"]);
    assert_eq!(declined.status, 0);
    assert!(declined.stdout.ends_with("Permanently erase these records? [y] yes  [n] no nothing erased\n"));

    let mistyped = Session::terminal(folder.path(), "y\nnope\n").run(&["erase", id, "--revision", "1"]);
    assert_eq!(mistyped.status, 1);
    assert_eq!(mistyped.stderr, "ath: second confirmation did not exactly match the record ID; nothing was erased\n");
    assert_eq!(record_state(&fixture.path, id), (1, false));

    let erased = Session::terminal(folder.path(), &format!("yes\n{id}\n")).run(&["erase", id, "--revision", "1"]);
    assert_eq!(erased.status, 0, "{}", erased.stderr);
    let revision = fixture.database_revision + 1;
    assert!(erased.stdout.ends_with(&format!(
        "Type the record ID {id} to confirm permanent erasure: erased 1 record and 0 attachment files at database revision {revision}\n"
    )));
    assert_eq!(database_revision(&fixture.path), revision);
}

fn pasted(_: &str) -> Result<String, KeyRefusal> {
    Ok("  sk-test-key\n".to_string())
}

fn ctrl_c(_: &str) -> Result<String, KeyRefusal> {
    Err(KeyRefusal::new("nothing was saved."))
}

#[test]
fn key_set_saves_in_the_password_store_and_never_prints_the_key() {
    let folder = private_folder();
    let mut session = Session::terminal(folder.path(), "");
    let store = Rc::new(MemorySecretStore::new());
    session.services.secret_store = Some(store.clone());
    session.services.prompt_secret = pasted;

    let saved = session.run(&["key", "set", "vercel"]);
    assert_eq!(
        (saved.status, saved.stdout.as_str(), saved.stderr.as_str()),
        (0, "saved a vercel key in the password store. The key is not printed.\n", "")
    );
    let garmin = session.run(&["key", "set", "garmin"]);
    assert_eq!(garmin.stdout, "saved a garmin key in the password store. The key is not printed.\n");

    let status = session.run(&["key", "--json"]);
    assert_eq!(status.stdout, "{\n  \"saved\": [\n    \"vercel\"\n  ],\n  \"environment\": []\n}\n");
    assert!(!status.stdout.contains("sk-test-key"));

    let cleared = session.run(&["key", "clear"]);
    assert_eq!(cleared.stdout, "cleared vercel from the password store.\n");
    assert_eq!(session.run(&["key", "clear", "vercel"]).stdout, "no gateway key was saved.\n");

    session.services.prompt_secret = ctrl_c;
    let cancelled = session.run(&["key", "set", "openrouter"]);
    assert_eq!((cancelled.status, cancelled.stderr.as_str()), (1, "ath: nothing was saved.\n"));
}

#[test]
fn a_gateway_root_must_be_on_this_computer() {
    let folder = private_folder();
    let mut session = Session::piped(folder.path());
    session.env.insert("AI_GATEWAY_API_KEY".into(), "test-key".into());
    session.env.insert("ATH_GATEWAY_BASE_URL".into(), "https://gateway.example".into());
    let refused = session.run(&["models"]);
    assert_eq!(
        (refused.status, refused.stderr.as_str()),
        (
            1,
            "ath: ATH_GATEWAY_BASE_URL must name a test server on this computer, such as http://127.0.0.1:8080; nothing was sent\n"
        )
    );
}

fn newer() -> Result<String, String> {
    Ok("9.9.9".to_string())
}

fn same() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

fn offline() -> Result<String, String> {
    Err("registry status: offline".to_string())
}

#[test]
fn update_hands_a_checked_plan_to_the_launcher() {
    let folder = private_folder();
    let plan = folder.path().join("plan.json");
    let mut session = Session::piped(folder.path());
    session.env.insert("ATH_INSTALL_PATHS".into(), "/usr/local/lib/node_modules/athleticstandard".into());
    session.env.insert("ATH_UPDATE_HANDOFF".into(), plan.display().to_string());

    session.services.latest_version = same;
    let current = session.run(&["--update"]);
    assert_eq!(
        (current.status, current.stdout, current.stderr),
        (0, format!("ath {} is the current version.\n", env!("CARGO_PKG_VERSION")), String::new())
    );
    assert!(!plan.exists());

    session.services.latest_version = offline;
    let unreachable = session.run(&["--update"]);
    assert_eq!(
        (unreachable.status, unreachable.stderr.as_str()),
        (1, "ath: the npm registry could not be reached, so nothing was changed.\n")
    );

    session.services.latest_version = newer;
    let handed = session.run(&["--update"]);
    assert_eq!((handed.status, handed.stdout.as_str(), handed.stderr.as_str()), (0, "", ""));
    let text = std::fs::read_to_string(&plan).unwrap();
    assert!(text.contains("\"9.9.9\"") || text.contains("@9.9.9"), "{text}");

    session.env.insert("ATH_INSTALL_PATHS".into(), "/home/u/.npm/_npx/abc/node_modules/athleticstandard".into());
    let fetched = session.run(&["--update"]);
    assert_eq!(fetched.status, 0, "{}", fetched.stderr);
    assert!(!fetched.stdout.is_empty());
}

// ---- No write lock during a model call ----------------------------------------

const CATALOG: &str = r#"{"data":[{"id":"test/steady","name":"Steady","reasoning":true}]}"#;
const STEADY_REPLY: &str = r#"{"predicted_score":{"duration_s":290},"predicted_range":{"low":{"duration_s":280},"high":{"duration_s":300}},"confidence":"moderate","reasoning":"Anchored on the latest result."}"#;

/// A local model server. Before it answers a model call, it takes the write
/// lock on `database` with its own connection, writes, and commits. A
/// command that held the write lock during the call would make that fail
/// after one second. The result of each attempt is kept.
fn serve_while_writing(database: std::path::PathBuf) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = std::sync::Arc::clone(&writes);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            stream.set_read_timeout(Some(std::time::Duration::from_secs(30))).unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() {
                continue;
            }
            let mut length = 0usize;
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
                    break;
                }
                if let Some((name, value)) = header.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let reply = if request_line.starts_with("GET /models ") {
                CATALOG.to_string()
            } else if request_line.starts_with("POST /chat/completions ") {
                let connection = rusqlite::Connection::open(&database).unwrap();
                connection.busy_timeout(std::time::Duration::from_secs(1)).unwrap();
                let written = connection.execute_batch(
                    "BEGIN IMMEDIATE;
                     UPDATE database_state SET current_revision = current_revision + 1 WHERE singleton_id = 1;
                     COMMIT;",
                );
                log.lock().unwrap().push(match written {
                    Ok(()) => "written".to_string(),
                    Err(error) => error.to_string(),
                });
                serde_json::json!({"model": "test/steady", "choices": [{"message": {"content": STEADY_REPLY}}]})
                    .to_string()
            } else {
                r#"{"error":"not found"}"#.to_string()
            };
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                reply.len()
            );
        }
    });
    (format!("http://{address}"), writes)
}

/// Spec section 8: no writer lock is held while a model is called. Another
/// connection writes during the call, and the prediction is saved after it,
/// as 2.1.0 saved it: a prediction adds a record and checks no revision.
#[test]
fn no_write_lock_is_held_during_a_model_call() {
    use athleticstandard::store::migrate::{execute_confirmed_migration, MigrationOptions};
    use athleticstandard::store::{OpenOptions, Store};
    let folder = private_folder();
    let path = folder.path().join("athlete.ath.sqlite");
    std::fs::copy(common::demo_database(), &path).unwrap();
    athleticstandard::files::set_owner_only(&path).unwrap();
    let mut store = Store::open(&path, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
    let options = MigrationOptions { backup_path: Some(folder.path().join("backup.sqlite")), now_millis: None };
    execute_confirmed_migration(&mut store, &options).unwrap();
    store.close().unwrap();
    let before = database_revision(&path);

    let (base_url, writes) = serve_while_writing(path.clone());
    let mut session = Session::piped(folder.path());
    session.env.insert("ATH_GATEWAY_BASE_URL".into(), base_url);
    session.env.insert("AI_GATEWAY_API_KEY".into(), "test-vercel-key".into());
    session.services.secret_store = Some(Rc::new(MemorySecretStore::new()));
    let file = path.display().to_string();
    let output = session.run(&["predict", "fran", "--model", "test/steady", "--yes", "--file", &file]);

    assert_eq!(*writes.lock().unwrap(), ["written"], "the model call held the write lock");
    assert_eq!(output.status, 0, "{}", output.stderr);
    assert!(output.stdout.contains(&format!(" at database revision {}\n", before + 2)), "{}", output.stdout);
    assert_eq!(database_revision(&path), before + 2);
}
