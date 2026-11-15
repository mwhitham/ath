//! Working out which kind of input a path holds, so nobody passes a vendor flag.
//!
//! Detection reads structure. It does not trust a file name an athlete may
//! have changed, except where 2.1.0 did: `.gpx`, `.fit`, `.xml`, `.csv`,
//! `.json`, and `.zip` endings choose which structure to look for. An input
//! that matches nothing is refused with a message that names what was expected.
//!
//! `classify` applies the route order of `ath import` in 2.1.0:
//!
//! 1. `--mapping` was given: a mapped JSON import.
//! 2. A Garmin or Strava input: the shared import.
//! 3. A path that is not a package: an Apple Health, WHOOP, or Oura export.
//! 4. Otherwise: a package.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::input_policy::{DETECTION_JSON_HEAD_CHARS, MAX_PROVIDER_FILES, MAX_PROVIDER_TEXT_BYTES};

use super::input::{self, ZipIndex};

/// The four `ath import` routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportRoute {
    /// A JSON file read through a mapping file.
    Mapped,
    /// A Garmin or Strava file, folder, or archive.
    Shared(SharedImport),
    /// An Apple Health, WHOOP, or Oura export. `detect_export` chooses which.
    Provider,
    /// An Athletic Standard package.
    Package,
}

/// The routes that 2.1.0 read in its Rust helper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedImport {
    Garmin,
    Strava,
}

impl SharedImport {
    pub fn as_str(self) -> &'static str {
        match self {
            SharedImport::Garmin => "garmin",
            SharedImport::Strava => "strava",
        }
    }
}

/// Choose the `ath import` route for an input path.
pub fn classify(path: &Path, with_mapping: bool) -> Result<ImportRoute> {
    if with_mapping {
        return Ok(ImportRoute::Mapped);
    }
    if let Some(shared) = detect_shared_import(path)? {
        return Ok(ImportRoute::Shared(shared));
    }
    if is_package_path(path) {
        Ok(ImportRoute::Package)
    } else {
        Ok(ImportRoute::Provider)
    }
}

/// A path ending in `.json` in any case, or a folder that holds `athlete.ath.json`.
pub fn is_package_path(path: &Path) -> bool {
    path.to_string_lossy().to_lowercase().ends_with(".json") || path.join("athlete.ath.json").exists()
}

fn lower_base_name(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Recognize a Garmin or Strava input. A symbolic link is never recognized.
pub fn detect_shared_import(path: &Path) -> Result<Option<SharedImport>> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(None);
    };
    if metadata.file_type().is_symlink() {
        return Ok(None);
    }
    if metadata.is_dir() {
        return Ok(detect_shared_names(&input::dir_names(path)?));
    }
    if !metadata.is_file() {
        return Ok(None);
    }
    let lower = lower_base_name(path);
    if lower.ends_with(".gpx") || lower.ends_with(".fit") {
        return Ok(Some(SharedImport::Garmin));
    }
    if lower.ends_with(".json") {
        let head = json_head(path)?;
        if head.contains("activityName") || head.contains("summarizedActivities") || head.contains("startTimeGMT") {
            return Ok(Some(SharedImport::Garmin));
        }
    }
    if lower.ends_with(".zip") {
        let zip = input::open_zip(path)?;
        let names: Vec<String> = zip.entries.iter().map(|entry| entry.name.clone()).collect();
        return Ok(detect_shared_names(&names));
    }
    Ok(None)
}

/// The first 8,000 UTF-16 code units of a file's text. At most four bytes
/// make one code unit, so only the bytes that can hold them are read.
fn json_head(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(DETECTION_JSON_HEAD_CHARS as u64 * 4).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut units = 0;
    let mut end = 0;
    for (index, character) in text.char_indices() {
        let width = character.len_utf16();
        if units + width > DETECTION_JSON_HEAD_CHARS {
            break;
        }
        units += width;
        end = index + character.len_utf8();
    }
    Ok(text[..end].to_string())
}

