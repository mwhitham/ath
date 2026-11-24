//! Version 1 of the host API (spec section 6.5, D266).
//!
//! Every request carries an OAuth access token. The host checks the token, then the
//! caller's relation to the athlete: owner, coach with a grant through this app, or
//! nothing. A caller with no relation gets `not_found`, the same as for a record that
//! does not exist. Every write carries an `Idempotency-Key`; a repeated request with
//! the same body returns the original result (D261).

use std::path::PathBuf;
use std::sync::Arc;

use athleticstandard::json;
use athleticstandard::model::{BecameKnownAtBasis, ErasureReasonCode, RecordActor, RecordData, RecordType};
use athleticstandard::package::{
    apply_with_attachments, erase_record, export_package, preview_erasure, ExportOptions, PreparedAttachment,
};
use athleticstandard::store::hosting::WriteReceipt;
use athleticstandard::store::read::ListOptions;
use athleticstandard::store::write::StaleStyle;
use athleticstandard::store::{Changes, Correction, Deletion, ExtensionsUpdate, NewRecord, Store};
use axum::body::Bytes;
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json as j, Value};
use sha2::{Digest, Sha256};

use crate::auth::Verifier;
use crate::config::Config;
use crate::directory::{hex, now, Athlete, Directory, Grant, Permission, Principal};
use crate::error::{ApiError, ApiResult};
use crate::storage::Storage;

pub struct AppState {
    pub config: Config,
    pub directory: Directory,
    pub storage: Storage,
    pub verifier: Verifier,
    /// The key that makes receipt digests (D261). It never leaves the host.
    pub receipt_key: String,
}

pub type Shared = Arc<AppState>;

pub fn router(state: Shared) -> Router {
    let body_limit = state.config.limits.body_bytes;
    Router::new()
        .route("/.well-known/oauth-protected-resource", get(protected_resource))
        .route("/v1/capabilities", get(capabilities))
        .route("/v1/me", get(me))
        .route("/v1/athletes", get(list_athletes).post(create_athlete))
        .route("/v1/athletes/{id}", get(athlete_summary))
        .route("/v1/athletes/{id}/package", put(put_package))
        .route("/v1/athletes/{id}/export", get(export))
        .route("/v1/athletes/{id}/changes", get(changes))
        .route("/v1/athletes/{id}/records", get(list_records).post(create_record))
        .route("/v1/athletes/{id}/records/{record_id}", get(get_record))
        .route("/v1/athletes/{id}/records/{record_id}/corrections", post(correct_record))
        .route("/v1/athletes/{id}/records/{record_id}/deletion", post(delete_record))
        .route("/v1/athletes/{id}/records/{record_id}/erasure", post(erase))
        .route("/v1/athletes/{id}/uploads", post(upload))
        .route("/v1/athletes/{id}/attachments/{record_id}", get(download_attachment))
        .route("/v1/athletes/{id}/grant-offers", post(create_grant_offer))
        .route("/v1/athletes/{id}/grants", get(list_grants))
        .route("/v1/athletes/{id}/grants/{grant_id}", delete(revoke_grant))
        .route("/v1/grant-offers/accept", post(accept_grant_offer))
        .route("/v1/roster", get(roster))
        .route("/v1/athletes/{id}/workouts", get(workout_summaries))
        .route("/v1/athletes/{id}/notes", post(add_note))
        .route("/v1/athletes/{id}/programs", post(add_program))
        .merge(crate::moves::routes())
        .layer(axum::extract::DefaultBodyLimit::max(body_limit))
        .with_state(state)
}

// ---- The caller -------------------------------------------------------------

/// A signed-in caller: the person and the app the token was issued to.
pub struct Caller {
    pub principal: Principal,
    pub client_id: String,
}

impl FromRequestParts<Shared> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &Shared) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or_else(|| ApiError::sign_in_required("Sign in and send the access token as a Bearer token"))?;
        let verified = state.verifier.verify(token.trim()).await?;
        let principal = state
            .directory
            .principal(&verified.issuer, &verified.subject, verified.name.as_deref())
            .await
            .map_err(|error| ApiError::internal("directory", error))?;
        Ok(Caller { principal, client_id: verified.client_id })
    }
}

/// How the caller relates to one athlete's record.
enum Relation {
    Owner(Athlete),
    Coach(Athlete, Grant),
}

