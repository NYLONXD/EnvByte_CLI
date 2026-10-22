use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("authentication required")]
    Unauthorized,
    #[error("you do not have permission to perform this action")]
    Forbidden,
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    /// A write edited from a version that is no longer current. Carries the
    /// current commit, so the client can fetch it, merge and try again.
    #[error("{message}")]
    Stale {
        message: String,
        current_commit_id: Option<Uuid>,
    },
    #[error("request timed out")]
    Timeout,
    #[error("internal server error")]
    Internal,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) | Self::Stale { .. } => StatusCode::CONFLICT,
            Self::Timeout => StatusCode::REQUEST_TIMEOUT,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = match &self {
            Self::Stale {
                current_commit_id, ..
            } => json!({
                "error": self.to_string(),
                "code": "stale_base",
                "current_commit_id": current_commit_id,
            }),
            _ => json!({ "error": self.to_string() }),
        };
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        tracing::error!(error = %error, "database operation failed");
        match &error {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                Self::Conflict("resource already exists".to_string())
            }
            _ => Self::Internal,
        }
    }
}
