//! Character listing + detail pages. Route paths remain `/council` and
//! `/council/{id}` because "the Council" is Lennon's in-world label.

use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use tower_sessions::Session;

use crate::auth::{SESSION_USER_KEY, SessionUser};
use crate::cast::Character;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Template)]
#[template(path = "council.html")]
struct CouncilTemplate<'a> {
    display_name: String,
    characters: Vec<&'a Character>,
}

#[derive(Template)]
#[template(path = "character.html")]
struct CharacterTemplate<'a> {
    display_name: String,
    character: &'a Character,
}

pub async fn list_characters(
    State(state): State<AppState>,
    session: Session,
) -> AppResult<Response> {
    let Some(user) = require_user(&session).await? else {
        return Ok(Redirect::to("/login").into_response());
    };

    let mut characters: Vec<&Character> = state.cast.all().collect();
    characters.sort_by(|a, b| a.name.cmp(&b.name));

    let tpl = CouncilTemplate {
        display_name: user.display_name,
        characters,
    };
    Ok(render(&tpl)?.into_response())
}

pub async fn show_character(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let Some(user) = require_user(&session).await? else {
        return Ok(Redirect::to("/login").into_response());
    };

    let character = state.cast.get(&id).ok_or(AppError::NotFound)?;

    let tpl = CharacterTemplate {
        display_name: user.display_name,
        character,
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
