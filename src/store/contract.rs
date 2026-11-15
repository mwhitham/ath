//! The published direct-SQL contract and checks against it.
//!
//! `spec/sql/contract.json` declares what direct SQL clients may rely on: the
//! schema step, the ordered migration files, header pragmas, stable views and
//! their columns, writable and internal tables, collection replacement rules,
//! transaction recipes, and enforcement triggers. This module reads that file
//! and checks two things:
//!
//! - The parts that follow from code agree with it: the migration chain, the
//!   schema step, the application ID, the trigger names in the published SQL,
//!   and the typed tables the record encoder writes. The child tables the
//!   encoder replaces are compared in the integration tests, which hold a
//!   record of every type.
//! - A live database matches it: every listed view, table, column, and
//!   trigger exists, no unlisted table or trigger exists, and the header
//!   pragmas mirror the metadata.

use std::collections::BTreeSet;

use rusqlite::Connection;
use serde::Deserialize;

use super::encode;
use super::migrate::{MIGRATION_CHAIN, SQL_SCHEMA_VERSION};
use super::{read_metadata, Store, SQLITE_APPLICATION_ID};
use crate::error::{Error, Result};
use crate::model::RecordType;

/// The contract file, embedded unchanged.
pub const CONTRACT_JSON: &str = include_str!("../../spec/sql/contract.json");

