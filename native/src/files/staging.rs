//! Export staging directories.
//!
//! An export is written into a hidden staging directory next to its final
//! location, then made visible with one rename. Each staging directory holds a
//! marker that names the process that owns it. Cleanup removes a staging
//! directory only when it has that marker and its owner has stopped. A
//! directory that merely has a matching name is left alone.

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
    let mut file = super::create_new_private(&directory.join(EXPORT_STAGING_MARKER))?;
    io::Write::write_all(&mut file, export_staging_marker_text(current_pid()).as_bytes())?;
    file.sync_all()
}

/// The owner named by a staging marker, when the marker is readable.
pub fn read_export_staging_marker(directory: &Path) -> Option<u32> {
    let text = fs::read_to_string(directory.join(EXPORT_STAGING_MARKER)).ok()?;
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
    let mut removed = Vec::new();
    for entry in fs::read_dir(parent)? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with(EXPORT_STAGING_PREFIX) {
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
        let Some(pid) = read_export_staging_marker(&path) else { continue };
        if is_process_alive(pid) {
            continue;
        }
        if fs::remove_dir_all(&path).is_ok() {
            removed.push(path);
        }
    }
    Ok(removed)
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
}