async fn relation(state: &AppState, caller: &Caller, resource_id: &str) -> ApiResult<Relation> {
    let athlete = state
        .directory
        .athlete(resource_id)
        .await
        .map_err(|error| ApiError::internal("directory", error))?
        .filter(|athlete| athlete.state != "retired")
        .ok_or_else(ApiError::not_found)?;
    if athlete.owner_principal_id == caller.principal.principal_id {
        return Ok(Relation::Owner(athlete));
    }
    match state
        .directory
        .active_grant(resource_id, &caller.principal.principal_id, &caller.client_id)
        .await
        .map_err(|error| ApiError::internal("directory", error))?
    {
        Some(grant) => Ok(Relation::Coach(athlete, grant)),
        None => Err(ApiError::not_found()),
    }
}

async fn owner(state: &AppState, caller: &Caller, resource_id: &str) -> ApiResult<Athlete> {
    match relation(state, caller, resource_id).await? {
        Relation::Owner(athlete) => Ok(athlete),
        Relation::Coach(..) => Err(ApiError::permission_denied("Only the athlete can do this")),
    }
}

/// The coach's grant for `permission`, with the access epoch read before the grant
/// check (D262): a write committed after a revocation then fails.
async fn coach_grant(
    state: &AppState,
    caller: &Caller,
    resource_id: &str,
    permission: Permission,
) -> ApiResult<(Grant, u64)> {
    let epoch = state.storage.access_epoch(resource_id).await.map_err(hide_missing)?;
    match relation(state, caller, resource_id).await? {
        Relation::Coach(_, grant) if grant.allows(permission) => Ok((grant, epoch)),
        Relation::Coach(..) => Err(ApiError::permission_denied("Your access to this athlete does not include this")),
        Relation::Owner(_) => Err(ApiError::permission_denied("Use the owner operations for your own record")),
    }
}

fn hide_missing(error: ApiError) -> ApiError {
    if error.code == "not_found" {
        ApiError::not_found()
    } else {
        error
    }
}

fn not_moving(athlete: &Athlete) -> ApiResult<()> {
    if athlete.state == "frozen" {
        return Err(ApiError::moving());
    }
    Ok(())
}

// ---- Receipts ----------------------------------------------------------------

fn idempotency_key(headers: &HeaderMap) -> ApiResult<String> {
    let key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|key| !key.is_empty() && key.len() <= 200)
        .ok_or_else(|| ApiError::invalid("Send an Idempotency-Key header of 1 to 200 characters with every write"))?;
    Ok(key.to_string())
}

fn receipt(state: &AppState, caller: &Caller, operation: &str, request_id: String, body: &[u8]) -> WriteReceipt {
    let mut digest = Sha256::new();
    digest.update(state.receipt_key.as_bytes());
    digest.update([0]);
    digest.update(operation.as_bytes());
    digest.update([0]);
    digest.update(body);
    WriteReceipt {
        principal_id: caller.principal.principal_id.clone(),
        client_id: caller.client_id.clone(),
        request_id,
        operation: operation.to_string(),
        request_digest: hex(&digest.finalize()),
    }
}

enum Outcome {
    Done(Value),
    Conflict,
}

/// Run one write with a receipt. `build` makes the change; it runs again if the server
/// moved on. A repeated request returns the stored result.
async fn write_with_receipt(
    state: &AppState,
    resource_id: &str,
    guard_epoch: Option<u64>,
    receipt: WriteReceipt,
    attachments: Vec<PathBuf>,
    build: impl Fn(&Store) -> athleticstandard::Result<Changes> + Send + Sync + 'static,
) -> ApiResult<Value> {
    let outcome = state
        .storage
        .write(resource_id, guard_epoch, move |store, epoch| {
            if let Some(found) = store.find_receipt(&receipt.principal_id, &receipt.client_id, &receipt.request_id)? {
                if found.request_digest != receipt.request_digest || found.operation != receipt.operation {
                    return Ok(Outcome::Conflict);
                }
                return Ok(Outcome::Done(found.result));
            }
            let mut changes = build(store)?;
            changes.receipt = Some(receipt.clone());
            changes.expected_access_epoch = epoch;
            let applied = if attachments.is_empty() {
                store.apply(changes)?
            } else {
                let actor = changes.creates.first().map(|create| create.changed_by.clone()).unwrap_or(RecordActor::Athlete);
                let prepared = athleticstandard::package::prepare_attachments(store, &attachments, &actor, &now())?;
                let mut changes = changes;
                let ids: Vec<String> = prepared.iter().map(|attachment| attachment.record_id.clone()).collect();
                link_attachments(&mut changes, &ids)?;
                changes.creates.splice(0..0, prepared.iter().map(PreparedAttachment::new_record));
                apply_with_attachments(store, &prepared, changes)?
            };
            Ok(Outcome::Done(j!({
                "database_revision": applied.database_revision,
                "records": applied.records.iter().map(|record| j!({ "record_id": record.record_id, "revision": record.current_revision })).collect::<Vec<_>>(),
            })))
        })
        .await?;
    match outcome {
        Outcome::Done(value) => Ok(value),
        Outcome::Conflict => Err(ApiError::request_conflict()),
    }
}

