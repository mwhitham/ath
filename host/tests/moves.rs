//! Moving a record between hosts with one writer at every step (D267), and backups
//! and restores of one athlete (D264). Spec 2.1.3 build step 8.

mod support;

use athleticstandard::model::RecordType;
use athleticstandard::store::read::ListOptions;
use serde_json::json;
use support::*;

fn directory_host_at(public_url: &str, jwks: &str) -> Host {
    let mut config = config(&temporary("ath-host-move-"), Mode::Directory, jwks);
    config.public_url = public_url.into();
    host(config)
}

fn create(host: &Host, owner: &str, body: serde_json::Value) -> String {
    let reply = call("POST", &format!("{}/v1/athletes", host.url), owner, Some(body), None);
    assert_eq!(reply.status, 201, "{}", reply.json);
    reply.json["athlete_id"].as_str().unwrap().to_string()
}

fn url(host: &Host, id: &str, rest: &str) -> String {
    format!("{}/v1/athletes/{id}{rest}", host.url)
}

fn add_report(host: &Host, owner: &str, id: &str, note: &str, key: &str) -> serde_json::Value {
    let reply = call(
        "POST",
        &url(host, id, "/records"),
        owner,
        Some(json!({ "record_type": "athlete_report", "data": report(note) })),
        Some(key),
    );
    assert_eq!(reply.status, 201, "{}", reply.json);
    reply.json
}

fn reports(host: &Host, id: &str) -> Vec<String> {
    runtime()
        .block_on(host.state.storage.read(id, |store| {
            store.list_records(&ListOptions {
                record_type: Some(RecordType::AthleteReport),
                include_deleted: false,
                limit: 100,
            })
        }))
        .unwrap()
        .into_iter()
        .map(|record| match record.data {
            athleticstandard::model::RecordData::AthleteReport(data) => data.note.unwrap_or_default(),
            _ => unreachable!(),
        })
        .collect()
}

#[test]
fn a_record_moves_with_one_writer_at_every_step() {
    let jwks = key_server();
    let source = directory_host_at("https://source.test", &jwks);
    let destination = directory_host_at("https://destination.test", &jwks);
    let owner = token("athlete-1", ATHLETE_APP);

    let id = create(&source, &owner, json!({}));
    add_report(&source, &owner, &id, "Before the move.", "r-1");

    let frozen = call(
        "POST",
        &url(&source, &id, "/move/freeze"),
        &owner,
        Some(json!({ "destination": "https://destination.test" })),
        None,
    );
    assert_eq!(frozen.status, 200, "{}", frozen.json);
    let transfer = frozen.json["transfer_id"].as_str().unwrap().to_string();
    let digest = frozen.json["manifest_digest"].as_str().unwrap().to_string();
    // Frozen: no writes, but the package can be read.
    let refused = call(
        "POST",
        &url(&source, &id, "/records"),
        &owner,
        Some(json!({ "record_type": "athlete_report", "data": report("x") })),
        Some("r-2"),
    );
    assert_eq!((refused.status, refused.json["error"]["code"].as_str()), (409, Some("host_moving")));
    let exported = call("GET", &url(&source, &id, "/export"), &owner, None, None);
    assert_eq!(exported.headers.get("x-manifest-digest"), Some(&digest));

    let incoming = create(&destination, &owner, json!({ "incoming": true }));
    let early = call(
        "POST",
        &url(&destination, &incoming, "/records"),
        &owner,
        Some(json!({ "record_type": "athlete_report", "data": report("x") })),
        Some("d-1"),
    );
    assert_eq!(early.status, 409, "a staged record takes no writes");
    assert_eq!(send_bytes("PUT", &url(&destination, &incoming, "/package"), &owner, &exported.bytes).status, 200);

    // The destination cannot activate before the source retires.
    let activate = |activation: &str| {
        call(
            "POST",
            &url(&destination, &incoming, "/move/activate"),
            &owner,
            Some(json!({ "source": source.url, "transfer_id": transfer, "activation": activation })),
            None,
        )
    };
    assert_eq!(activate("not-a-code").status, 403);

    let retired =
        call("POST", &url(&source, &id, "/move/retire"), &owner, Some(json!({ "transfer_id": transfer })), None);
    assert_eq!(retired.status, 200, "{}", retired.json);
    // A lost reply: retiring again issues the proof again and keeps one writer.
    let again =
        call("POST", &url(&source, &id, "/move/retire"), &owner, Some(json!({ "transfer_id": transfer })), None);
    assert_eq!(again.status, 200, "{}", again.json);
    let activation = again.json["activation"].as_str().unwrap().to_string();
    let stale_code = retired.json["activation"].as_str().unwrap();
    assert_eq!(activate(stale_code).status, 403, "only the newest proof works");

    let activated = activate(&activation);
    assert_eq!(activated.status, 200, "{}", activated.json);
    assert_eq!(
        activated.json["manifest_digest"].as_str(),
        Some(digest.as_str()),
        "the destination holds the same package"
    );
    // The proof works once.
    let reused = call(
        "POST",
        &format!("{}/v1/transfers/{transfer}/verify", source.url),
        &owner,
        Some(json!({ "activation": activation, "destination": "https://destination.test", "manifest_digest": digest })),
        None,
    );
    assert_eq!(reused.status, 404);
    assert_eq!(activate(&activation).status, 409, "an active record is not activated twice");

    // The source is retired: nothing reads or writes it, except the owner's recovery copy.
    assert_eq!(call("GET", &url(&source, &id, ""), &owner, None, None).status, 404);
    let late = call(
        "POST",
        &url(&source, &id, "/records"),
        &owner,
        Some(json!({ "record_type": "athlete_report", "data": report("x") })),
        Some("r-3"),
    );
    assert_eq!(late.status, 404);
    assert_eq!(call("GET", &url(&source, &id, "/move/recovery-copy"), &owner, None, None).status, 200);
    assert_eq!(
        call("POST", &url(&source, &id, "/move/abort"), &owner, Some(json!({ "transfer_id": transfer })), None).status,
        409
    );

    // The destination now takes writes.
    add_report(&destination, &owner, &incoming, "After the move.", "d-2");
    assert_eq!(reports(&destination, &incoming).len(), 2);
}

