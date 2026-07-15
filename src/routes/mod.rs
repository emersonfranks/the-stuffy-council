//! HTTP routes.

pub mod auth;
pub mod characters;
pub mod home;

use axum::Router;
use axum::routing::{get, post};
use tower_http::services::ServeDir;

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        // Public
        .route("/login", get(auth::show_login))
        .route("/auth/google/verify", post(auth::google_verify))
        .route("/logout", post(auth::do_logout))
        .route("/healthz", get(|| async { "ok" }))
        // Protected — each handler calls the local `require_user(&session)`
        // helper at entry and redirects to `/login` when it returns `None`.
        // There is no dedicated extractor; the check lives in the handler.
        .route("/", get(home::index))
        .route("/story/today", get(home::today))
        .route("/council", get(characters::list_characters))
        .route("/council/{id}", get(characters::show_character))
        // Static assets (css, self-hosted fonts, favicon, textures, portraits).
        // Path is relative to the process CWD (repo root in dev; the image
        // copies `static/` next to the binary for prod).
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
}
