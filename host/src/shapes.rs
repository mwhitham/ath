//! The request and response bodies of host API version 1 (D266).
//!
//! Each body is one Rust type with its published shape, declared with the library's
//! `model_object!`. The handlers read and write these types, and the API description
//! in `schema/host-api/1/` is generated from the same shapes, so the two cannot
//! drift apart. Request bodies are checked against their shape before they are read.

use athleticstandard::model::schema::{
    any_string, array_of, boolean, json_value, leak, nonnegative_int, positive_int, string, text, Check, Kind, Schema,
    SchemaRef, Shape,
};
use athleticstandard::model::{
    current_record_schema, record_id, record_revision_schema, utc_timestamp, ErasureReasonCode, RecordType,
};
use athleticstandard::{model_enum, model_object};
use serde_json::Value;

use crate::directory::{valid_resource_id, AppConnection, Grant};

model_enum! {
    /// The three grants (D262).
    pub enum Permission {
        ReadWorkoutSummaries = "read_workout_summaries",
        AddCoachNotes = "add_coach_notes",
        AddProgramming = "add_programming",
    }
}

fn nullable(inner: SchemaRef) -> SchemaRef {
    leak(Schema::new(Kind::Nullable(inner)))
}

fn one_of(values: &'static [&'static str]) -> SchemaRef {
    leak(Schema::new(Kind::Enum(values)))
}

fn described(schema: Schema, description: &'static str) -> SchemaRef {
    leak(schema.describe(description))
}

/// An athlete ID this host issued.
pub fn athlete_id() -> SchemaRef {
    static SCHEMA: std::sync::LazyLock<Schema> = std::sync::LazyLock::new(|| {
        string()
            .check(Check::Pattern {
                source: "^ath_[0-9a-f]{32}$",
                test: valid_resource_id,
                message: "an athlete ID this host issued",
            })
            .describe("An athlete ID: `ath_` and 32 lowercase hexadecimal digits")
    });
    &SCHEMA
}

/// Any JSON object. Record data is checked by the library after the request is read.
pub fn json_object() -> SchemaRef {
    static SCHEMA: std::sync::LazyLock<Schema> =
        std::sync::LazyLock::new(|| Schema::new(Kind::Record { key: any_string(), value: json_value() }));
    &SCHEMA
}

fn record_data(description: &'static str) -> SchemaRef {
    leak(Schema::new(Kind::Record { key: any_string(), value: json_value() }).describe(description))
}

fn revision() -> SchemaRef {
    described(
        Schema::new(Kind::Number { integer: true }).check(Check::Minimum { value: 1.0, inclusive: true }),
        "A record revision, from 1",
    )
}

fn count() -> SchemaRef {
    nonnegative_int()
}

fn sha256() -> SchemaRef {
    described(string(), "A SHA-256 digest as 64 lowercase hexadecimal digits")
}

const STATES: &[&str] = &["active", "frozen", "staged"];
const CHANGE_KINDS: &[&str] = &["revision", "deletion", "erasure"];
const STORAGE: &[&str] = &["directory", "libsql"];
const ROLES: &[&str] = &["owner", "coach"];
const APP_STATES: &[&str] = &["connected", "disconnected"];
/// Every stable error code of version 1.
pub const ERROR_CODES: &[&str] = &[
    "sign_in_required",
    "permission_denied",
    "app_disconnected",
    "not_found",
    "stale",
    "request_conflict",
    "host_moving",
    "limit_reached",
    "invalid",
    "refused",
    "internal",
];

// ---- Requests -------------------------------------------------------------------------

model_object! {
    /// `POST /v1/athletes`. An empty body is allowed.
    pub struct NewAthleteRequest {
        display_name?: String,
        /// An unused athlete invitation, needed when the host has no open enrollment.
        invitation?: String,
        /// A record arriving by a move. It is created staged (D267).
        incoming?: bool,
    }
}

