//! Reading a package: limits, containment, hashes, and complete validation.
//!
//! Loading reads and checks every package file before anything is written.
//! The main document stays in memory. Each measurement-series file is read,
//! validated, and released; restore reads it again and requires the same hash.
//!
//! A package with fields this version does not know, or with records it cannot
//! represent, is reported as a whole (`PackageRepresentation`). Nothing in it
//! is imported. No option discards the unknown parts.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::attachments::{check_attachment_bytes, relative_attachment_path, stored_attachment_name};
use super::{MANIFEST_DOCUMENT, PACKAGE_DOCUMENT};
use crate::error::{Error, Result};
use crate::files::{self, durable};
use crate::input_policy::*;
use crate::json::{self, Value};
use crate::model::schema::{from_checked_value, Issue, IssueCode, PathSegment, MAX_SAFE_INTEGER};
use crate::model::*;

/// One measurement-series file of a loaded package.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedSeriesFile {
    pub reference: MeasurementSeriesFileReference,
    pub path: PathBuf,
}

/// A package that passed every check. Samples stay in their files.
#[derive(Debug, Clone)]
pub struct LoadedPackage {
    pub athleticstandard_version: String,
    pub database_id: String,
    pub database_revision: u64,
    /// Current records of the main document, in package order.
    pub records: Vec<CurrentRecord>,
    pub revisions: Vec<RecordRevision>,
    pub deletions: Vec<RecordDeletion>,
    pub erasure_markers: Vec<RecordErasureMarker>,
    pub import_keys: Vec<RecordImportKey>,
    /// Series files in the order they are processed.
    pub series_files: Vec<LoadedSeriesFile>,
    /// The first sample IDs, at most `MAX_PREVIEW_RECORD_IDS`.
    pub sample_record_id_examples: Vec<String>,
    /// The absolute path that was named.
    pub source_path: String,
    /// SHA-256 over every package file's name, size, and hash.
    pub source_fingerprint: String,
    pub source_directory: PathBuf,
    /// Attachment record ID to its file in the package.
    pub attachment_files: BTreeMap<String, PathBuf>,
}

impl LoadedPackage {
    /// The number of samples in all series files.
    pub fn sample_count(&self) -> u64 {
        self.series_files.iter().map(|file| file.reference.sample_count).sum()
    }
}

/// The parts of a package this version cannot import. The lists are sorted
/// and hold at most `MAX_REPRESENTATION_ISSUES` lines each.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackageRepresentation {
    pub source_database_id: Option<String>,
    pub source_database_revision: Option<u64>,
    /// `<path>: <reason>` for each record this version cannot represent.
    pub unrepresentable_records: Vec<String>,
    /// The path of each field this version does not know.
    pub unknown_fields: Vec<String>,
}

impl PackageRepresentation {
    pub fn message(&self) -> String {
        let detail = self.unrepresentable_records.first().or_else(|| self.unknown_fields.first());
        match detail {
            Some(detail) => format!("package has unknown fields or records that v1 cannot represent: {detail}"),
            None => "package has unknown fields or records that v1 cannot represent".to_string(),
        }
    }
}

/// A loaded package, or the reason it cannot be imported.
#[derive(Debug, Clone)]
pub enum PackageLoad {
    Loaded(Box<LoadedPackage>),
    Unrepresentable(PackageRepresentation),
}

/// Load and check a complete package. A package with unknown fields or
/// unrepresentable records is refused with the first problem.
pub fn load_package(input: &Path) -> Result<LoadedPackage> {
    match load_package_checked(input)? {
        PackageLoad::Loaded(loaded) => Ok(*loaded),
        PackageLoad::Unrepresentable(representation) => Err(Error::refused(representation.message())),
    }
}