#[test]
fn a_move_can_stop_before_the_source_retires() {
    let jwks = key_server();
    let source = directory_host_at("https://source.test", &jwks);
    let owner = token("athlete-1", ATHLETE_APP);
    let id = create(&source, &owner, json!({}));
    let frozen = call(
        "POST",
        &url(&source, &id, "/move/freeze"),
        &owner,
        Some(json!({ "destination": "https://elsewhere.test" })),
        None,
    );
    let transfer = frozen.json["transfer_id"].as_str().unwrap();
    let aborted =
        call("POST", &url(&source, &id, "/move/abort"), &owner, Some(json!({ "transfer_id": transfer })), None);
    assert_eq!(aborted.status, 200, "{}", aborted.json);
    add_report(&source, &owner, &id, "Still here.", "r-1");
    // A coach or stranger cannot start a move.
    let stranger = token("someone", ATHLETE_APP);
    assert_eq!(
        call("POST", &url(&source, &id, "/move/freeze"), &stranger, Some(json!({ "destination": "x" })), None).status,
        404
    );
}

#[test]
fn a_restore_from_an_older_backup_keeps_later_erasures() {
    let jwks = key_server();
    let host = directory_host_at("https://host.test", &jwks);
    let owner = token("athlete-1", ATHLETE_APP);
    let id = create(&host, &owner, json!({}));
    let first = add_report(&host, &owner, &id, "Erase me later.", "r-1");
    add_report(&host, &owner, &id, "Keep me.", "r-2");

    let output = temporary("ath-host-backups-");
    let backed_up = runtime().block_on(ath_host::backup::backup_all(&host.state, &output)).unwrap();
    assert_eq!(backed_up.len(), 1);
    let zip = std::fs::read(&backed_up[0].file).unwrap();

    let erased_id = first["records"][0]["record_id"].as_str().unwrap();
    let erased = call(
        "POST",
        &url(&host, &id, &format!("/records/{erased_id}/erasure")),
        &owner,
        Some(json!({ "expected_revision": 1, "reason_code": "athlete_request" })),
        None,
    );
    assert_eq!(erased.status, 200, "{}", erased.json);
    add_report(&host, &owner, &id, "After the backup.", "r-3");
    let epoch_before = runtime().block_on(host.state.storage.access_epoch(&id)).unwrap();

    runtime().block_on(ath_host::backup::restore_athlete(&host.state, &id, &zip)).unwrap();
    let mut notes = reports(&host, &id);
    notes.sort();
    assert_eq!(
        notes,
        vec!["Keep me.".to_string()],
        "the erased report stays erased; the later report is not in the backup"
    );
    let epoch_after = runtime().block_on(host.state.storage.access_epoch(&id)).unwrap();
    assert!(epoch_after > epoch_before, "a restore raises the access epoch");
}

