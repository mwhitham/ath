//! The default model saved beside the athlete database, and model choice.
//! Ported from `tests/models.test.ts`. Model lists come from a local server.

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use athleticstandard::runtime::gateway::{CatalogModel, GatewayClient};
use athleticstandard::runtime::keyring::Gateway;
use athleticstandard::runtime::models::{
    choose_model, default_model_path, load_catalog, read_default_model, render_model_list, require_on_list,
    resolve_model, save_default_model, saved_default_message, write_default_model, Catalog,
};
use athleticstandard::store::{CreateOptions, Store};
use support::{Reply, Server};

/// A temporary directory under the real temporary path. The store refuses a
/// database path through a symbolic link, such as `/tmp` on macOS.
fn tempdir() -> tempfile::TempDir {
    let base = fs::canonicalize(std::env::temp_dir()).unwrap();
    tempfile::Builder::new().prefix("ath-models-").tempdir_in(base).unwrap()
}

fn catalog(ids: &[&str]) -> Vec<CatalogModel> {
    ids.iter().map(|id| CatalogModel::new(*id)).collect()
}

fn create_database(directory: &Path) -> PathBuf {
    let path = directory.join("athlete.ath.sqlite");
    Store::create(&path, CreateOptions { created_at: Some("2026-10-13T12:00:00.000Z".into()), ..Default::default() })
        .unwrap()
        .close()
        .unwrap();
    path
}

#[test]
fn maps_athlete_ath_sqlite_to_athlete_ath_model() {
    assert_eq!(default_model_path(Path::new("/data/athlete.ath.sqlite")), Path::new("/data/athlete.ath.model"));
    assert_eq!(default_model_path(Path::new("/data/other.ath.sqlite")), Path::new("/data/other.ath.model"));
    assert_eq!(default_model_path(Path::new("/data/no-suffix")), Path::new("/data/no-suffix"));
}

#[test]
fn writes_the_model_file_without_changing_the_database_bytes() {
    let directory = tempdir();
    let database = create_database(directory.path());
    let before = fs::read(&database).unwrap();

    write_default_model(&database, "openai/gpt-oss-120b").unwrap();
    assert_eq!(fs::read(&database).unwrap(), before);
    assert_eq!(fs::read_to_string(directory.path().join("athlete.ath.model")).unwrap(), "openai/gpt-oss-120b\n");

    write_default_model(&database, "qwen/qwen3-235b-a22b").unwrap();
    assert_eq!(fs::read(&database).unwrap(), before);
    assert_eq!(fs::read_to_string(directory.path().join("athlete.ath.model")).unwrap(), "qwen/qwen3-235b-a22b\n");
    assert_eq!(read_default_model(&database).unwrap().as_deref(), Some("qwen/qwen3-235b-a22b"));

    let leftovers: Vec<_> = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".model-"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    Store::open(&database, Default::default()).unwrap().close().unwrap();
}

#[test]
fn reads_the_first_line_and_treats_an_empty_file_as_no_default() {
    let directory = tempdir();
    let database = directory.path().join("athlete.ath.sqlite");
    assert_eq!(read_default_model(&database).unwrap(), None);
    fs::write(directory.path().join("athlete.ath.model"), "  \r\n  a/b  \r\nc/d\n").unwrap();
    assert_eq!(read_default_model(&database).unwrap().as_deref(), Some("a/b"));
    fs::write(directory.path().join("athlete.ath.model"), " \n").unwrap();
    assert_eq!(read_default_model(&database).unwrap(), None);
}

#[test]
fn the_saved_default_is_used_when_no_model_is_named() {
    let directory = tempdir();
    let database = create_database(directory.path());
    write_default_model(&database, "openai/gpt-oss-120b").unwrap();
    let models = catalog(&["openai/gpt-oss-120b", "qwen/qwen3-235b-a22b"]);
    assert_eq!(choose_model(&database, None, &models).unwrap(), "openai/gpt-oss-120b");
    assert_eq!(choose_model(&database, Some("  "), &models).unwrap(), "openai/gpt-oss-120b");
    assert_eq!(choose_model(&database, Some(" QWEN/qwen3-235b-a22b "), &models).unwrap(), "qwen/qwen3-235b-a22b");
}

#[test]
fn no_model_and_unknown_models_are_refused_with_the_remedy() {
    let directory = tempdir();
    let database = directory.path().join("athlete.ath.sqlite");
    let models = catalog(&["qwen/qwen3-235b-a22b", "openai/gpt-oss-120b"]);
    let none = choose_model(&database, None, &models).unwrap_err().to_string();
    assert!(none.starts_with("No model chosen. Pick one for this run:"));
    assert!(none.ends_with("  ath models --default <name>"));

    let unknown = require_on_list("qwen/qwen3", &models).unwrap_err().to_string();
    assert_eq!(
        unknown,
        "no model called 'qwen/qwen3' on the live list of text models that can reason. \
         Closest: qwen/qwen3-235b-a22b. See them all with `ath models`."
    );
    let far = require_on_list("zzz", &models).unwrap_err().to_string();
    assert_eq!(
        far,
        "no model called 'zzz' on the live list of text models that can reason. See them all with `ath models`."
    );
}

