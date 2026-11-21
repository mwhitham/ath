//! The native `ath` executable compared with what 2.1.0 printed.
//!
//! `tools/reference/cli-golden.ts` ran the 2.1.0 command line on
//! synthetic scenarios and wrote `tests/golden/cli/*.json`. Each scenario
//! names its starting fixture, its setup, and its steps. This test builds the
//! same folder, runs the built executable with pipes for its standard
//! streams, applies the same normalization, and compares the exit status,
//! standard output, and standard error of every step.
//!
//! The normalization rules are listed in each golden file. Two differences
//! are deliberate and applied to the expected side here:
//!
//! - A refusal is printed once (cli-changes.md 1, quirk S4). Where 2.1.0
//!   printed the same `ath:` line twice in a row, one copy is expected.
//! - The native program writes SQL schema step 6 (D211, D227). Steps 5 and
//!   6 are removed from native migration plans, and a source, target, or
//!   required step 5 or 6 reads 4.
//! - `log` lists record data in published field order (deliberate-changes.md
//!   25), so its output lines are compared without their order.
//! - `share` finds the report `backtest` writes (deliberate-changes.md 5).
//! - `conflicts show` without a conflict file says how to make one
//!   (deliberate-changes.md 32).
//! - A keyed import (Garmin, Strava, mapped, pull) advances the database
//!   revision once and matches an unchanged repeat (deliberate-changes.md 42
//!   and 44), so later revision numbers in those scenarios read `<n>`.
//!
//! Model calls go to a local 127.0.0.1 server through ATH_GATEWAY_BASE_URL.
//! A fake `security` and `secret-tool` first on the path stand in for the
//! password store, so the real one is never read.

#![cfg(unix)]

mod common;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use athleticstandard::store::migrate::{execute_confirmed_migration, MigrationOptions};
use athleticstandard::store::{OpenOptions, Store};
use serde_json::Value;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

// ---------------------------------------------------------------------------
// Normalization: the rules `cli-golden.ts` applied to the 2.1.0 output.

struct Window {
    from: i64,
    to: i64,
    ids: HashMap<String, String>,
}

impl Window {
    fn within(&self, millis: Option<i64>) -> bool {
        millis.is_some_and(|millis| millis >= self.from && millis <= self.to)
    }
}

/// Milliseconds since the epoch for `YYYY-MM-DDTHH:MM:SS.sssZ`.
fn iso_millis(text: &str) -> Option<i64> {
    athleticstandard::model::time::parse_utc_timestamp(text)
}

/// Replace each match of a fixed-shape pattern. `shape` uses `9` for a
/// digit and any other byte for itself.
fn replace_shape(text: &str, shape: &str, mut replace: impl FnMut(&str) -> Option<String>) -> String {
    let bytes = text.as_bytes();
    let pattern = shape.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        if index + pattern.len() <= bytes.len() {
            let candidate = &bytes[index..index + pattern.len()];
            let matches = candidate.iter().zip(pattern).all(|(byte, expected)| {
                if *expected == b'9' {
                    byte.is_ascii_digit()
                } else {
                    byte == expected
                }
            });
            if matches {
                let found = &text[index..index + pattern.len()];
                if let Some(replacement) = replace(found) {
                    out.push_str(&replacement);
                    index += pattern.len();
                    continue;
                }
            }
        }
        let character = text[index..].chars().next().unwrap();
        out.push(character);
        index += character.len_utf8();
    }
    out
}

fn is_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

/// A lowercase UUIDv7 at the start of `bytes`.
fn uuid7_at(bytes: &[u8]) -> bool {
    if bytes.len() < 36 {
        return false;
    }
    let dashes = [8, 13, 18, 23];
    for (position, byte) in bytes[..36].iter().enumerate() {
        if dashes.contains(&position) {
            if *byte != b'-' {
                return false;
            }
        } else if !is_hex(*byte) {
            return false;
        }
    }
    bytes[14] == b'7' && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
}

