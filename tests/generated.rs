//! One entry point for every generated file (spec 2.1.1 section 6).
//!
//! ```sh
//! cargo test --test generated                          # check the committed files
//! ATH_WRITE_GENERATED=1 cargo test --test generated    # write them
//! ```
//!
//! Each test builds its files from the Rust sources, in memory or in a
//! temporary folder, and fails when a committed copy differs. With
//! `ATH_WRITE_GENERATED=1` it writes the files instead. The generated files are:
//!
//! - `schema/<version>/*.schema.json` for the current version, and
//!   `schema/athleticstandard.schema.json`: JSON Schema from `src/model/`.
//! - `packages/npm/src/generated/protocol.ts`: the npm client's protocol
//!   module, from the operation definitions in `src/protocol/`.
//! - `skills/athletic-standard/references/cli.md`: the command reference, from
//!   the command table (D80).
//! - `examples/demo-athlete/`: the demo database and its attachment, from
//!   `tests/demo_support/history.json`.
//!
//! Generators never write a published historical schema, the SQL migrations
//! or contract in `spec/sql/`, or a fixture under `tests/fixtures/`.

mod demo_support;
#[path = "temporary_root.rs"]
mod temporary_root;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use athleticstandard::model::{json_schema, package, Shape};
use athleticstandard::protocol::typescript;
use athleticstandard::{cli, json, store};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};

const DEMO_FOLDER: &str = "examples/demo-athlete";

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn writing() -> bool {
    std::env::var("ATH_WRITE_GENERATED").as_deref() == Ok("1")
}

/// Refuse a generator target that is published history or a fixture.
fn assert_generated_path(relative: &str) {
    let current = format!("schema/{}/", package::ATHLETIC_STANDARD_VERSION);
    let published_schema = relative.starts_with("schema/") && relative.matches('/').count() > 1;
    assert!(
        !(published_schema && !relative.starts_with(&current))
            && !relative.starts_with("spec/")
            && !relative.starts_with("tests/fixtures/")
            && !relative.starts_with("build-history/"),
        "{relative} is published history or a fixture; a generator must not write it"
    );
}

/// Compare one generated text file with its committed copy, or write it.
fn check_text(relative: &str, expected: &str) {
    assert_generated_path(relative);
    let path = repository().join(relative);
    if writing() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, expected).unwrap();
        return;
    }
    let committed = fs::read_to_string(&path).unwrap_or_default();
    if committed != expected {
        let first = committed.lines().zip(expected.lines()).position(|(a, b)| a != b);
        panic!(
            "{relative} does not match its generator (first differing line {first:?}). \
             Run `ATH_WRITE_GENERATED=1 cargo test --test generated` and commit the result."
        );
    }
}

fn schema_text(schema: athleticstandard::model::schema::SchemaRef, file: &str, title: &str) -> String {
    let id = format!("https://athleticstandard.ai/schema/{}/{file}", package::ATHLETIC_STANDARD_VERSION);
    let mut text = json::to_pretty(&json_schema::document(schema, &id, title));
    text.push('\n');
    text
}

#[test]
fn json_schemas_match_the_model() {
    let version = package::ATHLETIC_STANDARD_VERSION;
    let package_schema = schema_text(
        package::package_schema(),
        "athleticstandard.schema.json",
        "Athletic Standard portable package document",
    );
    let documents = [
        ("athleticstandard.schema.json", package_schema.clone()),
        (
            "export-manifest.schema.json",
            schema_text(
                package::ExportManifest::schema(),
                "export-manifest.schema.json",
                "Athletic Standard export manifest",
            ),
        ),
        (
            "measurement-series.schema.json",
            schema_text(
                package::measurement_series_file_schema(),
                "measurement-series.schema.json",
                "Athletic Standard measurement-series file",
            ),
        ),
    ];
    for (file, text) in documents {
        check_text(&format!("schema/{version}/{file}"), &text);
    }
    // The unversioned file is the current package schema.
    check_text("schema/athleticstandard.schema.json", &package_schema);
}

#[test]
fn the_npm_protocol_module_matches_the_operation_definitions() {
    check_text(typescript::GENERATED_PATH, &typescript::generate());
}

