//! `ath check`: confirm that a database still obeys every rule of the format.
//!
//! The check reads one snapshot. It never changes the file. It reports SQLite
//! integrity, header and metadata versions, foreign keys, the two-tier wall,
//! record and revision shapes, revision chains, deletions, erasure markers,
//! attachment files, workout segment order, saved relationship rules, and
//! whether each record's typed row and child rows still match its current
//! revision. The typed comparison uses the record encoder's own field
//! description (`encode::typed_columns` and `encode::child_rows`), so there is
//! no second list of columns to keep in step.
//!
//! The work is one ordered pass over `records` merged with one ordered pass
//! over `record_revisions`. Each stored JSON text is parsed once. Typed rows and
//! child rows are read by primary key. Issues are listed in the 2.1.0 order and
//! capped at 1,000, with a final warning that counts the rest.

use rusqlite::types::Value as Sql;
use rusqlite::{Connection, OptionalExtension, Row};
use serde::Serialize;

use super::encode::{self, Layout};
use super::pending;
use super::write::validate_relationships;
use super::{read_metadata, DatabaseMetadata, Store, SQLITE_APPLICATION_ID, SQL_SCHEMA_VERSION};
use crate::error::Result;
use crate::files::{self, attachments as attachment_files};
use crate::json::{self, Map, Value};
use crate::model::schema::decode;
use crate::model::*;

/// Issues listed before the rest are only counted.
pub const MAX_REPORTED_ISSUES: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// One finding. `record_id` is present when the finding concerns one record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckIssue {
    pub severity: Severity,
    pub check: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    pub message: String,
}

/// `checkDatabase`: the check without the file name or export step.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DatabaseCheckResult {
    /// Null when the metadata cannot be read.
    pub metadata: Option<DatabaseMetadata>,
    /// True when no issue has severity `error`.
    pub valid: bool,
    /// Rows in `records`, including deleted records.
    pub record_count: u64,
    pub issues: Vec<CheckIssue>,
}

/// The `ath check --json` result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckReport {
    pub file: String,
    #[serde(flatten)]
    pub result: DatabaseCheckResult,
    /// True only when an export check ran and passed.
    pub export_validated: bool,
}

/// Builds and validates a complete temporary package, then removes it. The
/// package module supplies this for `--validate-export`. An error stops the
/// check, as it did in 2.1.0.
pub type ExportValidator<'a> = &'a dyn Fn(&Store) -> Result<()>;

/// `ath check`: the database check, then the optional export check.
pub fn check(store: &Store, validate_export: Option<ExportValidator<'_>>) -> Result<CheckReport> {
    let result = check_database(store)?;
    let mut export_validated = false;
    if let Some(validate) = validate_export {
        validate(store)?;
        export_validated = true;
    }
    Ok(CheckReport { file: store.path().display().to_string(), result, export_validated })
}

// ---- Issue collection ---------------------------------------------------

fn issue(severity: Severity, check: &str, record_id: Option<&str>, message: impl Into<String>) -> CheckIssue {
    CheckIssue { severity, check: check.to_string(), record_id: record_id.map(str::to_string), message: message.into() }
}

/// Issues of one kind, kept in order. Past the overall limit an issue can
/// never be listed, so it is only counted.
#[derive(Default)]
struct Bucket {
    issues: Vec<CheckIssue>,
    overflow: usize,
    overflow_errors: usize,
}

impl Bucket {
    fn push(&mut self, issue: CheckIssue) {
        if self.issues.len() < MAX_REPORTED_ISSUES {
            self.issues.push(issue);
        } else {
            self.overflow += 1;
            if issue.severity == Severity::Error {
                self.overflow_errors += 1;
            }
        }
    }

    fn error(&mut self, check: &str, record_id: Option<&str>, message: impl Into<String>) {
        self.push(issue(Severity::Error, check, record_id, message));
    }
}

#[derive(Default)]
struct Limited {
    issues: Vec<CheckIssue>,
    truncated: usize,
    truncated_errors: usize,
}

impl Limited {
    fn take(&mut self, bucket: Bucket) {
        for item in bucket.issues {
            if self.issues.len() < MAX_REPORTED_ISSUES {
                self.issues.push(item);
            } else {
                self.truncated += 1;
                if item.severity == Severity::Error {
                    self.truncated_errors += 1;
                }
            }
        }
        self.truncated += bucket.overflow;
        self.truncated_errors += bucket.overflow_errors;
    }
}

