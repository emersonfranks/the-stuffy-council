//! HTTP routes.

pub mod auth;
pub mod characters;
pub mod home;

use axum::Router;
use axum::routing::{get, post};

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        // Public
        .route("/login", get(auth::show_login))
        .route("/auth/google", get(auth::start_google))
        .route("/auth/google/callback", get(auth::google_callback))
        .route("/logout", post(auth::do_logout))
        .route("/healthz", get(|| async { "ok" }))
        // Protected — the login middleware is applied inside routes we care about
        // via the SessionUser extractor, so we don't need a separate layer here.
        .route("/", get(home::index))
        .route("/story/today", get(home::today))
        .route("/council", get(characters::list_characters))
        .route("/council/{id}", get(characters::show_character))
        .with_state(state)
}