#[test]
fn the_skill_command_reference_matches_the_commands() {
    let reference = cli::help::cli_reference();
    check_text("skills/athletic-standard/references/cli.md", &reference);
}

#[test]
fn the_skill_lists_every_top_level_command() {
    let skill = fs::read_to_string(repository().join("skills/athletic-standard/SKILL.md")).unwrap();
    for command in cli::spec::find(&[]).unwrap().subcommands {
        let row = format!("| `ath {}", command.name);
        let listed = skill.match_indices(&row).any(|(index, _)| {
            let next = skill[index + row.len()..].chars().next();
            !next.is_some_and(|character| character.is_alphanumeric() || character == '-')
        });
        assert!(listed, "SKILL.md has no table row for ath {}", command.name);
    }
}

// ---- The demo database ---------------------------------------------------

/// Copy a database into a new private folder so reading it leaves no
/// `-shm` file beside the committed copy.
fn private_copy(path: &Path, label: &str) -> PathBuf {
    let directory = temporary_root::new_dir(&format!("ath-generated-{label}-"));
    let copy = directory.join("copy.ath.sqlite");
    fs::copy(path, &copy).unwrap();
    copy
}

fn open_read_only(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

/// The schema and every row of every table, with the header values that SQL
/// can read. Two databases with the same dump hold the same data, even when a
/// different SQLite build laid out the bytes differently.
fn logical_dump(path: &Path) -> BTreeMap<String, Vec<String>> {
    let connection = open_read_only(path);
    let mut dump = BTreeMap::new();
    let pragmas: Vec<String> = ["application_id", "user_version", "page_size", "journal_mode", "encoding"]
        .iter()
        .map(|name| {
            let value: rusqlite::types::Value = connection.pragma_query_value(None, name, |row| row.get(0)).unwrap();
            format!("{name} = {value:?}")
        })
        .collect();
    dump.insert("pragma".to_string(), pragmas);
    let mut statement =
        connection.prepare("SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY type, name").unwrap();
    let schema: Vec<(String, String, String, Option<String>)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    dump.insert("sqlite_master".to_string(), schema.iter().map(|entry| format!("{entry:?}")).collect());
    for (kind, name, _, _) in &schema {
        if kind != "table" || name.starts_with("sqlite_") {
            continue;
        }
        dump.insert(format!("table {name}"), table_rows(&connection, name));
    }
    dump
}

fn table_rows(connection: &Connection, table: &str) -> Vec<String> {
    let mut statement = connection.prepare(&format!("SELECT * FROM \"{table}\"")).unwrap();
    let columns = statement.column_count();
    let mut rows: Vec<String> = statement
        .query_map([], |row| {
            let mut parts = Vec::new();
            for index in 0..columns {
                let value: rusqlite::types::Value = row.get(index)?;
                parts.push(format!("{value:?}"));
            }
            Ok(parts.join(" | "))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    rows.sort();
    rows
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Build the demo into a new temporary folder.
fn generated_demo() -> PathBuf {
    let directory = temporary_root::new_dir("ath-generated-demo-");
    demo_support::write(&directory);
    directory
}

#[test]
fn the_demo_database_matches_its_generator() {
    let generated = generated_demo();
    let committed = repository().join(DEMO_FOLDER);
    let (attachment_name, attachment_bytes) = demo_support::attachment();
    let attachment = format!("{}/{attachment_name}", demo_support::ATTACHMENTS);
    assert_generated_path(&format!("{DEMO_FOLDER}/{}", demo_support::DATABASE));

    if writing() {
        fs::create_dir_all(committed.join(demo_support::ATTACHMENTS)).unwrap();
        for stale in ["athlete.ath.sqlite-wal", "athlete.ath.sqlite-shm"] {
            let _ = fs::remove_file(committed.join(stale));
        }
        fs::copy(generated.join(demo_support::DATABASE), committed.join(demo_support::DATABASE)).unwrap();
        fs::copy(generated.join(&attachment), committed.join(&attachment)).unwrap();
        return;
    }

    let committed_attachment = fs::read(committed.join(&attachment)).unwrap_or_default();
    assert_eq!(committed_attachment, attachment_bytes, "{DEMO_FOLDER}/{attachment} differs from its generator");

    let expected_bytes = fs::read(generated.join(demo_support::DATABASE)).unwrap();
    let committed_bytes = fs::read(committed.join(demo_support::DATABASE)).unwrap_or_default();
    if committed_bytes != expected_bytes {
        // The bytes can differ only in SQLite's own layout. The data must not.
        let expected = logical_dump(&generated.join(demo_support::DATABASE));
        let actual = logical_dump(&private_copy(&committed.join(demo_support::DATABASE), "committed"));
        let differing: Vec<&String> =
            expected.keys().chain(actual.keys()).filter(|key| expected.get(*key) != actual.get(*key)).collect();
        assert!(
            differing.is_empty(),
            "{DEMO_FOLDER}/{} differs from its generator in {differing:?}. \
             Run `ATH_WRITE_GENERATED=1 cargo test --test generated` and commit the result.",
            demo_support::DATABASE
        );
    }
}

#[test]
fn the_demo_holds_every_record_type_a_correction_a_deletion_and_an_attachment() {
    let generated = generated_demo();
    let path = generated.join(demo_support::DATABASE);
    let connection = open_read_only(&path);
    let count = |sql: &str| -> i64 { connection.query_row(sql, [], |row| row.get(0)).unwrap() };
    assert_eq!(count("SELECT count(DISTINCT record_type) FROM records"), 24);
    assert_eq!(count("SELECT count(*) FROM records"), 25);
    assert_eq!(count("SELECT count(*) FROM record_revisions WHERE revision = 2"), 1);
    assert_eq!(count("SELECT count(*) FROM record_deletions"), 1);
    assert_eq!(count("SELECT count(*) FROM measurement_samples"), 1);

    let (name, bytes) = demo_support::attachment();
    let stored: String = connection.query_row("SELECT sha256 FROM attachments", [], |row| row.get(0)).unwrap();
    assert_eq!(stored, sha256_hex(&bytes));
    assert_eq!(fs::read(generated.join(demo_support::ATTACHMENTS).join(name)).unwrap(), bytes);

    drop(connection);

    let opened =
        store::Store::open(&path, store::OpenOptions { read_only: true, allow_outdated_schema: false }).unwrap();
    let metadata = opened.metadata();
    assert_eq!(metadata.athleticstandard_version, package::ATHLETIC_STANDARD_VERSION);
    assert_eq!(metadata.sql_schema_version, store::SQL_SCHEMA_VERSION);
    let result = store::check::check_database(&opened).unwrap();
    assert!(result.valid, "{:?}", result.issues);
}

#[test]
fn the_demo_keeps_the_records_of_the_2_1_0_demo() {
    let generated = generated_demo();
    let new = logical_dump(&generated.join(demo_support::DATABASE));
    let old_path = repository().join("tests/fixtures/version-2-1-0").join(demo_support::DATABASE);
    let old = logical_dump(&private_copy(&old_path, "2-1-0"));
    let athlete_tables: Vec<&String> =
        old.keys().filter(|key| key.starts_with("table ") && *key != "table database_metadata").collect();
    assert!(athlete_tables.len() > 40, "{athlete_tables:?}");
    for table in athlete_tables {
        assert_eq!(new.get(table), old.get(table), "{table}");
    }
}

#[test]
fn generated_paths_avoid_published_history() {
    let current = package::ATHLETIC_STANDARD_VERSION;
    for relative in [
        format!("schema/{current}/athleticstandard.schema.json"),
        "schema/athleticstandard.schema.json".to_string(),
        typescript::GENERATED_PATH.to_string(),
        "skills/athletic-standard/references/cli.md".to_string(),
        format!("{DEMO_FOLDER}/athlete.ath.sqlite"),
    ] {
        assert_generated_path(&relative);
    }
    for protected in [
        "schema/2.1.0/athleticstandard.schema.json",
        "spec/sql/migrations/005-version-2-1-1.sql",
        "tests/fixtures/version-2-1-0/athlete.ath.sqlite",
    ] {
        assert!(std::panic::catch_unwind(|| assert_generated_path(protected)).is_err(), "{protected}");
    }
}