model_object! {
    pub struct CreateRecordRequest {
        record_type: RecordType,
        data: Value = record_data("The record's data, in the shape the package schema gives for `record_type`"),
        record_id?: String = record_id(),
        attachments?: Vec<String> = array_of(described(string(), "An `upload_id` from the uploads route")),
    }
}

model_object! {
    pub struct CorrectionRequest {
        expected_revision: u64 = revision(),
        data: Value = record_data("The complete new data, in the shape the package schema gives for the record's type"),
    }
}

model_object! {
    pub struct DeletionRequest {
        expected_revision: u64 = revision(),
    }
}

model_object! {
    pub struct ErasureRequest {
        expected_revision: u64 = revision(),
        reason_code: ErasureReasonCode,
    }
}

model_object! {
    pub struct GrantOfferRequest {
        permissions: Vec<Permission>,
        client_id: String = described(string().check(Check::MinLength(1)), "The app the coach must use to accept the offer"),
        coach_label?: String,
        valid_seconds?: i64 = described(
            Schema::new(Kind::Number { integer: true }),
            "How long the code works, from 60 seconds to 30 days. The default is 7 days",
        ),
    }
}

model_object! {
    pub struct AcceptOfferRequest {
        code: String,
    }
}

model_object! {
    pub struct Evidence {
        record_id: String = record_id(),
        revision: u64 = revision(),
    }
}

model_object! {
    pub struct NoteRequest {
        note: String,
        title?: String,
        applies_from?: String,
        applies_to?: String,
        evidence?: Vec<Evidence>,
    }
}

model_object! {
    pub struct ProgramRequest {
        plan: Value = record_data("Training plan data, without `authorship`"),
        sessions?: Vec<Value> = array_of(record_data(
            "Planned session data, without `authorship`, `training_plan_id`, `completion_status`, or `completed_workout_session_id`",
        )),
    }
}

model_object! {
    pub struct FreezeRequest {
        destination: String = described(string().check(Check::MinLength(1)), "The destination host's `public_url`"),
    }
}

model_object! {
    pub struct TransferRequest {
        transfer_id: String,
    }
}

model_object! {
    pub struct ActivateRequest {
        source: String = described(string().check(Check::MinLength(1)), "The source host's `public_url`"),
        transfer_id: String,
        activation: String,
    }
}

model_object! {
    pub struct VerifyRequest {
        activation: String,
        destination: String,
        manifest_digest: String = sha256(),
    }
}

// ---- Responses ------------------------------------------------------------------------

model_object! {
    pub struct ErrorDetail {
        code: String = one_of(ERROR_CODES),
        message: String = any_string(),
    }
}

model_object! {
    /// Every error answer.
    pub struct ErrorBody {
        error: ErrorDetail,
    }
}

model_object! {
    /// Protected-resource metadata (RFC 9728).
    pub struct ProtectedResource {
        resource: String,
        authorization_servers: Vec<String>,
        bearer_methods_supported: Vec<String>,
        resource_name: String,
    }
}

model_object! {
    pub struct Capabilities {
        api_version: u64 = positive_int(),
        athleticstandard_version: String,
        storage: String = one_of(STORAGE),
        permissions: Vec<Permission>,
        operator_disclosure: String = any_string(),
        open_enrollment: bool = boolean(),
    }
}

model_object! {
    pub struct Me {
        principal_id: String,
        client_id: String,
        /// The ID written into this person's coach notes (D255).
        coach_id: String,
        athletes: Vec<String> = array_of(athlete_id()),
    }
}

model_object! {
    pub struct CreatedAthlete {
        athlete_id: String = athlete_id(),
        database_revision: u64 = count(),
        operator_disclosure: String = any_string(),
    }
}

model_object! {
    pub struct AthleteEntry {
        athlete_id: String = athlete_id(),
        display_name: Option<String> = nullable(any_string()),
        state: String = one_of(STATES),
    }
}

model_object! {
    pub struct AthleteList {
        athletes: Vec<AthleteEntry>,
    }
}