#[test]
fn an_unfinished_erasure_is_finished_on_start() {
    let jwks = key_server();
    let host = directory_host_at("https://host.test", &jwks);
    let owner = token("athlete-1", ATHLETE_APP);
    let id = create(&host, &owner, json!({}));
    let created = add_report(&host, &owner, &id, "Erase me.", "r-1");
    let record_id = created["records"][0]["record_id"].as_str().unwrap();
    // The process stopped after noting the erasure and before applying it.
    runtime().block_on(host.state.directory.record_erasure_intent(&id, record_id, "athlete_request")).unwrap();
    assert_eq!(runtime().block_on(ath_host::backup::finish_erasures(&host.state)).unwrap(), 1);
    assert!(reports(&host, &id).is_empty());
    assert_eq!(runtime().block_on(ath_host::backup::finish_erasures(&host.state)).unwrap(), 0);
}

/// Run the whole move from `source` to `destination` and return the new athlete ID.
fn move_record(source: &Host, destination: &Host, destination_url: &str, owner: &str, id: &str) -> String {
    let frozen =
        call("POST", &url(source, id, "/move/freeze"), owner, Some(json!({ "destination": destination_url })), None);
    assert_eq!(frozen.status, 200, "{}", frozen.json);
    let transfer = frozen.json["transfer_id"].as_str().unwrap().to_string();
    let exported = call("GET", &url(source, id, "/export"), owner, None, None);
    let incoming = create(destination, owner, json!({ "incoming": true }));
    assert_eq!(send_bytes("PUT", &url(destination, &incoming, "/package"), owner, &exported.bytes).status, 200);
    let retired = call("POST", &url(source, id, "/move/retire"), owner, Some(json!({ "transfer_id": transfer })), None);
    assert_eq!(retired.status, 200, "{}", retired.json);
    let activated = call(
        "POST",
        &url(destination, &incoming, "/move/activate"),
        owner,
        Some(json!({ "source": source.url, "transfer_id": transfer, "activation": retired.json["activation"] })),
        None,
    );
    assert_eq!(activated.status, 200, "{}", activated.json);
    assert_eq!(activated.json["manifest_digest"], frozen.json["manifest_digest"]);
    incoming
}

#[test]
fn a_record_moves_from_directory_to_remote_and_back() {
    let jwks = key_server();
    let (bucket_url, _bucket) = bucket_server();
    let server = temporary("ath-host-move-server-");
    let native = directory_host_at("https://native.test", &jwks);
    let mut remote_config =
        config(&temporary("ath-host-move-remote-"), Mode::LocalServer { server: &server, bucket: &bucket_url }, &jwks);
    remote_config.public_url = "https://remote.test".into();
    let remote = host(remote_config);
    let second_native = directory_host_at("https://native-again.test", &jwks);
    let owner = token("athlete-1", ATHLETE_APP);

    let id = create(&native, &owner, json!({}));
    let uploaded = send_bytes("POST", &url(&native, &id, "/uploads"), &owner, &demo_photo());
    let with_photo = json!({ "record_type": "athlete_report", "data": report("With a photo."), "attachments": [uploaded.json["upload_id"]] });
    assert_eq!(call("POST", &url(&native, &id, "/records"), &owner, Some(with_photo), Some("p-1")).status, 201);

    let on_remote = move_record(&native, &remote, "https://remote.test", &owner, &id);
    add_report(&remote, &owner, &on_remote, "Written on the remote host.", "r-1");
    let back = move_record(&remote, &second_native, "https://native-again.test", &owner, &on_remote);

    let mut notes = reports(&second_native, &back);
    notes.sort();
    assert_eq!(notes, vec!["With a photo.".to_string(), "Written on the remote host.".to_string()]);
    let exported = call("GET", &url(&second_native, &back, "/export"), &owner, None, None);
    let (_, device) = open_on_another_device(&exported.bytes);
    let attachments = device
        .list_records(&ListOptions { record_type: Some(RecordType::Attachment), include_deleted: false, limit: 10 })
        .unwrap();
    let athleticstandard::model::RecordData::Attachment(data) = &attachments[0].data else { panic!() };
    let name = athleticstandard::files::attachments::stored_attachment_name(&attachments[0].record_id, data);
    assert_eq!(
        std::fs::read(device.path().parent().unwrap().join("attachments").join(name)).unwrap(),
        demo_photo(),
        "the photo made both moves intact"
    );
}
