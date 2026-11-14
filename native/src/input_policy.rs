//! Named limits for input this library reads.
//!
//! Every limit on package and archive input is a named constant here. A limit
//! may refuse work; it never truncates input silently so that work appears
//! complete. Values carry forward the limits of version 2.1.0.

/// Largest package JSON document read: the main document, a measurement-series
/// file, or the export manifest. Larger files are refused before reading.
pub const MAX_PACKAGE_DOCUMENT_BYTES: u64 = 512 * 1024 * 1024;

/// Most measurement-series files one package may reference.
pub const MAX_PACKAGE_SERIES_FILES: usize = 10_000;

/// Most files one export manifest may list: every series file, the main
/// document, and one more.
pub const MAX_PACKAGE_MANIFEST_FILES: usize = MAX_PACKAGE_SERIES_FILES + 2;

/// Most record and history objects in one package, counting current records,
/// revisions, deletions, import keys, and erasure markers in every file.
pub const MAX_PACKAGE_RECORD_OBJECTS: u64 = 100_000_000;

/// Deepest nesting of objects and arrays in a package JSON document.
pub const MAX_PACKAGE_JSON_NESTING: usize = 128;

/// Most unknown-field and unrepresentable-record lines one package preview
/// reports. The preview still refuses the whole package.
pub const MAX_REPRESENTATION_ISSUES: usize = 1_000;

/// Most record IDs one import preview lists in each list. The counts beside
/// the lists stay complete, and the preview says when a list was shortened.
pub const MAX_PREVIEW_RECORD_IDS: usize = 1_000;

/// Largest attachment file `ath log` and `ath correct` copy into a database.
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
