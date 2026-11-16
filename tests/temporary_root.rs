//! Temporary folders for one test program.
//!
//! Each run of a test program puts its folders under one root named after the
//! program and the run's process ID. When a run first asks for a folder, it
//! removes the roots of earlier runs whose process has stopped. A run that is
//! still going keeps its root, so two runs of one program can share a machine.
//! A run leaves behind at most its own folders. Tests that remove their own
//! folders still do; this bounds the ones that do not.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use athleticstandard::files::process::is_process_alive;

fn root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let program = std::env::current_exe().expect("test program path");
        let stem = program.file_stem().expect("test program name").to_string_lossy().to_string();
        let name = stem.rsplit_once('-').map_or(stem.as_str(), |(name, _)| name).to_string();
        let base =
            athleticstandard::files::simplified(std::fs::canonicalize(std::env::temp_dir()).expect("temporary folder"))
                .join("ath-native-tests");
        let program_root = base.join(name);
        athleticstandard::files::create_private_dir_all(&program_root).expect("test program root");
        if let Ok(entries) = std::fs::read_dir(&program_root) {
            for entry in entries.flatten() {
                // A name that is not a process ID comes from the earlier
                // layout, which kept every folder directly under this root.
                let running =
                    entry.file_name().to_str().and_then(|name| name.parse::<u32>().ok()).is_some_and(is_process_alive);
                if !running {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
        // A root with this run's process ID is from a stopped run that had
        // the same ID.
        let root = program_root.join(std::process::id().to_string());
        let _ = std::fs::remove_dir_all(&root);
        athleticstandard::files::create_private_dir_all(&root).expect("test root");
        root
    })
}

/// A new private folder under this run's root.
pub fn new_dir(prefix: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let number = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = root().join(format!("{prefix}{}-{number}", std::process::id()));
    athleticstandard::files::create_private_dir(&path).expect("test folder");
    path
}
