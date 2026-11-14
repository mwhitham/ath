//! `ath correct`, `ath delete`, and `ath link`, run in-process: previews,
//! confirmations, writes, and refusals (command-behavior.md S9–S20).

mod common;

use common::{database_revision, fixture, private_folder, record_state, Session};

fn json_lines(lines: &[&str]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

const NO_TERMINAL: &str =
    "ath: there is no terminal to ask on, so nothing was written. Pass --yes after reviewing the preview.\n";

#[test]
fn delete_previews_and_writes_only_after_yes() {
    let folder = private_folder();
    let fixture = fixture(folder.path());
    let id = fixture.benchmark_result.as_str();
    let file = fixture.path.display().to_string();
    let preview = json_lines(&[
        "{",
        &format!("  \"file\": \"{file}\","),
        &format!("  \"record_id\": \"{id}\","),
        "  \"record_type\": \"benchmark_result\",",
        "  \"expected_revision\": 1,",
        "  \"current_revision\": 1,",
        "  \"retained_history\": true,",
        "  \"write_confirmed\": false",
        "}",
    ]);
    let session = Session::piped(folder.path());

    let json = session.run(&["delete", id, "--revision", "1", "--json", "--yes"]);
    assert_eq!((json.status, json.stdout.as_str(), json.stderr.as_str()), (0, preview.as_str(), ""));

    let refused = session.run(&["delete", id, "--revision", "1"]);
    assert_eq!((refused.status, refused.stdout.as_str(), refused.stderr.as_str()), (1, preview.as_str(), NO_TERMINAL));

    let dry = session.run(&["delete", id, "--revision", "1", "--dry-run"]);
    assert_eq!(dry.stdout, format!("{preview}nothing written (--dry-run)\n"));
    assert_eq!(record_state(&fixture.path, id), (1, false));
    assert_eq!(database_revision(&fixture.path), fixture.database_revision);

    let stale = session.run(&["delete", id, "--revision", "2", "--yes"]);
    assert_eq!(stale.status, 1);
    assert_eq!(stale.stderr, format!("ath: Record {id} is at revision 1, not expected revision 2\n"));

    let written = session.run(&["delete", id, "--revision", "1", "--yes"]);
    assert_eq!(written.status, 0, "{}", written.stderr);
    let revision = fixture.database_revision + 1;
    assert_eq!(written.stdout, format!("{preview}deleted {id} at database revision {revision}; history retained\n"));
    assert_eq!(record_state(&fixture.path, id), (1, true));

    let again = session.run(&["delete", id, "--revision", "1", "--yes"]);
    assert_eq!((again.status, again.stderr), (1, format!("ath: current record not found: {id}\n")));
}

#[test]
fn a_record_id_must_be_a_uuid_v7() {
    let folder = private_folder();
    fixture(folder.path());
    let session = Session::piped(folder.path());
    for argv in [
        vec!["delete", "not-a-uuid", "--revision", "1"],
        vec!["correct", "not-a-uuid", "--revision", "1", "--data", "{}"],
        vec!["link", "not-a-uuid", "also-not", "--revision", "1"],
    ] {
        let output = session.run(&argv);
        assert_eq!(
            (output.status, output.stdout.as_str(), output.stderr.as_str()),
            (1, "", "ath: record: lowercase UUIDv7\n")
        );
    }
}

#[test]
fn a_terminal_answer_decides_the_write() {
    let folder = private_folder();
    let fixture = fixture(folder.path());
    let id = fixture.benchmark_result.as_str();

    let no = Session::terminal(folder.path(), "n\n").run(&["delete", id, "--revision", "1"]);
    assert_eq!(no.status, 0);
    assert!(
        no.stdout.ends_with("}\nDelete this record from current views? [y] yes  [n] no nothing written\n"),
        "{}",
        no.stdout
    );

    let silent = Session::terminal(folder.path(), "").run(&["delete", id, "--revision", "1"]);
    assert!(silent.stdout.ends_with("[n] no nothing written\n"));
    assert_eq!(record_state(&fixture.path, id), (1, false));

    // Standard input is a pipe, so the answer comes from the controlling terminal (S19).
    let mut session = Session::piped(folder.path());
    session.terminal.stdout = true;
    session.stdin = b"y\n".to_vec();
    session.controlling_terminal = Some(b" YES \n".to_vec());
    let yes = session.run(&["delete", id, "--revision", "1"]);
    assert_eq!(yes.status, 0, "{}", yes.stderr);
    assert!(yes.stdout.contains("[n] no deleted "), "{}", yes.stdout);
    assert_eq!(record_state(&fixture.path, id), (1, true));
}

#[test]
fn no_controlling_terminal_means_no() {
    let folder = private_folder();
    let fixture = fixture(folder.path());
    let mut session = Session::piped(folder.path());
    session.terminal.stdout = true;
    let output = session.run(&["delete", &fixture.benchmark_result, "--revision", "1"]);
    assert_eq!(output.status, 0);
    assert!(output.stdout.ends_with("nothing written\n"));
}

#[test]
fn correct_previews_then_writes_a_new_revision() {
    let folder = private_folder();
    let fixture = fixture(folder.path());
    let id = fixture.benchmark_definition.as_str();
    let session = Session::piped(folder.path());
    let data = r#"{"authorship":{"author":{"author_type":"athlete"},"authored_at":"2026-08-09T12:00:00.000Z","evidence":[],"content_version":2,"status":"active"},"benchmark_key":"fran","benchmark_type":"named_wod","score_type":"time","definition":"21-15-9 thrusters and pull-ups","tags":["short"]}"#;

    let preview = session.run(&["correct", id, "--revision", "1", "--data", data, "--json"]);
    assert_eq!(preview.status, 0, "{}", preview.stderr);
    let value: serde_json::Value = serde_json::from_str(&preview.stdout).unwrap();
    let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "file",
            "record_id",
            "record_type",
            "expected_revision",
            "current_revision",
            "before",
            "after",
            "attachments",
            "write_confirmed"
        ]
    );
    assert_eq!(value["before"]["definition"], "21-15-9 thrusters and pull-ups for time");
    assert_eq!(value["after"]["definition"], "21-15-9 thrusters and pull-ups");
    assert_eq!(value["attachments"], serde_json::json!([]));
    assert_eq!(value["write_confirmed"], false);
    assert_eq!(record_state(&fixture.path, id), (1, false));

    let written = session.run(&["correct", id, "--revision", "1", "--data", data, "--agent", "coach", "--yes"]);
    assert_eq!(written.status, 0, "{}", written.stderr);
    let revision = fixture.database_revision + 1;
    assert!(written.stdout.ends_with(&format!("corrected {id} to revision 2 (database revision {revision})\n")));
    assert_eq!(record_state(&fixture.path, id), (2, false));

    let store = athleticstandard::store::Store::open(
        &fixture.path,
        athleticstandard::store::OpenOptions { read_only: true, allow_outdated_schema: false },
    )
    .unwrap();
    let revisions = store.revisions(id).unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(athleticstandard::json::encode(&revisions[1].changed_by), r#"{"author_type":"agent","agent":"coach"}"#);
}