/// Buckets in the order 2.1.0 listed its findings.
#[derive(Default)]
struct Findings {
    header: Bucket,
    current: Bucket,
    revision: Bucket,
    chain: Bucket,
    deletion: Bucket,
    erasure: Bucket,
    typed_denormalized: Vec<Bucket>,
    typed_table: Vec<Bucket>,
    relationship: Bucket,
    pending: Bucket,
}

fn type_index(record_type: RecordType) -> usize {
    RecordType::ALL.iter().position(|candidate| *candidate == record_type).expect("every record type is listed")
}

// ---- The check ----------------------------------------------------------

/// Check an open database. Read-only and write-capable stores both work; the
/// check holds one read transaction and writes nothing.
pub fn check_database(store: &Store) -> Result<DatabaseCheckResult> {
    let snapshot = rusqlite::Transaction::new_unchecked(store.connection(), rusqlite::TransactionBehavior::Deferred)?;
    let result = check_snapshot(&snapshot, store);
    snapshot.rollback()?;
    result
}

fn check_snapshot(connection: &Connection, store: &Store) -> Result<DatabaseCheckResult> {
    let mut findings = Findings {
        typed_denormalized: RecordType::ALL.iter().map(|_| Bucket::default()).collect(),
        typed_table: RecordType::ALL.iter().map(|_| Bucket::default()).collect(),
        ..Findings::default()
    };

    let metadata = match read_metadata(connection) {
        Ok(metadata) => Some(metadata),
        Err(error) => {
            findings.header.error("metadata", None, error.to_string());
            None
        }
    };
    let foreign_keys: i64 = connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    if foreign_keys != 1 {
        findings.header.error("foreign_key", None, "SQLite foreign-key enforcement is disabled");
    }
    if let Some(metadata) = &metadata {
        if metadata.sql_schema_version != SQL_SCHEMA_VERSION {
            findings.header.error(
                "schema_version",
                None,
                format!(
                    "database metadata schema version is {}, expected {SQL_SCHEMA_VERSION}",
                    metadata.sql_schema_version
                ),
            );
        }
        let user_version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if user_version != i64::from(metadata.sql_schema_version) {
            findings.header.error(
                "schema_version",
                None,
                format!(
                    "SQLite user_version is {user_version}, but database metadata says {}",
                    metadata.sql_schema_version
                ),
            );
        }
        let application_id: i64 = connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
        if application_id != i64::from(SQLITE_APPLICATION_ID) {
            findings.header.error(
                "application_id",
                None,
                format!("SQLite application_id is {application_id}, expected {SQLITE_APPLICATION_ID}"),
            );
        }
    }
    {
        let mut statement = connection.prepare("PRAGMA integrity_check")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let message = sql_text(&row.get::<_, Sql>(0)?);
            if message != "ok" {
                findings.header.error("integrity", None, message);
            }
        }
    }
    {
        // SQLite lists violations in its own schema order, which differs
        // between SQLite versions. Sorting keeps the report stable.
        let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
        let mut rows = statement.query([])?;
        let mut violations = Vec::new();
        while let Some(row) = rows.next()? {
            let table = sql_text(&row.get::<_, Sql>(0)?);
            let rowid: Option<i64> = row.get(1)?;
            let parent = sql_text(&row.get::<_, Sql>(2)?);
            let fkid: i64 = row.get(3)?;
            violations.push((table, rowid, parent, fkid));
        }
        violations.sort();
        for (table, rowid, parent, fkid) in violations {
            let rowid = rowid.map_or("null".to_string(), |rowid| rowid.to_string());
            findings.header.error(
                "foreign_key",
                None,
                format!("{table} row {rowid} references missing row {parent} ({fkid})"),
            );
        }
    }
    for table in ["athlete_reports", "nutrition_entries", "reported_nutrition_items"] {
        if table_has_column(connection, table, "source_id")? {
            findings.header.error("two_tier_wall", None, format!("{table} must not have a source_id column"));
        }
    }
    let record_count: i64 = connection.query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))?;

    let context = RecordContext {
        connection,
        layout: store.layout(),
        attachments: attachment_files::attachment_directory(store.path()),
    };
    merge_records_and_revisions(&context, &mut findings)?;
    check_deletions(connection, &mut findings.deletion)?;
    check_erasures(connection, &mut findings.erasure)?;
    for operation in pending::pending_file_operations(connection)? {
        findings.pending.push(issue(Severity::Warning, "pending_file_operation", None, operation.describe()));
    }

    let mut limited = Limited::default();
    let Findings {
        header,
        current,
        revision,
        chain,
        deletion,
        erasure,
        typed_denormalized,
        typed_table,
        relationship,
        pending,
    } = findings;
    for bucket in [header, current, revision, chain, deletion, erasure] {
        limited.take(bucket);
    }
    for bucket in typed_denormalized {
        limited.take(bucket);
    }
    for bucket in typed_table {
        limited.take(bucket);
    }
    limited.take(relationship);
    limited.take(pending);

    let valid = limited.truncated_errors == 0 && !limited.issues.iter().any(|item| item.severity == Severity::Error);
    let mut issues = limited.issues;
    if limited.truncated > 0 {
        issues.push(issue(
            Severity::Warning,
            "issue_limit",
            None,
            format!(
                "{} more issue{} not listed after the first {MAX_REPORTED_ISSUES}",
                limited.truncated,
                if limited.truncated == 1 { " was" } else { "s were" }
            ),
        ));
    }
    Ok(DatabaseCheckResult { metadata, valid, record_count: record_count as u64, issues })
}

