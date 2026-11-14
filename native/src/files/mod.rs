//! File safety for athlete data.
//!
//! Athlete files are private health data. This module owns the rules every
//! file write follows: no path through a symbolic link, no directory that other
//! local users can write, owner-only permissions where the system supports
//! them, exclusive creation instead of overwriting, and synchronized writes
//! before a final rename.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};

/// Make a path absolute against the current directory without touching the
/// file system. `..` and `.` components are resolved lexically, as Node's
/// `path.resolve` does.
pub fn absolute(path: &Path) -> io::Result<PathBuf> {
    let joined = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir()?.join(path) };
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Ok(out)
}

/// Refuse a path when any existing component is a symbolic link. With
/// `allow_missing_tail`, stop at the first component that does not exist.
pub fn refuse_symbolic_link_components(path: &Path, allow_missing_tail: bool) -> Result<()> {
    let path = absolute(path)?;
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::refused(format!(
                    "Refusing to use path through a symbolic link: {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound && allow_missing_tail => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Refuse a database path whose directory other local users can write.
pub fn assert_parent_not_writable_by_others(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let parent = absolute(path)?.parent().map(Path::to_path_buf).unwrap_or_default();
        let metadata = fs::symlink_metadata(&parent)?;
        if metadata.permissions().mode() & 0o022 != 0 {
            return Err(Error::refused(format!(
                "Refusing database path in a directory writable by other local users: {}",
                parent.display()
            )));
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Warnings for a database and its sidecars that other local users can read.
pub fn readable_path_warnings(path: &Path) -> Vec<String> {
    let mut warnings = Vec::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let candidates = [path.to_path_buf(), sidecar(path, "-wal"), sidecar(path, "-shm"), parent];
        for candidate in candidates {
            if let Ok(metadata) = fs::symlink_metadata(&candidate) {
                if metadata.permissions().mode() & 0o044 != 0 {
                    warnings.push(format!(
                        "Sensitive athlete data path is readable by other local users: {}",
                        candidate.display()
                    ));
                }
            }
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    warnings
}

/// `path` with a suffix such as `-wal` added to the file name.
pub fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// Set owner-only permissions on a file when it exists. Windows keeps its
/// inherited access control list.
pub fn set_owner_only(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

/// Owner-only permissions on a database and its WAL sidecars.
pub fn set_owner_only_database(path: &Path) -> io::Result<()> {
    set_owner_only(path)?;
    set_owner_only(&sidecar(path, "-wal"))?;
    set_owner_only(&sidecar(path, "-shm"))
}

/// Create a new file that must not exist yet, readable only by its owner.
pub fn create_new_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Create a directory and its parents with owner-only permissions on new levels.
pub fn create_private_dir_all(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

/// Create one new directory with owner-only permissions. It must not exist.
pub fn create_private_dir(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

/// The identity of a file, used to detect replacement between steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    device: u64,
    inode: u64,
}

pub fn identity(path: &Path) -> io::Result<FileIdentity> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(FileIdentity { device: metadata.dev(), inode: metadata.ino() })
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Ok(FileIdentity { device: 0, inode: 0 })
    }
}

/// Refuse when a file was replaced or became a link since `expected` was read.
pub fn assert_same_file(path: &Path, expected: FileIdentity) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || identity(path)? != expected {
        return Err(Error::refused(format!("Database file changed while it was being opened: {}", path.display())));
    }
    Ok(())
}

/// Flush a directory entry change to disk where the system supports it.
pub fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

/// Write bytes to a new private file and synchronize them before returning.
pub fn write_new_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = create_new_private(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// A private temporary directory under the system temporary directory. The
/// system temporary directory may be behind an operating-system link, such as
/// `/var` on macOS, so the directory is created under its real path.
pub fn make_temporary_directory(prefix: &str) -> io::Result<PathBuf> {
    let base = fs::canonicalize(std::env::temp_dir())?;
    for _ in 0..16 {
        let candidate = base.join(format!("{prefix}{}", random_suffix()));
        match create_private_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("could not create a unique temporary directory"))
}

fn random_suffix() -> String {
    let id = uuid::Uuid::now_v7().simple().to_string();
    id[id.len() - 12..].to_string()
}
