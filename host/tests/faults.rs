//! Failures in remote mode leave the server unchanged and the request safe to retry;
//! a restart clears what the last process left; a large job for one athlete does not
//! delay another athlete's write. Spec 2.1.3 build step 10.

mod support;

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde_json::json;
use support::*;

struct Remote {
    host: Host,
    bucket: Bucket,
    server: std::path::PathBuf,
    config: ath_host::config::Config,
}

fn remote() -> Remote {
    let jwks = key_server();
    let (bucket_url, bucket) = bucket_server();
    let server = temporary("ath-host-fault-server-");
    let config =
        config(&temporary("ath-host-fault-"), Mode::LocalServer { server: &server, bucket: &bucket_url }, &jwks);
    Remote { host: host(config.clone()), bucket, server, config }
}

fn create(host: &Host, owner: &str) -> String {
    let reply = call("POST", &format!("{}/v1/athletes", host.url), owner, Some(json!({})), None);
    assert_eq!(reply.status, 201, "{}", reply.json);
    reply.json["athlete_id"].as_str().unwrap().to_string()
}

fn url(host: &Host, id: &str, rest: &str) -> String {
    format!("{}/v1/athletes/{id}{rest}", host.url)
}

/// The server's own rows, read straight from the stand-in server file.
fn server_count(remote: &Remote, id: &str, table: &str) -> i64 {
    let path = remote.server.join(format!("{}.sqlite", id.replace('_', "-")));
    let connection = rusqlite::Connection::open(path).unwrap();
    connection.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0)).unwrap()
}

fn server_sql(remote: &Remote, id: &str, sql: &str) {
    let path = remote.server.join(format!("{}.sqlite", id.replace('_', "-")));
    rusqlite::Connection::open(path).unwrap().execute_batch(sql).unwrap();
}

#[test]
fn a_failed_photo_upload_leaves_the_server_unchanged() {
    let remote = remote();
    let owner = token("athlete-1", ATHLETE_APP);
    let id = create(&remote.host, &owner);
    let uploaded = send_bytes("POST", &url(&remote.host, &id, "/uploads"), &owner, &demo_photo());
    let request = json!({ "record_type": "athlete_report", "data": report("With a photo."), "attachments": [uploaded.json["upload_id"]] });

    remote.bucket.fail_puts.store(true, Ordering::SeqCst);
    let failed = call("POST", &url(&remote.host, &id, "/records"), &owner, Some(request.clone()), Some("photo-1"));
    assert_eq!(failed.status, 500, "{}", failed.json);
    assert_eq!(
        server_count(&remote, &id, "records"),
        server_count(&remote, &id, "records WHERE record_type = 'benchmark_definition'"),
        "no record reached the server"
    );
    assert_eq!(server_count(&remote, &id, "internal_replication_log"), 0);

    remote.bucket.fail_puts.store(false, Ordering::SeqCst);
    let retried = call("POST", &url(&remote.host, &id, "/records"), &owner, Some(request), Some("photo-1"));
    assert_eq!(retried.status, 201, "{}", retried.json);
    assert_eq!(server_count(&remote, &id, "athlete_reports"), 1);
    assert_eq!(server_count(&remote, &id, "attachments"), 1);
    assert_eq!(remote.bucket.objects.lock().unwrap().len(), 1);
}

#[test]
fn a_commit_that_fails_partway_leaves_nothing_on_the_server() {
    let remote = remote();
    let owner = token("athlete-1", ATHLETE_APP);
    let id = create(&remote.host, &owner);
    let revision_before = server_count(&remote, &id, "internal_change_log");
    // Fail the commit after the replication log row and the record row are written.
    server_sql(&remote, &id, "CREATE TRIGGER injected_failure BEFORE INSERT ON athlete_reports BEGIN SELECT RAISE(ABORT, 'injected failure'); END;");
    let request = json!({ "record_type": "athlete_report", "data": report("Saved once.") });
    let failed = call("POST", &url(&remote.host, &id, "/records"), &owner, Some(request.clone()), Some("report-1"));
    assert_eq!(failed.status, 500, "{}", failed.json);
    assert_eq!(server_count(&remote, &id, "internal_replication_log"), 0, "the log row rolled back with the rest");
    assert_eq!(server_count(&remote, &id, "internal_change_log"), revision_before);
    assert_eq!(server_count(&remote, &id, "internal_write_receipts"), 0);

    server_sql(&remote, &id, "DROP TRIGGER injected_failure;");
    let retried = call("POST", &url(&remote.host, &id, "/records"), &owner, Some(request), Some("report-1"));
    assert_eq!(retried.status, 201, "{}", retried.json);
    assert_eq!(server_count(&remote, &id, "athlete_reports"), 1);
    assert_eq!(server_count(&remote, &id, "internal_write_receipts"), 1);
}