fn table_has_column(connection: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>("name")? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `String(value)` for a SQL value, as 2.1.0 printed it.
fn sql_text(value: &Sql) -> String {
    match value {
        Sql::Null => "null".to_string(),
        Sql::Integer(number) => json::format_number(*number as f64),
        Sql::Real(number) => json::format_number(*number),
        Sql::Text(text) => text.clone(),
        Sql::Blob(bytes) => bytes.iter().map(|byte| byte.to_string()).collect::<Vec<_>>().join(","),
    }
}

/// The 2.1.0 typed-column comparison: null only equals null, numbers compare
/// by value, and everything else compares as text.
fn sql_values_equal(stored: &Sql, expected: &Sql) -> bool {
    let number = |value: &Sql| match value {
        Sql::Integer(number) => Some(*number as f64),
        Sql::Real(number) => Some(*number),
        _ => None,
    };
    match (stored, expected) {
        (Sql::Null, Sql::Null) => true,
        (Sql::Null, _) | (_, Sql::Null) => false,
        (Sql::Blob(left), Sql::Blob(right)) => left == right,
        _ => match (number(stored), number(expected)) {
            (Some(left), Some(right)) => left == right,
            _ => sql_text(stored) == sql_text(expected),
        },
    }
}

// ---- Records and revisions ----------------------------------------------

struct RecordContext<'a> {
    connection: &'a Connection,
    layout: Layout,
    attachments: std::path::PathBuf,
}

struct RecordRow {
    record_id: String,
    record_type: String,
    current_revision: i64,
    created_at: String,
    is_deleted: i64,
}

struct RevisionRow {
    record_id: String,
    record_type: String,
    revision: i64,
    previous_revision: Option<i64>,
    became_known_at: String,
    became_known_at_basis: String,
    changed_by_json: String,
    record_json: String,
    extensions_json: Option<String>,
}

/// The ordered pass over records. Each statement here and below is listed by
/// `check_statements` so its query plan can be inspected.
pub const RECORDS_IN_ORDER: &str =
    "SELECT record_id, record_type, current_revision, created_at, is_deleted FROM records ORDER BY record_id";

/// The ordered pass over every retained revision.
pub const REVISIONS_IN_ORDER: &str = "SELECT record_id, record_type, revision, previous_revision, became_known_at,
        became_known_at_basis, changed_by_json, record_json, extensions_json
   FROM record_revisions ORDER BY record_id, revision";

fn record_row(row: &Row<'_>) -> rusqlite::Result<RecordRow> {
    Ok(RecordRow {
        record_id: row.get(0)?,
        record_type: row.get(1)?,
        current_revision: row.get(2)?,
        created_at: row.get(3)?,
        is_deleted: row.get(4)?,
    })
}

fn revision_row(row: &Row<'_>) -> rusqlite::Result<RevisionRow> {
    Ok(RevisionRow {
        record_id: row.get(0)?,
        record_type: row.get(1)?,
        revision: row.get(2)?,
        previous_revision: row.get(3)?,
        became_known_at: row.get(4)?,
        became_known_at_basis: row.get(5)?,
        changed_by_json: row.get(6)?,
        record_json: row.get(7)?,
        extensions_json: row.get(8)?,
    })
}

