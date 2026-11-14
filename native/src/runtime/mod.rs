//! Optional runtime services: the operating system's password store and
//! model gateways.
//!
//! Local record operations never use this module. A network client or a
//! password store starts only when a command that needs one constructs it.
//! Every service takes its environment, tool paths, and base URLs as
//! parameters, so tests never touch a real password store or a live service.

pub mod gateway;
pub mod keyring;
pub mod model_client;

mod collate;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Read access to environment variables. Tests pass a map instead of
/// changing the process environment.
pub trait Environment {
    /// The value of `name`, or None when it is unset or not valid Unicode.
    fn var(&self, name: &str) -> Option<String>;
}

/// The environment of this process.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessEnvironment;

impl Environment for ProcessEnvironment {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

impl Environment for HashMap<String, String> {
    fn var(&self, name: &str) -> Option<String> {
        self.get(name).cloned()
    }
}

impl Environment for [(&str, &str)] {
    fn var(&self, name: &str) -> Option<String> {
        self.iter().find(|(key, _)| *key == name).map(|(_, value)| (*value).to_string())
    }
}

impl<const N: usize> Environment for [(&str, &str); N] {
    fn var(&self, name: &str) -> Option<String> {
        self.as_slice().var(name)
    }
}

/// The first executable named `name` on `PATH`, as `which` or `where` finds it.
/// On Windows each `PATHEXT` extension is tried.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        if directory.as_os_str().is_empty() {
            continue;
        }
        for candidate in executable_names(name) {
            let full = directory.join(&candidate);
            if is_executable(&full) {
                return Some(full);
            }
        }
    }
    None
}

#[cfg(windows)]
fn executable_names(name: &str) -> Vec<String> {
    let mut names = Vec::new();
    if Path::new(name).extension().is_some() {
        names.push(name.to_string());
    }
    let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
    for extension in extensions.split(';').filter(|extension| !extension.is_empty()) {
        names.push(format!("{name}{}", extension.to_ascii_lowercase()));
    }
    names
}

#[cfg(not(windows))]
fn executable_names(name: &str) -> Vec<String> {
    vec![name.to_string()]
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
