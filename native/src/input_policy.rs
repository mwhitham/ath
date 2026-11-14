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

// ---- Apple Health exports (import) ---------------------------------------
//
// These carry forward the 2.1.0 limits in `src/import/apple.ts`. The 2.1.0
// helper also returned its whole result through a 64 MiB output buffer, so a
// large export failed outright. The native reader has no such limit: it
// streams `export.xml`, and records that must wait for later files (samples
// sorted by day, sleep fragments joined into nights, workouts waiting for
// their route files) wait in a private temporary SQLite file, not in memory.
//
// - `export.xml` in a folder, or as a zip entry measured while it expands, is
//   refused over `MAX_APPLE_XML_BYTES`.
// - Each GPX route file and ECG CSV is read whole, so each is refused over
//   `MAX_PROVIDER_TEXT_BYTES` in a folder or `MAX_ZIP_MEMORY_ENTRY_BYTES` in a
//   zip. Only one is held at a time.
// - A folder export lists at most `MAX_PROVIDER_FILES` files, and a zip at most
//   `MAX_ZIP_ENTRIES` entries.
// - A route file is read only from `workout-routes/` beside `export.xml`, and a
//   bare `export.xml` reads only its `workout-routes/` and
//   `electrocardiograms/` folders. A symbolic link refuses the export.

/// Bytes of one Apple Health `export.xml`: 16 GiB.
pub const MAX_APPLE_XML_BYTES: u64 = MAX_ZIP_ENTRY_BYTES;

/// Staged plan rows written or applied per batch. This bounds memory while one
/// transaction covers the complete import.
pub const IMPORT_BATCH_RECORDS: usize = 500;