/// Load and check a complete package, reporting unknown fields and
/// unrepresentable records as a result rather than an error.
pub fn load_package_checked(input: &Path) -> Result<PackageLoad> {
    let (document_path, source_directory) = package_document_path(input)?;
    let (document_bytes, document) = safe_read_json(&document_path)?;
    let mut fingerprint = vec![fingerprint_line(PACKAGE_DOCUMENT, &document_bytes)];
    drop(document_bytes);

    let version = document.get("athleticstandard_version").and_then(Value::as_str).map(str::to_string);
    if let Some(version) = &version {
        if version.starts_with("0.") {
            return Err(Error::refused(format!(
                "Athletic Standard {version} is a prerelease JSON format. v1 does not import 0.x files."
            )));
        }
    }
    let raw = raw_metadata(&document);
    match &version {
        None => {
            return Ok(PackageLoad::Unrepresentable(PackageRepresentation {
                source_database_id: raw.0,
                source_database_revision: raw.1,
                unrepresentable_records: vec!["athleticstandard_version: required version field is missing".into()],
                unknown_fields: Vec::new(),
            }));
        }
        Some(version) if !IMPORTABLE_PACKAGE_VERSIONS.contains(&version.as_str()) => {
            return Ok(PackageLoad::Unrepresentable(PackageRepresentation {
                source_database_id: raw.0,
                source_database_revision: raw.1,
                unrepresentable_records: Vec::new(),
                unknown_fields: vec![format!(
                    "athleticstandard_version: {version} is not supported by {ATHLETIC_STANDARD_VERSION}"
                )],
            }));
        }
        Some(_) => {}
    }
    let issues = package_schema().issues(&document);
    if !issues.is_empty() {
        return Ok(PackageLoad::Unrepresentable(representation(&document, &issues, None, raw.0, raw.1)));
    }
    let database_id = document["database_id"].as_str().unwrap_or_default().to_string();
    let database_revision = document["database_revision"].as_f64().unwrap_or(0.0) as u64;
    let references: Vec<MeasurementSeriesFileReference> = from_checked_value(&document["measurement_series_files"])?;
    if references.len() > MAX_PACKAGE_SERIES_FILES {
        return Err(Error::refused(format!("package has more than {MAX_PACKAGE_SERIES_FILES} series files")));
    }

    let manifest = match read_manifest(&source_directory, &database_id, database_revision, &mut fingerprint)? {
        ManifestRead::Absent => None,
        ManifestRead::Present(manifest) => Some(manifest),
        ManifestRead::Unrepresentable(representation) => return Ok(PackageLoad::Unrepresentable(representation)),
    };

    let mut object_count =
        ["records", "record_revisions", "record_deletions", "record_import_keys", "record_erasure_markers"]
            .iter()
            .map(|key| document[*key].as_array().map_or(0, Vec::len) as u64)
            .sum::<u64>();
    let mut ordered = references.clone();
    ordered.sort_by(|left, right| {
        locale_compare(&left.source_id, &right.source_id)
            .then_with(|| locale_compare(left.quantity.as_str(), right.quantity.as_str()))
            .then_with(|| locale_compare(&left.start_at, &right.start_at))
            .then_with(|| locale_compare(&left.end_at, &right.end_at))
            .then_with(|| locale_compare(&left.file, &right.file))
    });
    let mut series_files = Vec::new();
    let mut examples: Vec<String> = Vec::new();
    let mut boundaries: HashMap<(String, SeriesQuantity), (String, HashSet<String>)> = HashMap::new();
    for reference in ordered {
        let path = resolve_package_file(&source_directory, &reference.file)?;
        let (bytes, value) = safe_read_json(&path)?;
        let hash = durable::sha256_hex(&bytes);
        fingerprint.push(fingerprint_line(&reference.file, &bytes));
        drop(bytes);
        if hash != reference.sha256 {
            return Err(Error::refused(format!("series hash does not match package reference: {}", reference.file)));
        }
        let issues = measurement_series_file_schema().issues(&value);
        if !issues.is_empty() {
            return Ok(PackageLoad::Unrepresentable(representation(
                &value,
                &issues,
                Some(&reference.file),
                Some(database_id.clone()),
                Some(database_revision),
            )));
        }
        check_series_counts(&value, &reference, "series counts or grouping do not match")?;
        let start_at = value["start_at"].as_str().unwrap_or_default().to_string();
        let end_at = value["end_at"].as_str().unwrap_or_default().to_string();
        let records = value["records"].as_array().map(Vec::as_slice).unwrap_or_default();
        let group = (reference.source_id.clone(), reference.quantity);
        if let Some((previous_end, ids_at_end)) = boundaries.get(&group) {
            if start_at.as_str() < previous_end.as_str() {
                return Err(Error::refused(format!("measurement-series time ranges overlap: {}", reference.file)));
            }
            if &start_at == previous_end {
                let repeated = records.iter().find(|record| {
                    record["data"]["recorded_at"].as_str() == Some(start_at.as_str())
                        && ids_at_end.contains(record["record_id"].as_str().unwrap_or_default())
                });
                if let Some(repeated) = repeated {
                    return Err(Error::refused(format!(
                        "package repeats record ID across series files: {}",
                        repeated["record_id"].as_str().unwrap_or_default()
                    )));
                }
            }
        }
        let ids_at_end: HashSet<String> = records
            .iter()
            .filter(|record| record["data"]["recorded_at"].as_str() == Some(end_at.as_str()))
            .filter_map(|record| record["record_id"].as_str().map(str::to_string))
            .collect();
        boundaries.insert(group, (end_at, ids_at_end));
        for record in records {
            if examples.len() >= MAX_PREVIEW_RECORD_IDS {
                break;
            }
            examples.push(record["record_id"].as_str().unwrap_or_default().to_string());
        }
        object_count += ["records", "record_revisions", "record_deletions", "record_import_keys"]
            .iter()
            .map(|key| value[*key].as_array().map_or(0, Vec::len) as u64)
            .sum::<u64>();
        if object_count > MAX_PACKAGE_RECORD_OBJECTS {
            return Err(Error::refused(format!(
                "package has more than {MAX_PACKAGE_RECORD_OBJECTS} record and history objects"
            )));
        }
        series_files.push(LoadedSeriesFile { reference, path });
    }
    if object_count > MAX_PACKAGE_RECORD_OBJECTS {
        return Err(Error::refused(format!(
            "package has more than {MAX_PACKAGE_RECORD_OBJECTS} record and history objects"
        )));
    }

    let records: Vec<CurrentRecord> = document["records"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(CurrentRecord::from_value_non_sample)
        .collect::<std::result::Result<_, _>>()?;
    let revisions: Vec<RecordRevision> = document["record_revisions"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(RecordRevision::from_value_non_sample)
        .collect::<std::result::Result<_, _>>()?;
    let deletions: Vec<RecordDeletion> = from_checked_value(&document["record_deletions"])?;
    let erasure_markers: Vec<RecordErasureMarker> = from_checked_value(&document["record_erasure_markers"])?;
    let import_keys: Vec<RecordImportKey> = from_checked_value(&document["record_import_keys"])?;
    drop(document);

    let mut attachment_files = BTreeMap::new();
    for record in &records {
        let RecordData::Attachment(data) = &record.data else { continue };
        if record.is_deleted {
            continue;
        }
        let relative = relative_attachment_path(&stored_attachment_name(&record.record_id, data));
        let path = resolve_package_file(&source_directory, &relative)?;
        files::refuse_symbolic_link_components(&path, false)?;
        let bytes = fs::read(&path)?;
        check_attachment_bytes(&record.record_id, data, &bytes)?;
        fingerprint.push(fingerprint_line(&relative, &bytes));
        if bytes.len() as u64 != data.byte_count || durable::sha256_hex(&bytes) != data.sha256 {
            return Err(Error::refused(format!("attachment does not match record {}", record.record_id)));
        }
        attachment_files.insert(record.record_id.clone(), path);
        if let Some(manifest) = &manifest {
            if !manifest.files.iter().any(|file| file.file == relative && file.sha256 == data.sha256) {
                return Err(Error::refused(format!("attachment is missing from the manifest: {relative}")));
            }
        }
    }

    fingerprint.sort_by(|left, right| json::compare_utf16(left, right));
    Ok(PackageLoad::Loaded(Box::new(LoadedPackage {
        athleticstandard_version: version.unwrap_or_default(),
        database_id,
        database_revision,
        records,
        revisions,
        deletions,
        erasure_markers,
        import_keys,
        series_files,
        sample_record_id_examples: examples,
        source_path: files::absolute(input)?.display().to_string(),
        source_fingerprint: durable::sha256_hex(fingerprint.join("\n").as_bytes()),
        source_directory,
        attachment_files,
    })))
}

/// The records and history of one series file, read again for restore.
pub(crate) struct SeriesBatch {
    pub records: Vec<CurrentRecord>,
    pub revisions: Vec<RecordRevision>,
    pub deletions: Vec<RecordDeletion>,
    pub import_keys: Vec<RecordImportKey>,
}

/// Read a series file again. Its hash and counts must not have changed.
pub(crate) fn read_series_batch(file: &LoadedSeriesFile) -> Result<SeriesBatch> {
    let (bytes, value) = safe_read_json(&file.path)?;
    if durable::sha256_hex(&bytes) != file.reference.sha256 {
        return Err(Error::stale(format!("series hash changed after package load: {}", file.reference.file)));
    }
    drop(bytes);
    measurement_series_file_schema().validate(&value)?;
    check_series_counts(&value, &file.reference, "series counts or grouping changed after package load")?;
    let list = |key: &str| value[key].as_array().map(Vec::as_slice).unwrap_or_default();
    Ok(SeriesBatch {
        records: list("records").iter().map(CurrentRecord::from_value).collect::<std::result::Result<_, _>>()?,
        revisions: list("record_revisions")
            .iter()
            .map(RecordRevision::from_value)
            .collect::<std::result::Result<_, _>>()?,
        deletions: from_checked_value(&value["record_deletions"])?,
        import_keys: from_checked_value(&value["record_import_keys"])?,
    })
}

fn check_series_counts(value: &Value, reference: &MeasurementSeriesFileReference, message: &str) -> Result<()> {
    let count = |key: &str| value[key].as_array().map_or(0, Vec::len) as u64;
    if value["source_id"].as_str() != Some(reference.source_id.as_str())
        || value["quantity"].as_str() != Some(reference.quantity.as_str())
        || count("records") != reference.sample_count
        || count("record_revisions") != reference.revision_count
        || count("record_deletions") != reference.deletion_count
        || count("record_import_keys") != reference.import_key_count
    {
        return Err(Error::refused(format!("{message}: {}", reference.file)));
    }
    Ok(())
}

enum ManifestRead {
    Absent,
    Present(ExportManifest),
    Unrepresentable(PackageRepresentation),
}

fn read_manifest(
    source_directory: &Path,
    database_id: &str,
    database_revision: u64,
    fingerprint: &mut Vec<String>,
) -> Result<ManifestRead> {
    let path = source_directory.join(MANIFEST_DOCUMENT);
    if matches!(fs::symlink_metadata(&path), Err(error) if error.kind() == std::io::ErrorKind::NotFound) {
        return Ok(ManifestRead::Absent);
    }
    let (bytes, value) = safe_read_json(&path)?;
    fingerprint.push(fingerprint_line(MANIFEST_DOCUMENT, &bytes));
    let issues = ExportManifest::schema().issues(&value);
    if !issues.is_empty() {
        return Ok(ManifestRead::Unrepresentable(representation(
            &value,
            &issues,
            Some(MANIFEST_DOCUMENT),
            Some(database_id.to_string()),
            Some(database_revision),
        )));
    }
    let manifest: ExportManifest = from_checked_value(&value)?;
    if manifest.files.len() > MAX_PACKAGE_MANIFEST_FILES {
        return Err(Error::refused(format!("package has more than {MAX_PACKAGE_MANIFEST_FILES} files")));
    }
    if manifest.source_database_id != database_id || manifest.database_revision != database_revision {
        return Err(Error::refused("export manifest does not match the package database"));
    }
    for file in &manifest.files {
        let path = resolve_package_file(source_directory, &file.file)?;
        files::refuse_symbolic_link_components(&path, false)?;
        let (sha256, byte_count) = hash_file(&path)?;
        if byte_count != file.byte_count || sha256 != file.sha256 {
            return Err(Error::refused(format!("package file does not match its manifest: {}", file.file)));
        }
    }
    Ok(ManifestRead::Present(manifest))
}

fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut file = File::open(path)?;
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
    Ok((durable::hex(&hasher.finalize()), total))
}

fn fingerprint_line(name: &str, bytes: &[u8]) -> String {
    format!("{name}\u{0}{}\u{0}{}", bytes.len(), durable::sha256_hex(bytes))
}

/// A directory names its `athlete.ath.json`; a file names itself.
fn package_document_path(input: &Path) -> Result<(PathBuf, PathBuf)> {
    let absolute = files::absolute(input)?;
    files::refuse_symbolic_link_components(&absolute, false)?;
    let metadata = fs::symlink_metadata(&absolute)?;
    if metadata.is_dir() {
        return Ok((absolute.join(PACKAGE_DOCUMENT), absolute));
    }
    if !metadata.is_file() {
        return Err(Error::refused(format!("import path is not a regular file or directory: {}", absolute.display())));
    }
    let directory = durable::parent_of(&absolute);
    Ok((absolute, directory))
}

/// A path named by a package, which must stay inside the package folder.
pub(crate) fn resolve_package_file(source_directory: &Path, file: &str) -> Result<PathBuf> {
    let base = files::absolute(source_directory)?;
    let path = files::absolute(&base.join(file))?;
    let escapes = || Error::refused(format!("package file escapes its directory: {file}"));
    let within = path.strip_prefix(&base).map_err(|_| escapes())?;
    let text = within.to_string_lossy();
    if text.is_empty() || text.starts_with("..") || within.is_absolute() {
        return Err(escapes());
    }
    Ok(path)
}

/// Read a package JSON file within the size and nesting limits.
pub(crate) fn safe_read_json(path: &Path) -> Result<(Vec<u8>, Value)> {
    files::refuse_symbolic_link_components(path, false)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() {
        return Err(Error::refused(format!("package path is not a regular file: {}", path.display())));
    }
    if metadata.len() > MAX_PACKAGE_DOCUMENT_BYTES {
        return Err(Error::refused(format!(
            "package JSON exceeds {MAX_PACKAGE_DOCUMENT_BYTES} bytes: {}",
            path.display()
        )));
    }
    let bytes = fs::read(path)?;
    check_nesting(&bytes, path)?;
    let value = json::parse_bytes(&bytes)
        .map_err(|error| Error::refused(format!("package JSON is invalid: {}: {}", path.display(), error.message)))?;
    Ok((bytes, value))
}

/// Refuse nesting deeper than the limit before parsing.
fn check_nesting(bytes: &[u8], path: &Path) -> Result<()> {
    let mut depth = 0i64;
    let mut in_string = false;
    let mut escaped = false;
    for byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_PACKAGE_JSON_NESTING as i64 {
                    return Err(Error::refused(format!(
                        "package JSON exceeds nesting limit {MAX_PACKAGE_JSON_NESTING}: {}",
                        path.display()
                    )));
                }
            }
            b'}' | b']' => depth -= 1,
            _ => {}
        }
    }
    Ok(())
}

