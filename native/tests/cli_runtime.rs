//! Shared command behavior, run in-process: database discovery (S10–S13),
//! migration on open (S15), read-only open (S16), `ath init` before
//! creation, the bare guide, and commands that are not connected yet.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use athleticstandard::cli::spec;
use athleticstandard::cli::{Context, Io, Services, Terminal};
use athleticstandard::store::{MigrationPreview, Store};
use common::{private_folder, Session, RESULT};

const DEMO_DATABASE_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000001";

/// A private copy of the demo database, which is at SQL schema step 4.
fn copy_demo(folder: &Path) -> PathBuf {
    let path = folder.join("athlete.ath.sqlite");
    std::fs::copy(common::demo_database(), &path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    path
}

fn refusal(text: &str) -> String {
    format!("ath: {text}\n")
}

#[test]
fn discovery_uses_only_the_default_name() {
    let folder = private_folder();
    let cwd = folder.path().display().to_string();
    let session = Session::piped(folder.path());
    let delete = |extra: &[&str]| {
        let mut argv = vec!["delete", RESULT, "--revision", "1"];
        argv.extend_from_slice(extra);
        session.run(&argv)
    };
    let missing = format!("no athlete.ath.sqlite in {cwd} — run `ath init` to create one, or pass --file");

    assert_eq!(delete(&[]).stderr, refusal(&missing));
    std::fs::write(folder.path().join("other.ath.sqlite"), b"").unwrap();
    assert_eq!(delete(&[]).stderr, refusal(&missing), "only athlete.ath.sqlite is discovered");
    assert_eq!(delete(&["--file", ""]).stderr, refusal(&missing), "an empty --file means discovery");

    std::fs::write(folder.path().join("old.ath.json"), b"{}").unwrap();
    assert_eq!(
        delete(&[]).stderr,
        refusal("found old.ath.json, but Athletic Standard prerelease JSON files are not supported by v1. Keep it unchanged and run `ath init` to create athlete.ath.sqlite.")
    );
    std::fs::write(folder.path().join("second.ath.json"), b"{}").unwrap();
    assert_eq!(delete(&[]).stderr, refusal(&missing));

    assert_eq!(
        delete(&["--file", "missing.ath.sqlite"]).stderr,
        refusal(&format!("file not found: {cwd}/missing.ath.sqlite"))
    );
    std::fs::write(folder.path().join("notes.txt"), b"").unwrap();
    assert_eq!(
        delete(&["--file", "./sub/../notes.txt"]).stderr,
        refusal(&format!("the live athlete record must be a .ath.sqlite SQLite file, not {cwd}/notes.txt"))
    );
    assert_eq!(
        delete(&["--file", "old.ath.json"]).stderr,
        refusal(&format!(
            "Athletic Standard prerelease JSON files are not supported by v1. Keep {cwd}/old.ath.json unchanged and run `ath init` to create a v1 SQLite database."
        ))
    );
}

#[test]
fn an_outdated_database_is_refused_without_a_terminal() {
    let folder = private_folder();
    copy_demo(folder.path());
    let expected =
        refusal("Database SQL schema 4 must be upgraded to 5 before other commands can run. Run ath migrate.");
    let piped = Session::piped(folder.path()).run(&["delete", RESULT, "--revision", "1", "--dry-run"]);
    assert_eq!((piped.status, piped.stdout.as_str(), piped.stderr.as_str()), (1, "", expected.as_str()));
    let json = Session::terminal(folder.path(), "y\n").run(&["delete", RESULT, "--revision", "1", "--json"]);
    assert_eq!((json.status, json.stdout.as_str(), json.stderr.as_str()), (1, "", expected.as_str()));
}

/// The first JSON object a run printed, and the text after it.
fn split_preview(stdout: &str) -> (serde_json::Value, &str) {
    let end = stdout.find("\n}\n").expect("a JSON preview") + 3;
    (serde_json::from_str(&stdout[..end]).unwrap(), &stdout[end..])
}

#[test]
fn a_terminal_can_refuse_the_migration() {
    let folder = private_folder();
    let path = copy_demo(folder.path());
    let before = std::fs::read(&path).unwrap();

    let no = Session::terminal(folder.path(), "n\n").run(&["delete", RESULT, "--revision", "1", "--dry-run"]);
    assert_eq!(no.status, 1);
    let (preview, rest) = split_preview(&no.stdout);
    let keys: Vec<&str> = preview.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "file",
            "migration_needed",
            "source_sql_schema_version",
            "target_sql_schema_version",
            "database_id",
            "planned_steps",
            "planned_backup_path",
            "compatibility_effect",
            "write_confirmed"
        ]
    );
    assert_eq!(preview["database_id"], DEMO_DATABASE_ID);
    assert_eq!(preview["write_confirmed"], false);
    assert_eq!(rest, "Create a verified backup and apply this migration? [y] yes  [n] no ");
    assert_eq!(no.stderr, refusal("migration was not confirmed; run ath migrate or answer yes when prompted on open"));

    let wrong = Session::terminal(folder.path(), "yes\n01890f3e\n").run(&["delete", RESULT, "--revision", "1"]);
    assert_eq!(wrong.status, 1);
    assert!(wrong.stdout.ends_with(&format!("Type the database ID {DEMO_DATABASE_ID} to confirm migration: ")));
    assert_eq!(
        wrong.stderr,
        refusal("second confirmation did not exactly match the database ID; nothing was migrated")
    );

    let unconnected = Session::terminal(folder.path(), &format!("y\n{DEMO_DATABASE_ID}\n")).run(&[
        "delete",
        RESULT,
        "--revision",
        "1",
    ]);
    assert_eq!(
        unconnected.stderr,
        refusal("the confirmed migration is not connected to the native library yet; nothing was migrated")
    );
    assert_eq!(std::fs::read(&path).unwrap(), before, "a refused migration leaves the file unchanged");
}

