//! Untrusted provider input, ported from `tests/import-safety.test.ts`, and the
//! `ath import` route order from `src/program.ts` and `src/shared-import.ts`.
//!
//! The attachment media-type check in that file belongs to attachment import,
//! not provider import, and is not covered here.

mod import_support;

use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::symlink;

use athleticstandard::import::{self, input, ImportOptions, ImportRoute, SharedImport};
use import_support::{new_store, TempDir};

fn replace_every(bytes: &[u8], before: &str, after: &str) -> Vec<u8> {
    assert_eq!(before.len(), after.len());
    let mut out = bytes.to_vec();
    let mut index = 0;
    while let Some(found) = out[index..].windows(before.len()).position(|window| window == before.as_bytes()) {
        let start = index + found;
        out[start..start + after.len()].copy_from_slice(after.as_bytes());
        index = start + after.len();
    }
    out
}

fn zip_with(path: &std::path::Path, name: &str, bytes: &[u8]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(bytes).unwrap();
    zip.finish().unwrap();
}

#[test]
fn rejects_archive_traversal_before_reading_an_entry() {
    let directory = TempDir::new();
    let safe = directory.path().join("safe.zip");
    zip_with(&safe, "aa/evil.csv", b"day,readiness_score\n2026-01-01,90\n");
    let unsafe_zip = directory.path().join("unsafe.zip");
    std::fs::write(&unsafe_zip, replace_every(&std::fs::read(&safe).unwrap(), "aa/evil.csv", "../evil.csv")).unwrap();
    let error = input::open_zip(&unsafe_zip).unwrap_err().to_string();
    assert_eq!(error, "invalid relative path: ../evil.csv");
    // Detection reads the same index, so the whole import is refused.
    let error = import::detect_export(&unsafe_zip).unwrap_err().to_string();
    assert_eq!(error, "invalid relative path: ../evil.csv");
    let error = import::classify(&unsafe_zip, false).unwrap_err().to_string();
    assert_eq!(error, "invalid relative path: ../evil.csv");
}

#[test]
fn rejects_absolute_and_backslash_archive_names() {
    let directory = TempDir::new();
    let safe = directory.path().join("safe.zip");
    zip_with(&safe, "aa/evil.csv", b"x\n");
    let absolute = directory.path().join("absolute.zip");
    std::fs::write(&absolute, replace_every(&std::fs::read(&safe).unwrap(), "aa/evil.csv", "/a/evil.csv")).unwrap();
    assert_eq!(input::open_zip(&absolute).unwrap_err().to_string(), "absolute path: /a/evil.csv");
    let drive = directory.path().join("drive.zip");
    std::fs::write(&drive, replace_every(&std::fs::read(&safe).unwrap(), "aa/evil.csv", "C:/evil.csv")).unwrap();
    assert_eq!(input::open_zip(&drive).unwrap_err().to_string(), "absolute path: C:/evil.csv");
    let backslash = directory.path().join("backslash.zip");
    std::fs::write(&backslash, replace_every(&std::fs::read(&safe).unwrap(), "aa/evil.csv", "aa\\evil.csv")).unwrap();
    assert_eq!(input::open_zip(&backslash).unwrap_err().to_string(), "invalid characters in fileName: aa\\evil.csv");
}

