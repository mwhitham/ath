//! Moving a record between hosts with one writer at every step (D267, spec 6.7).
//!
//! The owner's app drives the move:
//!
//! 1. On the source, `move/freeze` stops writes and returns the transfer ID and the
//!    SHA-256 of the final package manifest.
//! 2. The app downloads the package from the source, creates an `incoming` record on
//!    the destination, and uploads the package there. The destination keeps it
//!    staged: it accepts no other write and shows nothing to coaches.
//! 3. On the source, `move/retire` retires the writer and returns a single-use
//!    activation code bound to this transfer, the destination, and the digest.
//! 4. On the destination, `move/activate` asks the source to confirm the code for its
//!    own address and the digest of the package it holds. Only then does it accept
//!    writes.
//!
//! Before step 3 the owner can `move/abort` and reopen the source. After step 3 the
//! source cannot reopen by itself; a lost reply is retried with the same transfer ID.

use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{body_json, export_zip, owner, Caller, Shared};
use crate::directory::{new_code, Athlete};
use crate::error::{ApiError, ApiResult};

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/v1/athletes/{id}/move/freeze", post(freeze))
        .route("/v1/athletes/{id}/move/abort", post(abort))
        .route("/v1/athletes/{id}/move/retire", post(retire))
        .route("/v1/athletes/{id}/move/activate", post(activate))
        .route("/v1/athletes/{id}/move/recovery-copy", get(recovery_copy))
        .route("/v1/transfers/{transfer_id}/verify", post(verify))
}

fn internal(error: crate::db::DbError) -> ApiError {
    ApiError::internal("directory", error)
}

