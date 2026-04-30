// Cross-route error type. Maps storage + crd-index errors to HTTP
// status codes; renders a tiny JSON body the caller can show to the
// user without leaking internals.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use cracha_storage::StorageError;
use serde::Serialize;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("storage: {0}")]
    Storage(#[from] StorageError),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("bad request: {0}")]
    BadRequest(String),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            ApiError::Storage(ref e) => {
                error!(?e, "storage error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
            ApiError::NotFound(s) => (StatusCode::NOT_FOUND, s),
            ApiError::Forbidden(s) => (StatusCode::FORBIDDEN, s),
            ApiError::BadRequest(s) => (StatusCode::BAD_REQUEST, s),
        };
        (status, Json(ErrorBody { error: msg })).into_response()
    }
}
