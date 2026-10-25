//! One error type for HTTP handlers: `{"error": {"code": "...", "message": "..."}}`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not signed in")]
    Unauthorized,
    #[error("not allowed")]
    Forbidden,
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Conflict(String),
    #[error("not implemented yet")]
    NotImplemented,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            Self::NotImplemented => (StatusCode::NOT_IMPLEMENTED, "not_implemented"),
            Self::Database(_) | Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        // Internal details go to the log, never to the client.
        let message = match &self {
            Self::Database(e) => {
                tracing::error!(error = %e, "database error");
                "internal error".to_string()
            }
            Self::Internal(e) => {
                tracing::error!(error = ?e, "internal error");
                "internal error".to_string()
            }
            other => other.to_string(),
        };
        (status, Json(json!({ "error": { "code": code, "message": message } }))).into_response()
    }
}
