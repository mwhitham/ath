//! Limits on untrusted input.
//!
//! Every limit on archives, provider exports, and packages is a named constant
//! here, so a reviewer can see in one place what the program refuses. A limit
//! may reject work. It never truncates work and reports it as complete
//! (spec 2.1.1 section 8).

// ---- Provider exports and archives (import) -----------------------------
//
// These carry forward the 2.1.0 limits in `src/import/zip.ts` and
// `src/import/detect.ts`. Archive sizes are checked against the declared
// central-directory sizes before anything is read, and again against the
// bytes actually expanded while an entry is read.

/// Entries one provider archive may hold.
pub const MAX_ZIP_ENTRIES: usize = 100_000;

/// Expanded bytes one archive entry may hold: 16 GiB.
pub const MAX_ZIP_ENTRY_BYTES: u64 = 16 * 1024 * 1024 * 1024;

/// Expanded bytes all entries of one archive may hold together: 64 GiB.
pub const MAX_ZIP_TOTAL_BYTES: u64 = 64 * 1024 * 1024 * 1024;

/// Expanded bytes of an archive entry read whole into memory, such as a CSV: 64 MiB.
pub const MAX_ZIP_MEMORY_ENTRY_BYTES: u64 = 64 * 1024 * 1024;

/// Largest expanded-to-compressed ratio for one archive entry.
pub const MAX_ZIP_COMPRESSION_RATIO: u64 = 1_000;

/// Files one provider directory may hold.
pub const MAX_PROVIDER_FILES: usize = 100_000;

/// Bytes of one provider text file read whole into memory, such as a CSV.
pub const MAX_PROVIDER_TEXT_BYTES: u64 = MAX_ZIP_MEMORY_ENTRY_BYTES;

/// Bytes read from the start of a CSV to find its header row during detection.
pub const DETECTION_HEADER_BYTES: usize = 8192;

/// UTF-16 code units of a `.json` file inspected to recognize a Garmin activity.
pub const DETECTION_JSON_HEAD_CHARS: usize = 8000;

/// Staged plan rows written or applied per batch. This bounds memory while one
/// transaction covers the complete import.
pub const IMPORT_BATCH_RECORDS: usize = 500;

// ---- Limits by import route --------------------------------------------
//
// - Apple Health, WHOOP, and Oura exports: the archive limits, the folder
//   limit, and `MAX_PROVIDER_TEXT_BYTES` for each CSV read whole.
// - Garmin and Strava files, folders, and archives: the archive limits, the
//   folder limit, and `MAX_SHARED_FILE_BYTES` for each file read whole. The
//   2.1.0 helper read these inputs whole with no limit of its own, and it
//   followed links inside a folder. A link anywhere in the input is refused.
// - The Garmin pull: `MAX_PULL_RESPONSE_BYTES` for each response.
// - Mapped imports: `MAX_MAPPED_INPUT_BYTES` for the JSON file read whole.
//   The mapping file itself is read whole; it is a small text file a person
//   edits.

/// Bytes of one Garmin or Strava file read whole into memory: a FIT, GPX, CSV,
/// or JSON activity file. 256 MiB.
pub const MAX_SHARED_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// Bytes of one Garmin personal connection response: 10 MiB, the limit the
/// 2.1.0 HTTP client applied when it read a response as text.
pub const MAX_PULL_RESPONSE_BYTES: u64 = 10 * 1024 * 1024;

/// Bytes of the JSON file a mapped import reads whole into memory. 512 MiB.
pub const MAX_MAPPED_INPUT_BYTES: u64 = 512 * 1024 * 1024;
