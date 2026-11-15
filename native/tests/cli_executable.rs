//! The built `ath` executable, run as a separate process with pipes for
//! its standard streams, a temporary working directory, and an empty
//! environment. Prompts that need a terminal are tested in-process
//! (`cli_edit.rs`, `cli_runtime.rs`), where the terminal facts and answers
//! are injected; these tests cover the non-interactive paths.

mod common;

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use common::{fixture, private_folder, Output};

fn ath(cwd: &Path, argv: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(argv)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let output = child.wait_with_output().unwrap();
    Output {
        status: output.status.code().unwrap(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

fn inventory() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../build-history/v2.1.1/appendix/command-inventory.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn probes_match_through_the_executable() {
    let folder = private_folder();
    for probe in inventory()["probes"].as_array().unwrap() {
        let argv: Vec<&str> = probe["argv"].as_array().unwrap().iter().map(|word| word.as_str().unwrap()).collect();
        let expected = Output {
            status: probe["exit_status"].as_i64().unwrap() as i32,
            stdout: probe["stdout"].as_str().unwrap().replace("2.1.0", "2.1.1"),
            stderr: probe["stderr"].as_str().unwrap().replace("2.1.0", "2.1.1"),
        };
        assert_eq!(ath(folder.path(), &argv, b""), expected, "{argv:?}");
    }
}

#[test]
fn help_through_the_executable_matches_the_inventory() {
    let folder = private_folder();
    let inventory = inventory();
    let root = ath(folder.path(), &["--help"], b"");
    assert_eq!(root.stdout, inventory["program"]["help_text"].as_str().unwrap());
    for command in inventory["commands"].as_array().unwrap() {
        let mut argv: Vec<&str> = command["path"].as_str().unwrap().split(' ').collect();
        argv.push("-h");
        let output = ath(folder.path(), &argv, b"");
        assert_eq!((output.status, output.stdout.as_str()), (0, command["help_text"].as_str().unwrap()), "{argv:?}");
    }
}

#[test]
fn a_piped_delete_previews_refuses_and_writes_with_yes() {
    let folder = private_folder();
    let fixture = fixture(folder.path());
    let id = fixture.benchmark_result.as_str();

    let preview = ath(folder.path(), &["delete", id, "--revision", "1", "--json"], b"");
    assert_eq!(preview.status, 0, "{}", preview.stderr);
    let value: serde_json::Value = serde_json::from_str(&preview.stdout).unwrap();
    assert_eq!(value["record_id"], id);
    assert_eq!(value["write_confirmed"], false);

    let refused = ath(folder.path(), &["delete", id, "--revision", "1"], b"y\n");
    assert_eq!(refused.status, 1);
    assert_eq!(refused.stdout, preview.stdout, "the human preview is the same JSON");
    assert_eq!(
        refused.stderr,
        "ath: there is no terminal to ask on, so nothing was written. Pass --yes after reviewing the preview.\n"
    );
    assert_eq!(common::record_state(&fixture.path, id), (1, false));

    let written = ath(folder.path(), &["delete", id, "--revision", "1", "--yes"], b"");
    assert_eq!(written.status, 0, "{}", written.stderr);
    assert!(written.stdout.ends_with("; history retained\n"));
    assert_eq!(common::record_state(&fixture.path, id), (1, true));
}

#[test]
fn version_guide_and_refusals_through_the_executable() {
    let folder = private_folder();
    assert_eq!(ath(folder.path(), &["-V"], b"").stdout, "2.1.1\n");
    let guide = ath(folder.path(), &[], b"");
    assert_eq!(guide.status, 0);
    assert!(guide.stdout.contains("ath init                       make the file, here\n"));

    let init = ath(folder.path(), &["init"], b"Sam\n");
    assert_eq!(
        (init.status, init.stdout.as_str(), init.stderr.as_str()),
        (1, "", "ath: there is no terminal to ask on, so nothing was created. Pass --yes to skip the questions.\n")
    );

    let stats = ath(folder.path(), &["stats"], b"");
    assert_eq!(
        (stats.status, stats.stderr),
        (
            1,
            format!(
                "ath: no athlete.ath.sqlite in {} — run `ath init` to create one, or pass --file\n",
                folder.path().display()
            )
        )
    );

    let limit = ath(folder.path(), &["records", "list", "--limit", "0x0"], b"");
    assert_eq!((limit.status, limit.stderr.as_str()), (1, "ath: expected a positive whole number, not '0x0'\n"));
}
