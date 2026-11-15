//! Package export: byte comparison with version 2.1.0 and staging behavior.
//! Ported from tests/v1-export-import.test.ts and tests/v1-export-staging.test.ts.
#![cfg_attr(not(unix), allow(unused_imports, dead_code))]

#[path = "package_support/mod.rs"]
mod common;

use std::fs;

use athleticstandard::files::staging;
use athleticstandard::model::RecordFormatVersion;
use athleticstandard::package::{export_package, ExportOptions};
use common::*;

fn timed() -> ExportOptions {
    ExportOptions { exported_at: Some(EXPORTED_AT.into()) }
}

#[test]
fn demo_export_matches_the_2_1_0_bytes_after_the_version_change() {
    let directory = temporary("ath-export-demo-");
    let database = migrated_demo_copy(&directory);
    let store = open_read_only(&database);
    let output = directory.join("package");
    let result = export_package(&store, &output, &timed()).unwrap();
    assert_eq!(result.directory, output.display().to_string());
    assert_eq!(result.database_id, "01890f3e-e7d3-7cc0-88c8-000000000001");
    assert_eq!(result.database_revision, 3);
    assert_eq!(result.manifest.athleticstandard_version, RecordFormatVersion::V2_1_1);
    assert_eq!(result.manifest.exported_at.as_deref(), Some(EXPORTED_AT));

    let written = files_below(&output);
    assert_same_package(&as_version_2_1_0(&written), &golden_package_files("demo"));

    let untimed = directory.join("untimed");
    export_package(&store, &untimed, &ExportOptions::default()).unwrap();
    let untimed_files = as_version_2_1_0(&files_below(&untimed));
    assert_eq!(
        String::from_utf8(untimed_files["export-manifest.json"].clone()).unwrap(),
        fs::read_to_string(golden("demo").join("manifest-untimed.json")).unwrap()
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn the_result_serializes_like_the_old_json_output() {
    let directory = temporary("ath-export-json-");
    let database = migrated_demo_copy(&directory);
    let store = open_read_only(&database);
    let result = export_package(&store, &directory.join("package"), &ExportOptions::default()).unwrap();
    let value = athleticstandard::json::to_value(&result);
    let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["directory", "database_id", "database_revision", "manifest"]);
    let manifest_keys: Vec<&String> = value["manifest"].as_object().unwrap().keys().collect();
    assert_eq!(
        manifest_keys,
        [
            "athleticstandard_version",
            "source_database_id",
            "database_revision",
            "files",
            "record_counts",
            "erasure_marker_count"
        ]
    );
    let file_keys: Vec<&String> = value["manifest"]["files"][0].as_object().unwrap().keys().collect();
    assert_eq!(file_keys, ["file", "sha256", "byte_count"]);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn two_exports_of_one_revision_are_byte_identical() {
    let directory = temporary("ath-export-twice-");
    let database = migrated_demo_copy(&directory);
    let store = open_read_only(&database);
    export_package(&store, &directory.join("first"), &ExportOptions::default()).unwrap();
    export_package(&store, &directory.join("second"), &ExportOptions::default()).unwrap();
    assert_eq!(files_below(&directory.join("first")), files_below(&directory.join("second")));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn concurrent_exports_into_one_parent_both_succeed() {
    let directory = temporary("ath-export-concurrent-");
    let database = migrated_demo_copy(&directory);
    let handles: Vec<_> = ["package-a", "package-b"]
        .into_iter()
        .map(|name| {
            let database = database.clone();
            let output = directory.join(name);
            std::thread::spawn(move || {
                let store = open_read_only(&database);
                export_package(&store, &output, &ExportOptions::default()).unwrap()
            })
        })
        .collect();
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(fs::read_to_string(std::path::Path::new(&result.directory).join("export-manifest.json"))
            .unwrap()
            .contains("database_revision"));
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn an_existing_destination_or_link_is_refused_and_left_alone() {
    let directory = temporary("ath-export-existing-");
    let database = migrated_demo_copy(&directory);
    let store = open_read_only(&database);
    let existing = directory.join("existing");
    fs::create_dir(&existing).unwrap();
    fs::write(existing.join("keep.txt"), "keep").unwrap();
    let error = export_package(&store, &existing, &ExportOptions::default()).unwrap_err();
    assert_eq!(error.to_string(), format!("export directory already exists: {}", existing.display()));
    assert_eq!(fs::read_to_string(existing.join("keep.txt")).unwrap(), "keep");
    #[cfg(unix)]
    {
        let link = directory.join("link");
        std::os::unix::fs::symlink(&existing, &link).unwrap();
        let error = export_package(&store, &link, &ExportOptions::default()).unwrap_err();
        assert_eq!(error.to_string(), format!("refusing export through a symbolic link: {}", link.display()));
    }
    let leftovers: Vec<_> = fs::read_dir(&directory)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(staging::EXPORT_STAGING_PREFIX))
        .collect();
    assert!(leftovers.is_empty());
    let invalid =
        export_package(&store, &directory.join("x"), &ExportOptions { exported_at: Some("yesterday".into()) });
    assert!(invalid.is_err());
    assert!(!directory.join("x").exists());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn finished_packages_have_no_marker_and_survive_a_later_cleanup() {
    let directory = temporary("ath-export-finished-");
    let database = migrated_demo_copy(&directory);
    let store = open_read_only(&database);
    let finished = directory.join("package");
    export_package(&store, &finished, &ExportOptions::default()).unwrap();
    let names: Vec<String> =
        fs::read_dir(&finished).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into()).collect();
    assert!(!names.contains(&staging::EXPORT_STAGING_MARKER.to_string()));
    assert!(names.contains(&"athlete.ath.json".to_string()));

    // A staging folder left by a stopped export is removed by the next export.
    let abandoned = directory.join(staging::export_staging_directory_name(99_999_997, 1, "abandoned"));
    fs::create_dir(&abandoned).unwrap();
    fs::write(abandoned.join(staging::EXPORT_STAGING_MARKER), staging::export_staging_marker_text(99_999_997)).unwrap();
    fs::write(abandoned.join("athlete.ath.json"), "{}").unwrap();
    export_package(&store, &directory.join("package-two"), &ExportOptions::default()).unwrap();
    assert!(!abandoned.exists());
    assert!(finished.join("athlete.ath.json").exists());
    fs::remove_dir_all(directory).unwrap();
}

/// A missing attachment file is named with its record (deliberate-changes.md,
/// entry 38), and nothing is published.
#[test]
fn a_missing_attachment_file_is_named() {
    let directory = temporary("ath-export-attachment-missing-");
    let database = migrated_demo_copy(&directory);
    let store = open_read_only(&database);
    let file = directory.join("attachments").join(DEMO_ATTACHMENT);
    fs::remove_file(&file).unwrap();
    let error = export_package(&store, &directory.join("package"), &ExportOptions::default()).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("attachment file is missing: {} (01890f3e-e7d3-7cc0-88c8-00000000000e)", file.display())
    );
    assert!(!directory.join("package").exists());
    fs::remove_dir_all(directory).unwrap();
}

/// Export copies an attachment only from the file itself. A file, or the
/// attachments folder, that is a symbolic link is refused, even when it leads
/// to the right bytes. 2.1.0 followed the link (deliberate-changes.md,
/// packages entry 3).
#[cfg(unix)]
#[test]
fn an_attachment_reached_through_a_symbolic_link_is_refused() {
    let directory = temporary("ath-export-attachment-link-");
    let database = migrated_demo_copy(&directory);
    let store = open_read_only(&database);
    let attachments = directory.join("attachments");
    let real = directory.join("elsewhere");
    fs::create_dir(&real).unwrap();
    fs::rename(attachments.join(DEMO_ATTACHMENT), real.join(DEMO_ATTACHMENT)).unwrap();

    std::os::unix::fs::symlink(real.join(DEMO_ATTACHMENT), attachments.join(DEMO_ATTACHMENT)).unwrap();
    let error = export_package(&store, &directory.join("package"), &ExportOptions::default()).unwrap_err();
    let linked = attachments.join(DEMO_ATTACHMENT);
    assert_eq!(error.to_string(), format!("Refusing to use path through a symbolic link: {}", linked.display()));
    assert!(!directory.join("package").exists());

    fs::remove_file(&linked).unwrap();
    fs::remove_dir(&attachments).unwrap();
    std::os::unix::fs::symlink(&real, &attachments).unwrap();
    let error = export_package(&store, &directory.join("package"), &ExportOptions::default()).unwrap_err();
    assert_eq!(error.to_string(), format!("Refusing to use path through a symbolic link: {}", attachments.display()));
    assert!(!directory.join("package").exists());
    let leftovers = fs::read_dir(&directory)
        .unwrap()
        .filter(|entry| {
            entry.as_ref().unwrap().file_name().to_string_lossy().starts_with(staging::EXPORT_STAGING_PREFIX)
        })
        .count();
    assert_eq!(leftovers, 0);
    fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn package_files_and_folders_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let directory = temporary("ath-export-private-");
    let database = migrated_demo_copy(&directory);
    let store = open_read_only(&database);
    let output = directory.join("package");
    export_package(&store, &output, &ExportOptions::default()).unwrap();
    for folder in [output.clone(), output.join("series"), output.join("attachments")] {
        assert_eq!(fs::metadata(&folder).unwrap().permissions().mode() & 0o777, 0o700, "{}", folder.display());
    }
    for name in files_below(&output).keys() {
        let mode = fs::metadata(output.join(name)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{name}");
    }
    fs::remove_dir_all(directory).unwrap();
}
