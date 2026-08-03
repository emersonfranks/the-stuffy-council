//! Admin-only surfaces. Currently just the archive backup.

use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Redirect, Response};
use tower_sessions::Session;

use crate::error::AppResult;
use crate::routes::require_admin;
use crate::state::AppState;
use crate::story_repo::{self, ExportedStory};

/// Exists so the archive can be captured before anything that resets the dev
/// database — notably the sqlx checksum failure that follows editing an applied
/// migration, whose only local fix is deleting `stuffy-council.sqlite*`.
pub async fn export_stories(
    State(state): State<AppState>,
    session: Session,
) -> AppResult<Response> {
    if require_admin(&session).await?.is_none() {
        return Ok(Redirect::to("/login").into_response());
    }

    let stories: Vec<ExportedStory> = story_repo::export_all(&state.db).await?;
    Ok(Json(stories).into_response())
}