/// Walk `records` and `record_revisions` together, both in `record_id`
/// order. Each record meets its own revisions once.
fn merge_records_and_revisions(context: &RecordContext<'_>, findings: &mut Findings) -> Result<()> {
    let mut record_statement = context.connection.prepare(RECORDS_IN_ORDER)?;
    let mut revision_statement = context.connection.prepare(REVISIONS_IN_ORDER)?;
    let mut records = record_statement.query([])?;
    let mut revisions = revision_statement.query([])?;
    fn advance(revisions: &mut rusqlite::Rows<'_>) -> Result<Option<RevisionRow>> {
        Ok(match revisions.next()? {
            Some(row) => Some(revision_row(row)?),
            None => None,
        })
    }
    let mut next_revision = advance(&mut revisions)?;
    while let Some(row) = records.next()? {
        let record = record_row(row)?;
        // Revisions without a records row are checked on their own. SQLite's
        // foreign-key check reports the missing parent.
        while next_revision.as_ref().is_some_and(|revision| revision.record_id < record.record_id) {
            let orphan = next_revision.take().expect("checked above");
            check_revision(&orphan, &mut findings.revision);
            next_revision = advance(&mut revisions)?;
        }
        let mut group = Vec::new();
        while next_revision.as_ref().is_some_and(|revision| revision.record_id == record.record_id) {
            group.push(next_revision.take().expect("checked above"));
            next_revision = advance(&mut revisions)?;
        }
        check_record(context, &record, group, findings)?;
    }
    while let Some(orphan) = next_revision.take() {
        check_revision(&orphan, &mut findings.revision);
        next_revision = advance(&mut revisions)?;
    }
    Ok(())
}

fn parse_extensions(text: Option<&str>) -> std::result::Result<Option<Value>, String> {
    match text {
        None => Ok(None),
        Some(text) => match json::parse(text) {
            Ok(Value::Null) => Ok(None),
            Ok(value) => Ok(Some(value)),
            Err(error) => Err(format!("extensions_json: {error}")),
        },
    }
}

/// Validate one stored revision. Returns its parsed data for reuse.
fn check_revision(revision: &RevisionRow, bucket: &mut Bucket) -> Option<Value> {
    let fail = |bucket: &mut Bucket, message: String| bucket.error("revision", Some(&revision.record_id), message);
    let data = match json::parse(&revision.record_json) {
        Ok(data) => data,
        Err(error) => {
            fail(bucket, format!("record_json: {error}"));
            return None;
        }
    };
    let changed_by = match json::parse(&revision.changed_by_json) {
        Ok(value) => value,
        Err(error) => {
            fail(bucket, format!("changed_by_json: {error}"));
            return Some(data);
        }
    };
    let extensions = match parse_extensions(revision.extensions_json.as_deref()) {
        Ok(extensions) => extensions,
        Err(message) => {
            fail(bucket, message);
            return Some(data);
        }
    };
    let mut payload = Map::new();
    payload.insert("record_id".into(), Value::String(revision.record_id.clone()));
    payload.insert("record_type".into(), Value::String(revision.record_type.clone()));
    payload.insert("revision".into(), Value::from(revision.revision));
    payload.insert("previous_revision".into(), revision.previous_revision.map_or(Value::Null, Value::from));
    payload.insert("became_known_at".into(), Value::String(revision.became_known_at.clone()));
    payload.insert("became_known_at_basis".into(), Value::String(revision.became_known_at_basis.clone()));
    payload.insert("changed_by".into(), changed_by);
    payload.insert("data".into(), data);
    if let Some(extensions) = extensions {
        payload.insert("extensions".into(), extensions);
    }
    let payload = Value::Object(payload);
    // `RecordRevision` is a plain union, so checking it tries up to 24
    // variants. The variant for the stored type decides validity on its own;
    // the union runs only to word a failure exactly as 2.1.0 did.
    let valid = RecordType::parse(&revision.record_type)
        .is_some_and(|record_type| revision_variant(record_type).validate(&payload).is_ok());
    if valid {
        // Valid without the union.
    } else if let Err(error) = record_revision_schema().validate(&payload) {
        fail(bucket, error.to_string());
    }
    match payload {
        Value::Object(mut map) => map.remove("data"),
        _ => None,
    }
}

/// The revision chain rule: revisions 1..n with no gaps, each naming the one
/// before, all of the record's type, ending at the current revision.
fn check_chain(record: &RecordRow, revisions: &[RevisionRow], bucket: &mut Bucket) {
    let Some(final_revision) = revisions.iter().map(|revision| revision.revision).max() else { return };
    for (index, revision) in revisions.iter().enumerate() {
        let expected = index as i64 + 1;
        let expected_previous = if expected == 1 { None } else { Some(expected - 1) };
        if revision.revision != expected
            || revision.previous_revision != expected_previous
            || revision.record_type != record.record_type
            || final_revision != record.current_revision
        {
            bucket.error(
                "revision_chain",
                Some(&revision.record_id),
                format!(
                    "revision chain breaks at stored revision {}; expected {expected} after {}",
                    revision.revision,
                    expected_previous.map_or("null".to_string(), |value| value.to_string())
                ),
            );
        }
    }
}