model_object! {
    pub struct AthleteSummary {
        athlete_id: String = athlete_id(),
        role: String = one_of(ROLES),
        state: String = one_of(STATES),
        operator_disclosure: String = any_string(),
        /// For the owner only.
        database_revision?: u64 = count(),
    }
}

model_object! {
    pub struct DatabaseRevision {
        database_revision: u64 = count(),
    }
}

model_object! {
    pub struct LoggedChange {
        database_revision: u64 = count(),
        record_id: String = record_id(),
        record_revision: u64 = described(Schema::new(Kind::Number { integer: true }).check(Check::Minimum { value: 0.0, inclusive: true }), "0 for an erasure"),
        kind: String = one_of(CHANGE_KINDS),
    }
}

model_object! {
    pub struct ChangeList {
        from: u64 = count(),
        to: u64 = count(),
        /// False when the log does not reach back to `since`; download the export.
        complete: bool = boolean(),
        changes: Vec<LoggedChange>,
    }
}

model_object! {
    pub struct RecordList {
        records: Vec<Value> = array_of(current_record_schema()),
    }
}

model_object! {
    pub struct RecordWithHistory {
        record: Value = current_record_schema(),
        revisions: Vec<Value> = array_of(record_revision_schema()),
    }
}

model_object! {
    pub struct WrittenRecord {
        record_id: String = record_id(),
        revision: u64 = revision(),
    }
}

model_object! {
    /// The result of a record write.
    pub struct WriteResult {
        database_revision: u64 = count(),
        records: Vec<WrittenRecord>,
    }
}

model_object! {
    pub struct Upload {
        upload_id: String,
        byte_count: u64 = count(),
        sha256: String = sha256(),
        media_type: String,
    }
}

model_object! {
    pub struct GrantOffer {
        /// Give this code to the coach. It works once and is never shown again.
        code: String,
        expires_at: String = utc_timestamp(),
        permissions: Vec<Permission>,
        client_id: String,
    }
}

model_object! {
    pub struct PendingOffer {
        offer_id: String,
        permissions: Vec<Permission>,
        client_id: String,
        coach_label: Option<String> = nullable(any_string()),
        created_at: String = utc_timestamp(),
        expires_at: String = utc_timestamp(),
    }
}

model_object! {
    pub struct PendingOffers {
        offers: Vec<PendingOffer>,
    }
}

model_object! {
    pub struct CancelledOffer {
        cancelled: String,
    }
}

model_object! {
    pub struct GrantView {
        grant_id: String,
        resource_id: String = athlete_id(),
        coach_principal_id: String,
        client_id: String,
        permissions: Vec<Permission>,
        coach_label: Option<String> = nullable(any_string()),
        created_at: String = utc_timestamp(),
        revoked_at: Option<String> = nullable(utc_timestamp()),
    }
}

impl From<&Grant> for GrantView {
    fn from(grant: &Grant) -> GrantView {
        GrantView {
            grant_id: grant.grant_id.clone(),
            resource_id: grant.resource_id.clone(),
            coach_principal_id: grant.coach_principal_id.clone(),
            client_id: grant.client_id.clone(),
            permissions: grant.permissions.clone(),
            coach_label: grant.coach_label.clone(),
            created_at: grant.created_at.clone(),
            revoked_at: grant.revoked_at.clone(),
        }
    }
}

model_object! {
    pub struct GrantList {
        grants: Vec<GrantView>,
    }
}

model_object! {
    pub struct AcceptedGrant {
        grant: GrantView,
    }
}

model_object! {
    pub struct RevokedGrant {
        revoked: String,
        access_epoch: u64 = count(),
    }
}

model_object! {
    /// An app the owner has used with one record (D270).
    pub struct AppView {
        client_id: String,
        state: String = one_of(APP_STATES),
        connected_at: String = utc_timestamp(),
        /// Updated at most once an hour.
        last_used_at: String = utc_timestamp(),
        disconnected_at: Option<String> = nullable(utc_timestamp()),
        /// True for the app that made the request.
        this_app: bool = boolean(),
    }
}

