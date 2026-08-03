//! Read-only browsing of past stories.
//!
//! Split from `home`: that module owns the landing page and the live
//! generate-if-missing path, while everything here only reads the cache.
//! Nothing in this module generates a story, so visiting an uncached date is a
//! 404 rather than a trigger.

use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use time::Date;
use time::format_description::well_known::Iso8601;
use tower_sessions::Session;

use crate::error::{AppError, AppResult};
use crate::routes::require_user;
use crate::routes::story_view::StoryTemplate;
use crate::state::AppState;
use crate::story_repo;
use crate::web::csrf;

/// How many entries the archive index shows. Stories accrue one per day, so
/// this is roughly a year before pagination is worth building.
const ARCHIVE_PAGE_SIZE: i64 = 365;

#[derive(Template)]
#[template(path = "archive.html")]
struct ArchiveTemplate {
    csrf_token: String,
    entries: Vec<ArchiveEntry>,
}

struct ArchiveEntry {
    iso: String,
    title: String,
}

pub async fn list(State(state): State<AppState>, session: Session) -> AppResult<Response> {
    if require_user(&state.access, &session).await?.is_none() {
        return Ok(Redirect::to("/login").into_response());
    }

    let entries = story_repo::list_recent(&state.db, ARCHIVE_PAGE_SIZE)
        .await?
        .into_iter()
        .map(|summary| ArchiveEntry {
            iso: summary.date.to_string(),
            title: summary.title,
        })
        .collect();

    let tpl = ArchiveTemplate {
        csrf_token: csrf::token(&session).await?,
        entries,
    };
    Ok(render(&tpl)?.into_response())
}

pub async fn by_date(
    State(state): State<AppState>,
    session: Session,
    Path(date): Path<String>,
) -> AppResult<Response> {
    if require_user(&state.access, &session).await?.is_none() {
        return Ok(Redirect::to("/login").into_response());
    }

    // Parse before touching SQL so unparseable input never reaches the query
    // layer, and so a bad date is a 400 rather than an empty-looking 404.
    let date = Date::parse(&date, &Iso8601::DATE)
        .map_err(|_| AppError::BadRequest("story date must be YYYY-MM-DD".into()))?;

    let Some(cached) = story_repo::get(&state.db, date).await? else {
        return Err(AppError::NotFound);
    };

    let tpl = StoryTemplate::from_story(
        csrf::token(&session).await?,
        cached.date,
        cached.title,
        &cached.body,
        &cached.cast,
        cached.model,
        &state.cast,
    );
    Ok(render(&tpl)?.into_response())
}

fn render<T: Template>(tpl: &T) -> AppResult<Html<String>> {
    let body = tpl
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("template render: {e}")))?;
    Ok(Html(body))
}
