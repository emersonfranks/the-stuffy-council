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
    characters: Vec<&'a Character>,
}

#[derive(Template)]
#[template(path = "character.html")]
struct CharacterTemplate<'a> {
    character: &'a Character,
    /// Relationships resolved to the target's display name (+ id for the
    /// link). Built here because the template only holds one `Character` and
    /// can't reach the registry to turn a `with` id into a name.
    relationships: Vec<RelationshipView>,
}

struct RelationshipView {
    id: String,
    name: String,
    bond: String,
}

pub async fn list_characters(
    State(state): State<AppState>,
    session: Session,
) -> AppResult<Response> {
    if require_user(&session).await?.is_none() {
        return Ok(Redirect::to("/login").into_response());
    }

    let mut characters: Vec<&Character> = state.cast.all().collect();
    characters.sort_by(|a, b| a.name.cmp(&b.name));

    let tpl = CouncilTemplate { characters };
    Ok(render(&tpl)?.into_response())
}

pub async fn show_character(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> AppResult<Response> {
    if require_user(&session).await?.is_none() {
        return Ok(Redirect::to("/login").into_response());
    }

    let character = state.cast.get(&id).ok_or(AppError::NotFound)?;

    let relationships = character
        .relationships
        .iter()
        .map(|r| RelationshipView {
            id: r.with.clone(),
            name: state
                .cast
                .get(&r.with)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| r.with.clone()),
            bond: r.bond.clone(),
        })
        .collect();

    let tpl = CharacterTemplate {
        character,
        relationships,
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