impl AppView {
    pub fn new(app: &AppConnection, caller_client_id: &str) -> AppView {
        AppView {
            client_id: app.client_id.clone(),
            state: if app.disconnected_at.is_some() { "disconnected" } else { "connected" }.to_string(),
            connected_at: app.connected_at.clone(),
            last_used_at: app.last_used_at.clone(),
            disconnected_at: app.disconnected_at.clone(),
            this_app: app.client_id == caller_client_id,
        }
    }
}

model_object! {
    pub struct AppList {
        apps: Vec<AppView>,
    }
}

model_object! {
    pub struct DisconnectedApp {
        app: AppView,
        access_epoch: u64 = count(),
    }
}

model_object! {
    pub struct ConnectedApp {
        app: AppView,
    }
}

model_object! {
    pub struct RosterEntry {
        athlete_id: String = athlete_id(),
        display_name: Option<String> = nullable(any_string()),
        permissions: Vec<Permission>,
        grant_id: String,
    }
}

model_object! {
    pub struct Roster {
        athletes: Vec<RosterEntry>,
    }
}

model_object! {
    /// Exactly these fields of a workout, under the read grant (D262).
    pub struct WorkoutSummary {
        record_id: String = record_id(),
        revision: u64 = revision(),
        start_at: String = utc_timestamp(),
        end_at: String = utc_timestamp(),
        activity_type: Option<String> = nullable(any_string()),
    }
}

model_object! {
    pub struct WorkoutPage {
        workouts: Vec<WorkoutSummary>,
        /// The value to send as `after` for the next page, or null on the last page.
        next: Option<String> = nullable(text()),
    }
}

model_object! {
    pub struct FrozenRecord {
        transfer_id: String,
        manifest_digest: Option<String> = nullable(sha256()),
    }
}

model_object! {
    pub struct RetiredRecord {
        transfer_id: String,
        /// A single-use code bound to this transfer, the destination, and the digest.
        activation: String,
        manifest_digest: Option<String> = nullable(sha256()),
    }
}

model_object! {
    pub struct ReopenedRecord {
        state: String = one_of(&["active"]),
    }
}

model_object! {
    pub struct ActivatedRecord {
        state: String = one_of(&["active"]),
        manifest_digest: String = sha256(),
    }
}

model_object! {
    pub struct VerifiedTransfer {
        verified: bool = boolean(),
    }
}

