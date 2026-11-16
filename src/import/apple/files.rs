//! Finding `export.xml` and the files beside it.
//!
//! An Apple export is a zip, a folder, or a bare `export.xml`. Beside the XML
//! sit `workout-routes/` and `electrocardiograms/`. A bare XML reads only
//! those two folders next to it, never the rest of the folder it sits in.
//! Route files are read only from `workout-routes/` next to the XML, so a
//! `FileReference` cannot name a file outside the export. Every file is
//! refused when it is a symbolic link or over the limits in `input_policy`.
//!
//! These are the rules of `src/import/apple.ts` and `src/import/detect.ts`.

use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::input_policy::{MAX_APPLE_XML_BYTES, MAX_PROVIDER_FILES};

use super::super::detect::{Container, DetectedExport};
use super::super::input::{self, ZipIndex};

/// Where the XML is.
pub enum XmlSource {
    File(PathBuf),
    ZipEntry(String),
}

/// Find `export.xml`. A folder may hold it at its top or under
/// `apple_health_export/`; a zip holds the first entry whose name ends in
/// `export.xml`.
pub fn locate_xml(export: &DetectedExport) -> Result<XmlSource> {
    let open_local = |path: PathBuf| -> Result<XmlSource> {
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(Error::refused(format!(
                "refusing Apple Health XML that is not a regular file: {}",
                path.display()
            )));
        }
        if metadata.len() > MAX_APPLE_XML_BYTES {
            return Err(Error::refused(format!(
                "Apple Health XML exceeds {MAX_APPLE_XML_BYTES} bytes: {}",
                path.display()
            )));
        }
        Ok(XmlSource::File(path))
    };
    match export.container {
        Container::File => open_local(export.path.clone()),
        Container::Directory => {
            let candidates =
                [export.path.join("export.xml"), export.path.join("apple_health_export").join("export.xml")];
            match candidates.into_iter().find(|candidate| candidate.exists()) {
                Some(found) => open_local(found),
                None => Err(Error::not_found(format!("no export.xml under {}", export.path.display()))),
            }
        }
        Container::Zip => {
            let zip = zip_index(export)?;
            zip.entries
                .iter()
                .find(|entry| entry.name.to_lowercase().ends_with("export.xml"))
                .map(|entry| XmlSource::ZipEntry(entry.name.clone()))
                .ok_or_else(|| Error::not_found(format!("no export.xml inside {}", export.path.display())))
        }
    }
}

fn zip_index(export: &DetectedExport) -> Result<ZipIndex> {
    match &export.zip {
        Some(zip) => Ok(zip.clone()),
        None => input::open_zip(&export.path),
    }
}

/// Stream the XML through `read`.
pub fn with_xml<T>(
    export: &DetectedExport,
    source: &XmlSource,
    read: impl FnOnce(&mut dyn std::io::BufRead) -> Result<T>,
) -> Result<T> {
    match source {
        XmlSource::File(path) => {
            let file = Limited { inner: fs::File::open(path)?, read: 0, path: path.clone() };
            let mut reader = BufReader::with_capacity(1 << 16, file);
            read(&mut reader)
        }
        XmlSource::ZipEntry(name) => zip_index(export)?.read_entry(name, |entry| {
            let mut reader = BufReader::with_capacity(1 << 16, entry);
            read(&mut reader)
        }),
    }
}

/// A file read that refuses more bytes than the XML limit, so a file that
/// grows while it is read is refused rather than read in part.
struct Limited {
    inner: fs::File,
    read: u64,
    path: PathBuf,
}

impl Read for Limited {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(buffer)?;
        self.read += count as u64;
        if self.read > MAX_APPLE_XML_BYTES {
            return Err(std::io::Error::other(format!(
                "Apple Health XML exceeds {MAX_APPLE_XML_BYTES} bytes: {}",
                self.path.display()
            )));
        }
        Ok(count)
    }
}

