//! Home + today's story routes. All routes here require a logged-in user.

use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Redirect, Response};
use time::OffsetDateTime;
use tower_sessions::Session;

use crate::auth::{SESSION_USER_KEY, SessionUser};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::story_repo;
use crate::web::csrf;

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    display_name: String,
    csrf_token: String,
    has_today: bool,
    today_title: String,
    today_iso: String,
}

#[derive(Template)]
#[template(path = "story.html")]
struct StoryTemplate {
    display_name: String,
    csrf_token: String,
    title: String,
    body_paragraphs: Vec<String>,
    cast_names: Vec<String>,
    date_display: String,
    model: String,
}

pub async fn index(State(state): State<AppState>, session: Session) -> AppResult<Response> {
    let Some(user) = require_user(&session).await? else {
        return Ok(Redirect::to("/login").into_response());
    };

    let today = OffsetDateTime::now_utc().date();
    let cached = story_repo::get(&state.db, today).await?;

    let tpl = HomeTemplate {
        display_name: user.display_name,
        csrf_token: csrf::token(&session).await?,
        has_today: cached.is_some(),
        today_title: cached.as_ref().map(|c| c.title.clone()).unwrap_or_default(),
        today_iso: today.to_string(),
    };
    Ok(render(&tpl)?.into_response())
}

pub async fn today(State(state): State<AppState>, session: Session) -> AppResult<Response> {
    let Some(user) = require_user(&session).await? else {
        return Ok(Redirect::to("/login").into_response());
    };

    let today = OffsetDateTime::now_utc().date();

    // Cache-then-generate. If the model call fails (e.g. Ollama offline) we return an error.
    let (title, body, cast_ids, model) = if let Some(cached) =
        story_repo::get(&state.db, today).await?
    {
        (cached.title, cached.body, cached.cast, cached.model)
    } else {
        tracing::info!(date = %today, "no cached story; generating");
        let generated = state
            .stories
            .generate_for(today)
            .await
            .map_err(AppError::Internal)?;
        story_repo::put(&state.db, today, &generated).await?;
        (
            generated.title,
            generated.body,
            generated.cast,
            generated.model,
        )
    };

    let cast_names = cast_ids
        .iter()
        .filter_map(|id| state.cast.get(id).map(|c| c.name.clone()))
        .collect();

    let body_paragraphs = body
        .split("\n\n")
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();

    let tpl = StoryTemplate {
        display_name: user.display_name,
        csrf_token: csrf::token(&session).await?,
        title,
        body_paragraphs,
        cast_names,
        date_display: today.to_string(),
        model,
    };
    Ok(render(&tpl)?.into_response())
}

async fn require_user(session: &Session) -> AppResult<Option<SessionUser>> {
    session
        .get::<SessionUser>(SESSION_USER_KEY)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("session get user: {e}")))
}

fn render<T: Template>(tpl: &T) -> AppResult<Html<String>> {
    let body = tpl
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("template render: {e}")))?;
    Ok(Html(body))
}