fn normalize(text: &str, work: &str, window: &mut Window) -> String {
    let mut out = text
        .replace(work, "<work>")
        .replace("2.1.0", "<version>")
        .replace(athleticstandard::model::package::ATHLETIC_STANDARD_VERSION, "<version>");
    let current_step = athleticstandard::store::SQL_SCHEMA_VERSION.to_string();
    for step in ["4", current_step.as_str()] {
        // `\b` after the digit: the next byte is not a word character.
        let from = format!("\"sql_schema_version\": {step}");
        let mut result = String::new();
        let mut rest = out.as_str();
        while let Some(position) = rest.find(&from) {
            let after = &rest[position + from.len()..];
            let boundary = after.bytes().next().is_none_or(|byte| !(byte.is_ascii_alphanumeric() || byte == b'_'));
            result.push_str(&rest[..position]);
            result.push_str(if boundary { "\"sql_schema_version\": <step>" } else { &from });
            rest = after;
        }
        result.push_str(rest);
        out = result;
    }
    out = manifest_hashes(&out);
    let window_ref = &*window;
    out = replace_shape(&out, "9999-99-99T99:99:99.999Z", |found| {
        window_ref.within(iso_millis(found)).then(|| "<now>".to_string())
    });
    out = replace_shape(&out, "99999999T999999.999Z", |found| {
        let iso = format!(
            "{}-{}-{}T{}:{}:{}",
            &found[0..4],
            &found[4..6],
            &found[6..8],
            &found[9..11],
            &found[11..13],
            &found[13..]
        );
        window_ref.within(iso_millis(&iso)).then(|| "<now>".to_string())
    });
    out = replace_shape(&out, "9999-99-99T999999999Z", |found| {
        let iso = format!("{}:{}:{}.{}Z", &found[0..13], &found[13..15], &found[15..17], &found[17..20]);
        window_ref.within(iso_millis(&iso)).then(|| "<now>".to_string())
    });
    let bytes = out.as_bytes();
    let mut result = String::with_capacity(out.len());
    let mut index = 0;
    while index < bytes.len() {
        if uuid7_at(&bytes[index..]) {
            let id = &out[index..index + 36];
            let hex: String = id[..8].chars().chain(id[9..13].chars()).collect();
            let millis = i64::from_str_radix(&hex, 16).ok();
            if window.within(millis) {
                let next = window.ids.len() + 1;
                let name = window.ids.entry(id.to_string()).or_insert_with(|| format!("<id-{next}>")).clone();
                result.push_str(&name);
                index += 36;
                continue;
            }
        }
        let character = out[index..].chars().next().unwrap();
        result.push(character);
        index += character.len_utf8();
    }
    result
}

/// Package file hashes in an export manifest: a `"sha256"` line right after a
/// `"file"` line. Package files name the program version, so their hashes
/// differ by design; package bytes are compared in `package_golden.rs`.
fn manifest_hashes(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let after_file = index > 0 && {
            let previous = lines[index - 1].trim_start();
            previous.starts_with("\"file\": \"") && previous.ends_with("\",")
        };
        let hash = trimmed.strip_prefix("\"sha256\": \"").and_then(|rest| rest.get(..65));
        match hash {
            Some(hash) if after_file && hash.ends_with('"') && hash[..64].bytes().all(is_hex) => {
                let indent = &line[..line.len() - trimmed.len()];
                out.push(format!("{indent}\"sha256\": \"<sha256>\"{}", &trimmed[11 + 65..]));
            }
            _ => out.push(line.to_string()),
        }
    }
    out.join("\n")
}

/// The native program writes SQL schema step 6 (2.1.1 added step 5, 2.1.2
/// step 6); 2.1.0 wrote step 4.
fn step_five_as_four(text: &str) -> String {
    let mut out = text.to_string();
    for file in ["migrations/005-version-2-1-1.sql", "migrations/006-version-2-1-2.sql"] {
        let marker = format!("\"migration_file\": \"{file}\"");
        while let Some(position) = out.find(&marker) {
            let object = out[..position].rfind('{').expect("the step object opens");
            let start = out[..object].rfind(',').expect("a later step follows another step");
            let end = position + out[position..].find('}').expect("the step object closes") + 1;
            out.replace_range(start..end, "");
        }
    }
    for step in ["5", "6"] {
        out = out
            .replace(&format!("\"target_sql_schema_version\": {step}"), "\"target_sql_schema_version\": 4")
            .replace(&format!("\"source_sql_schema_version\": {step}"), "\"source_sql_schema_version\": 4")
            .replace(&format!(" to {step} with verified backup"), " to 4 with verified backup")
            .replace(&format!("must be upgraded to {step} before"), "must be upgraded to 4 before");
    }
    out
}