/// The database ID and revision a document states, when they are usable.
fn raw_metadata(value: &Value) -> (Option<String>, Option<u64>) {
    let id = value.get("database_id").and_then(Value::as_str).map(str::to_string);
    let revision = value.get("database_revision").and_then(Value::as_f64).and_then(|number| {
        (number.fract() == 0.0 && (0.0..=MAX_SAFE_INTEGER).contains(&number)).then_some(number as u64)
    });
    (id, revision)
}

/// Turn validation issues into the unknown-field and unrepresentable lists.
fn representation(
    value: &Value,
    issues: &[Issue],
    prefix: Option<&str>,
    source_database_id: Option<String>,
    source_database_revision: Option<u64>,
) -> PackageRepresentation {
    let raw = raw_metadata(value);
    let format_path = |path: &[PathSegment], tail: Option<&str>| -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(prefix) = prefix {
            parts.push(prefix.to_string());
        }
        parts.extend(path.iter().map(ToString::to_string));
        if let Some(tail) = tail {
            parts.push(tail.to_string());
        }
        parts.into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(".")
    };
    let mut unknown = Vec::new();
    let mut unrepresentable = Vec::new();
    for issue in issues {
        if issue.code == IssueCode::UnrecognizedKeys {
            for key in unknown_keys(value_at(value, &issue.path), &issue.message) {
                unknown.push(format_path(&issue.path, Some(&key)));
            }
            continue;
        }
        let mut path = format_path(&issue.path, None);
        if path.is_empty() {
            path = prefix.unwrap_or("package").to_string();
        }
        unrepresentable.push(format!("{path}: {}", issue.message));
    }
    let finish = |mut list: Vec<String>| {
        list.sort_by(|left, right| json::compare_utf16(left, right));
        list.dedup();
        list.truncate(MAX_REPRESENTATION_ISSUES);
        list
    };
    PackageRepresentation {
        source_database_id: source_database_id.or(raw.0),
        source_database_revision: source_database_revision.or(raw.1),
        unrepresentable_records: finish(unrepresentable),
        unknown_fields: finish(unknown),
    }
}

