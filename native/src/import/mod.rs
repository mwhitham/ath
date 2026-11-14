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
//! - `whoop` is the WHOOP reader, and `oura` the Oura reader.
//! - `garmin`, `fit`, and `strava` read Garmin and Strava files and the
//!   Garmin pull's responses. `shared` holds what they share with `mapping`,
//!   which reads a JSON file through a mapping file.

pub mod candidate;
pub mod collate;
pub mod csv;
pub mod detect;
pub mod fit;
pub mod garmin;
pub mod input;
pub mod instant;
pub mod mapping;
pub mod oura;
pub mod provider;
pub mod shared;
mod staging;
pub mod strava;
pub mod units;
pub mod whoop;

use std::path::Path;

use crate::error::Result;
use crate::store::Store;

pub use candidate::{Candidate, KeyPart, MatchKey, Section};
pub use detect::{classify, detect_export, DetectedExport, ExportFormat, ImportRoute, SharedImport};
pub use garmin::PersonalDay;
pub use mapping::Mapping;
pub use provider::{
    preview, preview_export, preview_route, CandidateSink, ImportConflict, ImportOptions, KeyedPolicy, MatchPolicy,
    PlannedRecordRef, ProviderImportResult, ProviderPreview, ProviderReader, ReportKind, Route, RouteReader,
    RouteReport, Skipped,
};
pub use staging::{Action, PlannedRecord};

/// Preview a Garmin or Strava file, folder, or archive (`ath import`).
pub fn preview_shared(
    store: &Store,
    path: &Path,
    kind: SharedImport,
    options: &ImportOptions,
) -> Result<ProviderPreview> {
    match kind {
        SharedImport::Garmin => preview_route(store, &garmin::GarminReader::new(path), options),
        SharedImport::Strava => preview_route(store, &strava::StravaReader::new(path), options),
    }
}

/// Preview a JSON file read through a checked mapping (`ath import --mapping`).
pub fn preview_mapped(
    store: &Store,
    input: &Path,
    mapping: &Mapping,
    options: &ImportOptions,
) -> Result<ProviderPreview> {
    preview_route(store, &mapping::MappedReader::new(mapping.clone(), input), options)
}

/// Preview one day the Garmin personal connection returned (`ath pull garmin`).
/// `crate::runtime::garmin_pull::fetch_day` reads the day; no database lock
/// is held while it waits for the network.
pub fn preview_pull(store: &Store, day: PersonalDay, options: &ImportOptions) -> Result<ProviderPreview> {
    preview_route(store, &garmin::PersonalReader { day }, options)
}