/// The 2.1.1 calculation files, kept in `tests/fixtures/calculations-2-1-1`.
/// A scenario that applies a calculation gets the 2.1.1 copy installed, as a
/// person's unchanged copy would be, so the recorded 2.1.0 output describes the
/// operation that runs (D230). The 2.1.2 methods are tested in
/// `calculate_methods.rs` and `cli_calculate.rs`.
fn install_2_1_1_calculations(scenario: &Value, work: &std::path::Path) {
    let fixtures = repo().join("tests/fixtures/calculations-2-1-1");
    for step in scenario["steps"].as_array().unwrap() {
        let argv = step["argv"].as_array().unwrap();
        if argv.first().and_then(Value::as_str) != Some("apply") {
            continue;
        }
        let name = argv[1].as_str().unwrap().trim_end_matches(".md").rsplit(['/', '\\']).next().unwrap();
        let source = fixtures.join(name).join("SKILL.md");
        if source.exists() {
            let folder = work.join(".agents/skills").join(name);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::copy(&source, folder.join("SKILL.md")).unwrap();
        }
    }
}

/// `apply` output gains the operation name, and a notice for an operation
/// replaced in 2.1.2 (D230). Both are checked in `calculate_golden.rs`; they
/// are removed here so the rest is compared with the 2.1.0 output.
fn without_2_1_2_apply_fields(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("\"operation\": ")
                || trimmed.starts_with("note: ")
                || trimmed.starts_with("\"method_notice\": "))
        })
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        .replace("\"result\": null,\n}", "\"result\": null\n}")
        + if text.ends_with('\n') { "\n" } else { "" }
}

/// Calculation skills added after 2.1.0. `ath init` installs them too; the
/// lines and entries that name them are removed so the rest of the 2.1.0
/// output is compared exactly. They are tested in `skill_examples.rs`.
const SKILLS_ADDED_AFTER_2_1_0: &[&str] = &["hyrox-race-splits"];

