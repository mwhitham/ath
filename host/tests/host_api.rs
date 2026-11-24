//! The host API end to end, in directory mode and in remote mode with a stand-in
//! server and bucket (spec 2.1.3 build steps 5–7).

mod support;

use std::collections::BTreeSet;

use athleticstandard::model::RecordType;
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::{Changes, NewRecord};
use serde_json::json;
use support::*;

struct Setup {
    hosts: Vec<Host>,
    _bucket: Option<Bucket>,
}

fn directory_host() -> Setup {
    let jwks = key_server();
    Setup { hosts: vec![host(config(&temporary("ath-host-dir-"), Mode::Directory, &jwks))], _bucket: None }
}

/// `copies` host processes sharing one stand-in server and one bucket.
fn remote_hosts(copies: usize) -> Setup {
    let jwks = key_server();
    let (bucket_url, bucket) = bucket_server();
    let server = temporary("ath-host-server-");
    let hosts = (0..copies)
        .map(|_| {
            host(config(
                &temporary("ath-host-remote-"),
                Mode::LocalServer { server: &server, bucket: &bucket_url },
                &jwks,
            ))
        })
        .collect();
    Setup { hosts, _bucket: Some(bucket) }
}

fn create_athlete(host: &Host, owner: &str) -> String {
    let reply = call("POST", &format!("{}/v1/athletes", host.url), owner, Some(json!({})), None);
    assert_eq!(reply.status, 201, "{}", reply.json);
    reply.json["athlete_id"].as_str().unwrap().to_string()
}

fn athlete_url(host: &Host, id: &str, rest: &str) -> String {
    format!("{}/v1/athletes/{id}{rest}", host.url)
}

// ---- Step 5: an owner saves a report with a photo and opens both elsewhere --------

fn report_with_photo(host: &Host) {
    let owner = token("athlete-1", ATHLETE_APP);
    let id = create_athlete(host, &owner);

    let uploaded = send_bytes("POST", &athlete_url(host, &id, "/uploads"), &owner, &demo_photo());
    assert_eq!(uploaded.status, 201, "{}", uploaded.json);
    let upload_id = uploaded.json["upload_id"].as_str().unwrap().to_string();

    let request =
        json!({ "record_type": "athlete_report", "data": report("Felt strong."), "attachments": [upload_id] });
    let created = call("POST", &athlete_url(host, &id, "/records"), &owner, Some(request.clone()), Some("report-1"));
    assert_eq!(created.status, 201, "{}", created.json);
    assert_eq!(created.json["records"].as_array().unwrap().len(), 2, "the attachment and the report");

    // A retry after a lost reply returns the same result and writes nothing new.
    let again = call("POST", &athlete_url(host, &id, "/records"), &owner, Some(request), Some("report-1"));
    assert_eq!(again.json, created.json);
    // The same key with a different body is a conflict.
    let other = json!({ "record_type": "athlete_report", "data": report("Something else.") });
    let conflict = call("POST", &athlete_url(host, &id, "/records"), &owner, Some(other), Some("report-1"));
    assert_eq!((conflict.status, conflict.json["error"]["code"].as_str()), (409, Some("request_conflict")));
    // A write without a key is refused.
    let unkeyed = call(
        "POST",
        &athlete_url(host, &id, "/records"),
        &owner,
        Some(json!({ "record_type": "athlete_report", "data": report("x") })),
        None,
    );
    assert_eq!(unkeyed.status, 400);

    let exported = call("GET", &athlete_url(host, &id, "/export"), &owner, None, None);
    assert_eq!(exported.status, 200);
    assert_eq!(exported.headers.get("x-manifest-digest").map(String::len), Some(64));
    let (_, device) = open_on_another_device(&exported.bytes);
    let reports = device
        .list_records(&ListOptions { record_type: Some(RecordType::AthleteReport), include_deleted: false, limit: 100 })
        .unwrap();
    assert_eq!(reports.len(), 1);
    let attachments = device
        .list_records(&ListOptions { record_type: Some(RecordType::Attachment), include_deleted: false, limit: 100 })
        .unwrap();
    assert_eq!(attachments.len(), 1);
    let athleticstandard::model::RecordData::Attachment(data) = &attachments[0].data else { panic!() };
    let name = athleticstandard::files::attachments::stored_attachment_name(&attachments[0].record_id, data);
    let copied = std::fs::read(device.path().parent().unwrap().join("attachments").join(name)).unwrap();
    assert_eq!(copied, demo_photo(), "the photo arrived byte for byte");

    let download =
        call("GET", &athlete_url(host, &id, &format!("/attachments/{}", attachments[0].record_id)), &owner, None, None);
    assert_eq!((download.status, download.bytes.clone()), (200, demo_photo()));

    let changes = call("GET", &athlete_url(host, &id, "/changes?since=0"), &owner, None, None);
    assert_eq!(changes.json["complete"], json!(true));
    assert_eq!(changes.json["changes"].as_array().unwrap().len(), 2);

    // Someone else cannot tell the record exists.
    let stranger = token("athlete-2", ATHLETE_APP);
    for path in ["", "/export", "/records", "/changes?since=0"] {
        assert_eq!(call("GET", &athlete_url(host, &id, path), &stranger, None, None).status, 404, "{path}");
    }
}

