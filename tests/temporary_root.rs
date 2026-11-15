//! Temporary folders for one test program.
//!
//! Each test program puts its folders under one root named after the program.
//! The root is emptied when the program first asks for a folder, so a run
//! leaves behind at most the folders of its own last run. Tests that remove
//! their own folders still do; this bounds the ones that do not.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

fn root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let program = std::env::current_exe().expect("test program path");
        let stem = program.file_stem().expect("test program name").to_string_lossy().to_string();
        let name = stem.rsplit_once('-').map_or(stem.as_str(), |(name, _)| name).to_string();
        let base =
            athleticstandard::files::simplified(std::fs::canonicalize(std::env::temp_dir()).expect("temporary folder"))
                .join("ath-native-tests");
        let root = base.join(name);
        let _ = std::fs::remove_dir_all(&root);
        athleticstandard::files::create_private_dir_all(&root).expect("test root");
        root
    })
}

/// A new private folder under this program's root.
pub fn new_dir(prefix: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let number = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = root().join(format!("{prefix}{}-{number}", std::process::id()));
    athleticstandard::files::create_private_dir(&path).expect("test folder");
    path
}