fn link_attachments(changes: &mut Changes, ids: &[String]) -> athleticstandard::Result<()> {
    let create = changes
        .creates
        .first_mut()
        .ok_or_else(|| athleticstandard::Error::refused("attachments need a record to belong to"))?;
    match &mut create.data {
        RecordData::AthleteReport(data) => data.attachment_ids.get_or_insert_with(Vec::new).extend(ids.iter().cloned()),
        RecordData::NutritionEntry(data) => {
            data.attachment_ids.get_or_insert_with(Vec::new).extend(ids.iter().cloned())
        }
        _ => {
            return Err(athleticstandard::Error::refused(
                "attachments belong to an athlete report or a nutrition entry",
            ))
        }
    }
    Ok(())
}

// ---- Helpers --------------------------------------------------------------------

fn to_json(value: &json::Value) -> Value {
    serde_json::from_str(&json::to_string(value)).unwrap_or(Value::Null)
}

fn from_json(value: &Value) -> ApiResult<json::Value> {
    json::parse(&value.to_string())
        .map_err(|error| ApiError::invalid(format!("The request is not valid JSON: {error}")))
}

fn decode(record_type: RecordType, data: &Value) -> ApiResult<RecordData> {
    RecordData::decode(record_type, &from_json(data)?).map_err(|error| ApiError::invalid(error.to_string()))
}

fn body_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> ApiResult<T> {
    serde_json::from_slice(body).map_err(|error| ApiError::invalid(format!("The request body is not valid: {error}")))
}

fn coach_author(caller: &Caller, grant: &Grant) -> Value {
    let name = grant
        .coach_label
        .clone()
        .or_else(|| caller.principal.display_name.clone())
        .unwrap_or_else(|| "Coach".to_string());
    j!({ "author_type": "coach", "coach": name, "coach_id": caller.principal.coach_id })
}

fn coach_actor(caller: &Caller, grant: &Grant) -> ApiResult<RecordActor> {
    serde_json::from_value(coach_author(caller, grant)).map_err(|error| ApiError::internal("coach author", error))
}

fn staged_upload(state: &AppState, resource_id: &str, upload_id: &str) -> ApiResult<PathBuf> {
    if upload_id.len() != 32 || !upload_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::invalid("An upload ID is 32 hexadecimal characters"));
    }
    let path = state.storage.staging().join("uploads").join(resource_id).join(upload_id);
    if !path.is_file() {
        return Err(ApiError::invalid(format!("There is no upload {upload_id}")));
    }
    Ok(path)
}

// ---- Discovery -------------------------------------------------------------------

async fn protected_resource(State(state): State<Shared>) -> Json<Value> {
    Json(j!({
        "resource": state.config.public_url,
        "authorization_servers": [state.verifier.issuer()],
        "bearer_methods_supported": ["header"],
        "resource_name": "Athletic Standard host",
    }))
}

async fn capabilities(State(state): State<Shared>) -> Json<Value> {
    Json(j!({
        "api_version": 1,
        "athleticstandard_version": athleticstandard::model::package::ATHLETIC_STANDARD_VERSION,
        "storage": if state.storage.is_remote() { "libsql" } else { "directory" },
        "permissions": ["read_workout_summaries", "add_coach_notes", "add_programming"],
        "operator_disclosure": state.config.operator_disclosure,
        "open_enrollment": state.config.open_enrollment,
    }))
}

async fn me(State(state): State<Shared>, caller: Caller) -> ApiResult<Json<Value>> {
    let owned = state
        .directory
        .athletes_owned_by(&caller.principal.principal_id)
        .await
        .map_err(|e| ApiError::internal("directory", e))?;
    Ok(Json(j!({
        "principal_id": caller.principal.principal_id,
        "client_id": caller.client_id,
        "coach_id": caller.principal.coach_id,
        "athletes": owned.iter().map(|athlete| athlete.resource_id.clone()).collect::<Vec<_>>(),
    })))
}

// ---- Owner: records -----------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewAthlete {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    invitation: Option<String>,
}