fn value_at<'a>(value: &'a Value, path: &[PathSegment]) -> Option<&'a Value> {
    path.iter().try_fold(value, |current, segment| match segment {
        PathSegment::Key(key) => current.get(key.as_str()),
        PathSegment::Index(index) => current.get(*index),
    })
}

/// The keys an unrecognized-key message lists, matched against the object's
/// own keys so that a key containing a comma or quote is read correctly.
fn unknown_keys(object: Option<&Value>, message: &str) -> Vec<String> {
    let list = message
        .strip_prefix("Unrecognized keys: ")
        .or_else(|| message.strip_prefix("Unrecognized key: "))
        .unwrap_or_default();
    let Some(Value::Object(map)) = object else { return Vec::new() };
    let mut keys = Vec::new();
    let mut rest = list;
    while !rest.is_empty() {
        let found = map
            .keys()
            .filter(|key| {
                let quoted = format!("\"{key}\"");
                rest.strip_prefix(&quoted).is_some_and(|after| after.is_empty() || after.starts_with(", "))
            })
            .max_by_key(|key| key.len());
        let Some(key) = found else { break };
        keys.push(key.clone());
        rest = &rest[key.len() + 2..];
        rest = rest.strip_prefix(", ").unwrap_or(rest);
    }
    keys
}