#[test]
fn resolving_loads_the_live_list_before_choosing() {
    let directory = tempdir();
    let database = create_database(directory.path());
    write_default_model(&database, "openai/gpt-oss-120b").unwrap();
    let server = Server::start(vec![Reply::json(
        r#"{"data":[{"id":"openai/gpt-oss-120b","reasoning":true},{"id":"qwen/qwen3-235b-a22b","reasoning":true}]}"#,
    )]);
    let client = GatewayClient::new(Gateway::Vercel, "fake", server.base_url.clone());
    let resolved = resolve_model(&client, &database, None).unwrap();
    assert_eq!((resolved.gateway, resolved.model.as_str()), (Gateway::Vercel, "openai/gpt-oss-120b"));
    assert_eq!(resolved.models.len(), 2);
    assert_eq!(server.finish().len(), 1);
}

#[test]
fn saving_a_default_checks_the_list_and_reports_the_json_fields() {
    let directory = tempdir();
    let database = create_database(directory.path());
    let server = Server::start(vec![Reply::json(r#"{"data":[{"id":"openai/gpt-oss-120b","reasoning":true}]}"#)]);
    let client = GatewayClient::new(Gateway::OpenRouter, "fake", server.base_url.clone());
    let live = load_catalog(&client).unwrap();
    server.finish();
    assert_eq!(
        serde_json::to_string(&live).unwrap(),
        r#"{"gateway":"openrouter","models":[{"id":"openai/gpt-oss-120b"}]}"#
    );

    let saved = save_default_model(&database, "OpenAI/GPT-OSS-120B", &live).unwrap();
    assert_eq!(read_default_model(&database).unwrap().as_deref(), Some("openai/gpt-oss-120b"));
    let json = serde_json::to_value(&saved).unwrap();
    assert_eq!(json["default"], "openai/gpt-oss-120b");
    assert_eq!(json["file"], database.to_string_lossy().as_ref());
    assert_eq!(json["gateway"], "openrouter");
    assert_eq!(
        saved_default_message(&saved),
        "saved openai/gpt-oss-120b as the default model next to the athlete file."
    );

    let unknown = Catalog { gateway: Gateway::Vercel, models: vec![] };
    assert!(save_default_model(&database, "x/y", &unknown).is_err());
    assert_eq!(read_default_model(&database).unwrap().as_deref(), Some("openai/gpt-oss-120b"));
}

#[cfg(unix)]
#[test]
fn refuses_a_symlink_at_the_model_path_and_leaves_the_database_unchanged() {
    let directory = tempdir();
    let database = directory.path().join("athlete.ath.sqlite");
    fs::write(&database, "database-bytes").unwrap();
    let link = directory.path().join("athlete.ath.model");
    std::os::unix::fs::symlink(&database, &link).unwrap();

    let error = write_default_model(&database, "openai/gpt-oss-120b").unwrap_err().to_string();
    assert!(error.contains("symbolic link"), "{error}");
    assert_eq!(fs::read(&database).unwrap(), b"database-bytes");
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
}

#[cfg(unix)]
#[test]
fn refuses_a_model_path_that_is_the_database_through_a_hard_link() {
    let directory = tempdir();
    let database = directory.path().join("athlete.ath.sqlite");
    fs::write(&database, "database-bytes").unwrap();
    fs::hard_link(&database, directory.path().join("athlete.ath.model")).unwrap();
    // Two hard links have different real paths, so this is written as a new
    // file and the database keeps its bytes.
    write_default_model(&database, "a/b").unwrap();
    assert_eq!(fs::read(&database).unwrap(), b"database-bytes");
}

#[test]
fn refuses_to_write_beside_a_package_or_a_path_without_the_suffix() {
    let directory = tempdir();
    let package = write_default_model(&directory.path().join("athlete.ath.json"), "x").unwrap_err().to_string();
    assert!(package.starts_with("refusing to write the default model"), "{package}");
    assert!(package.ends_with("expected .ath.model next to a live .ath.sqlite database"));
    let plain = write_default_model(&directory.path().join("no-suffix"), "x").unwrap_err().to_string();
    assert!(plain.starts_with("refusing to write"), "{plain}");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn the_model_list_text_names_the_gateway_and_next_steps() {
    let models = vec![
        CatalogModel { id: "a/b".into(), name: Some("Model B".into()) },
        CatalogModel { id: "c/d".into(), name: Some("c/d".into()) },
    ];
    let text = render_model_list(&models, Gateway::Vercel);
    assert_eq!(
        text,
        "2 live text models that can reason, from vercel.\n\
         Open weights are included. Image, audio, and embedding models are out.\n\n  a/b  (Model B)\n  c/d\n\n\
         Pick one:  ath predict fran --model <name>\nSave one:  ath models --default <name>"
    );
    assert!(
        render_model_list(&[], Gateway::OpenRouter).contains("(none — the catalog marked no reasoning text models)")
    );
}
