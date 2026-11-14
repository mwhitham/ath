//! Provider imports: detection, reading, matching, preview, and write.
//!
//! See `README.md` in this directory for how a provider reader plugs in.
//!
//! - `detect` chooses the `ath import` route and recognizes provider exports.
//! - `input` reads folders, files, and zip archives within `input_policy`.
//! - `candidate` defines what a reader emits and the receipts all routes share.
//! - `provider` owns the one operation: stage, match, preview, and apply.
//! - `staging` holds the plan in a private temporary SQLite file.
//! - `csv`, `units`, `instant`, and `collate` are shared reading rules.
//! - `whoop` is the WHOOP reader.
//! - `apple` is the Apple Health reader, with its routes, ECGs, and HRV.

pub mod apple;
pub mod candidate;
pub mod collate;
pub mod csv;
pub mod detect;
pub mod input;
pub mod instant;
pub mod provider;
mod staging;
pub mod units;
pub mod whoop;

pub use candidate::{Candidate, KeyPart, MatchKey, Section};
pub use detect::{classify, detect_export, DetectedExport, ExportFormat, ImportRoute, SharedImport};
pub use provider::{
    preview, preview_export, CandidateSink, ImportConflict, ImportOptions, PlannedRecordRef, ProviderImportResult,
    ProviderPreview, ProviderReader, Route, Skipped,
};
pub use staging::{Action, PlannedRecord};
