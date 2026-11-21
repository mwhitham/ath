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
    ath_in(cwd, argv, stdin, None)
}

/// Run `ath` with only a temporary folder in its environment, when one is given.
fn ath_in(cwd: &Path, argv: &[&str], stdin: &[u8], temporary: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ath"));
    command.args(argv).current_dir(cwd).env_clear();
    if let Some(temporary) = temporary {
        command.env("TMPDIR", temporary).env("TEMP", temporary).env("TMP", temporary);
    }
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let output = child.wait_with_output().unwrap();
    Output {
        status: output.status.code().unwrap(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

fn inventory() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("build-history/v2.1.1/appendix/command-inventory.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn probes_match_through_the_executable() {
    let folder = private_folder();
    for probe in inventory()["probes"].as_array().unwrap() {
        let argv: Vec<&str> = probe["argv"].as_array().unwrap().iter().map(|word| word.as_str().unwrap()).collect();
        let expected = Output {
            status: probe["exit_status"].as_i64().unwrap() as i32,
            stdout: probe["stdout"]
                .as_str()
                .unwrap()
                .replace("2.1.0", athleticstandard::model::package::ATHLETIC_STANDARD_VERSION),
            stderr: probe["stderr"]
                .as_str()
                .unwrap()
                .replace("2.1.0", athleticstandard::model::package::ATHLETIC_STANDARD_VERSION),
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
        if argv[0] == "skill" {
            // 2.1.2 changes the skill help (v2.1.2 cli-changes.md, change 1);
            // tests/cli_inventory.rs checks its new words.
            assert_eq!(output.status, 0, "{argv:?}");
            assert!(output.stdout.contains("hyrox"), "{argv:?}: {}", output.stdout);
            continue;
        }
        // The `sqlite3` sentence left the migrate help (cli-changes.md, change 6).
        let expected = command["help_text"].as_str().unwrap().replace(
            "\n\nRequires the `sqlite3` command-line tool on PATH for verified backups (same as\n`ath repair` when repair candidates exist).",
            "",
        );
        assert_eq!((output.status, output.stdout.as_str()), (0, expected.as_str()), "{argv:?}");
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
    assert_eq!(
        ath(folder.path(), &["-V"], b"").stdout,
        format!("{}\n", athleticstandard::model::package::ATHLETIC_STANDARD_VERSION)
    );
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

#[test]
fn an_apple_zip_with_a_route_file_imports_through_the_executable() {
    let folder = private_folder();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exports/apple-file-reference");
    let zip_path = folder.path().join("export.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
    for (name, file) in [
        ("apple_health_export/export.xml", "export.xml"),
        ("apple_health_export/workout-routes/route_2026-06-14.gpx", "workout-routes/route_2026-06-14.gpx"),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(&std::fs::read(fixture.join(file)).unwrap()).unwrap();
    }
    zip.finish().unwrap();

    let created = ath(folder.path(), &["init", "-y", "--empty", "--no-skill"], b"");
    assert_eq!(created.status, 0, "{created:?}");
    // The import stages its candidates in a private temporary file.
    let temporary = private_folder();
    let imported = ath_in(folder.path(), &["import", "export.zip", "--yes"], b"", Some(temporary.path()));
    assert_eq!(imported.status, 0, "{imported:?}");
    assert!(imported.stdout.contains("Apple Health export preview"), "{imported:?}");
    assert!(imported.stdout.contains("route points stored: 2"), "{imported:?}");
    assert!(imported.stdout.contains("GPX route points we could not store"), "{imported:?}");
    assert!(!imported.stdout.contains("not a training or recovery file"), "{imported:?}");
    assert!(!imported.stdout.contains("had no location points"), "{imported:?}");

    let listed = ath(folder.path(), &["records", "list", "--json", "--limit", "20"], b"");
    assert_eq!(listed.status, 0, "{listed:?}");
    let body: serde_json::Value = serde_json::from_str(&listed.stdout).unwrap();
    let records = body["records"].as_array().unwrap();
    assert!(records.iter().any(
        |record| record["record_type"] == "point_measurement" && record["data"]["quantity"] == "resting_heart_rate"
    ));
    let workout = records.iter().find(|record| record["record_type"] == "workout_session").unwrap();
    let route = workout["data"]["route"].as_array().unwrap();
    assert_eq!(route.len(), 2);
    assert_eq!(route[0]["recorded_at"], "2026-06-14T13:01:00.000Z");
    assert_eq!(route[0]["latitude_deg"], 13.5);
    assert_eq!(route[1]["longitude_deg"], 57.75);
    assert!(route.iter().all(|point| point["input_timestamp"]["utc_offset_text"] == "-05:00"));
    let supplied = &workout["data"]["supplied_fields"]["WorkoutRoute"][0];
    assert_eq!(supplied["FileReference"], "/workout-routes/route_2026-06-14.gpx");
    assert_eq!(supplied["MetadataEntry"][0]["key"], "HKMetadataKeySyncVersion");

    let checked = ath(folder.path(), &["check"], b"");
    assert_eq!(checked.status, 0, "{checked:?}");
    assert!(checked.stdout.contains("valid"), "{checked:?}");
}

#[test]
fn refusals_name_the_remedy_for_an_empty_log_and_an_unknown_import() {
    let folder = private_folder();
    fixture(folder.path());
    let empty = ath(folder.path(), &["log", "--dry-run"], b"");
    assert_eq!(empty.status, 1, "{empty:?}");
    assert!(empty.stderr.contains("nothing to log"), "{empty:?}");

    std::fs::write(folder.path().join("nonsense.txt"), "this is not an export\n").unwrap();
    let import = ath(folder.path(), &["import", "nonsense.txt"], b"");
    assert_eq!(import.status, 1, "{import:?}");
    assert!(import.stderr.contains("is not a supported export"), "{import:?}");
    assert!(import.stderr.contains("nonsense.txt"), "{import:?}");
}

#[test]
fn validating_an_export_leaves_nothing_in_the_temporary_folder() {
    let folder = private_folder();
    let fixture = fixture(folder.path());
    let temporary = private_folder();
    let database = fixture.path.to_str().unwrap();
    let output =
        ath_in(folder.path(), &["check", "--validate-export", "--file", database], b"", Some(temporary.path()));
    assert_eq!(output.status, 0, "{output:?}");
    let left: Vec<_> = std::fs::read_dir(temporary.path()).unwrap().map(|entry| entry.unwrap().path()).collect();
    assert!(left.is_empty(), "the export check left {left:?}");
}

/// The names of `-wal`, `-shm`, and `-journal` files left in `folder`.
fn sidecars(folder: &Path) -> Vec<String> {
    std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with("-wal") || name.ends_with("-shm") || name.ends_with("-journal"))
        .collect()
}

/// A copy of the 2.1.1 demo database and its attachment in `folder`.
fn demo_copy(folder: &Path) -> std::path::PathBuf {
    let demo = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/demo-athlete");
    let path = folder.join("athlete.ath.sqlite");
    std::fs::copy(demo.join("athlete.ath.sqlite"), &path).unwrap();
    athleticstandard::files::set_owner_only(&path).unwrap();
    athleticstandard::files::create_private_dir(&folder.join("attachments")).unwrap();
    for entry in std::fs::read_dir(demo.join("attachments")).unwrap() {
        let entry = entry.unwrap();
        let copy = folder.join("attachments").join(entry.file_name());
        std::fs::copy(entry.path(), &copy).unwrap();
        athleticstandard::files::set_owner_only(&copy).unwrap();
    }
    path
}

fn use_delete_journal_mode(path: &Path) {
    let connection = rusqlite::Connection::open(path).unwrap();
    let mode: String = connection.query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0)).unwrap();
    assert_eq!(mode, "delete");
}

#[test]
fn refusals_leave_no_sidecars_and_reads_keep_a_rollback_journal_database_unchanged() {
    const PROFILE: &str = "01890f3e-e7d3-7cc0-88c8-000000000002";
    let folder = private_folder();
    let path = demo_copy(folder.path());
    for argv in [&["log", "felt", "good"][..], &["erase", PROFILE, "--revision", "1"]] {
        let before = std::fs::read(&path).unwrap();
        let output = ath(folder.path(), argv, b"");
        assert_eq!(output.status, 1, "{argv:?}: {output:?}");
        assert_eq!(sidecars(folder.path()), Vec::<String>::new(), "{argv:?}");
        assert_eq!(std::fs::read(&path).unwrap(), before, "{argv:?} changed the database");
    }

    use_delete_journal_mode(&path);
    let before = std::fs::read(&path).unwrap();
    let reads = [&["stats"][..], &["check"], &["records", "list"], &["conflicts", "show"], &["repair", "--dry-run"]];
    for argv in reads.into_iter().chain([&["log", "felt", "good"][..]]) {
        ath(folder.path(), argv, b"");
        assert_eq!(sidecars(folder.path()), Vec::<String>::new(), "{argv:?}");
        assert_eq!(std::fs::read(&path).unwrap(), before, "{argv:?} changed a rollback-journal database");
    }
}

#[test]
fn a_repair_from_revisions_restores_the_values_a_calculation_reads() {
    const DATABASE_ID: &str = "01890f3e-e7d3-7cc0-88c8-000000000001";
    const POINT: &str = "01890f3e-e7d3-7cc0-88c8-000000000007";
    let folder = private_folder();
    let path = demo_copy(folder.path());
    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute("UPDATE point_measurements SET standard_value = 999 WHERE record_id = ?1", [POINT]).unwrap();
    }
    let baseline = |folder: &Path| {
        let argv = ["calculate", "baseline", "hrv_rmssd", "--from", "2026-08-01", "--to", "2026-08-31", "--json"];
        let output = ath(folder, &argv, b"");
        assert_eq!(output.status, 0, "{output:?}");
        output.stdout
    };
    assert!(baseline(folder.path()).contains("999"), "the damaged typed column is what the calculation reads");
    assert_eq!(ath(folder.path(), &["check"], b"").status, 1, "check reports the damaged column");

    let temporary = private_folder();
    let argv = ["repair", "-y", "--from-revision", "--confirm-revision-align", DATABASE_ID];
    let repaired = ath_in(folder.path(), &argv, b"", Some(temporary.path()));
    assert_eq!(repaired.status, 0, "{repaired:?}");
    let checked = ath(folder.path(), &["check"], b"");
    assert_eq!(checked.status, 0, "{checked:?}");
    let after = baseline(folder.path());
    assert!(!after.contains("999") && after.contains("64"), "{after}");
}

#[test]
fn an_outdated_or_mismatched_database_is_refused_without_sidecars() {
    let npm100 = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/npm-100-encoding/athlete.ath.sqlite");
    for mismatch in [false, true] {
        let folder = private_folder();
        let path = folder.path().join("athlete.ath.sqlite");
        std::fs::copy(&npm100, &path).unwrap();
        athleticstandard::files::set_owner_only(&path).unwrap();
        use_delete_journal_mode(&path);
        if mismatch {
            let connection = rusqlite::Connection::open(&path).unwrap();
            connection.execute("UPDATE database_metadata SET sql_schema_version = 2", []).unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        let output = ath(folder.path(), &["log", "-y", "felt", "good"], b"");
        assert_eq!(output.status, 1, "{output:?}");
        assert_eq!(sidecars(folder.path()), Vec::<String>::new(), "mismatch {mismatch}");
        assert_eq!(std::fs::read(&path).unwrap(), before, "mismatch {mismatch}");
    }
}

#[cfg(unix)]
#[test]
fn a_read_only_database_file_is_refused_without_sidecars() {
    use std::os::unix::fs::PermissionsExt;
    let folder = private_folder();
    let path = demo_copy(folder.path());
    use_delete_journal_mode(&path);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
    let before = std::fs::read(&path).unwrap();
    let output = ath(folder.path(), &["log", "-y", "felt", "good"], b"");
    assert_eq!(output.status, 1, "{output:?}");
    assert_eq!(sidecars(folder.path()), Vec::<String>::new());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn a_corrupt_database_is_refused_without_sidecars_or_changes() {
    let folder = private_folder();
    let path = folder.path().join("athlete.ath.sqlite");
    std::fs::write(&path, "not a sqlite database").unwrap();
    for argv in [&["check"][..], &["stats"], &["records", "list"], &["log", "sore", "quads", "4/5", "--yes"]] {
        let output = ath(folder.path(), argv, b"");
        assert_eq!(output.status, 1, "{argv:?}: {output:?}");
        assert!(output.stderr.starts_with("ath: "), "{argv:?}: {output:?}");
        assert_eq!(std::fs::read(&path).unwrap(), b"not a sqlite database", "{argv:?} changed the file");
        for suffix in ["-wal", "-shm", "-journal"] {
            let sidecar = folder.path().join(format!("athlete.ath.sqlite{suffix}"));
            assert!(!sidecar.exists(), "{argv:?} left {}", sidecar.display());
        }
    }
}

/// Text a progress bar draws: a carriage return, a terminal escape, or the
/// bar's blocks.
fn has_progress_text(text: &str) -> bool {
    text.contains('\r') || text.contains('\u{1b}') || text.contains('█') || text.contains('░')
}

/// Progress goes to standard error, and only when standard error is a
/// terminal. Standard output holds the result alone, so `--json` output is
/// one JSON document. The long commands run here through the executable,
/// with pipes for every stream.
#[test]
fn progress_never_reaches_standard_output() {
    let folder = private_folder();
    let created = ath(folder.path(), &["init", "-y", "--empty", "--no-skill"], b"");
    assert_eq!(created.status, 0, "{created:?}");
    let whoop = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exports/whoop");
    let whoop = whoop.to_str().unwrap();
    let temporary = private_folder();
    let runs: [&[&str]; 8] = [
        &["import", whoop, "--json"],
        &["import", whoop, "--yes"],
        &["import", whoop],
        &["check", "--json"],
        &["check", "--validate-export", "--json"],
        &["stats", "--json"],
        &["export", "package", "--json"],
        &["records", "list", "--json", "--limit", "500"],
    ];
    for argv in runs {
        let output = ath_in(folder.path(), argv, b"", Some(temporary.path()));
        assert_eq!(output.status, 0, "{argv:?}: {output:?}");
        assert!(!has_progress_text(&output.stdout), "{argv:?} stdout: {:?}", output.stdout);
        assert!(!has_progress_text(&output.stderr), "{argv:?} stderr: {:?}", output.stderr);
        if argv.contains(&"--json") {
            assert_eq!(output.stderr, "", "{argv:?}");
            let parsed = serde_json::from_str::<serde_json::Value>(&output.stdout);
            assert!(parsed.is_ok(), "{argv:?} printed more than one JSON document: {:?}", output.stdout);
        }
    }

    // In process, standard error can claim to be a terminal while standard
    // output is a pipe. That is the one case where a bar draws, and it must
    // still stay off standard output.
    let mut session = common::Session::piped(folder.path());
    session.terminal.stderr = true;
    session.terminal.stderr_columns = Some(80);
    for argv in [&["import", whoop, "--json"][..], &["check", "--validate-export", "--json"]] {
        let output = session.run(argv);
        assert_eq!(output.status, 0, "{argv:?}: {output:?}");
        assert!(!has_progress_text(&output.stdout), "{argv:?} stdout: {:?}", output.stdout);
        assert!(serde_json::from_str::<serde_json::Value>(&output.stdout).is_ok(), "{argv:?}: {:?}", output.stdout);
    }
}

#[test]
fn a_tool_skill_left_by_an_older_ath_is_noted_until_ath_skill_install() {
    // D253: an ath from 0.x or 1.x cannot refresh its skill when it updates
    // itself, so 2.1.2 says so on standard error, even on --json runs.
    let folder = private_folder();
    demo_copy(folder.path());
    let skill = folder.path().join(".agents/skills/athletic-standard/SKILL.md");
    std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
    std::fs::write(
        &skill,
        "---\nname: athletic-standard\nmetadata:\n  version: \"1.1.0\"\n---\n\n# Old instructions\n",
    )
    .unwrap();
    let shipped = athleticstandard::assets::skill::shipped_skill_version();
    let notice = format!(
        "ath: the agent skill in .agents/skills is from 1.1.0, older than this ath ({shipped}); run ath skill install to update it\n"
    );

    let stats = ath(folder.path(), &["stats", "--json"], b"");
    assert_eq!((stats.status, stats.stderr.as_str()), (0, notice.as_str()));
    assert!(serde_json::from_str::<serde_json::Value>(&stats.stdout).is_ok(), "{}", stats.stdout);

    let version = ath(folder.path(), &["--version"], b"");
    assert_eq!((version.status, version.stderr.as_str()), (0, ""));
    let status = ath(folder.path(), &["skill"], b"");
    assert!(!status.stderr.contains("run ath skill install to update it"), "{}", status.stderr);

    let install = ath(folder.path(), &["skill", "install"], b"");
    assert_eq!(install.status, 0, "{}", install.stderr);
    assert!(!install.stderr.contains("older than this ath"), "{}", install.stderr);
    let after = ath(folder.path(), &["stats", "--json"], b"");
    assert_eq!((after.status, after.stderr.as_str()), (0, ""));
}
