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
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/version-2-0-0/database.ath.sqlite"),
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