fn check_record(
    context: &RecordContext<'_>,
    record: &RecordRow,
    revisions: Vec<RevisionRow>,
    findings: &mut Findings,
) -> Result<()> {
    let mut current_data = None;
    let mut current_row = None;
    for revision in &revisions {
        let data = check_revision(revision, &mut findings.revision);
        if revision.revision == record.current_revision {
            current_data = Some(data);
            current_row = Some(revision);
        }
    }
    check_chain(record, &revisions, &mut findings.chain);

    let Some(current) = current_row else {
        findings.current.error(
            "revision",
            Some(&record.record_id),
            format!("current revision {} is missing", record.current_revision),
        );
        return Ok(());
    };
    let Some(record_type) = RecordType::parse(&record.record_type) else {
        findings.current.error("domain", Some(&record.record_id), "record_type: Unknown record type");
        return Ok(());
    };
    let deleted = record.is_deleted == 1;

    // The typed row must exist and hold the current revision's exact text.
    let table = encode::table_for(record_type);
    let typed_row = read_typed_row(context.connection, table, &record.record_id)?;
    let typed_table_bucket = &mut findings.typed_table[type_index(record_type)];
    match &typed_row {
        None => typed_table_bucket.error(
            "typed_table",
            Some(&record.record_id),
            format!("{} is missing its {table} row", record.record_type),
        ),
        Some(row) if row.data_json.as_deref() != Some(current.record_json.as_str()) => typed_table_bucket.error(
            "typed_table",
            Some(&record.record_id),
            format!("{table}.data_json does not match the current revision"),
        ),
        Some(_) => {}
    }

    // The record as a whole.
    let data_value = match current_data.flatten() {
        Some(value) => value,
        None => match json::parse(&current.record_json) {
            Ok(value) => value,
            Err(error) => {
                findings.current.error("domain", Some(&record.record_id), format!("record_json: {error}"));
                if !deleted && typed_row.is_some() {
                    findings.typed_denormalized[type_index(record_type)].error(
                        "typed_denormalized",
                        Some(&record.record_id),
                        "record_json is not valid JSON",
                    );
                }
                return Ok(());
            }
        },
    };
    let extensions = match parse_extensions(current.extensions_json.as_deref()) {
        Ok(extensions) => extensions,
        Err(message) => {
            findings.current.error("domain", Some(&record.record_id), message);
            None
        }
    };
    let had_extensions = extensions.is_some();
    let mut payload = Map::new();
    payload.insert("record_id".into(), Value::String(record.record_id.clone()));
    payload.insert("record_type".into(), Value::String(record.record_type.clone()));
    payload.insert("current_revision".into(), Value::from(record.current_revision));
    payload.insert("created_at".into(), Value::String(record.created_at.clone()));
    payload.insert("is_deleted".into(), Value::Bool(deleted));
    payload.insert("data".into(), data_value);
    if let Some(extensions) = extensions {
        payload.insert("extensions".into(), extensions);
    }
    let mut payload = Value::Object(payload);
    let decoded = CurrentRecord::from_value(&payload);
    let typed_data: std::result::Result<RecordData, String> = match &decoded {
        Ok(current) => Ok(current.data.clone()),
        Err(error) if had_extensions => {
            // The typed comparison reads the data alone, without extensions.
            if let Value::Object(map) = &mut payload {
                map.remove("extensions");
            }
            CurrentRecord::from_value(&payload).map(|current| current.data).map_err(|_| error.to_string())
        }
        Err(error) => Err(error.to_string()),
    };

    match &decoded {
        Err(error) => findings.current.error("domain", Some(&record.record_id), error.to_string()),
        Ok(current) => {
            if let RecordData::Attachment(attachment) = &current.data {
                if !deleted {
                    check_attachment(context, &record.record_id, attachment, &mut findings.current);
                }
            }
            if let RecordData::WorkoutSession(workout) = &current.data {
                check_segments(&record.record_id, workout, &mut findings.current);
            }
            if !deleted && matches!(current.data, RecordData::PredictionResultLink(_) | RecordData::PredictionGrade(_))
            {
                if let Err(error) = validate_relationships(context.connection, &current.data) {
                    findings.relationship.error("record_relationship", Some(&record.record_id), error.to_string());
                }
            }
        }
    }

    // Typed columns and child rows of active records.
    if let (false, Some(row)) = (deleted, &typed_row) {
        let bucket = &mut findings.typed_denormalized[type_index(record_type)];
        match typed_data {
            Err(message) => bucket.error("typed_denormalized", Some(&record.record_id), message),
            Ok(data) => match typed_mismatch(context, &record.record_id, table, row, &data)? {
                TypedOutcome::Same => {}
                TypedOutcome::Different => bucket.error(
                    "typed_denormalized",
                    Some(&record.record_id),
                    "typed-table denormalized columns do not match the current revision",
                ),
                TypedOutcome::Unreadable => {
                    bucket.error("typed_denormalized", Some(&record.record_id), "record_json is not valid JSON")
                }
            },
        }
    }
    Ok(())
}