/// `String.prototype.localeCompare` for the text of series references: IDs,
/// quantity names, timestamps, and package paths. This follows the root
/// collation order for ASCII: punctuation, then digits, then letters without
/// regard to case, with lowercase first on a tie. Other characters follow
/// letters in code-point order. It sets only the order series files are read.
pub(crate) fn locale_compare(left: &str, right: &str) -> Ordering {
    fn primary(character: char) -> (u8, u32) {
        const PUNCTUATION: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";
        if character.is_whitespace() {
            return (0, character as u32);
        }
        if let Some(position) = PUNCTUATION.find(character) {
            return (1, position as u32);
        }
        if character.is_ascii_digit() {
            return (2, character as u32);
        }
        if character.is_ascii_alphabetic() {
            return (3, character.to_ascii_lowercase() as u32);
        }
        (4, character as u32)
    }
    let primary_order = left.chars().map(primary).cmp(right.chars().map(primary));
    primary_order
        .then_with(|| {
            let case = |character: char| u8::from(character.is_ascii_uppercase());
            left.chars().map(case).cmp(right.chars().map(case))
        })
        .then_with(|| left.cmp(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_order_puts_punctuation_before_digits_before_letters() {
        let mut words = vec!["b", "A", "a", "1", "_", "-", "a_b", "a1", "ab", "B"];
        words.sort_by(|left, right| locale_compare(left, right));
        assert_eq!(words, ["_", "-", "1", "a", "A", "a_b", "a1", "ab", "b", "B"]);
    }

    #[test]
    fn unknown_keys_are_read_from_the_object() {
        let value = json::parse(r#"{"a, \"b":1,"c":2,"known":3}"#).unwrap();
        assert_eq!(unknown_keys(Some(&value), r#"Unrecognized keys: "a, "b", "c""#), ["a, \"b", "c"]);
        assert_eq!(unknown_keys(Some(&value), r#"Unrecognized key: "c""#), ["c"]);
    }

    #[test]
    fn nesting_is_counted_outside_strings() {
        let path = Path::new("x.json");
        let deep = format!("{}null{}", "[".repeat(129), "]".repeat(129));
        assert!(check_nesting(deep.as_bytes(), path).is_err());
        let fine = format!("{}null{}", "[".repeat(128), "]".repeat(128));
        assert!(check_nesting(fine.as_bytes(), path).is_ok());
        let quoted = format!("\"{}\\\"\"", "[".repeat(200));
        assert!(check_nesting(quoted.as_bytes(), path).is_ok());
    }

    #[test]
    fn package_paths_must_stay_inside_the_package() {
        let base = Path::new("/package");
        assert!(resolve_package_file(base, "series/a.json").is_ok());
        for bad in ["../outside.json", "series/../../x", "", ".", "..x"] {
            assert!(resolve_package_file(base, bad).is_err(), "{bad}");
        }
    }
}
