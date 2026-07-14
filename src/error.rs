//! One error type the HTTP layer knows how to render.
//!
//! Handlers return `Result<T, AppError>`; anything unexpected becomes a 500
//! with a boring page, and the details go to the logs — never to the client.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("resource not found")]
    NotFound,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("csrf token mismatch")]
    CsrfMismatch,

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Log full details server-side.
        match &self {
            AppError::Internal(e) => tracing::error!(error = ?e, "internal error"),
            AppError::CsrfMismatch => tracing::warn!("csrf mismatch"),
            AppError::Unauthorized => tracing::debug!("unauthorized"),
            AppError::Forbidden => tracing::debug!("forbidden"),
            AppError::NotFound => tracing::debug!("not found"),
            AppError::BadRequest(m) => tracing::debug!(message = %m, "bad request"),
        }

        let (status, body) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Not found"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "Bad request"),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden"),
            AppError::CsrfMismatch => (StatusCode::FORBIDDEN, "Request could not be verified"),
            AppError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong. Please try again.",
            ),
        };

        (status, body).into_response()
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;