fn without_new_skills(text: &str) -> String {
    let names_new = |line: &str| SKILLS_ADDED_AFTER_2_1_0.iter().any(|name| line.contains(&format!("/skills/{name}")));
    if text.trim_start().starts_with('{') {
        let mut value: Value = serde_json::from_str(text).unwrap();
        if let Some(Value::Array(items)) = value.get_mut("skill_installed") {
            items.retain(|item| !names_new(item.as_str().unwrap_or("")));
        }
        return format!("{}\n", serde_json::to_string_pretty(&value).unwrap());
    }
    let kept: Vec<&str> = text.lines().filter(|line| !names_new(line)).collect();
    let mut out = kept.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// `ath skill` and `ath skill install` describe more in 2.1.2: each copy's
/// kind, an edited flag, new skills, and coaching skills that are available
/// (v2.1.2 cli-changes.md, changes 1 and 2). The recorded promises must still
/// hold: every recorded folder is listed with the same state and version for
/// the tool skill, and every folder recorded as written is written again.
fn skill_promises_hold(actual: &str, expected: &str) -> bool {
    if expected.trim_start().starts_with('{') {
        let (Ok(actual), Ok(expected)) =
            (serde_json::from_str::<Value>(actual), serde_json::from_str::<Value>(expected))
        else {
            return false;
        };
        if let Some(copies) = expected.get("copies").and_then(Value::as_array) {
            return copies.iter().all(|copy| {
                actual["copies"].as_array().into_iter().flatten().any(|other| {
                    let tool = copy["path"].as_str().unwrap_or("").ends_with("/athletic-standard");
                    other["path"] == copy["path"]
                        && other["state"] == copy["state"]
                        && (!tool || other["installed"] == copy["installed"])
                })
            });
        }
        let listed = |value: &Value, key: &str| -> Vec<String> {
            value[key].as_array().into_iter().flatten().filter_map(|item| item.as_str().map(str::to_string)).collect()
        };
        let written = listed(&actual, "skill_installed");
        return actual["shipped"] == expected["shipped"]
            && listed(&expected, "skill_installed").iter().all(|path| written.contains(path))
            && listed(&actual, "mappings_installed") == listed(&expected, "mappings_installed");
    }
    let state_of = |line: &str| -> Option<(String, String)> {
        let trimmed = line.trim_start();
        let (place, rest) = trimmed.split_once(' ')?;
        (place.contains("/skills/"))
            .then(|| (place.to_string(), rest.trim().split(' ').next().unwrap_or("").to_string()))
    };
    let actual_states: Vec<(String, String)> = actual.lines().filter_map(state_of).collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    expected.lines().all(|line| match state_of(line) {
        Some(state) => actual_states.contains(&state),
        None if line.starts_with("added ") || line.starts_with("agent skill, version") => actual_lines.contains(&line),
        None => true,
    })
}

/// `log` lists record data in published field order (deliberate-changes.md
/// 25); 2.1.0 used the order its planner built. The lines must be the same,
/// apart from the comma a reordered last field gains or loses.
fn same_lines_in_any_order(actual: &str, expected: &str) -> bool {
    let lines = |text: &str| {
        let mut lines: Vec<String> = text.lines().map(|line| line.trim_end_matches(',').to_string()).collect();
        lines.sort();
        lines
    };
    lines(actual) == lines(expected)
}

/// `ath share` finds the report `ath backtest` writes (deliberate-changes.md
/// 5). 2.1.0 looked for another name and never found one.
fn share_finds_the_report(expected: &str) -> String {
    let report = "<work>/backtest-<now>.json";
    expected
        .replace(
            "Run `ath backtest` first. The shareable part of a report is its summary.",
            &format!("The shareable part of a backtest is the summary in:\n  {report}"),
        )
        .replace("\"latest_report\": null", &format!("\"latest_report\": \"{report}\""))
}

/// Scenarios whose later output shows the revisions a keyed import wrote.
/// The 2.1.0 helper advanced the database revision once per record and
/// wrote a revision of every record on an unchanged repeat; the native import
/// advances it once per import and matches an unchanged repeat
/// (deliberate-changes.md 42 and 44). There, revision numbers in `stats` and
/// `records list` are compared as `<n>`.
const KEYED_REVISION_SCENARIOS: &[&str] =
    &["mapped_file_second_mapping_and_installed_copy", "activity_files", "personal_connection"];

fn keyed_revisions(text: &str) -> String {
    let mut out = Vec::new();
    for line in text.split('\n') {
        if line.starts_with("database revision: ") {
            out.push("database revision: <n>".to_string());
            continue;
        }
        let mut words = line.splitn(2, ' ');
        let first = words.next().unwrap_or("");
        let rest = words.next().unwrap_or("");
        let trimmed = rest.trim_start();
        let number_end = trimmed.find(' ').unwrap_or(trimmed.len());
        let is_record_row = first.starts_with("<id-") || first.len() == 36;
        if is_record_row && number_end > 0 && trimmed[..number_end].bytes().all(|byte| byte.is_ascii_digit()) {
            let padding = &rest[..rest.len() - trimmed.len()];
            out.push(format!("{first} {padding}<n>{}", &trimmed[number_end..]));
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// 2.1.0 printed some refusals twice (S4). The native program prints one.
fn refusal_once(stderr: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    for line in stderr.split_inclusive('\n') {
        if line.starts_with("ath: ") && lines.last() == Some(&line) {
            continue;
        }
        lines.push(line);
    }
    lines.concat()
}

// ---------------------------------------------------------------------------
// The local model server: the same replies `cli-golden.ts` served.

const CATALOG: &str = r#"{"data":[{"id":"test/steady","name":"Steady","reasoning":true},{"id":"test/down","reasoning":true},{"id":"test/embedder","type":"embedding","reasoning":true}]}"#;
const STEADY_REPLY: &str = r#"{"predicted_score":{"duration_s":290},"predicted_range":{"low":{"duration_s":280},"high":{"duration_s":300}},"confidence":"moderate","reasoning":"Anchored on the latest result."}"#;

const PERSONAL_ACTIVITIES: &str = r#"[{"activityId":7001,"activityName":"Invented personal loop","startTimeGMT":"2026-08-09T15:00:00.000Z","duration":600,"activityType":{"typeKey":"running"}},{"summaryOnly":true,"distance":5}]"#;
const PERSONAL_HRV: &str = r#"{"LastNightAvg":41,"weeklyAvg":44,"status":"BALANCED"}"#;

/// The Garmin personal connection's three requests.
fn personal(post: bool, path: &str) -> (u16, String) {
    if post && path.ends_with("/personal-api-key/token") {
        (200, "aaa.bbb.ccc".to_string())
    } else if path.starts_with("/activitylist-service/activities/search/activities") {
        (200, PERSONAL_ACTIVITIES.to_string())
    } else if path == "/hrv-service/hrv/2026-08-09" {
        (200, PERSONAL_HRV.to_string())
    } else {
        (404, r#"{"error":"not found"}"#.to_string())
    }
}

fn serve_model_requests() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
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
            let path = request_line.split(' ').nth(1).unwrap_or("");
            let (status, reply) = if let Some(rest) = path.strip_prefix("/personal") {
                personal(request_line.starts_with("POST "), rest)
            } else if request_line.starts_with("GET /models ") {
                (200, CATALOG.to_string())
            } else if request_line.starts_with("POST /chat/completions ") {
                let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                if request["model"] == "test/steady" {
                    let reply = serde_json::json!({"model": "test/steady", "choices": [{"message": {"content": STEADY_REPLY}}]});
                    (200, reply.to_string())
                } else {
                    (503, r#"{"error":{"message":"the test model is down"}}"#.to_string())
                }
            } else {
                (404, r#"{"error":"not found"}"#.to_string())
            };
            let reason = match status {
                200 => "OK",
                503 => "Service Unavailable",
                _ => "Not Found",
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                reply.len()
            );
        }
    });
    format!("http://{address}")
}

// ---------------------------------------------------------------------------
// Scenarios.

const FAKE_STORE: &str = "#!/bin/sh\necho 'the test password store is read-only' >&2\nexit 44\n";

fn fixture_path(name: &str) -> PathBuf {
    repo().join(match name {
        "demo" => "tests/fixtures/version-2-1-0/athlete.ath.sqlite",
        "v200" => "tests/fixtures/version-2-0-0/database.ath.sqlite",
        "npm100" => "tests/fixtures/npm-100-encoding/athlete.ath.sqlite",
        other => panic!("unknown fixture {other}"),
    })
}

/// The approved synthetic files a scenario can copy.
fn copyable_path(name: &str) -> PathBuf {
    repo().join(match name {
        "wearable_csv" => "tests/fixtures/exports/whoop",
        "package_2_0_0" => "tests/fixtures/version-2-0-0/package",
        "png" => "tests/fixtures/version-2-1-0/attachments/01890f3e-e7d3-7cc0-88c8-00000000000e.png",
        "mapped_summaries" => "tests/fixtures/exports/open-wearables/summaries.json",
        "junction_summaries" => "tests/fixtures/exports/junction/summaries.json",
        "rmssd_mapping" => "tests/fixtures/mappings/last-night-rmssd.md",
        "activity_json" => "tests/fixtures/exports/garmin/park-loop.json",
        "gpx_track" => "tests/fixtures/exports/garmin/namespaced-hr.gpx",
        "fit_file" => "tests/fixtures/exports/garmin/synthetic-position.fit",
        "ride_folder" => "tests/fixtures/exports/strava",
        "ring_csv" => "tests/fixtures/exports/oura",
        other => panic!("unknown copyable files {other}"),
    })
}

fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn private_dir(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn prepare(scenario: &Value, work: &Path, home: &Path) {
    for folder in scenario["dirs"].as_array().into_iter().flatten() {
        std::fs::create_dir_all(work.join(folder.as_str().unwrap())).unwrap();
    }
    for (name, key) in scenario["copy"].as_object().into_iter().flatten() {
        let from = copyable_path(key.as_str().unwrap());
        if from.is_dir() {
            copy_folder(&from, &work.join(name));
        } else {
            std::fs::copy(&from, work.join(name)).unwrap();
        }
    }
    for (name, text) in scenario["write_files"].as_object().into_iter().flatten() {
        std::fs::write(work.join(name), text.as_str().unwrap()).unwrap();
    }
    let fixture = scenario["fixture"].as_str().unwrap();
    if fixture == "none" {
        return;
    }
    let database = work.join("athlete.ath.sqlite");
    std::fs::copy(fixture_path(fixture), &database).unwrap();
    std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o600)).unwrap();
    let attachments = match fixture {
        "demo" => Some("tests/fixtures/version-2-1-0/attachments"),
        "v200" => Some("tests/fixtures/version-2-0-0/package/attachments"),
        _ => None,
    };
    if let Some(attachments) = attachments {
        copy_folder(&repo().join(attachments), &work.join("attachments"));
    }
    if scenario["current"].as_bool() == Some(true) {
        let mut store = Store::open(&database, OpenOptions { read_only: false, allow_outdated_schema: true }).unwrap();
        let options = MigrationOptions { backup_path: Some(home.join("backup.sqlite")), now_millis: None };
        execute_confirmed_migration(&mut store, &options).unwrap();
        store.close().unwrap();
    }
    if let Some(statements) = scenario["sql"].as_array() {
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
        for statement in statements {
            connection.execute_batch(statement.as_str().unwrap()).unwrap();
        }
    }
}

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

fn run(argv: &[String], work: &Path, env: &[(String, String)], stdin: &str) -> Run {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(argv)
        .current_dir(work)
        .env_clear()
        .envs(env.iter().map(|(key, value)| (key.as_str(), value.as_str())))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    Run {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

/// Run every scenario in one golden file. Returns one line per difference.
fn compare_group(group: &str) -> Vec<String> {
    let text = std::fs::read_to_string(repo().join(format!("tests/golden/cli/{group}.json"))).unwrap();
    let document: Value = serde_json::from_str(&text).unwrap();
    let root = common::private_folder();
    let bin = root.path().join("bin");
    private_dir(&bin);
    for tool in ["security", "secret-tool"] {
        std::fs::write(bin.join(tool), FAKE_STORE).unwrap();
        std::fs::set_permissions(bin.join(tool), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let gateway = serve_model_requests();
    let mut differences = Vec::new();
    for scenario in document["scenarios"].as_array().unwrap() {
        let name = scenario["name"].as_str().unwrap();
        let work = root.path().join("work").join(name);
        let home = root.path().join("home").join(name);
        private_dir(&work);
        private_dir(&home);
        let mut window = Window { from: now_millis() - 5_000, to: 0, ids: HashMap::new() };
        prepare(scenario, &work, &home);
        install_2_1_1_calculations(scenario, &work);
        let mut runs = Vec::new();
        for step in scenario["steps"].as_array().unwrap() {
            let argv: Vec<String> = step["argv"]
                .as_array()
                .unwrap()
                .iter()
                .map(|word| word.as_str().unwrap().replace("<server>", &gateway))
                .collect();
            let mut env = vec![
                ("PATH".to_string(), format!("{}:/usr/bin:/bin", bin.display())),
                ("HOME".to_string(), home.display().to_string()),
                ("TMPDIR".to_string(), home.display().to_string()),
                ("TZ".to_string(), "America/Chicago".to_string()),
                ("ATH_GATEWAY_BASE_URL".to_string(), gateway.clone()),
            ];
            for (key, value) in step["env"].as_object().into_iter().flatten() {
                env.push((key.clone(), value.as_str().unwrap().to_string()));
            }
            runs.push((argv.clone(), run(&argv, &work, &env, step["stdin"].as_str().unwrap_or(""))));
        }
        window.to = now_millis() + 5_000;
        let work_text = work.display().to_string();
        for (index, (step, (argv, actual))) in scenario["steps"].as_array().unwrap().iter().zip(runs).enumerate() {
            let mut stdout = step_five_as_four(&normalize(&actual.stdout, &work_text, &mut window));
            if argv.first().map(String::as_str) == Some("apply") {
                stdout = without_2_1_2_apply_fields(&stdout);
            }
            if argv.first().map(String::as_str) == Some("init") {
                stdout = without_new_skills(&stdout);
            }
            if argv.first().map(String::as_str) == Some("calculate") {
                // Receipt version 2 (D244); its additions are tested in calculate_range_v2.rs.
                stdout = stdout.replace(
                    "\"calculation_version\": \"ath-runtime-v2\"",
                    "\"calculation_version\": \"ath-runtime-v1\"",
                );
            }
            let stderr = step_five_as_four(&normalize(&actual.stderr, &work_text, &mut window));
            let expected_status = step["status"].as_i64().unwrap() as i32;
            let mut expected_stdout = step["stdout"].as_str().unwrap().to_string();
            let expected_stderr = refusal_once(step["stderr"].as_str().unwrap());
            if (name, argv.first().map(String::as_str)) == ("backtest", Some("share")) {
                expected_stdout = share_finds_the_report(&expected_stdout);
            }
            let mut expected_stderr = expected_stderr;
            if argv.starts_with(&["conflicts".to_string(), "show".to_string()]) && expected_stderr.contains("ENOENT") {
                // deliberate-changes.md 32: the missing file says how to make one.
                expected_stderr =
                    "ath: no temporary import conflicts; run ath import <package> to create a preview\n".to_string();
            }
            if KEYED_REVISION_SCENARIOS.contains(&name)
                && matches!(argv.first().map(String::as_str), Some("stats" | "records"))
            {
                stdout = keyed_revisions(&stdout);
                expected_stdout = keyed_revisions(&expected_stdout);
            }
            let same_stdout = if argv.first().map(String::as_str) == Some("log") {
                same_lines_in_any_order(&stdout, &expected_stdout)
            } else if argv.first().map(String::as_str) == Some("skill") {
                skill_promises_hold(&stdout, &expected_stdout)
            } else {
                stdout == expected_stdout
            };
            if actual.status != expected_status || !same_stdout || stderr != expected_stderr {
                differences.push(format!(
                    "{group}/{name} step {index} {argv:?}\n  status {} (expected {expected_status})\n--- stdout\n{}\n--- expected stdout\n{}\n--- stderr\n{}\n--- expected stderr\n{}",
                    actual.status,
                    stdout,
                    expected_stdout,
                    stderr,
                    expected_stderr
                ));
            }
        }
    }
    differences
}

fn assert_group(group: &str) {
    let differences = compare_group(group);
    assert!(differences.is_empty(), "{} differences:\n\n{}", differences.len(), differences.join("\n\n"));
}

#[test]
fn set_up_commands_match_2_1_0() {
    assert_group("setup");
}

#[test]
fn read_commands_match_2_1_0() {
    assert_group("read");
}

#[test]
fn model_commands_match_2_1_0() {
    assert_group("model");
}

#[test]
fn write_commands_match_2_1_0() {
    assert_group("write");
}

#[test]
fn package_commands_match_2_1_0() {
    assert_group("package");
}

#[test]
fn keyed_import_routes_and_the_personal_connection_match_2_1_0() {
    assert_group("routes");
}

#[test]
fn normalization_keeps_old_values_and_numbers_new_ones() {
    let now = now_millis();
    let mut window = Window { from: now - 1_000, to: now + 1_000, ids: HashMap::new() };
    let new_id = format!("{:08x}-{:04x}-7000-8000-000000000001", now >> 16, now & 0xffff);
    let text = format!(
        "{new_id} 01890f3e-e7d3-7cc0-88c8-000000000001 {new_id} {} 2026-01-01T00:00:00.000Z /w/x {} \"sql_schema_version\": {}",
        athleticstandard::model::time::format_utc_millis(now),
        athleticstandard::model::package::ATHLETIC_STANDARD_VERSION,
        athleticstandard::store::SQL_SCHEMA_VERSION,
    );
    assert_eq!(
        normalize(&text, "/w", &mut window),
        "<id-1> 01890f3e-e7d3-7cc0-88c8-000000000001 <id-1> <now> 2026-01-01T00:00:00.000Z <work>/x <version> \"sql_schema_version\": <step>"
    );
    assert_eq!(refusal_once("ath: no\nath: no\n"), "ath: no\n");
    let hash = "ab".repeat(32);
    let manifest =
        format!("  {{\n    \"file\": \"a.json\",\n    \"sha256\": \"{hash}\",\n  }}\n  \"sha256\": \"{hash}\"");
    assert_eq!(
        manifest_hashes(&manifest),
        format!("  {{\n    \"file\": \"a.json\",\n    \"sha256\": \"<sha256>\",\n  }}\n  \"sha256\": \"{hash}\"")
    );
}
