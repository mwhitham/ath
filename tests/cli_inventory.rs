//! The command line matches the 2.1.0 inventory: every command's help text,
//! byte for byte, and every parse-only probe's stdout, stderr, and exit
//! status. The expected differences are the version and the changes listed in
//! the 2.1.1 and 2.1.2 `cli-changes.md` appendices.

use std::collections::BTreeMap;
use std::path::Path;

use athleticstandard::cli::{self, Io, Services, Terminal};
use serde_json::Value;

const OLD_VERSION: &str = "2.1.0";
const NEW_VERSION: &str = athleticstandard::model::package::ATHLETIC_STANDARD_VERSION;

/// The 2.1.0 `migrate` help said the backup needed a `sqlite3` program. The
/// native backup uses SQLite's own interface, so the sentence is gone
/// (`build-history/v2.1.1/appendix/cli-changes.md`, change 6).
const SQLITE3_SENTENCE: &str = "\n\nRequires the `sqlite3` command-line tool on PATH for verified backups (same as\n`ath repair` when repair candidates exist).";

fn inventory() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("build-history/v2.1.1/appendix/command-inventory.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Run `ath` in-process with no terminal, empty input, and an empty folder.
fn run(argv: &[String]) -> (i32, String, String) {
    let folder = tempfile::tempdir().unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin: &[u8] = b"";
    let mut open_terminal = || None;
    let io = Io {
        stdout: &mut stdout,
        stderr: &mut stderr,
        stdin: &mut stdin,
        terminal: Terminal::default(),
        open_terminal: &mut open_terminal,
        cwd: folder.path().to_path_buf(),
        env: BTreeMap::new(),
    };
    let status = cli::run(argv, io, Services::default());
    (status, String::from_utf8(stdout).unwrap(), String::from_utf8(stderr).unwrap())
}

/// Help texts that 2.1.2 changes on purpose, with the words each must hold.
fn changed_in_2_1_2(path: &[String]) -> Option<&'static [&'static str]> {
    match path.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["skill"] => Some(&[
            "Usage: ath skill [options] [command]",
            "$ ath skill install hyrox",
            "it is installed only when you\nname it",
            "One you edited is kept, and ath skill marks it edited.",
        ]),
        ["skill", "install"] => {
            Some(&["Usage: ath skill install [options] [name...]", "a coaching skill: crossfit or\n"])
        }
        _ => None,
    }
}

fn words(value: &Value) -> Vec<String> {
    value.as_array().unwrap().iter().map(|word| word.as_str().unwrap().to_string()).collect()
}

#[test]
fn every_help_text_matches_the_inventory() {
    let inventory = inventory();
    let mut cases = vec![(Vec::new(), inventory["program"]["help_text"].as_str().unwrap().to_string())];
    for command in inventory["commands"].as_array().unwrap() {
        let path: Vec<String> = command["path"].as_str().unwrap().split(' ').map(ToString::to_string).collect();
        cases.push((path, command["help_text"].as_str().unwrap().to_string()));
    }
    assert_eq!(cases.len(), 33, "the root and 32 commands");
    let mut failures = Vec::new();
    for (path, expected) in cases {
        let mut argv = path.clone();
        argv.push("--help".to_string());
        let (status, stdout, stderr) = run(&argv);
        let expected = expected.replace(OLD_VERSION, NEW_VERSION).replace(SQLITE3_SENTENCE, "");
        if let Some(required) = changed_in_2_1_2(&path) {
            // build-history/v2.1.2/appendix/cli-changes.md, change 1: the full
            // text is checked by tests/generated.rs through references/cli.md.
            let missing: Vec<&&str> = required.iter().filter(|part| !stdout.contains(**part)).collect();
            if status != 0 || !missing.is_empty() || !stderr.is_empty() {
                failures.push(format!("ath {} --help lacks {missing:?}:\n{stdout}", path.join(" ")));
            }
            continue;
        }
        if status != 0 || stdout != expected || !stderr.is_empty() {
            failures.push(format!(
                "ath {} --help: status {status}, stderr {stderr:?}\n--- expected\n{expected}\n--- got\n{stdout}",
                path.join(" ")
            ));
        }
    }
    assert!(failures.is_empty(), "{} help texts differ:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn every_probe_matches_the_inventory() {
    let inventory = inventory();
    let probes = inventory["probes"].as_array().unwrap();
    assert_eq!(probes.len(), 31);
    let mut failures = Vec::new();
    for probe in probes {
        let argv = words(&probe["argv"]);
        let expected_status = probe["exit_status"].as_i64().unwrap() as i32;
        let expected_stdout = probe["stdout"].as_str().unwrap().replace(OLD_VERSION, NEW_VERSION);
        let expected_stderr = probe["stderr"].as_str().unwrap().replace(OLD_VERSION, NEW_VERSION);
        let (status, stdout, stderr) = run(&argv);
        if status != expected_status || stdout != expected_stdout || stderr != expected_stderr {
            failures.push(format!(
                "{argv:?}: expected {expected_status} {expected_stdout:?} {expected_stderr:?}, got {status} {stdout:?} {stderr:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{} probes differ:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn version_is_the_package_version() {
    assert_eq!(run(&["--version".to_string()]), (0, format!("{NEW_VERSION}\n"), String::new()));
    assert_eq!(run(&["-V".to_string()]), (0, format!("{NEW_VERSION}\n"), String::new()));
}

#[test]
fn help_follows_a_terminal_width() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin: &[u8] = b"";
    let mut open_terminal = || None;
    let io = Io {
        stdout: &mut stdout,
        stderr: &mut stderr,
        stdin: &mut stdin,
        terminal: Terminal { stdout: true, stdout_columns: Some(120), ..Terminal::default() },
        open_terminal: &mut open_terminal,
        cwd: std::env::temp_dir(),
        env: BTreeMap::new(),
    };
    assert_eq!(cli::run(&["init".to_string(), "--help".to_string()], io, Services::default()), 0);
    let text = String::from_utf8(stdout).unwrap();
    assert!(text.contains(
        "  --units <units>      metric | imperial — display only; stored values are always metric (default: \"metric\")\n"
    ));
}