/// The owner's athlete in any state, including retired: a retired record keeps an
/// owner recovery copy until cleanup (D267).
async fn owned_any_state(state: &Shared, caller: &Caller, id: &str) -> ApiResult<Athlete> {
    let athlete = state.directory.athlete(id).await.map_err(internal)?.ok_or_else(ApiError::not_found)?;
    if athlete.owner_principal_id != caller.principal.principal_id {
        return Err(ApiError::not_found());
    }
    crate::api::owner_app(state, caller, id).await?;
    Ok(athlete)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FreezeRequest {
    /// The destination host's public address.
    destination: String,
}

async fn freeze(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let athlete = owner(&state, &caller, &id).await?;
    let request: FreezeRequest = body_json(&body)?;
    if let Some(open) = state.directory.open_transfer_for(&id).await.map_err(internal)? {
        if open.direction == "outgoing" && open.state == "frozen" {
            return Ok(Json(json!({ "transfer_id": open.transfer_id, "manifest_digest": open.manifest_digest })));
        }
    }
    if athlete.state != "active"
        || !state.directory.set_athlete_state(&id, "active", "frozen").await.map_err(internal)?
    {
        return Err(ApiError::moving());
    }
    // Nothing writes now, so this package is the final one.
    let (_, digest) = match export_zip(&state, &id).await {
        Ok(exported) => exported,
        Err(error) => {
            let _ = state.directory.set_athlete_state(&id, "frozen", "active").await;
            return Err(error);
        }
    };
    let transfer_id = format!("transfer_{}", uuid::Uuid::new_v4().simple());
    state
        .directory
        .record_transfer(&transfer_id, &id, "outgoing", Some(request.destination.trim_end_matches('/')), "frozen")
        .await
        .map_err(internal)?;
    state.directory.update_transfer(&transfer_id, "frozen", "frozen", Some(&digest), None).await.map_err(internal)?;
    Ok(Json(json!({ "transfer_id": transfer_id, "manifest_digest": digest })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferRequest {
    transfer_id: String,
}

async fn abort(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    owned_any_state(&state, &caller, &id).await?;
    let request: TransferRequest = body_json(&body)?;
    let transfer = state.directory.transfer(&request.transfer_id).await.map_err(internal)?;
    if !transfer.is_some_and(|transfer| transfer.resource_id == id) {
        return Err(ApiError::not_found());
    }
    if !state
        .directory
        .update_transfer(&request.transfer_id, "frozen", "aborted", None, None)
        .await
        .map_err(internal)?
    {
        return Err(ApiError::stale("This move can no longer be stopped: the source has retired its writer"));
    }
    state.directory.set_athlete_state(&id, "frozen", "active").await.map_err(internal)?;
    Ok(Json(json!({ "state": "active" })))
}

async fn retire(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    owned_any_state(&state, &caller, &id).await?;
    let request: TransferRequest = body_json(&body)?;
    let transfer = state
        .directory
        .transfer(&request.transfer_id)
        .await
        .map_err(internal)?
        .filter(|transfer| transfer.resource_id == id && transfer.direction == "outgoing")
        .ok_or_else(ApiError::not_found)?;
    match transfer.state.as_str() {
        "frozen" => {
            state
                .directory
                .update_transfer(&transfer.transfer_id, "frozen", "retired", None, None)
                .await
                .map_err(internal)?;
            state.directory.set_athlete_state(&id, "frozen", "retired").await.map_err(internal)?;
        }
        // A lost reply: issue the same proof again.
        "retired" => {}
        _ => return Err(ApiError::stale("This move is not waiting to be retired")),
    }
    let activation = new_code();
    state.directory.set_activation(&transfer.transfer_id, &activation).await.map_err(internal)?;
    Ok(Json(
        json!({ "transfer_id": transfer.transfer_id, "activation": activation, "manifest_digest": transfer.manifest_digest }),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyRequest {
    activation: String,
    destination: String,
    manifest_digest: String,
}

/// Called by the destination host. The code is the only credential; it is used once.
async fn verify(State(state): State<Shared>, Path(transfer_id): Path<String>, body: Bytes) -> ApiResult<Json<Value>> {
    let request: VerifyRequest = body_json(&body)?;
    let used = state
        .directory
        .use_activation(
            &transfer_id,
            &request.activation,
            request.destination.trim_end_matches('/'),
            &request.manifest_digest,
        )
        .await
        .map_err(internal)?;
    if !used {
        return Err(ApiError::not_found());
    }
    Ok(Json(json!({ "verified": true })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActivateRequest {
    /// The source host's public address.
    source: String,
    transfer_id: String,
    activation: String,
}

async fn activate(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<String>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let athlete = owned_any_state(&state, &caller, &id).await?;
    if athlete.state != "staged" {
        return Err(ApiError::stale("Only a staged incoming record can be activated"));
    }
    let request: ActivateRequest = body_json(&body)?;
    let (_, digest) = export_zip(&state, &id).await?;
    let destination = state.config.public_url.trim_end_matches('/').to_string();
    let url = format!("{}/v1/transfers/{}/verify", request.source.trim_end_matches('/'), request.transfer_id);
    let body = json!({ "activation": request.activation, "destination": destination, "manifest_digest": digest });
    let verified =
        tokio::task::spawn_blocking(move || match ureq::post(&url).timeout(Duration::from_secs(30)).send_json(body) {
            Ok(response) => Ok(response.status() == 200),
            Err(ureq::Error::Status(_, _)) => Ok(false),
            Err(error) => Err(error.to_string()),
        })
        .await
        .map_err(|error| ApiError::internal("verifying a move", error))?
        .map_err(|error| ApiError::internal("verifying a move", error))?;
    if !verified {
        return Err(ApiError::permission_denied(
            "The source host did not confirm this move for this host and this package",
        ));
    }
    state.directory.set_athlete_state(&id, "staged", "active").await.map_err(internal)?;
    state
        .directory
        .record_transfer(&request.transfer_id, &id, "incoming", None, "activated")
        .await
        .map_err(internal)?;
    Ok(Json(json!({ "state": "active", "manifest_digest": digest })))
}

/// The owner's copy of a retired record, until cleanup (D267).
async fn recovery_copy(State(state): State<Shared>, caller: Caller, Path(id): Path<String>) -> ApiResult<Response> {
    let athlete = owned_any_state(&state, &caller, &id).await?;
    if athlete.state != "retired" {
        return Err(ApiError::stale("Use the export for a record that is not retired"));
    }
    let (bytes, digest) = export_zip(&state, &id).await?;
    let mut response = (StatusCode::OK, bytes).into_response();
    response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    response.headers_mut().insert("x-manifest-digest", HeaderValue::from_str(&digest).expect("hex digest"));
    Ok(response)
}
