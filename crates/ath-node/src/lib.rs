#![deny(clippy::all)]

use napi_derive::napi;

#[napi]
pub fn sql_schema_version() -> i32 {
    ath_core::SQL_SCHEMA_VERSION
}

#[napi]
pub fn initial_migration_byte_length() -> u32 {
    ath_core::initial_migration_sql().len() as u32
}
