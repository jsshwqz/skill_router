//! Unified API error handling
//!
//! Converts internal `anyhow::Error` into structured JSON responses
//! with appropriate HTTP status codes.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// Structured API error response
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ApiError {
    pub fn internal(msg: impl ToString) -> (StatusCode, Json<ApiError>) {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: msg.to_string(),
                code: 500,
                detail: None,
            }),
        )
    }

    pub fn bad_request(msg: impl ToString) -> (StatusCode, Json<ApiError>) {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: msg.to_string(),
                code: 400,
                detail: None,
            }),
        )
    }

    #[allow(dead_code)]
    pub fn not_found(msg: impl ToString) -> (StatusCode, Json<ApiError>) {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: msg.to_string(),
                code: 404,
                detail: None,
            }),
        )
    }
}

/// Wrapper for `anyhow::Error` that implements `IntoResponse`
pub struct AppError(pub anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = ApiError::internal(self.0);
        (status, body).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