#[test]
fn correct_refuses_bad_data_only_when_writing() {
    let folder = private_folder();
    let fixture = fixture(folder.path());
    let id = fixture.benchmark_definition.as_str();
    let session = Session::piped(folder.path());

    let not_json = session.run(&["correct", id, "--revision", "1", "--data", "{x"]);
    assert_eq!(not_json.status, 1);
    assert_eq!(not_json.stdout, "");
    assert!(not_json.stderr.starts_with("ath: --data is not valid JSON: "), "{}", not_json.stderr);
    assert_eq!(not_json.stderr.lines().count(), 1, "printed once");

    let preview = session.run(&["correct", id, "--revision", "1", "--data", "5", "--json"]);
    assert_eq!(preview.status, 0);
    assert!(preview.stdout.contains("\n  \"after\": 5,\n"));

    let write = session.run(&["correct", id, "--revision", "1", "--data", "5", "--yes"]);
    assert_eq!(write.status, 1);
    assert_eq!(write.stderr, "ath: data: Invalid input: expected object, received number\n");

    let stale = session.run(&["correct", id, "--revision", "3", "--data", "{}", "--yes"]);
    assert_eq!(stale.stderr, format!("ath: Record {id} is at revision 1, not expected revision 3\n"));
    assert_eq!(record_state(&fixture.path, id), (1, false));
    assert_eq!(database_revision(&fixture.path), fixture.database_revision);

    let attachment = session.run(&["correct", id, "--revision", "1", "--data", "{}", "--attachment", "a.png"]);
    assert_eq!(attachment.status, 1);
    assert!(attachment.stderr.contains("is not connected to the native library yet"));
}

#[test]
fn link_moves_a_result_to_another_session() {
    let folder = private_folder();
    let fixture = fixture(folder.path());
    let result = fixture.benchmark_result.as_str();
    let other = fixture.other_session.as_str();
    let session = Session::piped(folder.path());

    let preview = session.run(&["link", result, other, "--revision", "1", "--json"]);
    assert_eq!(preview.status, 0, "{}", preview.stderr);
    assert_eq!(
        preview.stdout,
        json_lines(&[
            "{",
            &format!("  \"file\": \"{}\",", fixture.path.display()),
            &format!("  \"record_id\": \"{result}\","),
            "  \"expected_revision\": 1,",
            "  \"current_revision\": 1,",
            &format!("  \"old_workout_session_id\": \"{}\",", fixture.session),
            &format!("  \"new_workout_session_id\": \"{other}\","),
            "  \"write_confirmed\": false",
            "}",
        ])
    );

    let human = session.run(&["link", result, other, "--revision", "1", "--dry-run"]);
    assert_eq!(
        human.stdout,
        format!(
            "result {result} revision 1\n  workout session: {} → {other}\nnothing written (--dry-run)\n",
            fixture.session
        )
    );

    let refused = session.run(&["link", result, other, "--revision", "1"]);
    assert_eq!(refused.stderr, NO_TERMINAL);

    let written = session.run(&["link", result, other, "--revision", "1", "--yes"]);
    assert_eq!(written.status, 0, "{}", written.stderr);
    assert!(written.stdout.ends_with(&format!("linked {result} to {other} at revision 2\n")));
    assert_eq!(record_state(&fixture.path, result), (2, false));

    let wrong = session.run(&["link", other, result, "--revision", "1"]);
    assert_eq!(wrong.stderr, format!("ath: {other} is not a current benchmark result\n"));
    let wrong = session.run(&["link", result, &fixture.benchmark_definition, "--revision", "2"]);
    assert_eq!(wrong.stderr, format!("ath: {} is not a current workout session\n", fixture.benchmark_definition));
}

#[test]
fn readable_paths_are_warned_about_except_in_json() {
    let folder = private_folder();
    let fixture = fixture(folder.path());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fixture.path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let session = Session::piped(folder.path());
        let human = session.run(&["delete", &fixture.benchmark_result, "--revision", "1", "--dry-run"]);
        assert_eq!(
            human.stderr,
            format!(
                "ath: warning: Sensitive athlete data path is readable by other local users: {}\n",
                fixture.path.display()
            )
        );
        let json = session.run(&["delete", &fixture.benchmark_result, "--revision", "1", "--json"]);
        assert_eq!(json.stderr, "");
    }
}