#[test]
fn a_restart_clears_working_copies_and_staging() {
    let remote = remote();
    let owner = token("athlete-1", ATHLETE_APP);
    let id = create(&remote.host, &owner);
    call(
        "POST",
        &url(&remote.host, &id, "/records"),
        &owner,
        Some(json!({ "record_type": "athlete_report", "data": report("Before the restart.") })),
        Some("r-1"),
    );
    let data = remote.config.data_directory.clone();
    assert!(data.join("working").join(&id).exists());
    std::fs::create_dir_all(data.join("staging/exports/left-over")).unwrap();
    std::fs::create_dir_all(data.join("staging/uploads").join(&id)).unwrap();
    std::fs::write(data.join("staging/uploads").join(&id).join("0123456789abcdef0123456789abcdef"), b"stale").unwrap();

    let restarted = host(remote.config.clone());
    assert!(!data.join("working").join(&id).exists(), "working copies are rebuilt, not trusted");
    assert!(!data.join("staging/exports/left-over").exists());
    assert!(!data.join("staging/uploads").join(&id).join("0123456789abcdef0123456789abcdef").exists());
    let listed = call("GET", &url(&restarted, &id, "/records?record_type=athlete_report"), &owner, None, None);
    assert_eq!(listed.json["records"].as_array().unwrap().len(), 1);
}

#[test]
fn an_expired_upload_cannot_be_used() {
    let jwks = key_server();
    let host = host(config(&temporary("ath-host-expiry-"), Mode::Directory, &jwks));
    let owner = token("athlete-1", ATHLETE_APP);
    let id = create(&host, &owner);
    let uploaded = send_bytes("POST", &url(&host, &id, "/uploads"), &owner, &demo_photo());
    host.state.storage.expire_uploads(Duration::ZERO);
    let request = json!({ "record_type": "athlete_report", "data": report("Late."), "attachments": [uploaded.json["upload_id"]] });
    assert_eq!(call("POST", &url(&host, &id, "/records"), &owner, Some(request), Some("late-1")).status, 400);
}

fn large_job_does_not_delay_another_athlete(host: &Host, records: usize) {
    let first_owner = token("athlete-1", ATHLETE_APP);
    let second_owner = token("athlete-2", ATHLETE_APP);
    let first = create(host, &first_owner);
    let second = create(host, &second_owner);
    let package = synthetic_package(records);

    let job_url = url(host, &first, "/package");
    let job = std::thread::spawn(move || {
        let begun = Instant::now();
        let reply = send_bytes("PUT", &job_url, &first_owner, &package);
        (reply.status, begun.elapsed(), Instant::now())
    });
    std::thread::sleep(Duration::from_millis(50));
    let begun = Instant::now();
    let small = call(
        "POST",
        &url(host, &second, "/records"),
        &second_owner,
        Some(json!({ "record_type": "athlete_report", "data": report("Small write.") })),
        Some("small-1"),
    );
    let small_done = Instant::now();
    assert_eq!(small.status, 201, "{}", small.json);
    let (status, job_time, job_done) = job.join().unwrap();
    assert_eq!(status, 200);
    println!("large job: {records} records in {job_time:?}; small write for another athlete: {:?}", small_done - begun);
    assert!(small_done < job_done, "the small write finished before the large job");
}

#[test]
fn directory_mode_large_job_does_not_delay_another_athlete() {
    let jwks = key_server();
    large_job_does_not_delay_another_athlete(
        &host(config(&temporary("ath-host-busy-"), Mode::Directory, &jwks)),
        3_000,
    );
}

#[test]
fn remote_mode_large_job_does_not_delay_another_athlete() {
    large_job_does_not_delay_another_athlete(&remote().host, 3_000);
}