/// Every named shape in the API description, in the order it lists them. Each
/// request and response body is one of these; the description refuses a body
/// without a name.
pub fn named() -> Vec<(&'static str, SchemaRef)> {
    vec![
        ("NewAthleteRequest", NewAthleteRequest::schema()),
        ("CreateRecordRequest", CreateRecordRequest::schema()),
        ("CorrectionRequest", CorrectionRequest::schema()),
        ("DeletionRequest", DeletionRequest::schema()),
        ("ErasureRequest", ErasureRequest::schema()),
        ("GrantOfferRequest", GrantOfferRequest::schema()),
        ("AcceptOfferRequest", AcceptOfferRequest::schema()),
        ("Evidence", Evidence::schema()),
        ("NoteRequest", NoteRequest::schema()),
        ("ProgramRequest", ProgramRequest::schema()),
        ("FreezeRequest", FreezeRequest::schema()),
        ("TransferRequest", TransferRequest::schema()),
        ("ActivateRequest", ActivateRequest::schema()),
        ("VerifyRequest", VerifyRequest::schema()),
        ("Error", ErrorBody::schema()),
        ("ErrorDetail", ErrorDetail::schema()),
        ("ProtectedResource", ProtectedResource::schema()),
        ("Capabilities", Capabilities::schema()),
        ("Me", Me::schema()),
        ("CreatedAthlete", CreatedAthlete::schema()),
        ("AthleteEntry", AthleteEntry::schema()),
        ("AthleteList", AthleteList::schema()),
        ("AthleteSummary", AthleteSummary::schema()),
        ("DatabaseRevision", DatabaseRevision::schema()),
        ("LoggedChange", LoggedChange::schema()),
        ("ChangeList", ChangeList::schema()),
        ("RecordList", RecordList::schema()),
        ("RecordWithHistory", RecordWithHistory::schema()),
        ("WrittenRecord", WrittenRecord::schema()),
        ("WriteResult", WriteResult::schema()),
        ("Upload", Upload::schema()),
        ("GrantOffer", GrantOffer::schema()),
        ("PendingOffer", PendingOffer::schema()),
        ("PendingOffers", PendingOffers::schema()),
        ("CancelledOffer", CancelledOffer::schema()),
        ("Grant", GrantView::schema()),
        ("GrantList", GrantList::schema()),
        ("AcceptedGrant", AcceptedGrant::schema()),
        ("RevokedGrant", RevokedGrant::schema()),
        ("App", AppView::schema()),
        ("AppList", AppList::schema()),
        ("DisconnectedApp", DisconnectedApp::schema()),
        ("ConnectedApp", ConnectedApp::schema()),
        ("RosterEntry", RosterEntry::schema()),
        ("Roster", Roster::schema()),
        ("WorkoutSummary", WorkoutSummary::schema()),
        ("WorkoutPage", WorkoutPage::schema()),
        ("FrozenRecord", FrozenRecord::schema()),
        ("RetiredRecord", RetiredRecord::schema()),
        ("ReopenedRecord", ReopenedRecord::schema()),
        ("ActivatedRecord", ActivatedRecord::schema()),
        ("VerifiedTransfer", VerifiedTransfer::schema()),
        ("Permission", Permission::schema()),
        ("RecordType", RecordType::schema()),
        ("ErasureReasonCode", ErasureReasonCode::schema()),
    ]
}

/// The record shapes, which the description points to in the published package
/// schemas instead of copying them. Measurement samples are in the series schema.
pub fn external() -> Vec<(&'static str, SchemaRef, Value)> {
    let version = athleticstandard::model::package::ATHLETIC_STANDARD_VERSION;
    let package = format!("../../{version}/athleticstandard.schema.json");
    let series = format!("../../{version}/measurement-series.schema.json");
    vec![
        (
            "CurrentRecord",
            current_record_schema(),
            serde_json::json!({
                "description": "A current record, as in `records` of the package schema, or a measurement sample, as in `records` of the measurement-series schema.",
                "anyOf": [
                    { "$ref": format!("{package}#/properties/records/items") },
                    { "$ref": format!("{series}#/properties/records/items") }
                ]
            }),
        ),
        (
            "RecordRevision",
            record_revision_schema(),
            serde_json::json!({
                "description": "One revision of a record, as in `record_revisions` of the package schema or of the measurement-series schema.",
                "anyOf": [
                    { "$ref": format!("{package}#/properties/record_revisions/items") },
                    { "$ref": format!("{series}#/properties/record_revisions/items") }
                ]
            }),
        ),
    ]
}

/// Read a JSON request body: check it against its shape, then decode it.
pub fn read<T: Shape + serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, String> {
    let value: Value =
        serde_json::from_slice(body).map_err(|error| format!("The request body is not JSON: {error}"))?;
    athleticstandard::model::schema::decode::<T>(&value)
        .map_err(|error| format!("The request body is not valid: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_is_checked_against_its_shape() {
        assert!(read::<NoteRequest>(br#"{"note": "Well done."}"#).is_ok());
        assert!(read::<NoteRequest>(br#"{"note": ""}"#).is_err(), "an empty note");
        assert!(read::<NoteRequest>(br#"{"note": "x", "extra": 1}"#).is_err(), "an unknown field");
        assert!(read::<DeletionRequest>(br#"{"expected_revision": 0}"#).is_err(), "revisions start at 1");
        let offer: GrantOfferRequest =
            read(br#"{"permissions": ["add_coach_notes"], "client_id": "coach-app"}"#).unwrap();
        assert_eq!(offer.permissions, vec![Permission::AddCoachNotes]);
    }
}
