//! Timings at a larger size (spec 2.1.3 section 9). Ignored by default; run with
//! `ATH_SCALE_RECORDS=100000 cargo test --release -p athleticstandard-host --test scale -- --ignored --nocapture`.
//! The workload is invented athlete reports only, not the project's 100,000- and
//! 3-million-record performance workloads, which need the 2.1.0 generator.

mod support;

use std::time::Instant;

use serde_json::json;
use support::*;

fn run(host: &Host, label: &str, records: usize, package: &[u8]) {
    let owner = token(&format!("scale-{label}"), ATHLETE_APP);
    let created = call("POST", &format!("{}/v1/athletes", host.url), &owner, Some(json!({})), None);
    let id = created.json["athlete_id"].as_str().unwrap().to_string();
    let url = |rest: &str| format!("{}/v1/athletes/{id}{rest}", host.url);

    let begun = Instant::now();
    assert_eq!(send_bytes("PUT", &url("/package"), &owner, package).status, 200);
    let first_transfer = begun.elapsed();

    let begun = Instant::now();
    let exported = call("GET", &url("/export"), &owner, None, None);
    assert_eq!(exported.status, 200);
    let export = begun.elapsed();
    let revision = runtime().block_on(host.state.storage.read(&id, |store| store.database_revision())).unwrap();

    let begun = Instant::now();
    let unchanged = call("GET", &url(&format!("/changes?since={revision}")), &owner, None, None);
    assert_eq!(unchanged.json["changes"].as_array().unwrap().len(), 0);
    let unchanged_refresh = begun.elapsed();

    let begun = Instant::now();
    let note = call(
        "POST",
        &url("/records"),
        &owner,
        Some(json!({ "record_type": "athlete_report", "data": report("One more.") })),
        Some("one-more"),
    );
    assert_eq!(note.status, 201, "{}", note.json);
    let one_write = begun.elapsed();
    let begun = Instant::now();
    let one = call("GET", &url(&format!("/changes?since={revision}")), &owner, None, None);
    assert_eq!(one.json["changes"].as_array().unwrap().len(), 1);
    let one_note_refresh = begun.elapsed();

    let backups = temporary("ath-host-scale-backup-");
    let begun = Instant::now();
    let backed_up = runtime().block_on(ath_host::backup::backup_all(&host.state, &backups)).unwrap();
    let backup = begun.elapsed();
    let mine = backed_up.iter().find(|entry| entry.athlete_id == id).unwrap();
    let restore = if host.state.storage.is_remote() {
        None
    } else {
        let zip = std::fs::read(&mine.file).unwrap();
        let begun = Instant::now();
        runtime().block_on(ath_host::backup::restore_athlete(&host.state, &id, &zip)).unwrap();
        Some(begun.elapsed())
    };

    println!(
        "{label}: {records} records, package {} bytes, export {} bytes\n  first transfer {first_transfer:?}\n  export {export:?}\n  unchanged refresh {unchanged_refresh:?}\n  one write {one_write:?}\n  one-note refresh {one_note_refresh:?}\n  backup (with trial restore) {backup:?}\n  one-athlete restore {restore:?}",
        package.len(),
        exported.bytes.len(),
    );
}

#[test]
#[ignore = "slow: run by hand for release evidence"]
fn timings_at_a_larger_size() {
    let records: usize =
        std::env::var("ATH_SCALE_RECORDS").ok().and_then(|value| value.parse().ok()).unwrap_or(100_000);
    let begun = Instant::now();
    let package = synthetic_package(records);
    println!("built a {records}-record package in {:?}", begun.elapsed());

    let jwks = key_server();
    run(&host(config(&temporary("ath-host-scale-dir-"), Mode::Directory, &jwks)), "directory", records, &package);

    let (bucket_url, _bucket) = bucket_server();
    let server = temporary("ath-host-scale-server-");
    let remote = host(config(
        &temporary("ath-host-scale-remote-"),
        Mode::LocalServer { server: &server, bucket: &bucket_url },
        &jwks,
    ));
    run(&remote, "remote (local stand-in server)", records, &package);

    // A second host copy builds its working copy from the server.
    let second = host(config(
        &temporary("ath-host-scale-second-"),
        Mode::LocalServer { server: &server, bucket: &bucket_url },
        &jwks,
    ));
    let owner = token("scale-remote (local stand-in server)", ATHLETE_APP);
    let athletes = call("GET", &format!("{}/v1/athletes", second.url), &owner, None, None);
    let id = athletes.json["athletes"][0]["athlete_id"].as_str().unwrap().to_string();
    let begun = Instant::now();
    let summary = call("GET", &format!("{}/v1/athletes/{id}", second.url), &owner, None, None);
    assert_eq!(summary.status, 200);
    println!("second host copy: working copy built from the server in {:?}", begun.elapsed());
}