fn detect_shared_names(names: &[String]) -> Option<SharedImport> {
    let lower: Vec<String> = names.iter().map(|name| name.replace('\\', "/").to_lowercase()).collect();
    // An Apple export carries workout-routes/*.gpx beside export.xml. Those
    // routes belong to the Apple export, not to a Garmin activity file.
    if lower.iter().any(|name| input::base_name(name) == "export.xml" || name.contains("apple_health_export/")) {
        return None;
    }
    if lower.iter().any(|name| name.ends_with("activities.csv")) {
        return Some(SharedImport::Strava);
    }
    if lower
        .iter()
        .any(|name| name.ends_with(".gpx") || name.ends_with(".fit") || name.ends_with("summarizedactivities.json"))
    {
        return Some(SharedImport::Garmin);
    }
    None
}

/// The provider export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Apple,
    Whoop,
    Oura,
}

impl ExportFormat {
    /// The preview's `provider` value.
    pub fn as_str(self) -> &'static str {
        match self {
            ExportFormat::Apple => "apple",
            ExportFormat::Whoop => "whoop",
            ExportFormat::Oura => "oura",
        }
    }

    /// The vendor name used in labels and source details.
    pub fn vendor_label(self) -> &'static str {
        match self {
            ExportFormat::Apple => "Apple Health",
            ExportFormat::Whoop => "WHOOP",
            ExportFormat::Oura => "Oura",
        }
    }

    /// The source `data_route`, which also names the match key method.
    pub fn data_route(self) -> &'static str {
        match self {
            ExportFormat::Apple => "apple_health",
            ExportFormat::Whoop => "whoop_csv",
            ExportFormat::Oura => "oura_csv",
        }
    }
}

/// How an export was laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Zip,
    Directory,
    File,
}

/// A recognized provider export.
#[derive(Debug, Clone)]
pub struct DetectedExport {
    pub format: ExportFormat,
    pub container: Container,
    pub path: PathBuf,
    /// Where the folders beside the main file are: an Apple export's
    /// `electrocardiograms/` and `workout-routes/`. For a bare file, the folder
    /// that holds it.
    pub aux_root: PathBuf,
    /// The checked archive index, for a zip export.
    pub zip: Option<ZipIndex>,
}

impl DetectedExport {
    /// How the export was laid out, for source details: `zip export`,
    /// `export folder`, `export.xml`, or `CSV export`.
    pub fn container_label(&self) -> &'static str {
        match self.container {
            Container::Zip => "zip export",
            Container::Directory => "export folder",
            Container::File if self.format == ExportFormat::Apple => "export.xml",
            Container::File => "CSV export",
        }
    }
}

const WHOOP_MARKER: &str = "physiological_cycles.csv";
/// WHOOP's older privacy-portal archive, which has a different sleep schema.
const WHOOP_GDPR_MARKER: &str = "health/";
const APPLE_XML_NAMES: [&str; 2] = ["export.xml", "apple_health_export/export.xml"];
/// Column names that only appear in an Oura export.
const OURA_MARKERS: [&str; 6] =
    ["average_hrv", "bedtime_start", "total_sleep_duration", "lowest_heart_rate", "readiness_score", "contributors"];

/// True when a header row names an Oura column.
pub fn is_oura_header(header: &str) -> bool {
    let mut normalized = String::new();
    let mut in_space = false;
    for character in header.to_lowercase().chars() {
        // JavaScript's `\s`: it includes U+FEFF and leaves out U+0085.
        if (character.is_whitespace() && character != '\u{85}') || character == '\u{feff}' {
            if !in_space {
                normalized.push('_');
            }
            in_space = true;
        } else {
            normalized.push(character);
            in_space = false;
        }
    }
    OURA_MARKERS.iter().any(|marker| normalized.contains(marker))
}

fn whoop_gdpr_message(path: &Path) -> String {
    format!(
        "{} looks like WHOOP's older privacy-portal archive, which stores sleep in a different shape. \
         Use the export from the WHOOP app (More → Data Export); it contains {WHOOP_MARKER}.",
        base_name(path)
    )
}

