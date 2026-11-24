//! Moving a record between hosts (D267). Built in step 8.

use axum::Router;

use crate::api::Shared;

pub fn routes() -> Router<Shared> {
    Router::new()
}
