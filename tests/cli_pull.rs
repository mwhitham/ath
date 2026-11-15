//! `ath pull garmin` when Garmin answers with an HTTP error, run through the
//! built executable against a server on 127.0.0.1. The pull names the request
//! that failed and its status, never prints the key, and writes nothing.
//! No 2.1.0 test covered this.

mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

use common::private_folder;

const KEY: &str = "test-personal-key";
const TOKEN_PATH: &str = "/personalapitoken-service/personal-api/personal-api-key/token";
const ACTIVITIES_PATH: &str = "/activitylist-service/activities/search/activities?start=0&limit=20";

/// A server that answers every request by its path with `respond`, and
/// returns its root URL.
fn serve(respond: fn(&str) -> (u16, &'static str)) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => request.extend_from_slice(&buffer[..read]),
                }
            }
            let request = String::from_utf8_lossy(&request);
            let path = request.split_whitespace().nth(1).unwrap_or("").to_string();
            let (status, body) = respond(&path);
            let reason = if status == 200 { "OK" } else { "Error" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    root
}

fn ath(cwd: &Path, argv: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(argv)
        .current_dir(cwd)
        .env_clear()
        .env("GARMIN_PERSONAL_API_KEY", KEY)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// The record count and database revision.
fn state(folder: &Path) -> (i64, i64) {
    let connection = rusqlite::Connection::open(folder.join("athlete.ath.sqlite")).unwrap();
    connection
        .query_row("SELECT (SELECT COUNT(*) FROM records), (SELECT current_revision FROM database_state)", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap()
}

fn assert_refused(respond: fn(&str) -> (u16, &'static str), expected: impl Fn(&str) -> String) {
    let folder = private_folder();
    let (status, _, stderr) = ath(folder.path(), &["init", "-y", "--no-skill"]);
    assert_eq!(status, 0, "{stderr}");
    let before = state(folder.path());
    let root = serve(respond);
    let (status, stdout, stderr) =
        ath(folder.path(), &["pull", "garmin", "--date", "2026-08-09", "--base-url", &root, "--json"]);
    assert_eq!((status, stdout.as_str(), stderr.as_str()), (1, "", expected(&root).as_str()));
    assert!(!stderr.contains(KEY));
    assert_eq!(state(folder.path()), before, "a failed pull wrote to the database");
}

#[test]
fn a_refused_token_request_names_the_status_and_writes_nothing() {
    assert_refused(
        |_| (401, "invalid key"),
        |root| format!("ath: Garmin token request failed: {root}{TOKEN_PATH}: status code 401\n"),
    );
}

#[test]
fn a_server_error_on_the_activity_list_names_the_request_and_writes_nothing() {
    assert_refused(
        |path| if path == TOKEN_PATH { (200, "aaa.bbb.ccc") } else { (500, "server error") },
        |root| format!("ath: Garmin request {ACTIVITIES_PATH} failed: {root}{ACTIVITIES_PATH}: status code 500\n"),
    );
}
