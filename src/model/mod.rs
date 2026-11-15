//! The Athletic Standard record model.
//!
//! This module has no database, terminal, network, or process dependency. It
//! defines every record shape once, validates input against the published
//! rules, and decodes it into typed values.

pub mod common;
pub mod data;
pub mod extensions;
pub mod grade;
pub mod json_schema;
pub mod package;
pub mod record;
pub mod registry;
pub mod schema;
pub mod time;

pub use common::*;
pub use data::*;
pub use package::*;
pub use record::*;
pub use schema::{Issue, IssueCode, PathSegment, Shape, ValidationError};
