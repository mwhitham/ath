//! UniFFI Swift/Kotlin bindings will be generated from this crate once the mobile
//! toolchain job is wired. For now it shares the same surface as the future FFI layer.

/// SQL schema version exposed to Swift hosts.
pub fn sql_schema_version() -> i32 {
    ath_core::SQL_SCHEMA_VERSION
}
