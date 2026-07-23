//! Home + today's story routes. All routes here require a logged-in user.

use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Redirect, Response};
use time::OffsetDateTime;
use tower_sessions::Session;

use crate::auth::{SESSION_USER_KEY, SessionUser};
use crate::cast::CastRegistry;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::stories::StoryGenerationError;
use crate::story_repo;
use crate::web::csrf;
use crate::web::portrait::{self, CharacterPortrait};

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate<'a> {
    display_name: String,
    csrf_token: String,
    has_today: bool,
    today_title: String,
    today_iso: String,
    spotlight: Vec<CharacterPortrait<'a>>,
}

#[derive(Template)]
#[template(path = "story.html")]
struct StoryTemplate {
    csrf_token: String,
    title: String,
    is_unavailable: bool,
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

    let spotlight = landing_spotlight(&state.cast);

    let tpl = HomeTemplate {
        display_name: user.display_name,
        csrf_token: csrf::token(&session).await?,
        has_today: cached.is_some(),
        today_title: cached.as_ref().map(|c| c.title.clone()).unwrap_or_default(),
        today_iso: today.to_string(),
        spotlight,
    };
    Ok(render(&tpl)?.into_response())
}

fn landing_spotlight(cast: &CastRegistry) -> Vec<CharacterPortrait<'_>> {
    let mut spotlight: Vec<_> = cast.all().map(portrait::for_character).collect();
    spotlight.sort_by(|left, right| left.character.name.cmp(&right.character.name));
    spotlight
}

pub async fn today(State(state): State<AppState>, session: Session) -> AppResult<Response> {
    if require_user(&session).await?.is_none() {
        return Ok(Redirect::to("/login").into_response());
    }

    let today = OffsetDateTime::now_utc().date();

    // Cache-then-generate. Temporary generator outages render an in-page retry
    // state; internal failures still use the generic 500 path.
    let (title, body, cast_ids, model) = if let Some(cached) =
        story_repo::get(&state.db, today).await?
    {
        (cached.title, cached.body, cached.cast, cached.model)
    } else {
        tracing::info!(date = %today, "no cached story; generating");
        let generated = match state.stories.generate_for(today).await {
            Ok(generated) => generated,
            Err(StoryGenerationError::Unavailable(error)) => {
                tracing::warn!(error = ?error, date = %today, "story generator unavailable");
                let tpl = StoryTemplate {
                    csrf_token: csrf::token(&session).await?,
                    title: "Today's story isn't ready yet".into(),
                    is_unavailable: true,
                    body_paragraphs: vec!["The story elf is offline. Try again shortly.".into()],
                    cast_names: Vec::new(),
                    date_display: today.to_string(),
                    model: String::new(),
                };
                return Ok(render(&tpl)?.into_response());
            }
            Err(StoryGenerationError::Internal(error)) => {
                return Err(AppError::Internal(error));
            }
        };
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
        csrf_token: csrf::token(&session).await?,
        title,
        is_unavailable: false,
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

#[cfg(test)]
mod tests {
    // Home portrait rendering is a stateless template projection. Functional,
    // ordering, and fallback branches are covered; negative, dependency-error,
    // and state-transition dimensions belong to portrait::canonical_src tests.
    use crate::cast::{CastRegistry, Character};

    use super::*;

    fn current_cast() -> CastRegistry {
        CastRegistry::load_from_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("cast"))
            .expect("load current cast")
    }

    #[test]
    fn landing_spotlight_current_cast_includes_off_council_character_sorted_by_name() {
        let cast = current_cast();

        let spotlight = landing_spotlight(&cast);
        let names: Vec<_> = spotlight
            .iter()
            .map(|item| item.character.name.as_str())
            .collect();
        let ruff_ruff = spotlight
            .iter()
            .find(|item| item.character.id == "ruff-ruff")
            .expect("Ruff Ruff in landing spotlight");

        assert_eq!(
            names,
            vec!["Bar Bar", "Dad", "Lennon", "Ruff Ruff", "Woofy"]
        );
        assert!(!ruff_ruff.character.on_council);
    }

    #[test]
    fn landing_spotlight_current_stuffies_use_canonical_portraits() {
        let cast = current_cast();

        let spotlight = landing_spotlight(&cast);
        let canonical_portraits: Vec<_> = spotlight
            .iter()
            .filter_map(|item| {
                item.image_src
                    .as_deref()
                    .map(|src| (item.character.id.as_str(), src))
            })
            .collect();

        assert_eq!(
            canonical_portraits,
            vec![
                ("bar-bar", "/static/stuffies/bar-bar.png"),
                ("ruff-ruff", "/static/stuffies/ruff-ruff.png"),
                ("woofy", "/static/stuffies/woofy.png"),
            ]
        );
    }

    #[test]
    fn landing_spotlight_humans_without_canonical_art_use_silhouette_fallbacks() {
        let cast = current_cast();

        let spotlight = landing_spotlight(&cast);
        let fallback_ids: Vec<_> = spotlight
            .iter()
            .filter(|item| item.image_src.is_none())
            .map(|item| item.character.id.as_str())
            .collect();

        assert_eq!(fallback_ids, vec!["dad", "lennon"]);
    }

    fn character() -> Character {
        Character {
            id: "woofy".into(),
            name: "Woofy".into(),
            species: "plush wolf".into(),
            title: "President of the Universe".into(),
            kind: "stuffy".into(),
            image: Some("woofy.png".into()),
            color_palette: Vec::new(),
            traits: Vec::new(),
            speech_style: "Hums".into(),
            fears: Vec::new(),
            loves: Vec::new(),
            catchphrase: None,
            role: "council co-president".into(),
            faction: Some("Avocatts".into()),
            faction_role: Some("leader".into()),
            on_council: true,
            relationships: Vec::new(),
            lore: None,
        }
    }

    #[test]
    fn home_template_spotlight_renders_canonical_portrait() {
        let character = character();
        let template = HomeTemplate {
            display_name: "Lennon".into(),
            csrf_token: "token".into(),
            has_today: false,
            today_title: String::new(),
            today_iso: "2026-07-16".into(),
            spotlight: vec![CharacterPortrait {
                character: &character,
                image_src: Some("/static/stuffies/woofy.png".into()),
            }],
        };

        let body = template.render().expect("render home template");

        assert!(body.contains("src=\"/static/stuffies/woofy.png\" alt=\"\""));
        assert!(!body.contains("sc-portrait__ph"));
    }

    #[test]
    fn home_template_spotlight_without_image_renders_fallback() {
        let character = character();
        let template = HomeTemplate {
            display_name: "Lennon".into(),
            csrf_token: "token".into(),
            has_today: false,
            today_title: String::new(),
            today_iso: "2026-07-16".into(),
            spotlight: vec![CharacterPortrait {
                character: &character,
                image_src: None,
            }],
        };

        let body = template.render().expect("render home template");

        assert!(body.contains("sc-portrait__ph"));
        assert!(!body.contains("/static/stuffies/woofy.png"));
    }
}