async fn create_athlete(
    State(state): State<Shared>,
    caller: Caller,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let request: NewAthlete =
        if body.is_empty() { NewAthlete { display_name: None, invitation: None } } else { body_json(&body)? };
    let invited = match &request.invitation {
        Some(code) => state
            .directory
            .use_athlete_invitation(code, &caller.principal.principal_id)
            .await
            .map_err(|e| ApiError::internal("directory", e))?,
        None => false,
    };
    if !invited && !state.config.open_enrollment {
        return Err(ApiError::permission_denied("This host needs an invitation to create a record"));
    }
    let athlete = state
        .directory
        .create_athlete(&caller.principal.principal_id, request.display_name.as_deref())
        .await
        .map_err(|e| ApiError::internal("directory", e))?;
    let revision = state.storage.create(&athlete.resource_id).await?;
    Ok((
        StatusCode::CREATED,
        Json(
            j!({ "athlete_id": athlete.resource_id, "database_revision": revision, "operator_disclosure": state.config.operator_disclosure }),
        ),
    ))
}

async fn list_athletes(State(state): State<Shared>, caller: Caller) -> ApiResult<Json<Value>> {
    let owned = state
        .directory
        .athletes_owned_by(&caller.principal.principal_id)
        .await
        .map_err(|e| ApiError::internal("directory", e))?;
    Ok(Json(
        j!({ "athletes": owned.iter().map(|a| j!({ "athlete_id": a.resource_id, "display_name": a.display_name, "state": a.state })).collect::<Vec<_>>() }),
    ))
}

async fn athlete_summary(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let relation = relation(&state, &caller, &id).await?;
    let (athlete, role) = match &relation {
        Relation::Owner(athlete) => (athlete, "owner"),
        Relation::Coach(athlete, _) => (athlete, "coach"),
    };
    let mut summary = j!({ "athlete_id": athlete.resource_id, "role": role, "state": athlete.state, "operator_disclosure": state.config.operator_disclosure });
    if let Relation::Owner(_) = relation {
        let revision = state.storage.read(&id, |store| store.database_revision()).await?;
        summary["database_revision"] = j!(revision);
    }
    Ok(Json(summary))
}

/// Connect an existing record: restore a complete package (a zip of the package
/// folder) into an athlete's empty record.
async fn put_package(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let athlete = owner(&state, &caller, &id).await?;
    not_moving(&athlete)?;
    let folder = state.storage.staging().join("packages").join(uuid::Uuid::new_v4().simple().to_string());
    crate::package_zip::unpack(&body, &folder)
        .map_err(|error| ApiError::invalid(format!("The package is not a readable zip: {error}")))?;
    let restore_from = folder.clone();
    let result = state
        .storage
        .write(&id, None, move |store, _| {
            let preview = athleticstandard::package::preview_package_import(store, &restore_from)?;
            if !preview.restore {
                return Err(athleticstandard::Error::refused("A package can only be restored into an empty record"));
            }
            athleticstandard::package::apply_package_import(store, &preview, &[])?;
            store.database_revision()
        })
        .await;
    let _ = std::fs::remove_dir_all(&folder);
    Ok(Json(j!({ "database_revision": result? })))
}

async fn export(State(state): State<Shared>, caller: Caller, Path(id): Path<String>) -> ApiResult<Response> {
    owner(&state, &caller, &id).await?;
    let (bytes, digest) = export_zip(&state, &id).await?;
    let mut response = (StatusCode::OK, bytes).into_response();
    response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    response.headers_mut().insert("x-manifest-digest", HeaderValue::from_str(&digest).expect("hex digest"));
    Ok(response)
}

/// The complete package as a zip, and the SHA-256 of its manifest (D263).
pub async fn export_zip(state: &AppState, id: &str) -> ApiResult<(Vec<u8>, String)> {
    state.storage.ensure_attachment_files(id).await?;
    let exports = state.storage.staging().join("exports");
    std::fs::create_dir_all(&exports).map_err(|error| ApiError::internal("creating the export folder", error))?;
    let folder = exports.join(uuid::Uuid::new_v4().simple().to_string());
    let output = folder.clone();
    let result = state
        .storage
        .read(id, move |store| {
            export_package(store, &output, &ExportOptions::default())?;
            Ok(())
        })
        .await;
    let packed = result.and_then(|_| {
        let digest = std::fs::read(folder.join(athleticstandard::package::MANIFEST_DOCUMENT))
            .map(|bytes| hex(&Sha256::digest(&bytes)))
            .map_err(|error| ApiError::internal("reading the manifest", error))?;
        let bytes =
            crate::package_zip::pack(&folder).map_err(|error| ApiError::internal("packing an export", error))?;
        Ok((bytes, digest))
    });
    let _ = std::fs::remove_dir_all(&folder);
    packed
}