/// The names of an export's entries, as 2.1.0 listed them: every zip entry,
/// every file under a folder, or the two Apple folders beside a bare XML.
pub struct Listing {
    pub names: Vec<String>,
    /// The folder that holds `export.xml` inside the export, without a
    /// trailing slash. Empty for the export's top level and for a bare XML.
    pub root: String,
}

impl Listing {
    pub fn read(export: &DetectedExport) -> Result<Listing> {
        let names = match export.container {
            Container::Zip => zip_index(export)?.entries.iter().map(|entry| entry.name.clone()).collect(),
            Container::Directory => input::walk_files(&export.aux_root)?,
            Container::File => beside_bare_xml(&export.aux_root)?,
        };
        let root = if export.container == Container::File { String::new() } else { export_root(&names) };
        Ok(Listing { names, root })
    }

    /// Route files: GPX files directly inside `workout-routes/` next to the XML.
    pub fn route_files(&self) -> Vec<&str> {
        let prefix =
            if self.root.is_empty() { "workout-routes/".to_string() } else { format!("{}/workout-routes/", self.root) }
                .to_lowercase();
        self.names
            .iter()
            .filter(|name| {
                let lower = name.replace('\\', "/").to_lowercase();
                lower.starts_with(&prefix) && !lower[prefix.len()..].contains('/') && lower.ends_with(".gpx")
            })
            .map(String::as_str)
            .collect()
    }

    /// ECG recordings: CSV entries whose path names an electrocardiogram.
    pub fn ecg_files(&self) -> Vec<&str> {
        self.names
            .iter()
            .filter(|name| {
                let lower = name.to_lowercase();
                lower.contains("electrocardiogram") && lower.ends_with(".csv")
            })
            .map(String::as_str)
            .collect()
    }

    /// The route file a `FileReference` path names, matched without regard to
    /// case. A path is read relative to the XML's folder.
    pub fn wanted_name(&self, reference: &str) -> String {
        let relative = reference.replace('\\', "/");
        let relative = relative.trim_start_matches('/');
        if self.root.is_empty() {
            relative.to_lowercase()
        } else {
            format!("{}/{relative}", self.root).to_lowercase()
        }
    }
}

/// The shortest path among entries named `export.xml`, without the file name.
fn export_root(names: &[String]) -> String {
    let mut xmls: Vec<String> = names
        .iter()
        .map(|name| name.replace('\\', "/"))
        .filter(|name| name.rsplit('/').next().is_some_and(|base| base.to_lowercase() == "export.xml"))
        .collect();
    // A stable sort by UTF-16 length, as `sort((a, b) => a.length - b.length)`.
    xmls.sort_by_key(|name| crate::json::utf16_len(name));
    match xmls.first() {
        Some(xml) if xml.contains('/') => xml[..xml.rfind('/').unwrap_or(0)].to_string(),
        _ => String::new(),
    }
}

/// Files directly inside `workout-routes/` and `electrocardiograms/` next to a
/// bare `export.xml`. A folder that is a link or not a folder is passed over;
/// a link inside one refuses the export.
pub(crate) fn beside_bare_xml(folder: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for name in ["workout-routes", "electrocardiograms"] {
        let directory = folder.join(name);
        if !directory.exists() {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&directory) else {
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        for entry in input::sorted_dir(&directory)? {
            let relative = format!("{name}/{}", entry.file_name().to_string_lossy());
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Err(Error::refused(format!("refusing symbolic link in provider export: {relative}")));
            }
            if !kind.is_file() {
                continue;
            }
            out.push(relative);
            if out.len() > MAX_PROVIDER_FILES {
                return Err(Error::refused(format!("provider export has more than {MAX_PROVIDER_FILES} files")));
            }
        }
    }
    Ok(out)
}

/// One entry as text, within the text limit.
pub fn read_text(export: &DetectedExport, name: &str) -> Result<String> {
    match export.container {
        Container::Zip => zip_index(export)?.read_text(name),
        Container::Directory | Container::File => input::read_provider_text(&export.aux_root.join(name)),
    }
}

/// The last path segment, as Node's `basename` gives it.
pub fn base_name(name: &str) -> &str {
    input::base_name(name)
}