fn check_attachment(context: &RecordContext<'_>, record_id: &str, data: &AttachmentData, bucket: &mut Bucket) {
    let path = context.attachments.join(attachment_files::stored_attachment_name(record_id, data));
    let facts = files::refuse_symbolic_link_components(&path, false)
        .map_err(|error| error.to_string())
        .and_then(|()| attachment_files::read_file_facts(&path).map_err(|error| error.to_string()));
    let facts = match facts {
        Ok(facts) => facts,
        Err(message) => {
            bucket.error(
                "attachment",
                Some(record_id),
                format!("referenced attachment is missing: {} ({message})", path.display()),
            );
            return;
        }
    };
    if facts.byte_count != data.byte_count {
        bucket.error(
            "attachment",
            Some(record_id),
            format!("attachment byte count is {}, expected {}", facts.byte_count, data.byte_count),
        );
    }
    if facts.sha256 != data.sha256 {
        bucket.error(
            "attachment",
            Some(record_id),
            format!("attachment SHA-256 is {}, expected {}", facts.sha256, data.sha256),
        );
    }
    if !attachment_files::bytes_match_media_type(&data.media_type, &facts.leading) {
        bucket.error(
            "attachment",
            Some(record_id),
            format!("attachment bytes do not match supported media type {}: {record_id}", data.media_type),
        );
    }
}

fn check_segments(record_id: &str, workout: &WorkoutSessionData, bucket: &mut Bucket) {
    let duration = match (js_date_parse(&workout.end_at), js_date_parse(&workout.start_at)) {
        (Some(end), Some(start)) => Some((end - start) as f64),
        _ => None,
    };
    for (index, segment) in workout.segments.as_deref().unwrap_or(&[]).iter().enumerate() {
        if let (Some(start), Some(end)) = (segment.start_offset_ms, segment.end_offset_ms) {
            if end < start {
                bucket.error("session_order", Some(record_id), format!("segment {} ends before it starts", index + 1));
            }
        }
        // A duration that cannot be computed compares false, as NaN did.
        let outside = duration.is_some_and(|duration| {
            segment.start_offset_ms.unwrap_or(0) as f64 > duration
                || segment.end_offset_ms.unwrap_or(0) as f64 > duration
        });
        if outside {
            bucket.error(
                "session_order",
                Some(record_id),
                format!("segment {} falls outside the workout session", index + 1),
            );
        }
    }
}

// ---- Typed rows ---------------------------------------------------------

struct TypedRow {
    data_json: Option<String>,
    columns: Vec<(String, Sql)>,
}

impl TypedRow {
    fn column(&self, name: &str) -> &Sql {
        self.columns.iter().find(|(column, _)| column == name).map_or(&Sql::Null, |(_, value)| value)
    }
}

fn read_typed_row(connection: &Connection, table: &str, record_id: &str) -> Result<Option<TypedRow>> {
    let mut statement = connection.prepare_cached(&typed_row_query(table))?;
    let names: Vec<String> = statement.column_names().map(str::to_string).collect();
    Ok(statement
        .query_row([record_id], |row| {
            let mut columns = Vec::with_capacity(names.len());
            for (index, name) in names.iter().enumerate() {
                columns.push((name.clone(), row.get::<_, Sql>(index)?));
            }
            Ok(columns)
        })
        .optional()?
        .map(|columns| {
            let data_json = columns.iter().find(|(name, _)| name == "data_json").and_then(|(_, value)| match value {
                Sql::Text(text) => Some(text.clone()),
                _ => None,
            });
            TypedRow { data_json, columns }
        }))
}

fn typed_row_query(table: &str) -> String {
    format!("SELECT * FROM {table} WHERE record_id = ?1")
}

fn child_query(child: &encode::ChildRows) -> String {
    let order: Vec<&str> = child.columns.iter().copied().filter(|column| column.ends_with("_order")).collect();
    format!(
        "SELECT {} FROM {} WHERE {} = ?1 ORDER BY {}",
        child.columns.join(", "),
        child.table,
        child.owner_column,
        order.join(", ")
    )
}