#[derive(Deserialize)]
struct Since {
    since: u64,
}

async fn changes(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    Query(since): Query<Since>,
) -> ApiResult<Json<Value>> {
    owner(&state, &caller, &id).await?;
    let found = state.storage.read(&id, move |store| store.changes_since(since.since)).await?;
    Ok(Json(j!({
        "from": found.from,
        "to": found.to,
        "complete": found.complete,
        "changes": found.changes.iter().map(|change| j!({
            "database_revision": change.database_revision,
            "record_id": change.record_id,
            "record_revision": change.record_revision,
            "kind": change.kind,
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    record_type: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
}

async fn list_records(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    owner(&state, &caller, &id).await?;
    let record_type = match &query.record_type {
        Some(name) => {
            Some(RecordType::parse(name).ok_or_else(|| ApiError::invalid(format!("Unknown record type: {name}")))?)
        }
        None => None,
    };
    let limit = query.limit.unwrap_or(state.config.limits.page_size).clamp(1, state.config.limits.page_size);
    let records = state
        .storage
        .read(&id, move |store| store.list_records(&ListOptions { record_type, include_deleted: false, limit }))
        .await?;
    Ok(Json(j!({ "records": records.iter().map(|record| to_json(&record.to_value())).collect::<Vec<_>>() })))
}

async fn get_record(
    State(state): State<Shared>,
    caller: Caller,
    Path((id, record_id)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    owner(&state, &caller, &id).await?;
    let lookup = record_id.clone();
    let (record, revisions) =
        state.storage.read(&id, move |store| Ok((store.get_record(&lookup)?, store.revisions(&lookup)?))).await?;
    let record = record.ok_or_else(ApiError::not_found)?;
    Ok(Json(j!({
        "record": to_json(&record.to_value()),
        "revisions": revisions.iter().map(|revision| to_json(&revision.to_value())).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateRequest {
    record_type: String,
    data: Value,
    #[serde(default)]
    record_id: Option<String>,
    #[serde(default)]
    attachments: Vec<String>,
}

async fn create_record(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let athlete = owner(&state, &caller, &id).await?;
    not_moving(&athlete)?;
    let request: CreateRequest = body_json(&body)?;
    let record_type = RecordType::parse(&request.record_type)
        .ok_or_else(|| ApiError::invalid(format!("Unknown record type: {}", request.record_type)))?;
    let data = decode(record_type, &request.data)?;
    let uploads =
        request.attachments.iter().map(|upload| staged_upload(&state, &id, upload)).collect::<ApiResult<Vec<_>>>()?;
    let receipt = receipt(&state, &caller, "create_record", idempotency_key(&headers)?, &body);
    let record_id = request.record_id.clone();
    let result = write_with_receipt(&state, &id, None, receipt, uploads, move |_| {
        Ok(Changes {
            creates: vec![NewRecord {
                record_id: record_id.clone(),
                ..NewRecord::new(data.clone(), RecordActor::Athlete)
            }],
            ..Changes::default()
        })
    })
    .await?;
    Ok((StatusCode::CREATED, Json(result)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CorrectionRequest {
    expected_revision: u64,
    data: Value,
}

async fn correct_record(
    State(state): State<Shared>,
    caller: Caller,
    Path((id, record_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let athlete = owner(&state, &caller, &id).await?;
    not_moving(&athlete)?;
    let request: CorrectionRequest = body_json(&body)?;
    let receipt = receipt(&state, &caller, "correct_record", idempotency_key(&headers)?, &body);
    let data = request.data.clone();
    let result = write_with_receipt(&state, &id, None, receipt, Vec::new(), move |store| {
        let current = store
            .get_record(&record_id)?
            .ok_or_else(|| athleticstandard::Error::not_found(format!("No record {record_id}")))?;
        let data = RecordData::decode(
            current.record_type(),
            &json::parse(&data.to_string()).map_err(|e| athleticstandard::Error::refused(e.to_string()))?,
        )?;
        Ok(Changes {
            corrections: vec![Correction {
                record_id: record_id.clone(),
                expected_revision: request.expected_revision,
                data,
                extensions: ExtensionsUpdate::Keep,
                changed_by: RecordActor::Athlete,
                became_known_at: None,
                became_known_at_basis: BecameKnownAtBasis::LocalWrite,
                stale_style: StaleStyle::Revision,
            }],
            ..Changes::default()
        })
    })
    .await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeletionRequest {
    expected_revision: u64,
}

async fn delete_record(
    State(state): State<Shared>,
    caller: Caller,
    Path((id, record_id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let athlete = owner(&state, &caller, &id).await?;
    not_moving(&athlete)?;
    let request: DeletionRequest = body_json(&body)?;
    let receipt = receipt(&state, &caller, "delete_record", idempotency_key(&headers)?, &body);
    let result = write_with_receipt(&state, &id, None, receipt, Vec::new(), move |_| {
        Ok(Changes {
            deletions: vec![Deletion {
                record_id: record_id.clone(),
                expected_revision: request.expected_revision,
                deleted_by: RecordActor::Athlete,
                deleted_at: None,
            }],
            ..Changes::default()
        })
    })
    .await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ErasureRequest {
    expected_revision: u64,
    reason_code: String,
}

/// Privacy erasure (D107). The library's erasure runs on its own transaction; it has
/// no receipt, and a repeated request finds nothing left to erase.
async fn erase(
    State(state): State<Shared>,
    caller: Caller,
    Path((id, record_id)): Path<(String, String)>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let athlete = owner(&state, &caller, &id).await?;
    not_moving(&athlete)?;
    let request: ErasureRequest = body_json(&body)?;
    let reason: ErasureReasonCode = serde_json::from_value(Value::String(request.reason_code.clone()))
        .map_err(|_| ApiError::invalid(format!("Unknown erasure reason: {}", request.reason_code)))?;
    let revision = state
        .storage
        .write(&id, None, move |store, _| {
            let preview = preview_erasure(store, &record_id, request.expected_revision)?;
            Ok(erase_record(store, &preview, reason, None)?.database_revision)
        })
        .await?;
    Ok(Json(j!({ "database_revision": revision })))
}

async fn upload(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let athlete = owner(&state, &caller, &id).await?;
    not_moving(&athlete)?;
    if body.is_empty() {
        return Err(ApiError::invalid("An upload needs a body"));
    }
    let media_type = athleticstandard::files::attachments::sniff_media_type(&body)
        .ok_or_else(|| ApiError::refused("An attachment must be a PNG, JPEG, WebP, or PDF file"))?;
    let upload_id = uuid::Uuid::new_v4().simple().to_string();
    let folder = state.storage.staging().join("uploads").join(&id);
    std::fs::create_dir_all(&folder).map_err(|e| ApiError::internal("staging an upload", e))?;
    std::fs::write(folder.join(&upload_id), &body).map_err(|e| ApiError::internal("staging an upload", e))?;
    Ok((
        StatusCode::CREATED,
        Json(
            j!({ "upload_id": upload_id, "byte_count": body.len(), "sha256": hex(&Sha256::digest(&body)), "media_type": media_type }),
        ),
    ))
}

async fn download_attachment(
    State(state): State<Shared>,
    caller: Caller,
    Path((id, record_id)): Path<(String, String)>,
) -> ApiResult<Response> {
    owner(&state, &caller, &id).await?;
    let lookup = record_id.clone();
    let record =
        state.storage.read(&id, move |store| store.get_record(&lookup)).await?.ok_or_else(ApiError::not_found)?;
    let RecordData::Attachment(data) = &record.data else {
        return Err(ApiError::not_found());
    };
    let name = athleticstandard::files::attachments::stored_attachment_name(&record.record_id, data);
    let bytes = state.storage.attachment_bytes(&id, &name).await?;
    let mut response = (StatusCode::OK, bytes).into_response();
    if let Ok(value) = HeaderValue::from_str(&data.media_type.to_string()) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    Ok(response)
}

// ---- Owner: grants (D262) ---------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OfferRequest {
    permissions: Vec<Permission>,
    client_id: String,
    #[serde(default)]
    coach_label: Option<String>,
    #[serde(default)]
    valid_seconds: Option<i64>,
}

async fn create_grant_offer(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<Value>)> {
    owner(&state, &caller, &id).await?;
    let request: OfferRequest = body_json(&body)?;
    if request.permissions.is_empty() {
        return Err(ApiError::invalid("Choose at least one permission"));
    }
    let mut permissions = request.permissions.clone();
    permissions.sort();
    permissions.dedup();
    let offer = state
        .directory
        .create_grant_offer(
            &id,
            &permissions,
            &request.client_id,
            request.coach_label.as_deref(),
            &caller.principal.principal_id,
            request.valid_seconds.unwrap_or(7 * 24 * 3600).clamp(60, 30 * 24 * 3600),
        )
        .await
        .map_err(|e| ApiError::internal("directory", e))?;
    Ok((
        StatusCode::CREATED,
        Json(
            j!({ "code": offer.code, "expires_at": offer.expires_at, "permissions": permissions, "client_id": request.client_id }),
        ),
    ))
}

async fn list_grants(State(state): State<Shared>, caller: Caller, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    owner(&state, &caller, &id).await?;
    let grants = state.directory.grants_for_athlete(&id).await.map_err(|e| ApiError::internal("directory", e))?;
    Ok(Json(j!({ "grants": grants })))
}

/// Revoke a grant: mark it revoked, then raise the access epoch (D262). A coach write
/// that read the epoch before this point and commits after it fails.
async fn revoke_grant(
    State(state): State<Shared>,
    caller: Caller,
    Path((id, grant_id)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    owner(&state, &caller, &id).await?;
    let revoked = state.directory.revoke(&id, &grant_id).await.map_err(|e| ApiError::internal("directory", e))?;
    if !revoked {
        return Err(ApiError::not_found());
    }
    let epoch = state.storage.raise_access_epoch(&id).await?;
    Ok(Json(j!({ "revoked": grant_id, "access_epoch": epoch })))
}

// ---- Coach -------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptRequest {
    code: String,
}

async fn accept_grant_offer(State(state): State<Shared>, caller: Caller, body: Bytes) -> ApiResult<Json<Value>> {
    let request: AcceptRequest = body_json(&body)?;
    let grant = state
        .directory
        .accept_grant_offer(&request.code, &caller.principal, &caller.client_id)
        .await
        .map_err(|e| ApiError::internal("directory", e))?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(j!({ "grant": grant })))
}

async fn roster(State(state): State<Shared>, caller: Caller) -> ApiResult<Json<Value>> {
    let roster = state
        .directory
        .roster(&caller.principal.principal_id, &caller.client_id)
        .await
        .map_err(|e| ApiError::internal("directory", e))?;
    Ok(Json(j!({
        "athletes": roster.iter().map(|(athlete, grant)| j!({
            "athlete_id": athlete.resource_id,
            "display_name": athlete.display_name,
            "permissions": grant.permissions,
            "grant_id": grant.grant_id,
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct WorkoutQuery {
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    after: Option<String>,
}

/// Workout summaries under the read grant: record ID, revision, start, end, and
/// activity type. Nothing else from the workout (D262).
async fn workout_summaries(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    Query(query): Query<WorkoutQuery>,
) -> ApiResult<Json<Value>> {
    let (_, _) = coach_grant(&state, &caller, &id, Permission::ReadWorkoutSummaries).await?;
    let limit = query.limit.unwrap_or(state.config.limits.page_size).clamp(1, state.config.limits.page_size) as usize;
    let mut summaries: Vec<(String, String, Value)> = state
        .storage
        .read(&id, |store| {
            let records = store.list_records(&ListOptions {
                record_type: Some(RecordType::WorkoutSession),
                include_deleted: false,
                limit: 10_000,
            })?;
            Ok(records
                .into_iter()
                .filter_map(|record| match &record.data {
                    RecordData::WorkoutSession(data) => Some((
                        data.start_at.clone(),
                        record.record_id.clone(),
                        j!({
                            "record_id": record.record_id,
                            "revision": record.current_revision,
                            "start_at": data.start_at,
                            "end_at": data.end_at,
                            "activity_type": data.activity_type,
                        }),
                    )),
                    _ => None,
                })
                .collect())
        })
        .await?;
    summaries.sort_by(|left, right| (&right.0, &right.1).cmp(&(&left.0, &left.1)));
    let start = match &query.after {
        Some(after) => summaries
            .iter()
            .position(|(_, record_id, _)| record_id == after)
            .map(|index| index + 1)
            .unwrap_or(summaries.len()),
        None => 0,
    };
    let page: Vec<Value> = summaries.iter().skip(start).take(limit).map(|(_, _, summary)| summary.clone()).collect();
    let next = if start + page.len() < summaries.len() {
        page.last().map(|summary| summary["record_id"].clone())
    } else {
        None
    };
    Ok(Json(j!({ "workouts": page, "next": next })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoteRequest {
    note: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    applies_from: Option<String>,
    #[serde(default)]
    applies_to: Option<String>,
    #[serde(default)]
    evidence: Vec<EvidenceRequest>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct EvidenceRequest {
    record_id: String,
    revision: u64,
}

async fn add_note(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let (grant, epoch) = coach_grant(&state, &caller, &id, Permission::AddCoachNotes).await?;
    let athlete = state
        .directory
        .athlete(&id)
        .await
        .map_err(|e| ApiError::internal("directory", e))?
        .ok_or_else(ApiError::not_found)?;
    not_moving(&athlete)?;
    let request: NoteRequest = body_json(&body)?;
    if !request.evidence.is_empty() && !grant.allows(Permission::ReadWorkoutSummaries) {
        return Err(ApiError::permission_denied("Evidence must be a workout you can read"));
    }
    let mut data = j!({
        "authorship": { "author": coach_author(&caller, &grant), "authored_at": now(), "evidence": request.evidence.iter().map(|e| j!({ "record_id": e.record_id, "revision": e.revision })).collect::<Vec<_>>(), "content_version": 1, "status": "active" },
        "note": request.note,
    });
    for (field, value) in
        [("title", &request.title), ("applies_from", &request.applies_from), ("applies_to", &request.applies_to)]
    {
        if let Some(value) = value {
            data[field] = j!(value);
        }
    }
    let record = decode(RecordType::CoachingNote, &data)?;
    let actor = coach_actor(&caller, &grant)?;
    let evidence = request.evidence.clone();
    let receipt = receipt(&state, &caller, "add_note", idempotency_key(&headers)?, &body);
    let result = write_with_receipt(&state, &id, Some(epoch), receipt, Vec::new(), move |store| {
        check_evidence(store, &evidence)?;
        Ok(Changes { creates: vec![NewRecord::new(record.clone(), actor.clone())], ..Changes::default() })
    })
    .await?;
    Ok((StatusCode::CREATED, Json(result)))
}

/// Evidence must name the current revision of a workout the read grant shows.
fn check_evidence(store: &Store, evidence: &[EvidenceRequest]) -> athleticstandard::Result<()> {
    for reference in evidence {
        let visible = store.get_record(&reference.record_id)?.is_some_and(|record| {
            !record.is_deleted
                && record.record_type() == RecordType::WorkoutSession
                && record.current_revision == reference.revision
        });
        if !visible {
            return Err(athleticstandard::Error::refused(format!(
                "Evidence {} at revision {} is not a current workout you can read",
                reference.record_id, reference.revision
            )));
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgramRequest {
    plan: Value,
    #[serde(default)]
    sessions: Vec<Value>,
}

/// A training plan and its planned sessions, created together (D262). Sessions start
/// as planned; the coach cannot mark one completed or link a completed workout.
async fn add_program(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let (grant, epoch) = coach_grant(&state, &caller, &id, Permission::AddProgramming).await?;
    let athlete = state
        .directory
        .athlete(&id)
        .await
        .map_err(|e| ApiError::internal("directory", e))?
        .ok_or_else(ApiError::not_found)?;
    not_moving(&athlete)?;
    let request: ProgramRequest = body_json(&body)?;
    let authorship = j!({ "author": coach_author(&caller, &grant), "authored_at": now(), "evidence": [], "content_version": 1, "status": "active" });
    let mut plan = request.plan.as_object().cloned().ok_or_else(|| ApiError::invalid("plan must be an object"))?;
    if plan.contains_key("authorship") {
        return Err(ApiError::invalid("The host writes the authorship"));
    }
    plan.insert("authorship".into(), authorship.clone());
    let plan_id = athleticstandard::ids::new_record_id();
    let plan_data = decode(RecordType::TrainingPlan, &Value::Object(plan))?;
    let mut sessions = Vec::new();
    for session in &request.sessions {
        let mut session =
            session.as_object().cloned().ok_or_else(|| ApiError::invalid("each session must be an object"))?;
        for forbidden in ["authorship", "training_plan_id", "completion_status", "completed_workout_session_id"] {
            if session.contains_key(forbidden) {
                return Err(ApiError::invalid(format!("A coach cannot set {forbidden}")));
            }
        }
        session.insert("authorship".into(), authorship.clone());
        session.insert("training_plan_id".into(), j!(plan_id));
        session.insert("completion_status".into(), j!("planned"));
        sessions.push(decode(RecordType::PlannedSession, &Value::Object(session))?);
    }
    let actor = coach_actor(&caller, &grant)?;
    let receipt = receipt(&state, &caller, "add_program", idempotency_key(&headers)?, &body);
    let result = write_with_receipt(&state, &id, Some(epoch), receipt, Vec::new(), move |_| {
        let mut creates =
            vec![NewRecord { record_id: Some(plan_id.clone()), ..NewRecord::new(plan_data.clone(), actor.clone()) }];
        creates.extend(sessions.iter().map(|session| NewRecord::new(session.clone(), actor.clone())));
        Ok(Changes { creates, ..Changes::default() })
    })
    .await?;
    Ok((StatusCode::CREATED, Json(result)))
}
