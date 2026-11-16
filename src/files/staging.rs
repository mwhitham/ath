//! Export staging directories and owned temporary directories.
//!
//! An export is written into a hidden staging directory next to its final
//! location, then made visible with one rename. Each staging directory holds a
//! marker that names the process that owns it. Cleanup removes a staging
//! directory only when it has that marker and its owner has stopped. A
//! directory that merely has a matching name is left alone. Temporary
//! directories for import working data follow the same rule.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::process::{current_pid, is_process_alive};

/// The marker file written into every export staging directory this tool creates.
pub const EXPORT_STAGING_MARKER: &str = ".athletic-standard-export-staging";

/// The name prefix of export staging directories.
pub const EXPORT_STAGING_PREFIX: &str = ".ath-export-";

/// `.ath-export-<pid>-<millis>-<final name>`.
pub fn export_staging_directory_name(pid: u32, timestamp_ms: i64, final_name: &str) -> String {
    format!("{EXPORT_STAGING_PREFIX}{pid}-{timestamp_ms}-{final_name}")
}

/// The marker text: `{"pid":<pid>}`.
pub fn export_staging_marker_text(pid: u32) -> String {
    format!("{{\"pid\":{pid}}}")
}

/// Write this process's marker into a new staging directory.
pub fn write_export_staging_marker(directory: &Path) -> io::Result<()> {
    write_marker(directory, EXPORT_STAGING_MARKER)
}

/// The owner named by a staging marker, when the marker is readable.
pub fn read_export_staging_marker(directory: &Path) -> Option<u32> {
    read_marker(directory, EXPORT_STAGING_MARKER)
}

fn write_marker(directory: &Path, marker: &str) -> io::Result<()> {
    let mut file = super::create_new_private(&directory.join(marker))?;
    io::Write::write_all(&mut file, export_staging_marker_text(current_pid()).as_bytes())?;
    file.sync_all()
}

fn read_marker(directory: &Path, marker: &str) -> Option<u32> {
    let text = fs::read_to_string(directory.join(marker)).ok()?;
    let value = crate::json::parse(&text).ok()?;
    let pid = value.get("pid")?.as_f64()?;
    if !pid.is_finite() || pid.fract() != 0.0 || pid < 0.0 || pid > f64::from(u32::MAX) {
        return None;
    }
    Some(pid as u32)
}

/// Remove abandoned export staging directories in `parent`. A directory is
/// abandoned when it has a marker and its owner process has stopped. Errors
/// are ignored: this is best-effort cleanup and must not stop a new export.
/// Returns the directories removed.
pub fn cleanup_stale_export_staging_directories(parent: &Path, except: Option<&Path>) -> io::Result<Vec<PathBuf>> {
    remove_abandoned(parent, EXPORT_STAGING_PREFIX, EXPORT_STAGING_MARKER, except)
}

fn remove_abandoned(parent: &Path, prefix: &str, marker: &str, except: Option<&Path>) -> io::Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    for entry in fs::read_dir(parent)? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with(prefix) {
            continue;
        }
        let path = parent.join(name);
        if except.is_some_and(|except| except == path) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else { continue };
        if !metadata.is_dir() {
            continue;
        }
        let Some(pid) = read_marker(&path, marker) else { continue };
        if is_process_alive(pid) {
            continue;
        }
        if fs::remove_dir_all(&path).is_ok() {
            removed.push(path);
        }
    }
    Ok(removed)
}

/// The marker in a private temporary directory that names its owner process.
pub const TEMPORARY_OWNER_MARKER: &str = ".athletic-standard-owner";

/// A new private directory in the system temporary folder for working data,
/// such as an import plan. It holds a marker that names this process.
///
/// A process that is stopped cannot remove its directory, and the data in it
/// is as private as the athlete record. So each new directory first removes
/// the directories with the same prefix whose marker names a stopped process.
/// A directory without a marker, or with a running owner, is left alone.
pub fn make_owned_temporary_directory(prefix: &str) -> io::Result<PathBuf> {
    let directory = super::make_temporary_directory(prefix)?;
    if let Err(error) = write_marker(&directory, TEMPORARY_OWNER_MARKER) {
        let _ = fs::remove_dir_all(&directory);
        return Err(error);
    }
    if let Some(parent) = directory.parent() {
        // Best effort: a failed cleanup must not stop the new work.
        let _ = remove_abandoned(parent, prefix, TEMPORARY_OWNER_MARKER, Some(&directory));
    }
    Ok(directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary() -> PathBuf {
        crate::files::make_temporary_directory("ath-staging-").unwrap()
    }

    fn stage(parent: &Path, pid: u32, name: &str) -> PathBuf {
        let path = parent.join(export_staging_directory_name(pid, 1_700_000_000_000, name));
        fs::create_dir(&path).unwrap();
        fs::write(path.join(EXPORT_STAGING_MARKER), export_staging_marker_text(pid)).unwrap();
        fs::write(path.join("athlete.ath.json"), "{}").unwrap();
        path
    }

    #[test]
    fn removes_a_marked_directory_whose_owner_stopped() {
        let parent = temporary();
        let stale = stage(&parent, 99_999_999, "package");
        cleanup_stale_export_staging_directories(&parent, None).unwrap();
        assert!(!stale.exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn keeps_a_marked_directory_whose_owner_is_running() {
        let parent = temporary();
        let live = stage(&parent, current_pid(), "package");
        cleanup_stale_export_staging_directories(&parent, None).unwrap();
        assert!(live.exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn ignores_unmarked_directories_with_a_matching_name() {
        let parent = temporary();
        let copy = parent.join(".ath-export-424242-old-copy");
        fs::create_dir(&copy).unwrap();
        fs::write(copy.join("notes.txt"), "keep me").unwrap();
        cleanup_stale_export_staging_directories(&parent, None).unwrap();
        assert!(copy.join("notes.txt").exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn leaves_finished_packages_and_the_excepted_directory() {
        let parent = temporary();
        let finished = parent.join("package");
        fs::create_dir(&finished).unwrap();
        fs::write(finished.join("athlete.ath.json"), "{}").unwrap();
        let stale = stage(&parent, 99_999_998, "other-package");
        let excepted = stage(&parent, 99_999_997, "mine");
        cleanup_stale_export_staging_directories(&parent, Some(&excepted)).unwrap();
        assert!(finished.join("athlete.ath.json").exists());
        assert!(!stale.exists());
        assert!(excepted.exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn a_new_owned_directory_removes_only_abandoned_ones_with_its_prefix() {
        // A prefix of its own, so other tests' folders are never touched.
        let prefix = format!("ath-owned-{}-", crate::ids::new_record_id());
        let make = |pid: Option<u32>| {
            let path = crate::files::make_temporary_directory(&prefix).unwrap();
            if let Some(pid) = pid {
                fs::write(path.join(TEMPORARY_OWNER_MARKER), export_staging_marker_text(pid)).unwrap();
            }
            fs::write(path.join("plan.sqlite"), "working data").unwrap();
            path
        };
        let abandoned = make(Some(99_999_996));
        let running = make(Some(current_pid()));
        let unmarked = make(None);
        let created = make_owned_temporary_directory(&prefix).unwrap();
        assert_eq!(read_marker(&created, TEMPORARY_OWNER_MARKER), Some(current_pid()));
        assert!(!abandoned.exists());
        assert!(running.join("plan.sqlite").exists());
        assert!(unmarked.join("plan.sqlite").exists());
        for path in [running, unmarked, created] {
            fs::remove_dir_all(path).unwrap();
        }
    }
}