#[derive(Debug, Clone, Deserialize)]
pub struct SqlContract {
    pub contract: String,
    pub sql_schema_version: u32,
    pub minimum_sqlite_version: String,
    pub required_pragmas: Vec<RequiredPragma>,
    pub migrations: Vec<ContractMigration>,
    pub metadata_insert: MetadataInsert,
    pub stable_views: Vec<StableView>,
    pub stable_writable_tables: Vec<String>,
    pub internal_tables: Vec<String>,
    pub collection_replacement: CollectionReplacement,
    pub enforcement_triggers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequiredPragma {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContractMigration {
    pub version: u32,
    pub file: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetadataInsert {
    pub header_pragmas: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StableView {
    pub name: String,
    pub sql_schema_version: u32,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollectionReplacement {
    pub tables: Vec<ReplacedCollection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReplacedCollection {
    pub table: String,
    pub parent_column: String,
}

/// Parse the embedded contract.
pub fn published_contract() -> Result<SqlContract> {
    serde_json::from_str(CONTRACT_JSON).map_err(|error| Error::other(format!("spec/sql/contract.json: {error}")))
}

/// Trigger names in the published migration SQL, in file order.
pub fn published_trigger_names() -> Vec<String> {
    let mut names = Vec::new();
    for step in MIGRATION_CHAIN {
        let mut rest = step.sql;
        while let Some(index) = rest.find("CREATE TRIGGER") {
            rest = &rest[index + "CREATE TRIGGER".len()..];
            let name: String =
                rest.trim_start().chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            if !name.is_empty() {
                names.push(name);
            }
        }
    }
    names
}

/// Check that the contract file agrees with this program's code.
pub fn verify_contract_metadata(contract: &SqlContract) -> Vec<String> {
    let mut problems = Vec::new();
    if contract.contract != "athleticstandard-sql" {
        problems.push(format!("contract name is {}", contract.contract));
    }
    if contract.sql_schema_version != SQL_SCHEMA_VERSION {
        problems.push(format!(
            "contract sql_schema_version is {}, this program writes {SQL_SCHEMA_VERSION}",
            contract.sql_schema_version
        ));
    }
    let chain: Vec<(u32, &str, &str)> =
        MIGRATION_CHAIN.iter().map(|step| (step.version, step.file, step.description)).collect();
    let listed: Vec<(u32, &str, &str)> =
        contract.migrations.iter().map(|step| (step.version, step.file.as_str(), step.description.as_str())).collect();
    if chain != listed {
        problems.push("contract migrations do not match the embedded migration chain".to_string());
    }
    let expected_header = vec![
        format!("PRAGMA application_id = {SQLITE_APPLICATION_ID};"),
        format!("PRAGMA user_version = {SQL_SCHEMA_VERSION};"),
    ];
    if contract.metadata_insert.header_pragmas != expected_header {
        problems.push(format!("contract header pragmas are {:?}", contract.metadata_insert.header_pragmas));
    }
    let required: Vec<(&str, &str)> =
        contract.required_pragmas.iter().map(|pragma| (pragma.name.as_str(), pragma.value.as_str())).collect();
    let busy = super::SQLITE_BUSY_TIMEOUT.as_millis().to_string();
    if required != vec![("foreign_keys", "ON"), ("busy_timeout", busy.as_str())] {
        problems.push(format!("contract required pragmas are {required:?}"));
    }
    if contract.enforcement_triggers != published_trigger_names() {
        problems.push("contract enforcement triggers do not match the published migration SQL".to_string());
    }
    let writable: BTreeSet<&str> = contract.stable_writable_tables.iter().map(String::as_str).collect();
    for record_type in RecordType::ALL {
        let table = encode::table_for(*record_type);
        if !writable.contains(table) {
            problems.push(format!("typed table {table} is not a stable writable table"));
        }
    }
    for collection in &contract.collection_replacement.tables {
        if !writable.contains(collection.table.as_str()) {
            problems.push(format!("replaced collection {} is not a stable writable table", collection.table));
        }
    }
    problems
}

fn names(connection: &Connection, kind: &str) -> Result<BTreeSet<String>> {
    let mut statement = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = ?1 AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'")?;
    let rows = statement.query_map([kind], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn columns(connection: &Connection, table: &str) -> Result<Vec<String>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>("name"))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Check a live database at the current schema step against the contract.
/// Returns a list of problems; an empty list means the schema conforms.
pub fn verify_live_schema(store: &Store, contract: &SqlContract) -> Result<Vec<String>> {
    let connection = store.connection();
    let mut problems = Vec::new();
    let metadata = read_metadata(connection)?;
    if metadata.sql_schema_version != contract.sql_schema_version {
        problems.push(format!(
            "database_metadata.sql_schema_version is {}, contract says {}",
            metadata.sql_schema_version, contract.sql_schema_version
        ));
    }
    let user_version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if user_version != i64::from(metadata.sql_schema_version) {
        problems.push(format!("user_version {user_version} does not mirror sql_schema_version"));
    }
    let application_id: i64 = connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    if application_id != i64::from(SQLITE_APPLICATION_ID) {
        problems.push(format!("application_id is {application_id}"));
    }

    let tables = names(connection, "table")?;
    let declared: BTreeSet<String> =
        contract.stable_writable_tables.iter().chain(contract.internal_tables.iter()).cloned().collect();
    for table in declared.difference(&tables) {
        problems.push(format!("contract table {table} is missing"));
    }
    for table in tables.difference(&declared) {
        problems.push(format!("table {table} is not in the contract"));
    }

    let views = names(connection, "view")?;
    for view in &contract.stable_views {
        if !views.contains(&view.name) {
            problems.push(format!("stable view {} is missing", view.name));
            continue;
        }
        let live = columns(connection, &view.name)?;
        if live != view.columns {
            problems.push(format!("stable view {} has columns {live:?}, contract lists {:?}", view.name, view.columns));
        }
    }
    let listed_views: BTreeSet<String> = contract.stable_views.iter().map(|view| view.name.clone()).collect();
    for view in views.difference(&listed_views) {
        problems.push(format!("view {view} is not in the contract"));
    }

    let triggers = names(connection, "trigger")?;
    let listed_triggers: BTreeSet<String> = contract.enforcement_triggers.iter().cloned().collect();
    for trigger in listed_triggers.difference(&triggers) {
        problems.push(format!("enforcement trigger {trigger} is missing"));
    }
    for trigger in triggers.difference(&listed_triggers) {
        problems.push(format!("trigger {trigger} is not in the contract"));
    }

    for collection in &contract.collection_replacement.tables {
        if tables.contains(&collection.table)
            && !columns(connection, &collection.table)?.contains(&collection.parent_column)
        {
            problems.push(format!("{} has no column {}", collection.table, collection.parent_column));
        }
    }
    Ok(problems)
}
