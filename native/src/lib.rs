//! Athletic Standard: an open, local-first record of an athlete's training and
//! recovery data. SQLite is the live local record. JSON is the complete
//! portable package.
//!
//! Module responsibilities are described in `docs/architecture.md`.

pub mod calculate;
pub mod error;
pub mod files;
pub mod ids;
pub mod json;
pub mod log;
pub mod model;
pub mod prediction;
pub mod store;
pub mod view;

pub use error::{Error, Result};
