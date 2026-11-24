//! The errors the API returns. Every error is JSON with a stable code (spec section
//! 6.5). A request about an athlete the caller has no relation to gets the same
//! `not_found` answer as a request about an athlete that does not exist.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

pub type ApiResult<T> = std::result::Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> ApiError {
        ApiError { status, code, message: message.into() }
    }

    pub fn sign_in_required(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::UNAUTHORIZED, "sign_in_required", message)
    }

    pub fn permission_denied(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::FORBIDDEN, "permission_denied", message)
    }

    /// The athlete disconnected the app this token was issued to (D270).
    pub fn app_disconnected() -> ApiError {
        ApiError::new(
            StatusCode::FORBIDDEN,
            "app_disconnected",
            "The athlete disconnected this app from this record. Connect it again from another app the athlete uses",
        )
    }

    pub fn not_found() -> ApiError {
        ApiError::new(StatusCode::NOT_FOUND, "not_found", "There is no such resource, or it is not shared with you")
    }

    pub fn stale(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::CONFLICT, "stale", message)
    }

    pub fn request_conflict() -> ApiError {
        ApiError::new(
            StatusCode::CONFLICT,
            "request_conflict",
            "This Idempotency-Key was already used for a different request",
        )
    }

    pub fn moving() -> ApiError {
        ApiError::new(
            StatusCode::CONFLICT,
            "host_moving",
            "This record is moving to another host and accepts no changes",
        )
    }

    pub fn limit(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "limit_reached", message)
    }

    pub fn invalid(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::BAD_REQUEST, "invalid", message)
    }

    pub fn refused(message: impl Into<String>) -> ApiError {
        ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "refused", message)
    }

    /// An error whose details stay in the host's own log. The response names no
    /// file path, credential, or record value.
    pub fn internal(context: &str, error: impl std::fmt::Display) -> ApiError {
        eprintln!("ath-host: {context}: {error}");
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", "The host could not finish this request")
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}): {}", self.code, self.status.as_u16(), self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<athleticstandard::Error> for ApiError {
    fn from(error: athleticstandard::Error) -> ApiError {
        match error.code() {
            "invalid" => ApiError::invalid(error.to_string()),
            "not_found" => ApiError::not_found(),
            "stale" => ApiError::stale(error.to_string()),
            "refused" => ApiError::refused(error.to_string()),
            _ => ApiError::internal("library", error),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({ "error": { "code": self.code, "message": self.message } });
        (self.status, Json(body)).into_response()
    }
}