/// The statements the check runs: the two ordered passes, then the lookups
/// it makes for one record with this data. Each lookup reads by key.
pub fn check_statements(data: &RecordData) -> Vec<String> {
    let mut statements = vec![RECORDS_IN_ORDER.to_string(), REVISIONS_IN_ORDER.to_string()];
    statements.push(typed_row_query(encode::table_for(data.record_type())));
    let authored = data.authorship().is_some();
    for child in encode::child_rows(data) {
        if child.table == "authored_record_evidence" && !authored {
            continue;
        }
        statements.push(child_query(&child));
    }
    statements
}

enum TypedOutcome {
    Same,
    Different,
    /// A child JSON text that this reader cannot parse.
    Unreadable,
}

fn typed_mismatch(
    context: &RecordContext<'_>,
    record_id: &str,
    table: &str,
    row: &TypedRow,
    data: &RecordData,
) -> Result<TypedOutcome> {
    for (column, expected) in encode::typed_columns(data) {
        if !context.layout.has_column(table, column) {
            continue;
        }
        if !sql_values_equal(row.column(column), &expected) {
            return Ok(TypedOutcome::Different);
        }
    }
    let authored = data.authorship().is_some();
    for child in encode::child_rows(data) {
        // 2.1.0 compared evidence rows only for authored records.
        if child.table == "authored_record_evidence" && !authored {
            continue;
        }
        if !context.layout.has_table(child.table) {
            if child.rows.is_empty() {
                continue;
            }
            return Ok(TypedOutcome::Different);
        }
        let mut statement = context.connection.prepare_cached(&child_query(&child))?;
        let mut rows = statement.query([record_id])?;
        let mut expected_rows = child.rows.iter();
        while let Some(stored) = rows.next()? {
            let Some(expected) = expected_rows.next() else { return Ok(TypedOutcome::Different) };
            for (index, column) in child.columns.iter().enumerate() {
                let value: Sql = stored.get(index)?;
                if column.ends_with("_json") {
                    let (Sql::Text(stored_text), Sql::Text(expected_text)) = (&value, &expected[index]) else {
                        return Ok(TypedOutcome::Different);
                    };
                    match json::parse(stored_text) {
                        Ok(parsed) if json::to_string(&parsed) == *expected_text => {}
                        Ok(_) => return Ok(TypedOutcome::Different),
                        Err(_) => return Ok(TypedOutcome::Unreadable),
                    }
                } else if !sql_values_equal(&value, &expected[index]) {
                    return Ok(TypedOutcome::Different);
                }
            }
        }
        if expected_rows.next().is_some() {
            return Ok(TypedOutcome::Different);
        }
    }
    Ok(TypedOutcome::Same)
}

/// A typed-row finding for one active record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedDenormalizedFinding {
    /// Typed columns or child rows disagree with the current revision.
    Mismatch { record_id: String },
    /// The current revision's data does not decode.
    ParseError { record_id: String, message: String },
}

/// Typed-row findings for every active record, by record type and then
/// record ID. `ath repair` uses the mismatches to plan an encoding repair.
pub(crate) fn typed_denormalized_findings(
    connection: &Connection,
    layout: Layout,
) -> Result<Vec<TypedDenormalizedFinding>> {
    let context = RecordContext { connection, layout, attachments: std::path::PathBuf::new() };
    let mut findings = Vec::new();
    for record_type in RecordType::ALL {
        let table = encode::table_for(*record_type);
        let mut statement = connection.prepare(&format!(
            "SELECT records.record_id, records.current_revision, records.created_at, record_revisions.record_json
             FROM records
             JOIN record_revisions
               ON record_revisions.record_id = records.record_id
              AND record_revisions.revision = records.current_revision
             JOIN {table} ON {table}.record_id = records.record_id
             WHERE records.record_type = ?1 AND records.is_deleted = 0
             ORDER BY records.record_id"
        ))?;
        let mut rows = statement.query([record_type.as_str()])?;
        while let Some(row) = rows.next()? {
            let record_id: String = row.get(0)?;
            let current_revision: i64 = row.get(1)?;
            let created_at: String = row.get(2)?;
            let record_json: String = row.get(3)?;
            let data = match json::parse(&record_json) {
                Ok(data) => data,
                Err(_) => {
                    findings.push(TypedDenormalizedFinding::ParseError {
                        record_id,
                        message: "record_json is not valid JSON".to_string(),
                    });
                    continue;
                }
            };
            let mut payload = Map::new();
            payload.insert("record_id".into(), Value::String(record_id.clone()));
            payload.insert("record_type".into(), Value::String(record_type.as_str().to_string()));
            payload.insert("created_at".into(), Value::String(created_at));
            payload.insert("current_revision".into(), Value::from(current_revision));
            payload.insert("is_deleted".into(), Value::Bool(false));
            payload.insert("data".into(), data);
            let record = match CurrentRecord::from_value(&Value::Object(payload)) {
                Ok(record) => record,
                Err(error) => {
                    findings.push(TypedDenormalizedFinding::ParseError { record_id, message: error.to_string() });
                    continue;
                }
            };
            let Some(typed_row) = read_typed_row(connection, table, &record_id)? else { continue };
            match typed_mismatch(&context, &record_id, table, &typed_row, &record.data)? {
                TypedOutcome::Same => {}
                TypedOutcome::Different => findings.push(TypedDenormalizedFinding::Mismatch { record_id }),
                TypedOutcome::Unreadable => findings.push(TypedDenormalizedFinding::ParseError {
                    record_id,
                    message: "record_json is not valid JSON".to_string(),
                }),
            }
        }
    }
    Ok(findings)
}

