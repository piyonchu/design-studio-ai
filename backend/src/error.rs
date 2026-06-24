use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Unified API error. All handlers return `Result<T, AppError>`; this maps DB
/// and validation failures to clean JSON responses.
#[derive(Debug)]
pub enum AppError {
    /// Resource not found → 404.
    NotFound,
    /// Caller error (bad input, FK violation) → 400.
    BadRequest(String),
    /// Unexpected database error → 500.
    Db(sqlx::Error),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::NotFound => write!(f, "not found"),
            AppError::BadRequest(msg) => write!(f, "{msg}"),
            AppError::Db(err) => write!(f, "database error: {err}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            // No row where the query expected one (e.g. fetch_one on a missing id).
            sqlx::Error::RowNotFound => AppError::NotFound,
            // Foreign-key violation → the referenced parent doesn't exist.
            sqlx::Error::Database(db) if db.code().as_deref() == Some("23503") => {
                AppError::BadRequest("referenced resource does not exist".into())
            }
            _ => AppError::Db(err),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Db(err) => {
                tracing::error!(%err, "unhandled database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
