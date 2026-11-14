//! Athletic Standard: an open, local-first record of an athlete's training and
//! recovery data. SQLite is the live local record. JSON is the complete
//! portable package.
//!
//! Module responsibilities are described in `docs/architecture.md`.

pub mod assets;
pub mod calculate;
pub mod cli;
pub mod error;
pub mod files;
pub mod ids;
pub mod import;
pub mod input_policy;
pub mod json;
pub mod log;
pub mod model;
pub mod package;
pub mod prediction;
pub mod protocol;
pub mod runtime;
pub mod store;
pub mod view;

pub use error::{Error, Result};
