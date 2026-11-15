//! Reading provider exports: folders, single files, and zip archives.
//!
//! Provider exports are untrusted. Every reader here refuses symbolic links,
//! archive paths that leave the archive, archive links, and entries over the
//! limits in `input_policy`. An archive is checked completely from its central
//! directory before any entry is expanded. An expanded entry is measured as it
//! is read, so a false declared size cannot hide a larger entry.
//!
//! Error texts are the ones the 2.1.0 program printed.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::input_policy::{
    MAX_PROVIDER_FILES, MAX_PROVIDER_TEXT_BYTES, MAX_ZIP_COMPRESSION_RATIO, MAX_ZIP_ENTRIES, MAX_ZIP_ENTRY_BYTES,
    MAX_ZIP_MEMORY_ENTRY_BYTES, MAX_ZIP_TOTAL_BYTES,
};

/// One entry of a checked archive.
#[derive(Debug, Clone)]
pub struct ZipEntry {
    /// The name as the archive gives it, with `/` separators.
    pub name: String,
    /// Expanded size from the central directory.
    pub size: u64,
    index: usize,
}

/// An archive whose entries all passed the safety checks.
#[derive(Debug, Clone)]
pub struct ZipIndex {
    pub path: PathBuf,
    pub entries: Vec<ZipEntry>,
}

/// The file-name rules of the zip reader 2.1.0 used: no backslash, no absolute
/// path, and no `..` segment.
fn check_entry_name(name: &str) -> Result<()> {
    if name.contains('\\') {
        return Err(Error::refused(format!("invalid characters in fileName: {name}")));
    }
    let bytes = name.as_bytes();
    if name.starts_with('/') || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':') {
        return Err(Error::refused(format!("absolute path: {name}")));
    }
    if name.split('/').any(|part| part == "..") {
        return Err(Error::refused(format!("invalid relative path: {name}")));
    }
    if name.contains('\0') {
        return Err(Error::refused(format!("archive entry has an unsafe path: {name}")));
    }
    Ok(())
}

/// The external attributes from an entry's central-directory header. The
/// archive library hides them for some writers, so they are read directly.
fn external_attributes(file: &mut File, central_header_start: u64) -> Result<u32> {
    let mut bytes = [0_u8; 4];
    file.seek(SeekFrom::Start(central_header_start + 38))?;
    file.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn check_entry_size(name: &str, size: u64, compressed: u64) -> Result<()> {
    if size > MAX_ZIP_ENTRY_BYTES {
        return Err(Error::refused(format!("archive entry exceeds {MAX_ZIP_ENTRY_BYTES} expanded bytes: {name}")));
    }
    if size > 0 && (compressed == 0 || size as f64 / compressed as f64 > MAX_ZIP_COMPRESSION_RATIO as f64) {
        return Err(Error::refused(format!(
            "archive entry exceeds the {MAX_ZIP_COMPRESSION_RATIO}:1 expansion limit: {name}"
        )));
    }
    Ok(())
}

fn zip_error(path: &Path, error: zip::result::ZipError) -> Error {
    match error {
        zip::result::ZipError::Io(error) => Error::Io(error),
        other => Error::refused(format!("could not open {} as a zip: {other}", path.display())),
    }
}

fn open_archive(path: &Path) -> Result<zip::ZipArchive<File>> {
    let file = File::open(path)?;
    zip::ZipArchive::new(file).map_err(|error| zip_error(path, error))
}

/// Read and check an archive's central directory. No entry is expanded.
pub fn open_zip(path: &Path) -> Result<ZipIndex> {
    let mut archive = open_archive(path)?;
    let mut raw = File::open(path)?;
    let mut entries = Vec::new();
    let mut expanded: u64 = 0;
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(|error| zip_error(path, error))?;
        let name = entry.name().to_string();
        check_entry_name(&name)?;
        let mode = (external_attributes(&mut raw, entry.central_header_start())? >> 16) & 0o170000;
        if mode == 0o120000 {
            return Err(Error::refused(format!("archive entry is a symbolic link: {name}")));
        }
        check_entry_size(&name, entry.size(), entry.compressed_size())?;
        if entries.len() >= MAX_ZIP_ENTRIES {
            return Err(Error::refused(format!("archive has more than {MAX_ZIP_ENTRIES} entries")));
        }
        expanded = expanded.saturating_add(entry.size());
        if expanded > MAX_ZIP_TOTAL_BYTES {
            return Err(Error::refused(format!("archive exceeds {MAX_ZIP_TOTAL_BYTES} total expanded bytes")));
        }
        entries.push(ZipEntry { name, size: entry.size(), index });
    }
    Ok(ZipIndex { path: path.to_path_buf(), entries })
}

/// Counts expanded bytes and refuses more than a limit or a declared size.
struct Measured<R> {
    inner: R,
    read: u64,
    declared: u64,
    limit: u64,
    name: String,
    limit_message: fn(u64, &str) -> String,
}

impl<R: Read> Read for Measured<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(buffer)?;
        self.read += count as u64;
        if self.read > self.limit {
            return Err(std::io::Error::other((self.limit_message)(self.limit, &self.name)));
        }
        if self.read > self.declared {
            return Err(std::io::Error::other(format!(
                "too many bytes in the stream. expected {}. got at least {}",
                self.declared, self.read
            )));
        }
        if count == 0 && !buffer.is_empty() && self.read < self.declared {
            return Err(std::io::Error::other(format!(
                "not enough bytes in the stream. expected {}. got only {}",
                self.declared, self.read
            )));
        }
        Ok(count)
    }
}

