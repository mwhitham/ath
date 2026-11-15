//! The import plan's private staging file (spec section 8). Staging is
//! disposable working data, but it holds athlete values, so it gets the
//! live record's protection: an owner-only folder and file while a preview
//! lives, and no copy left behind when the import finishes, is refused, or
//! fails.
//!
//! Staging lives in the system temporary folder. This program points that
//! folder at a new private folder so it sees only its own staging files. It
//! holds one test, so no other test in this program reads the environment
//! while the test changes it.
#![cfg_attr(not(unix), allow(unused))]

mod import_support;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use athleticstandard::import::{self, CandidateSink, DetectedExport, ImportOptions, ProviderReader};
use athleticstandard::model::{RecordActor, RecordType};
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::{Changes, Deletion};
use import_support::{new_store, whoop_fixture};

const RECEIVED_AT: &str = "2026-10-13T12:00:00.000Z";
const STAGING_PREFIX: &str = "ath-import-";

fn options() -> ImportOptions {
    ImportOptions { received_at: Some(RECEIVED_AT.into()) }
}

/// The staging folders in a temporary folder.
fn staging(temporary: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(temporary)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(STAGING_PREFIX))
        .map(|entry| entry.path())
        .collect();
    found.sort();
    found
}

/// A staging folder is owner-only and holds only the owner-only plan file.
#[cfg(unix)]
fn assert_private(folder: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mode = |path: &Path| std::fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777;
    assert_eq!(mode(folder), 0o700, "{}", folder.display());
    let names: Vec<String> =
        std::fs::read_dir(folder).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into()).collect();
    assert_eq!(names, ["plan.sqlite"], "{}", folder.display());
    assert_eq!(mode(&folder.join("plan.sqlite")), 0o600, "{}", folder.display());
}

/// The WHOOP reader, checking the staging folder while candidates arrive,
/// then failing partway.
struct StopsPartway {
    temporary: PathBuf,
}

#[cfg(unix)]
impl ProviderReader for StopsPartway {
    fn read(&self, export: &DetectedExport, sink: &mut CandidateSink<'_>) -> athleticstandard::Result<()> {
        import::whoop::WhoopReader.read(export, sink)?;
        let found = staging(&self.temporary);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_private(&found[0]);
        Err(athleticstandard::Error::other("the reader stopped partway"))
    }
}

/// Run `ath` with its temporary folder set to `temporary`.
fn ath(cwd: &Path, temporary: &Path, argv: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(argv)
        .current_dir(cwd)
        .env_clear()
        .env("TMPDIR", temporary)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn private_dir(path: &Path) -> PathBuf {
    athleticstandard::files::create_private_dir(path).unwrap();
    path.to_path_buf()
}

#[cfg(unix)]
#[test]
fn the_staging_file_is_private_and_removed_after_every_ending() {
    let root = tempfile::tempdir().unwrap();
    let root = athleticstandard::files::simplified(root.path().canonicalize().unwrap());
    let temporary = private_dir(&root.join("temporary"));
    let work = private_dir(&root.join("work"));
    std::env::set_var("TMPDIR", &temporary);
    assert_eq!(std::env::temp_dir().canonicalize().unwrap(), temporary);

    // While two previews live, each has its own private staging folder.
    let mut store = new_store(&private_dir(&work.join("library")));
    let first = import::preview(&store, &whoop_fixture(), &options()).unwrap();
    let second = import::preview(&store, &whoop_fixture(), &options()).unwrap();
    let found = staging(&temporary);
    assert_eq!(found.len(), 2, "{found:?}");
    found.iter().for_each(|folder| assert_private(folder));

    // A finished import removes its staging folder.
    first.apply(&mut store).unwrap();
    drop(first);
    assert_eq!(staging(&temporary).len(), 1);

    // The second plan is now stale. Its refusal removes its folder.
    let error = second.apply(&mut store).unwrap_err();
    assert!(matches!(error, athleticstandard::Error::Stale(_)), "{error:?}");
    drop(second);
    assert_eq!(staging(&temporary), Vec::<PathBuf>::new());

    // A plan with a conflict is refused. A normally deleted record that an
    // import key still names is a conflict.
    let measurement = store
        .list_records(&ListOptions { record_type: Some(RecordType::PointMeasurement), ..ListOptions::default() })
        .unwrap()
        .remove(0);
    store
        .apply(Changes {
            deletions: vec![Deletion {
                record_id: measurement.record_id.clone(),
                expected_revision: measurement.current_revision,
                deleted_by: RecordActor::Athlete,
                deleted_at: None,
            }],
            ..Changes::default()
        })
        .unwrap();
    let conflicted = import::preview(&store, &whoop_fixture(), &options()).unwrap();
    assert!(!conflicted.conflicts.is_empty());
    let error = conflicted.apply(&mut store).unwrap_err();
    assert!(matches!(error, athleticstandard::Error::Refused(_)), "{error:?}");
    drop(conflicted);
    assert_eq!(staging(&temporary), Vec::<PathBuf>::new());

    // A reader that fails partway removes the staging folder it filled.
    let export = import::detect_export(&whoop_fixture()).unwrap();
    let reader = StopsPartway { temporary: temporary.clone() };
    let error = import::preview_export(&store, &reader, &export, &options()).unwrap_err();
    assert_eq!(error.to_string(), "the reader stopped partway");
    assert_eq!(staging(&temporary), Vec::<PathBuf>::new());
    store.close().unwrap();

    // The command line leaves no staging folder after a preview, a write, or
    // a refusal.
    let cli_temporary = private_dir(&root.join("cli-temporary"));
    let folder = private_dir(&work.join("cli"));
    new_store(&folder).close().unwrap();
    let fixture = whoop_fixture().display().to_string();
    let preview = ath(&folder, &cli_temporary, &["import", &fixture]);
    assert!(preview.status.success(), "{}", String::from_utf8_lossy(&preview.stderr));
    assert_eq!(staging(&cli_temporary), Vec::<PathBuf>::new());
    let written = ath(&folder, &cli_temporary, &["import", &fixture, "--yes"]);
    assert!(written.status.success(), "{}", String::from_utf8_lossy(&written.stderr));
    assert_eq!(staging(&cli_temporary), Vec::<PathBuf>::new());
    let store = athleticstandard::store::Store::open(
        folder.join("athlete.ath.sqlite"),
        athleticstandard::store::OpenOptions { read_only: true, allow_outdated_schema: false },
    )
    .unwrap();
    let measurement = store
        .list_records(&ListOptions { record_type: Some(RecordType::PointMeasurement), ..ListOptions::default() })
        .unwrap()
        .remove(0);
    drop(store);
    let delete = ath(&folder, &cli_temporary, &["delete", &measurement.record_id, "--revision", "1", "--yes"]);
    assert!(delete.status.success(), "{}", String::from_utf8_lossy(&delete.stderr));
    let refused = ath(&folder, &cli_temporary, &["import", &fixture, "--yes"]);
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("unresolved conflict"), "{stderr}");
    assert_eq!(staging(&cli_temporary), Vec::<PathBuf>::new());
}