static MIGRATIONS: AtomicUsize = AtomicUsize::new(0);

fn fake_migration(store: &mut Store, preview: &MigrationPreview) -> athleticstandard::Result<PathBuf> {
    MIGRATIONS.fetch_add(1, Ordering::SeqCst);
    assert_eq!(store.metadata().database_id, preview.database_id);
    Ok(PathBuf::from("/verified/backup.sqlite"))
}

#[test]
fn a_confirmed_migration_continues_the_command() {
    let folder = private_folder();
    copy_demo(folder.path());
    let mut session = Session::terminal(folder.path(), &format!("y\n {DEMO_DATABASE_ID} \n"));
    session.services = Services { migrate: fake_migration };
    let output = session.run(&["delete", RESULT, "--revision", "1", "--dry-run"]);
    assert_eq!(output.status, 0, "{}", output.stderr);
    assert_eq!(MIGRATIONS.load(Ordering::SeqCst), 1);
    let (_, rest) = split_preview(&output.stdout);
    let rest = rest
        .strip_prefix(&format!(
            "Create a verified backup and apply this migration? [y] yes  [n] no Type the database ID {DEMO_DATABASE_ID} to confirm migration: migrated SQL schema 4 to 5 with verified backup at /verified/backup.sqlite\n"
        ))
        .expect(&output.stdout);
    let (delete_preview, rest) = split_preview(rest);
    assert_eq!(delete_preview["record_id"], RESULT);
    assert_eq!(rest, "nothing written (--dry-run)\n");
}