fn entry_limit_message(limit: u64, name: &str) -> String {
    format!("archive entry exceeds {limit} expanded bytes: {name}")
}

fn text_limit_message(limit: u64, name: &str) -> String {
    format!("archive text entry exceeds {limit} bytes: {name}")
}

impl ZipIndex {
    /// The first entry with exactly this name.
    pub fn entry(&self, name: &str) -> Result<&ZipEntry> {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .ok_or_else(|| Error::not_found(format!("{name} not found in {}", self.path.display())))
    }

    /// Stream one entry through `read`. The stream refuses more than
    /// `MAX_ZIP_ENTRY_BYTES` and any byte count other than the declared size.
    pub fn read_entry<T>(&self, name: &str, read: impl FnOnce(&mut dyn Read) -> Result<T>) -> Result<T> {
        let entry = self.entry(name)?.clone();
        self.read_measured(&entry, MAX_ZIP_ENTRY_BYTES, entry_limit_message, read)
    }

    /// One entry as UTF-8 text, for entries small enough to hold in memory.
    /// Invalid UTF-8 becomes U+FFFD, as Node's decoder does.
    pub fn read_text(&self, name: &str) -> Result<String> {
        let entry = self.entry(name)?.clone();
        if entry.size > MAX_ZIP_MEMORY_ENTRY_BYTES {
            return Err(Error::refused(text_limit_message(MAX_ZIP_MEMORY_ENTRY_BYTES, name)));
        }
        let bytes = self.read_measured(&entry, MAX_ZIP_MEMORY_ENTRY_BYTES, text_limit_message, |reader| {
            let mut bytes = Vec::with_capacity(entry.size as usize);
            reader.read_to_end(&mut bytes)?;
            Ok(bytes)
        })?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn read_measured<T>(
        &self,
        entry: &ZipEntry,
        limit: u64,
        limit_message: fn(u64, &str) -> String,
        read: impl FnOnce(&mut dyn Read) -> Result<T>,
    ) -> Result<T> {
        check_entry_name(&entry.name)?;
        let mut archive = open_archive(&self.path)?;
        let file = archive.by_index(entry.index).map_err(|error| zip_error(&self.path, error))?;
        if file.name() != entry.name {
            return Err(Error::stale(format!("{} changed while it was read", self.path.display())));
        }
        check_entry_size(&entry.name, file.size(), file.compressed_size())?;
        let mut measured =
            Measured { inner: file, read: 0, declared: entry.size, limit, name: entry.name.clone(), limit_message };
        read(&mut measured)
    }
}

/// A directory entry name in the order Node's `readdirSync` lists them: sorted
/// by bytes on every system this program supports.
pub fn sorted_dir(path: &Path) -> Result<Vec<fs::DirEntry>> {
    let mut entries = fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    Ok(entries)
}

/// The entry names of a directory, as `readdirSync(path)` returns them.
pub fn dir_names(path: &Path) -> Result<Vec<String>> {
    Ok(sorted_dir(path)?.iter().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect())
}

/// A regular file's text, refused over `MAX_PROVIDER_TEXT_BYTES`.
pub fn read_provider_text(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::refused(format!("refusing provider input that is not a regular file: {}", path.display())));
    }
    if metadata.len() > MAX_PROVIDER_TEXT_BYTES {
        return Err(Error::refused(format!(
            "provider text file exceeds {MAX_PROVIDER_TEXT_BYTES} bytes: {}",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(MAX_PROVIDER_TEXT_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PROVIDER_TEXT_BYTES {
        return Err(Error::refused(format!(
            "provider text file exceeds {MAX_PROVIDER_TEXT_BYTES} bytes: {}",
            path.display()
        )));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The first line of a regular file, read from at most its first 8 KiB.
pub fn first_line(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::refused(format!("refusing provider input that is not a regular file: {}", path.display())));
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(crate::input_policy::DETECTION_HEADER_BYTES as u64).read_to_end(&mut bytes)?;
    Ok(first_text_line(&String::from_utf8_lossy(&bytes)))
}

/// `text.split(/\r?\n/)[0]`.
pub fn first_text_line(text: &str) -> String {
    let line = text.split('\n').next().unwrap_or_default();
    line.strip_suffix('\r').unwrap_or(line).to_string()
}

/// Every file under a directory, relative to it, depth first in sorted order.
/// A symbolic link anywhere refuses the export.
pub fn walk_files(root: &Path) -> Result<Vec<String>> {
    fn walk(directory: &Path, prefix: &str, out: &mut Vec<String>) -> Result<()> {
        for entry in sorted_dir(directory)? {
            let name = entry.file_name().to_string_lossy().into_owned();
            let relative = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
            let kind = entry.file_type()?;
            if kind.is_dir() {
                walk(&entry.path(), &relative, out)?;
            } else if kind.is_symlink() {
                return Err(Error::refused(format!("refusing symbolic link in provider export: {relative}")));
            } else if kind.is_file() {
                out.push(relative);
                if out.len() > MAX_PROVIDER_FILES {
                    return Err(Error::refused(format!("provider export has more than {MAX_PROVIDER_FILES} files")));
                }
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, "", &mut out)?;
    Ok(out)
}

/// The final path segment of a `/`-separated entry name.
pub fn base_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}
