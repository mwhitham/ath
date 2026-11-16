//! Durable file writes.
//!
//! A durable write synchronizes file contents before the file becomes visible
//! at its final name, then synchronizes the directory so the name survives a
//! power loss. Every new file is owner-only and created exclusively.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{create_new_private, sync_directory};

/// Lowercase hexadecimal SHA-256 of some bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    out
}

/// The directory that holds `path`.
pub fn parent_of(path: &Path) -> PathBuf {
    path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

/// Synchronize the directory that holds `path`.
pub fn sync_parent(path: &Path) -> io::Result<()> {
    sync_directory(&parent_of(path))
}

/// Write bytes to a new owner-only file. The bytes go to a temporary name,
/// are synchronized, and are then renamed into place. The directory is
/// synchronized after the rename.
pub fn write_owner_only_durable(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = temporary_sibling(path, "tmp");
    let written = (|| {
        let mut file = create_new_private(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    sync_parent(path)
}

/// A name next to `path` that this process owns: `<path>.<tag>-<pid>-<millis>`.
pub fn temporary_sibling(path: &Path, tag: &str) -> PathBuf {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    super::sidecar(path, &format!(".{tag}-{}-{millis}", std::process::id()))
}

/// A new owner-only file that hashes and counts every byte written to it.
/// `finish` synchronizes the contents and returns the hash and size.
pub struct HashingFile {
    path: PathBuf,
    writer: BufWriter<File>,
    hasher: Sha256,
    byte_count: u64,
}

impl HashingFile {
    /// Create the file. It must not exist.
    pub fn create(path: &Path) -> io::Result<Self> {
        let file = create_new_private(path)?;
        Ok(HashingFile {
            path: path.to_path_buf(),
            writer: BufWriter::with_capacity(1 << 20, file),
            hasher: Sha256::new(),
            byte_count: 0,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Flush, synchronize, and close. Returns the SHA-256 and byte count.
    pub fn finish(mut self) -> io::Result<(String, u64)> {
        self.writer.flush()?;
        let file = self.writer.into_inner().map_err(|error| error.into_error())?;
        file.sync_all()?;
        Ok((hex(&self.hasher.finalize()), self.byte_count))
    }
}

impl Write for HashingFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.writer.write(bytes)?;
        self.hasher.update(&bytes[..written]);
        self.byte_count += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

/// Remove a file that may already be gone.
pub fn remove_file_if_present(path: &Path) -> io::Result<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_write_creates_an_owner_only_file_and_refuses_to_replace() {
        let directory = crate::files::make_temporary_directory("ath-durable-").unwrap();
        let path = directory.join("file.json");
        write_owner_only_durable(&path, b"{}\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"{}\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let mut hashing = HashingFile::create(&directory.join("hashed")).unwrap();
        hashing.write_all(b"abc").unwrap();
        let (hash, count) = hashing.finish().unwrap();
        assert_eq!(hash, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(count, 3);
        assert!(HashingFile::create(&directory.join("hashed")).is_err());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
        fs::remove_dir_all(directory).unwrap();
    }
}