fn base_name(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Recognize an Apple Health, WHOOP, or Oura export.
pub fn detect_export(path: &Path) -> Result<DetectedExport> {
    if !path.exists() {
        return Err(Error::not_found(format!("no such file or directory: {}", path.display())));
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(Error::refused(format!("refusing provider input through a symbolic link: {}", path.display())));
    }
    let found = |format, container, aux_root: &Path, zip| {
        Ok(DetectedExport { format, container, path: path.to_path_buf(), aux_root: aux_root.to_path_buf(), zip })
    };

    if metadata.is_dir() {
        let names: Vec<String> = input::dir_names(path)?.iter().map(|name| name.to_lowercase()).collect();
        if names.iter().any(|name| name == "export.xml" || name == "apple_health_export") {
            return found(ExportFormat::Apple, Container::Directory, path, None);
        }
        if names.iter().any(|name| name == WHOOP_MARKER) {
            return found(ExportFormat::Whoop, Container::Directory, path, None);
        }
        if names.iter().any(|name| name == "health") {
            return Err(Error::refused(whoop_gdpr_message(path)));
        }
        for name in input::dir_names(path)? {
            if name.to_lowercase().ends_with(".csv") && is_oura_header(&input::first_line(&path.join(&name))?) {
                return found(ExportFormat::Oura, Container::Directory, path, None);
            }
        }
        return Err(Error::refused(format!(
            "{} does not look like an Apple Health, WHOOP, or Oura export. Expected export.xml, {WHOOP_MARKER}, or Oura CSVs.",
            path.display()
        )));
    }

    let lower = path.to_string_lossy().to_lowercase();
    let parent = path.parent().unwrap_or(path);
    if lower.ends_with(".zip") {
        let zip = input::open_zip(path)?;
        let names: Vec<String> = zip.entries.iter().map(|entry| entry.name.to_lowercase()).collect();
        if names.iter().any(|name| APPLE_XML_NAMES.iter().any(|apple| name.ends_with(apple))) {
            return found(ExportFormat::Apple, Container::Zip, path, Some(zip));
        }
        if names.iter().any(|name| name.ends_with(WHOOP_MARKER)) {
            return found(ExportFormat::Whoop, Container::Zip, path, Some(zip));
        }
        if names.iter().any(|name| name.contains(WHOOP_GDPR_MARKER) && name.ends_with("sleeps.csv")) {
            return Err(Error::refused(whoop_gdpr_message(path)));
        }
        let csv_names: Vec<String> = zip
            .entries
            .iter()
            .filter(|entry| entry.name.to_lowercase().ends_with(".csv"))
            .map(|entry| entry.name.clone())
            .collect();
        for name in csv_names {
            if is_oura_header(&input::first_text_line(&zip.read_text(&name)?)) {
                return found(ExportFormat::Oura, Container::Zip, path, Some(zip));
            }
        }
        return Err(Error::refused(format!(
            "{} is a zip, but not one we recognize. Expected an Apple Health export.xml, WHOOP's {WHOOP_MARKER}, or Oura CSVs inside it.",
            base_name(path)
        )));
    }
    if lower.ends_with(".xml") {
        return found(ExportFormat::Apple, Container::File, parent, None);
    }
    if lower.ends_with(".csv") {
        let header = input::first_line(path)?;
        if is_oura_header(&header) {
            return found(ExportFormat::Oura, Container::File, parent, None);
        }
        let lower_header = header.to_ascii_lowercase();
        if lower_header.contains("cycle start time") || lower_header.contains("cycle timezone") {
            // A single WHOOP CSV is readable, but the bundle carries sleep and workouts too.
            return found(ExportFormat::Whoop, Container::File, parent, None);
        }
        return Err(Error::refused(format!(
            "{} is a CSV we cannot place. Oura exports carry columns like average_hrv or bedtime_start; \
             WHOOP exports carry \"Cycle start time\".",
            base_name(path)
        )));
    }
    Err(Error::refused(format!(
        "{} is not a supported export. Pass an Apple Health export.zip, a WHOOP CSV export, or an Oura CSV export.",
        base_name(path)
    )))
}

/// Keep a file's text under its name. A name seen before keeps its position
/// and takes the newer text, as a JavaScript `Map` does.
fn put_text(out: &mut Vec<(String, String)>, name: String, text: String) {
    if let Some(existing) = out.iter_mut().find(|(known, _)| *known == name) {
        existing.1 = text;
    } else {
        out.push((name, text));
    }
}

/// The text of every CSV in an export, keyed by lowercase base name, in the
/// order each name was first found. A later file with a name already seen
/// replaces the earlier text and keeps the earlier position.
pub fn read_csv_bundle(export: &DetectedExport) -> Result<Vec<(String, String)>> {
    let mut out: Vec<(String, String)> = Vec::new();
    match export.container {
        Container::File => {
            let metadata = fs::symlink_metadata(&export.path)?;
            if metadata.len() > MAX_PROVIDER_TEXT_BYTES {
                return Err(Error::refused(format!(
                    "provider text file exceeds {MAX_PROVIDER_TEXT_BYTES} bytes: {}",
                    export.path.display()
                )));
            }
            put_text(&mut out, lower_base_name(&export.path), input::read_provider_text(&export.path)?);
        }
        Container::Directory => {
            fn walk(directory: &Path, out: &mut Vec<(String, String)>) -> Result<()> {
                for entry in input::sorted_dir(directory)? {
                    let full = entry.path();
                    let kind = entry.file_type()?;
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if kind.is_dir() {
                        walk(&full, out)?;
                    } else if kind.is_symlink() {
                        return Err(Error::refused(format!(
                            "refusing symbolic link in provider export: {}",
                            full.display()
                        )));
                    } else if name.ends_with(".csv") {
                        put_text(out, name, input::read_provider_text(&full)?);
                        if out.len() > MAX_PROVIDER_FILES {
                            return Err(Error::refused(format!(
                                "provider export has more than {MAX_PROVIDER_FILES} CSV files"
                            )));
                        }
                    }
                }
                Ok(())
            }
            walk(&export.path, &mut out)?;
        }
        Container::Zip => {
            let zip = match &export.zip {
                Some(zip) => zip.clone(),
                None => input::open_zip(&export.path)?,
            };
            for entry in &zip.entries {
                if entry.name.to_lowercase().ends_with(".csv") {
                    put_text(&mut out, input::base_name(&entry.name).to_lowercase(), zip.read_text(&entry.name)?);
                }
            }
        }
    }
    Ok(out)
}

/// A SHA-256 over every file an export's reader may read: the file or
/// archive itself, every file in a folder, or, for a bare Apple XML, the XML
/// and the files in the two folders beside it. Each file adds its name, byte
/// count, and SHA-256.
///
/// The JavaScript client previews an import in one call and writes it in a
/// later one, which reads the export again. The write compares this value
/// with the preview's, so an export that changed in between is refused, even
/// when its record counts stayed the same.
pub fn export_fingerprint(path: &Path) -> Result<String> {
    let export = detect_export(path)?;
    let mut lines = Vec::new();
    let mut add = |name: &str, file: &Path| -> Result<()> {
        let (sha256, byte_count) = hash_file(file)?;
        lines.push(format!("{name}\u{0}{byte_count}\u{0}{sha256}"));
        Ok(())
    };
    match export.container {
        Container::Directory => {
            for name in input::walk_files(&export.path)? {
                add(&name, &export.path.join(&name))?;
            }
        }
        Container::Zip | Container::File => {
            add(&base_name(&export.path), &export.path)?;
            if export.container == Container::File && export.format == ExportFormat::Apple {
                for name in super::apple::files::beside_bare_xml(&export.aux_root)? {
                    add(&name, &export.aux_root.join(&name))?;
                }
            }
        }
    }
    Ok(crate::files::durable::sha256_hex(lines.join("\n").as_bytes()))
}

fn hash_file(path: &Path) -> Result<(String, u64)> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut total = 0u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total += read as u64;
    }
    Ok((crate::files::durable::hex(&hasher.finalize()), total))
}