#[test]
fn rejects_archive_links_and_extreme_expansion_ratios() {
    let directory = TempDir::new();
    let linked = directory.path().join("linked.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&linked).unwrap());
    zip.add_symlink("export.xml", "target", zip::write::SimpleFileOptions::default()).unwrap();
    zip.finish().unwrap();
    let error = input::open_zip(&linked).unwrap_err().to_string();
    assert_eq!(error, "archive entry is a symbolic link: export.xml");

    let expanded = directory.path().join("expanded.zip");
    zip_with(&expanded, "export.xml", &vec![0_u8; 8 * 1024 * 1024]);
    let error = input::open_zip(&expanded).unwrap_err().to_string();
    assert_eq!(error, "archive entry exceeds the 1000:1 expansion limit: export.xml");
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_inside_provider_directories() {
    let directory = TempDir::new();
    let outside = directory.path().join("outside.csv");
    std::fs::write(
        &outside,
        "Cycle start time,Cycle end time,Cycle timezone\n2026-01-01 00:00:00,2026-01-01 08:00:00,UTC+00:00\n",
    )
    .unwrap();
    let provider = directory.path().join("provider");
    std::fs::create_dir(&provider).unwrap();
    symlink(&outside, provider.join("physiological_cycles.csv")).unwrap();
    let export = import::detect_export(&provider).unwrap();
    assert_eq!(export.format, import::ExportFormat::Whoop);
    let error = import::detect::read_csv_bundle(&export).unwrap_err().to_string();
    assert!(error.contains("symbolic link"), "{error}");

    let store_directory = TempDir::new();
    let store = new_store(store_directory.path());
    let error = import::preview(&store, &provider, &ImportOptions::default()).unwrap_err().to_string();
    assert_eq!(
        error,
        format!("refusing symbolic link in provider export: {}", provider.join("physiological_cycles.csv").display())
    );
    assert_eq!(store.database_revision().unwrap(), 0);
}

#[test]
fn a_zip_entry_is_measured_while_it_is_read() {
    let directory = TempDir::new();
    let path = directory.path().join("data.zip");
    zip_with(&path, "folder/physiological_cycles.csv", b"Cycle start time\n");
    let zip = input::open_zip(&path).unwrap();
    assert_eq!(zip.read_text("folder/physiological_cycles.csv").unwrap(), "Cycle start time\n");
    let length = zip.read_entry("folder/physiological_cycles.csv", |reader| {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(bytes.len())
    });
    assert_eq!(length.unwrap(), 17);
    assert!(zip.read_text("missing.csv").is_err());
}

#[test]
fn routes_follow_the_2_1_0_order() {
    let directory = TempDir::new();
    let root = directory.path();
    let whoop = root.join("whoop");
    import_support::write_directory(&whoop, &import_support::fixture_files());
    assert_eq!(import::classify(&whoop, false).unwrap(), ImportRoute::Provider);
    assert_eq!(import::classify(&whoop, true).unwrap(), ImportRoute::Mapped);

    std::fs::write(root.join("ride.gpx"), "<gpx/>").unwrap();
    assert_eq!(import::classify(&root.join("ride.gpx"), false).unwrap(), ImportRoute::Shared(SharedImport::Garmin));
    std::fs::write(root.join("activity.json"), r#"{"activityName":"x"}"#).unwrap();
    assert_eq!(
        import::classify(&root.join("activity.json"), false).unwrap(),
        ImportRoute::Shared(SharedImport::Garmin)
    );
    std::fs::write(root.join("package.json"), r#"{"athleticstandard_version":"2.1.0"}"#).unwrap();
    assert_eq!(import::classify(&root.join("package.json"), false).unwrap(), ImportRoute::Package);

    let strava = root.join("strava");
    std::fs::create_dir(&strava).unwrap();
    std::fs::write(strava.join("activities.csv"), "x\n").unwrap();
    assert_eq!(import::classify(&strava, false).unwrap(), ImportRoute::Shared(SharedImport::Strava));

    // An Apple export's routes are part of the Apple export.
    let apple = root.join("apple");
    std::fs::create_dir_all(apple.join("workout-routes")).unwrap();
    std::fs::write(apple.join("export.xml"), "<HealthData/>").unwrap();
    std::fs::write(apple.join("workout-routes/route.gpx"), "<gpx/>").unwrap();
    std::fs::write(apple.join("route.gpx"), "<gpx/>").unwrap();
    assert_eq!(import::classify(&apple, false).unwrap(), ImportRoute::Provider);

    let package = root.join("restored");
    std::fs::create_dir(&package).unwrap();
    std::fs::write(package.join("athlete.ath.json"), "{}").unwrap();
    assert_eq!(import::classify(&package, false).unwrap(), ImportRoute::Package);

    // A symbolic link is never a shared import. Creating one needs Unix or
    // extra Windows privileges, so this case runs on Unix.
    #[cfg(unix)]
    {
        symlink(root.join("ride.gpx"), root.join("linked.gpx")).unwrap();
        assert_eq!(import::classify(&root.join("linked.gpx"), false).unwrap(), ImportRoute::Provider);
    }
}
