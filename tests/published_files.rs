//! Published files keep their exact bytes.
//!
//! A released JSON Schema or SQL migration is part of the public format.
//! Readers, direct-SQL writers, and older programs depend on its exact text,
//! and a database records which migration steps ran. This test pins the
//! SHA-256 of each published file. A change to one must be deliberate: record
//! the decision in the current version's notes and update the hash here in the
//! same change. Never update a hash only to make this test pass.
//!
//! The current version's generated schema files (`schema/<version>/` and
//! `schema/athleticstandard.schema.json`) are checked by `tests/generated.rs`.
//! The host API description (`schema/host-api/`) is generated too, and
//! `host/tests/generated.rs` checks it (D271). It joins the list below when the
//! first version with it is released.
//! The current SQL migration step is still part of this build. When a version
//! is released, its files join the list below.

use std::path::{Path, PathBuf};

use athleticstandard::model::package::ATHLETIC_STANDARD_VERSION;
use athleticstandard::store::SQL_SCHEMA_VERSION;
use sha2::{Digest, Sha256};

/// Every published historical file and the SHA-256 of its committed bytes.
const PUBLISHED: &[(&str, &str)] = &[
    ("spec/sql/migrations/001-initial.sql", "fd94cff50848bb5906c12a1b8f5272977f1679dffaa2157f100d223361e1248f"),
    (
        "spec/sql/migrations/002-provider-encoding-repair.sql",
        "9f0580bd213da1ace26371c9c1c68c062e50ccab983b5c82b25019fdc2668b73",
    ),
    ("spec/sql/migrations/003-version-2.sql", "78f4c910f1f0fd80d0da0395ece2ab9681cbe351490d7af52752d95240041d80"),
    ("spec/sql/migrations/004-version-2-1.sql", "c31baa4d881d4a995912de209f902e90f4be695b5cb18e306364dc0331fa690c"),
    ("spec/sql/migrations/005-version-2-1-1.sql", "e393d69ba46a2f79cc40a34014e1fdf7da0cd8350a5d2dd151d88fe814ca2dbb"),
    ("spec/sql/migrations/006-version-2-1-2.sql", "b9e6be1fc880929aae2a585d0cd7b9b5185f48299981a4c07997df36131b7422"),
    ("schema/0.4.3/athleticstandard.schema.json", "9b8675c8bf69fe3bc4b9d9957bab617a10dbd665af67c17ea098415b94901c32"),
    ("schema/1.0.0/athleticstandard.schema.json", "68ebd0c8a0f6b1659d7d3df78cfcc9d9851937d38351ca182d72432d678af56e"),
    ("schema/1.0.0/export-manifest.schema.json", "22be5db19afec81fd4a2a68f1925974833ff474c245a31310f88eaa67666d236"),
    ("schema/1.0.0/measurement-series.schema.json", "b993fd4e494e576414fdfd64946588313df003be0c4e3bcc245a74e77292a502"),
    ("schema/2.0.0/athleticstandard.schema.json", "01fc267ca94fcc4c29f5676427f519da360ca3410cd75eba8dd424ac00963dc9"),
    ("schema/2.0.0/export-manifest.schema.json", "58fde42834791ead49da7a8fc1d9e46b26d2d9fea063b9f56a55b2ac31334021"),
    ("schema/2.0.0/measurement-series.schema.json", "467d8e184107024578842312477a47f7884f64dfbeb787ec9aeb5fbc381f70b8"),
    ("schema/2.1.0/athleticstandard.schema.json", "e41920ff8bed634eb5bd53031c85e7081075562e1222420add9b77a57402620b"),
    ("schema/2.1.0/export-manifest.schema.json", "2973fca078c455b6b6f600bec43e2ca09aad9d25c7ad43573b5e2fc377e89cb6"),
    ("schema/2.1.0/measurement-series.schema.json", "9668097a71b8ca4ae7431565a531ae25966f43e3ef5a924f62a977e96bf4e1b7"),
    ("schema/2.1.1/athleticstandard.schema.json", "7ea68b4a96caa64c55a08639fd8fb6f24e956c74382601fa7ef06c81202ecf5b"),
    ("schema/2.1.1/export-manifest.schema.json", "3c8c9435f6786be3b35fe69d9565e0ca296b0ef924347f3c898cac65ef8b97ac"),
    ("schema/2.1.1/measurement-series.schema.json", "69814a8d043c544077d262cd66e6b4e7536b704f5a93c232980da1646460914a"),
    ("schema/2.1.2/athleticstandard.schema.json", "6a5ba8b96a2f8146c173233f1ba76783b66de3c0d2d8649e31189025ae76116c"),
    ("schema/2.1.2/export-manifest.schema.json", "815eb3bfb1467306709095043f664cbe3c69b0cdf873aa14707ec4064d0260aa"),
    ("schema/2.1.2/measurement-series.schema.json", "d166428a39e7dbc81cd21a18be928c170f373808a4c449655d4a440d2566526f"),
];

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The `.json` and `.sql` files under a folder, as repository paths with `/`.
fn published_kind_files(folder: &str, out: &mut Vec<String>) {
    let mut entries: Vec<_> =
        std::fs::read_dir(repository().join(folder)).unwrap().map(|entry| entry.unwrap()).collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{folder}/{name}");
        if entry.file_type().unwrap().is_dir() {
            published_kind_files(&path, out);
        } else if Path::new(&name).extension().is_some_and(|extension| extension == "json" || extension == "sql") {
            out.push(path);
        }
    }
}

#[test]
fn published_files_keep_their_bytes() {
    let mut changed = Vec::new();
    for (path, expected) in PUBLISHED {
        let bytes = std::fs::read(repository().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"));
        let actual = sha256_hex(&bytes);
        if actual != *expected {
            changed.push(format!("{path}: pinned {expected}, now {actual}"));
        }
    }
    assert!(
        changed.is_empty(),
        "published files changed. A published file changes only by a recorded decision:\n{}",
        changed.join("\n")
    );
}

#[test]
fn every_historical_schema_and_migration_is_pinned() {
    let generated_folder = format!("schema/{ATHLETIC_STANDARD_VERSION}/");
    let current_step = format!("spec/sql/migrations/{SQL_SCHEMA_VERSION:03}-");
    let mut files = Vec::new();
    published_kind_files("schema", &mut files);
    published_kind_files("spec/sql/migrations", &mut files);
    let unpinned: Vec<&String> = files
        .iter()
        .filter(|path| !PUBLISHED.iter().any(|(pinned, _)| pinned == path))
        .filter(|path| !path.starts_with(&generated_folder) && *path != "schema/athleticstandard.schema.json")
        .filter(|path| !path.starts_with("schema/host-api/"))
        .filter(|path| !path.starts_with(&current_step))
        .collect();
    assert!(unpinned.is_empty(), "add these published files to PUBLISHED: {unpinned:?}");
}
