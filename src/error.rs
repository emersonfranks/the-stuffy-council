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
            AppError::NotFound => (StatusCode::NOT_FOUND, "That page wandered off."),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "Bad request"),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden"),
            AppError::CsrfMismatch => (StatusCode::FORBIDDEN, "Request could not be verified"),
            AppError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went sideways backstage. Please try again.",
            ),
        };

        (status, body).into_response()
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    // error → HTTP response mapping. Functional dimension: each user-facing
    // variant's status + public body. Security dimension: Internal must never
    // leak the underlying anyhow message to the client. Error-handling and
    // state-transition dimensions are N/A — this IS the terminal error mapper;
    // it is total and stateless.
    use super::*;
    use axum::body::to_bytes;

    async fn render(err: AppError) -> (StatusCode, String) {
        let resp = err.into_response();
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("read error body");
        (status, String::from_utf8(bytes.to_vec()).expect("utf8 body"))
    }

    #[tokio::test]
    async fn not_found_renders_404_with_in_voice_body() {
        let (status, body) = render(AppError::NotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, "That page wandered off.");
    }

    #[tokio::test]
    async fn internal_renders_500_and_never_leaks_details() {
        let (status, body) =
            render(AppError::Internal(anyhow::anyhow!("db url = secret-hunter2"))).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            !body.contains("hunter2"),
            "internal error body leaked details: {body}"
        );
        assert_eq!(body, "Something went sideways backstage. Please try again.");
    }
}