#[test]
fn directory_mode_saves_a_report_with_a_photo() {
    report_with_photo(&directory_host().hosts[0]);
}

#[test]
fn remote_mode_saves_a_report_with_a_photo() {
    let setup = remote_hosts(1);
    report_with_photo(&setup.hosts[0]);
    let objects = setup._bucket.as_ref().unwrap().objects.lock().unwrap();
    assert_eq!(objects.len(), 1, "the photo is in the bucket");
}

// ---- Step 6: a coach sees and adds only what was granted --------------------------

fn coach_flow(host: &Host) {
    let owner = token("athlete-1", ATHLETE_APP);
    let id = create_athlete(host, &owner);
    let restored = send_bytes("PUT", &athlete_url(host, &id, "/package"), &owner, &demo_package_zip());
    assert_eq!(restored.status, 200, "{}", restored.json);
    let other_owner = token("athlete-2", ATHLETE_APP);
    let other_id = create_athlete(host, &other_owner);

    let offer = call(
        "POST",
        &athlete_url(host, &id, "/grant-offers"),
        &owner,
        Some(
            json!({ "permissions": ["read_workout_summaries", "add_coach_notes"], "client_id": COACH_APP, "coach_label": "Coach Sam" }),
        ),
        None,
    );
    assert_eq!(offer.status, 201, "{}", offer.json);
    let code = offer.json["code"].as_str().unwrap();

    // The offer is for the coach app; another app cannot accept it.
    let coach_elsewhere = token("coach-1", ATHLETE_APP);
    assert_eq!(
        call(
            "POST",
            &format!("{}/v1/grant-offers/accept", host.url),
            &coach_elsewhere,
            Some(json!({ "code": code })),
            None
        )
        .status,
        404
    );
    let coach = token("coach-1", COACH_APP);
    let accepted =
        call("POST", &format!("{}/v1/grant-offers/accept", host.url), &coach, Some(json!({ "code": code })), None);
    assert_eq!(accepted.status, 200, "{}", accepted.json);
    // A code works once.
    assert_eq!(
        call("POST", &format!("{}/v1/grant-offers/accept", host.url), &coach, Some(json!({ "code": code })), None)
            .status,
        404
    );

    let roster = call("GET", &format!("{}/v1/roster", host.url), &coach, None, None);
    let listed: Vec<&str> =
        roster.json["athletes"].as_array().unwrap().iter().map(|a| a["athlete_id"].as_str().unwrap()).collect();
    assert_eq!(listed, vec![id.as_str()]);

    let workouts = call("GET", &athlete_url(host, &id, "/workouts"), &coach, None, None);
    assert_eq!(workouts.status, 200, "{}", workouts.json);
    let list = workouts.json["workouts"].as_array().unwrap();
    assert!(!list.is_empty());
    for summary in list {
        let keys: BTreeSet<&str> = summary.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, BTreeSet::from(["record_id", "revision", "start_at", "end_at", "activity_type"]));
    }
    let workout = &list[0];

    let note = json!({ "note": "Review pacing at the next session.", "evidence": [{ "record_id": workout["record_id"], "revision": workout["revision"] }] });
    let added = call("POST", &athlete_url(host, &id, "/notes"), &coach, Some(note.clone()), Some("note-1"));
    assert_eq!(added.status, 201, "{}", added.json);
    assert_eq!(call("POST", &athlete_url(host, &id, "/notes"), &coach, Some(note), Some("note-1")).json, added.json);
    let bad_evidence = json!({ "note": "x", "evidence": [{ "record_id": workout["record_id"], "revision": 99 }] });
    assert_eq!(call("POST", &athlete_url(host, &id, "/notes"), &coach, Some(bad_evidence), Some("note-2")).status, 422);

    // Not granted: programming, owner reads, the other athlete, the other app.
    let program = json!({ "plan": { "title": "Base block" }, "sessions": [] });
    assert_eq!(
        call("POST", &athlete_url(host, &id, "/programs"), &coach, Some(program), Some("program-1")).status,
        403
    );
    assert_eq!(call("GET", &athlete_url(host, &id, "/export"), &coach, None, None).status, 403);
    assert_eq!(call("GET", &athlete_url(host, &id, "/records"), &coach, None, None).status, 403);
    assert_eq!(call("GET", &athlete_url(host, &other_id, "/workouts"), &coach, None, None).status, 404);
    assert_eq!(call("GET", &athlete_url(host, &id, "/workouts"), &coach_elsewhere, None, None).status, 404);

    // The note keeps its coach author, with the host's coach ID.
    let me = call("GET", &format!("{}/v1/me", host.url), &coach, None, None);
    let note_id = added.json["records"][0]["record_id"].as_str().unwrap();
    let stored = call("GET", &athlete_url(host, &id, &format!("/records/{note_id}")), &owner, None, None);
    let author = &stored.json["record"]["data"]["authorship"]["author"];
    assert_eq!(author["author_type"], json!("coach"));
    assert_eq!(author["coach"], json!("Coach Sam"));
    assert_eq!(author["coach_id"], me.json["coach_id"]);
    assert_eq!(stored.json["revisions"][0]["changed_by"]["author_type"], json!("coach"));

    // Revoking stops the next request.
    let grants = call("GET", &athlete_url(host, &id, "/grants"), &owner, None, None);
    let grant_id = grants.json["grants"][0]["grant_id"].as_str().unwrap().to_string();
    let revoked = call("DELETE", &athlete_url(host, &id, &format!("/grants/{grant_id}")), &owner, None, None);
    assert_eq!(revoked.status, 200, "{}", revoked.json);
    assert_eq!(call("GET", &athlete_url(host, &id, "/workouts"), &coach, None, None).status, 404);
    assert_eq!(
        call("POST", &athlete_url(host, &id, "/notes"), &coach, Some(json!({ "note": "late" })), Some("note-3")).status,
        404
    );
    assert!(call("GET", &format!("{}/v1/roster", host.url), &coach, None, None).json["athletes"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn directory_mode_coach_sees_only_what_was_granted() {
    coach_flow(&directory_host().hosts[0]);
}

#[test]
fn remote_mode_coach_sees_only_what_was_granted() {
    coach_flow(&remote_hosts(1).hosts[0]);
}

fn programming(host: &Host) {
    let owner = token("athlete-1", ATHLETE_APP);
    let id = create_athlete(host, &owner);
    let offer = call(
        "POST",
        &athlete_url(host, &id, "/grant-offers"),
        &owner,
        Some(json!({ "permissions": ["add_programming"], "client_id": COACH_APP })),
        None,
    );
    let coach = token("coach-1", COACH_APP);
    call(
        "POST",
        &format!("{}/v1/grant-offers/accept", host.url),
        &coach,
        Some(json!({ "code": offer.json["code"] })),
        None,
    );
    let program = json!({
        "plan": { "title": "Base block", "objective": "Build an aerobic base" },
        "sessions": [
            { "title": "Easy row", "scheduled_date": "2026-12-01", "blocks": [{ "block_type": "main", "title": "30 minutes easy" }] },
            { "title": "Intervals", "scheduled_date": "2026-12-03", "blocks": [{ "block_type": "main", "title": "6 x 500 m" }] }
        ]
    });
    let added = call("POST", &athlete_url(host, &id, "/programs"), &coach, Some(program), Some("program-1"));
    assert_eq!(added.status, 201, "{}", added.json);
    assert_eq!(added.json["records"].as_array().unwrap().len(), 3);
    let forbidden = json!({ "plan": { "title": "x" }, "sessions": [{ "title": "Done", "scheduled_date": "2026-12-01", "blocks": [], "completion_status": "completed" }] });
    assert_eq!(
        call("POST", &athlete_url(host, &id, "/programs"), &coach, Some(forbidden), Some("program-2")).status,
        400
    );
    // The coach cannot write notes or read workouts with only programming.
    assert_eq!(
        call("POST", &athlete_url(host, &id, "/notes"), &coach, Some(json!({ "note": "x" })), Some("n")).status,
        403
    );
    assert_eq!(call("GET", &athlete_url(host, &id, "/workouts"), &coach, None, None).status, 403);
}

#[test]
fn directory_mode_coach_adds_a_program() {
    programming(&directory_host().hosts[0]);
}

#[test]
fn remote_mode_coach_adds_a_program() {
    programming(&remote_hosts(1).hosts[0]);
}

/// A write that read the access epoch before a revocation and commits after it fails
/// (D262), in both modes.
fn revocation_blocks_a_waiting_write(host: &Host) {
    let owner = token("athlete-1", ATHLETE_APP);
    let id = create_athlete(host, &owner);
    let storage = &host.state.storage;
    let epoch = runtime().block_on(storage.access_epoch(&id)).unwrap();
    runtime().block_on(storage.raise_access_epoch(&id)).unwrap();
    let note = athleticstandard::model::RecordData::decode(
        RecordType::AthleteReport,
        &athleticstandard::json::parse(&report("written under an old grant").to_string()).unwrap(),
    )
    .unwrap();
    let result = runtime().block_on(storage.write(&id, Some(epoch), move |store, epoch| {
        store.apply(Changes {
            expected_access_epoch: epoch,
            creates: vec![NewRecord::new(note.clone(), athleticstandard::model::RecordActor::Athlete)],
            ..Changes::default()
        })
    }));
    let error = result.expect_err("the write is refused");
    assert_eq!(error.code, "stale", "{error}");
    let count = runtime()
        .block_on(storage.read(&id, |store| {
            store.list_records(&ListOptions {
                record_type: Some(RecordType::AthleteReport),
                include_deleted: false,
                limit: 10,
            })
        }))
        .unwrap()
        .len();
    assert_eq!(count, 0, "nothing was written");
}

#[test]
fn directory_mode_revocation_blocks_a_waiting_write() {
    revocation_blocks_a_waiting_write(&directory_host().hosts[0]);
}

#[test]
fn remote_mode_revocation_blocks_a_waiting_write() {
    revocation_blocks_a_waiting_write(&remote_hosts(1).hosts[0]);
}

// ---- Step 7: several host copies share one record --------------------------------

#[test]
fn two_host_copies_share_a_record() {
    let setup = remote_hosts(2);
    let (a, b) = (&setup.hosts[0], &setup.hosts[1]);
    let owner = token("athlete-1", ATHLETE_APP);
    let id = create_athlete(a, &owner);

    let first = call(
        "POST",
        &athlete_url(a, &id, "/records"),
        &owner,
        Some(json!({ "record_type": "athlete_report", "data": report("From host A.") })),
        Some("a-1"),
    );
    assert_eq!(first.status, 201, "{}", first.json);
    // B builds its working copy from the server and sees A's write.
    let listed = call("GET", &athlete_url(b, &id, "/records?record_type=athlete_report"), &owner, None, None);
    assert_eq!(listed.json["records"].as_array().unwrap().len(), 1);

    // B writes; A catches up from the replication log.
    let second = call(
        "POST",
        &athlete_url(b, &id, "/records"),
        &owner,
        Some(json!({ "record_type": "athlete_report", "data": report("From host B.") })),
        Some("b-1"),
    );
    assert_eq!(second.status, 201, "{}", second.json);
    let third = call(
        "POST",
        &athlete_url(a, &id, "/records"),
        &owner,
        Some(json!({ "record_type": "athlete_report", "data": report("A again.") })),
        Some("a-2"),
    );
    assert_eq!(third.status, 201, "{}", third.json);
    for host in [a, b] {
        let listed = call("GET", &athlete_url(host, &id, "/records?record_type=athlete_report"), &owner, None, None);
        assert_eq!(listed.json["records"].as_array().unwrap().len(), 3);
    }
    // A retry sent to the other copy returns the original result.
    let retried = call(
        "POST",
        &athlete_url(b, &id, "/records"),
        &owner,
        Some(json!({ "record_type": "athlete_report", "data": report("From host A.") })),
        Some("a-1"),
    );
    assert_eq!(retried.json, first.json);

    // A photo saved through A downloads through B.
    let uploaded = send_bytes("POST", &athlete_url(a, &id, "/uploads"), &owner, &demo_photo());
    let with_photo = json!({ "record_type": "athlete_report", "data": report("Photo."), "attachments": [uploaded.json["upload_id"]] });
    let created = call("POST", &athlete_url(a, &id, "/records"), &owner, Some(with_photo), Some("a-3"));
    assert_eq!(created.status, 201, "{}", created.json);
    let attachment_id = created.json["records"][0]["record_id"].as_str().unwrap();
    let download = call("GET", &athlete_url(b, &id, &format!("/attachments/{attachment_id}")), &owner, None, None);
    assert_eq!((download.status, download.bytes), (200, demo_photo()));

    // Exports from both copies are identical.
    let export_a = call("GET", &athlete_url(a, &id, "/export"), &owner, None, None);
    let export_b = call("GET", &athlete_url(b, &id, "/export"), &owner, None, None);
    assert_eq!(export_a.headers.get("x-manifest-digest"), export_b.headers.get("x-manifest-digest"));

    // An erasure through B clears the log; A rebuilds and no longer has the report.
    let report_id = first.json["records"][0]["record_id"].as_str().unwrap();
    let erased = call(
        "POST",
        &athlete_url(b, &id, &format!("/records/{report_id}/erasure")),
        &owner,
        Some(json!({ "expected_revision": 1, "reason_code": "athlete_request" })),
        None,
    );
    assert_eq!(erased.status, 200, "{}", erased.json);
    let after = call("GET", &athlete_url(a, &id, &format!("/records/{report_id}")), &owner, None, None);
    assert_eq!(after.status, 404);
    let listed = call("GET", &athlete_url(a, &id, "/records?record_type=athlete_report"), &owner, None, None);
    assert_eq!(listed.json["records"].as_array().unwrap().len(), 3);
}
