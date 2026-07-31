//! HTTP routes.

pub mod admin;
pub mod archive;
pub mod auth;
pub mod characters;
pub mod home;
pub mod story_view;

use axum::Router;
use axum::routing::{get, post};
use tower_http::services::ServeDir;
use tower_sessions::Session;

use crate::auth::{SESSION_USER_KEY, SessionUser};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        // Public
        .route("/login", get(auth::show_login))
        .route("/auth/google/verify", post(auth::google_verify))
        .route("/logout", post(auth::do_logout))
        .route("/healthz", get(|| async { "ok" }))
        // Protected — each handler calls the shared `require_user(&session)`
        // helper at entry and redirects to `/login` when it returns `None`.
        // There is no dedicated extractor; the check lives in the handler.
        .route("/", get(home::index))
        .route("/story", get(archive::list))
        .route("/story/today", get(home::today))
        // The static `/story/today` above takes precedence over this parameter.
        .route("/story/{date}", get(archive::by_date))
        .route("/council", get(characters::list_characters))
        .route("/council/{id}", get(characters::show_character))
        .route("/admin/stories.json", get(admin::export_stories))
        // Static assets (css, self-hosted fonts, favicon, textures, portraits).
        // Path is relative to the process CWD (repo root in dev; the image
        // copies `static/` next to the binary for prod).
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
}

/// `None` means "not signed in" — callers redirect to `/login` rather than
/// erroring, so this is deliberately not `AppError::Unauthorized`.
pub(crate) async fn require_user(session: &Session) -> AppResult<Option<SessionUser>> {
    session
        .get::<SessionUser>(SESSION_USER_KEY)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("session get user: {e}")))
}

/// Signed-in non-admins get `Forbidden` rather than a redirect: they are
/// already authenticated, so repeating the login flow cannot grant the flag.
pub(crate) async fn require_admin(session: &Session) -> AppResult<Option<SessionUser>> {
    match require_user(session).await? {
        None => Ok(None),
        Some(user) if user.admin => Ok(Some(user)),
        Some(_) => Err(AppError::Forbidden),
    }
}