#[test]
fn a_read_only_open_does_not_migrate_or_change_the_file() {
    let folder = private_folder();
    let path = copy_demo(folder.path());
    let before = std::fs::read(&path).unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin: &[u8] = b"";
    let mut open_terminal = || None;
    let io = Io {
        stdout: &mut stdout,
        stderr: &mut stderr,
        stdin: &mut stdin,
        terminal: Terminal { stdin: true, stdout: true, ..Terminal::default() },
        open_terminal: &mut open_terminal,
        cwd: folder.path().to_path_buf(),
        env: BTreeMap::new(),
    };
    let mut context = Context { io, services: Services::default() };
    let store = context.open_read_only(&path, false).unwrap();
    assert_eq!(store.metadata().sql_schema_version, 4);
    assert!(store.get_record(RESULT).unwrap().is_some());
    store.close().unwrap();
    drop(context);
    assert!(stdout.is_empty() && stderr.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn init_refuses_questions_without_a_terminal() {
    let folder = private_folder();
    let output = Session::piped(folder.path()).run(&["init"]);
    assert_eq!(
        (output.status, output.stdout.as_str(), output.stderr.as_str()),
        (1, "", "ath: there is no terminal to ask on, so nothing was created. Pass --yes to skip the questions.\n")
    );
    assert!(!folder.path().join("athlete.ath.sqlite").exists());
}

#[test]
fn init_asks_each_missing_profile_question() {
    let folder = private_folder();
    let not_connected = "ath: ath init is not connected to the native library yet; nothing was created\n";
    let all = Session::terminal(folder.path(), "Sam\n1990\n\nAmerica/Chicago\n").run(&["init"]);
    assert_eq!(
        all.stdout,
        "Name (optional): Birth year (optional): Sex, male/female (optional): Default timezone, such as America/Chicago (optional): "
    );
    assert_eq!(all.stderr, not_connected);

    let some = Session::terminal(folder.path(), "1990\n").run(&[
        "init",
        "--name",
        "Sam",
        "--sex",
        "male",
        "--timezone",
        "UTC",
    ]);
    assert_eq!(some.stdout, "Birth year (optional): ");
    assert_eq!(some.stderr, not_connected);

    let ended = Session::terminal(folder.path(), "Sam\n").run(&["init"]);
    assert_eq!(ended.stdout, "Name (optional): Birth year (optional): ");
    assert_eq!(
        ended.stderr,
        "ath: the questions were not answered, so nothing was created. Pass --yes to skip the questions.\n"
    );

    for flags in [&["init", "-y"][..], &["init", "--json"][..]] {
        let output = Session::piped(folder.path()).run(flags);
        assert_eq!((output.stdout.as_str(), output.stderr.as_str()), ("", not_connected));
    }
}

#[test]
fn init_refuses_a_bad_target() {
    let folder = private_folder();
    let cwd = folder.path().display().to_string();
    let session = Session::piped(folder.path());
    let named = session.run(&["init", "-y", "--file", "athlete.sqlite"]);
    assert_eq!(named.stderr, refusal(&format!("the live database name must end in .ath.sqlite: {cwd}/athlete.sqlite")));
    std::fs::write(folder.path().join("athlete.ath.sqlite"), b"").unwrap();
    let existing = session.run(&["init", "-y"]);
    assert_eq!(
        existing.stderr,
        refusal(&format!("{cwd}/athlete.ath.sqlite already exists, and overwriting it would lose whatever is in it. Read it with `ath stats`, or start a second file with `--file <name>.ath.sqlite`."))
    );
}

#[test]
fn bare_ath_prints_the_guide() {
    let folder = private_folder();
    let empty = Session::piped(folder.path()).run(&[]);
    assert_eq!(empty.status, 0);
    assert!(empty.stdout.starts_with(
        "ath — Athletic Standard, an open file format for your training and recovery data.\n\nThere is no .ath.sqlite file in this folder"
    ));
    assert!(empty.stdout.ends_with("The whole list: `ath --help`.\n"));

    std::fs::write(folder.path().join("b.ath.sqlite"), b"").unwrap();
    std::fs::write(folder.path().join("a.ath.sqlite"), b"").unwrap();
    let with_file = Session::piped(folder.path()).run(&[]);
    assert!(with_file
        .stdout
        .starts_with("ath — Athletic Standard. Working on a.ath.sqlite in this folder.\n\n  ath stats "));

    let update = Session::piped(folder.path()).run(&["--update"]);
    assert_eq!(update.status, 1);
    assert_eq!(update.stderr, "ath: ath --update is not connected to the native library yet; nothing was changed\n");
}

/// The smallest words that reach a command's handler.
fn minimal_argv(path: &[&'static str], command: &spec::CommandSpec) -> Vec<String> {
    let mut argv: Vec<String> = path.iter().map(ToString::to_string).collect();
    for argument in command.arguments.iter().filter(|argument| argument.required) {
        argv.push(format!("{}-value", argument.name));
    }
    for option in command.options.iter().filter(|option| option.mandatory) {
        argv.push(option.long().to_string());
        argv.push("1".to_string());
    }
    argv
}

#[test]
fn unconnected_commands_say_so_and_touch_nothing() {
    let folder = private_folder();
    let connected = athleticstandard::cli::commands::CONNECTED;
    let mut checked = Vec::new();
    for (path, command) in spec::all_commands() {
        // `init` runs its checks and questions before its own "not connected" refusal.
        if !command.action || connected.contains(&path.as_slice()) || path == ["init"] {
            continue;
        }
        let argv = minimal_argv(&path, command);
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        let output = Session::piped(folder.path()).run(&words);
        assert_eq!(
            (output.status, output.stdout.as_str(), output.stderr),
            (
                1,
                "",
                format!(
                    "ath: ath {} is not connected to the native library yet; nothing was read or written\n",
                    path.join(" ")
                )
            ),
            "{argv:?}"
        );
        checked.push(path.join(" "));
    }
    assert_eq!(checked.len(), 26, "{checked:?}");
    assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 0);
}