// ---- Deletions and erasure markers --------------------------------------

fn check_deletions(connection: &Connection, bucket: &mut Bucket) -> Result<()> {
    let mut statement = connection.prepare(
        "SELECT record_id, record_revision, deleted_at, deleted_by_json FROM record_deletions ORDER BY record_id",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let record_id = sql_text(&row.get::<_, Sql>(0)?);
        let deleted_by = match json::parse(&row.get::<_, String>(3)?) {
            Ok(value) => value,
            Err(error) => {
                bucket.error("deletion", Some(&record_id), format!("deleted_by_json: {error}"));
                continue;
            }
        };
        let mut payload = Map::new();
        payload.insert("record_id".into(), sql_json(row.get::<_, Sql>(0)?));
        payload.insert("record_revision".into(), sql_json(row.get::<_, Sql>(1)?));
        payload.insert("deleted_at".into(), sql_json(row.get::<_, Sql>(2)?));
        payload.insert("deleted_by".into(), deleted_by);
        if let Err(error) = decode::<RecordDeletion>(&Value::Object(payload)) {
            bucket.error("deletion", Some(&record_id), error.to_string());
        }
    }
    drop(rows);
    let mut statement = connection.prepare(
        "SELECT records.record_id
         FROM records
         LEFT JOIN record_deletions ON record_deletions.record_id = records.record_id
         WHERE records.is_deleted <> CASE WHEN record_deletions.record_id IS NULL THEN 0 ELSE 1 END
         ORDER BY records.record_id",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let record_id: String = row.get(0)?;
        bucket.error("deletion", Some(&record_id), "records.is_deleted does not match record_deletions");
    }
    Ok(())
}

fn check_erasures(connection: &Connection, bucket: &mut Bucket) -> Result<()> {
    let mut statement = connection
        .prepare("SELECT record_id, erased_at, reason_code FROM record_erasure_markers ORDER BY record_id")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let record_id = sql_text(&row.get::<_, Sql>(0)?);
        let mut payload = Map::new();
        payload.insert("record_id".into(), sql_json(row.get::<_, Sql>(0)?));
        payload.insert("erased_at".into(), sql_json(row.get::<_, Sql>(1)?));
        payload.insert("reason_code".into(), sql_json(row.get::<_, Sql>(2)?));
        if let Err(error) = decode::<RecordErasureMarker>(&Value::Object(payload)) {
            bucket.error("erasure_marker", Some(&record_id), error.to_string());
        }
    }
    drop(rows);
    let mut statement = connection.prepare(
        "SELECT record_erasure_markers.record_id
         FROM record_erasure_markers
         JOIN records ON records.record_id = record_erasure_markers.record_id
         ORDER BY record_erasure_markers.record_id",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let record_id: String = row.get(0)?;
        bucket.error("erasure_marker", Some(&record_id), "erased record still has a current record row");
    }
    Ok(())
}

fn sql_json(value: Sql) -> Value {
    match value {
        Sql::Null => Value::Null,
        Sql::Integer(number) => Value::from(number),
        Sql::Real(number) => serde_json::Number::from_f64(number).map_or(Value::Null, Value::Number),
        Sql::Text(text) => Value::String(text),
        Sql::Blob(_) => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_values_compare_like_2_1_0() {
        assert!(sql_values_equal(&Sql::Null, &Sql::Null));
        assert!(!sql_values_equal(&Sql::Null, &Sql::Text(String::new())));
        assert!(sql_values_equal(&Sql::Real(80.0), &Sql::Integer(80)));
        assert!(sql_values_equal(&Sql::Integer(1), &Sql::Integer(1)));
        assert!(sql_values_equal(&Sql::Text("5".into()), &Sql::Integer(5)));
        assert!(!sql_values_equal(&Sql::Text("a".into()), &Sql::Text("b".into())));
    }
}
