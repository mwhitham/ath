//! Complete JSON packages, attachment files, package conflicts, and privacy
//! erasure.
//!
//! A package is the complete portable form of a database: `athlete.ath.json`
//! holds every record except measurement samples, with complete history;
//! `series/` files hold the samples; `attachments/` holds attachment files;
//! `export-manifest.json`, written last, names every file with its hash.
//!
//! Export reads one database snapshot and streams it to files. Restore and
//! merge validate the complete package and write it through the shared record
//! encoder. Restore keeps the package's database ID, record IDs, history, and
//! database revision. Merge keeps the conflict workflow for the same database.

pub mod attachments;
pub mod conflicts;
pub mod erase;
pub mod export;
pub mod load;
pub mod restore;
pub(crate) mod series;

#[cfg(test)]
pub(crate) mod failpoints;

#[cfg(test)]
mod tests;

/// Fail or stop here when a unit test armed this point. Expands to nothing
/// outside unit tests, so release builds have no failure points.
macro_rules! failpoint {
    ($name:expr) => {
        #[cfg(test)]
        $crate::package::failpoints::check($name)?;
    };
}

pub(crate) use failpoint;

pub use attachments::{
    apply_with_attachments, attachment_directory, prepare_attachments, resume_pending_file_operations,
    stored_attachment_name, PendingCleanupReport, PendingFileFailure, PendingFileOperation, PreparedAttachment,
};
pub use conflicts::{
    assert_conflict_set_current, clear_conflict_set, conflict_resolutions, conflict_set_path, read_conflict_set,
    resolve_conflict, save_conflict_set, show_conflicts, ConflictChoice, ConflictDetail, ConflictResolveResult,
    ConflictSet, ConflictsShow, SavedConflict,
};
pub use erase::{erase_record, preview_erasure, ErasureAttachment, ErasurePreview, ErasureRecord, ErasureResult};
pub use export::{export_package, ExportOptions, ExportResult};
pub use load::{
    load_package, load_package_checked, LoadedPackage, LoadedSeriesFile, PackageLoad, PackageRepresentation,
};
pub use restore::{
    apply_package_import, imported_record, preview_loaded, preview_package_import, PackageImportApplied,
    PackageImportConflict, PackageImportPreview, PackageImportResolution,
};

/// The main package document name.
pub const PACKAGE_DOCUMENT: &str = "athlete.ath.json";

/// The manifest name. The manifest is written last.
pub const MANIFEST_DOCUMENT: &str = "export-manifest.json";

/// Samples per measurement-series file. A source and quantity with more
/// samples is split into several files in time order.
pub const SERIES_CHUNK_RECORDS: usize = 50_000;
