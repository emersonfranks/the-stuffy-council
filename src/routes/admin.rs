//! Admin-only surfaces: the review dashboard and the archive backup.
//!
//! Read-only by design. Cast and allowlist changes go through a PR, so nothing
//! here writes to `cast/*.toml` or `authorized-users.toml` — a web handler
//! mutating repo-tracked files would be validated only at boot, and the
//! container filesystem is ephemeral in production anyway.

use std::path::Path as FsPath;

use anyhow::Context;
use askama::Template;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{Html, IntoResponse, Redirect, Response};
use tower_sessions::Session;

use crate::error::{AppError, AppResult};
use crate::routes::require_admin;
use crate::state::AppState;
use crate::story_repo::{self, ExportedStory};
use crate::user_repo;
use crate::web::candidates::{self, IMAGE_CANDIDATE_DIR, ImageCandidate};
use crate::web::csrf;

/// Enough history to see who has been around lately without paginating.
const RECENT_LOGIN_LIMIT: i64 = 50;

#[derive(Template)]
#[template(path = "admin.html")]
struct AdminTemplate {
    csrf_token: String,
    characters: Vec<CharacterFacts>,
    allowed: Vec<AllowedUserView>,
    logins: Vec<LoginView>,
}

struct CharacterFacts {
    id: String,
    name: String,
    kind: String,
    species: String,
    title: String,
    role: String,
    faction: String,
    on_council: bool,
    speech_style: String,
    catchphrase: String,
    traits: Vec<String>,
    loves: Vec<String>,
    fears: Vec<String>,
    relationships: Vec<String>,
    lore: String,
    portrait: Option<String>,
    candidates: Vec<ImageCandidate>,
}

struct AllowedUserView {
    email: String,
    admin: bool,
}

struct LoginView {
    email: String,
    display_name: String,
    last_login_at: String,
}

pub async fn dashboard(State(state): State<AppState>, session: Session) -> AppResult<Response> {
    if require_admin(&state.access, &session).await?.is_none() {
        return Ok(Redirect::to("/login").into_response());
    }

    let review_dir = FsPath::new(IMAGE_CANDIDATE_DIR);
    let mut characters: Vec<CharacterFacts> = Vec::new();
    for character in state.cast.all() {
        characters.push(CharacterFacts {
            id: character.id.clone(),
            name: character.name.clone(),
            kind: character.kind.clone(),
            species: character.species.clone(),
            title: character.title.clone(),
            role: character.role.clone(),
            faction: character.faction.clone().unwrap_or_default(),
            on_council: character.on_council,
            speech_style: character.speech_style.clone(),
            catchphrase: character.catchphrase.clone().unwrap_or_default(),
            traits: character.traits.clone(),
            loves: character.loves.clone(),
            fears: character.fears.clone(),
            relationships: character
                .relationships
                .iter()
                .map(|r| {
                    let name = state
                        .cast
                        .get(&r.with)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| r.with.clone());
                    format!("{name} — {}", r.bond)
                })
                .collect(),
            lore: character.lore.clone().unwrap_or_default(),
            portrait: crate::web::portrait::for_character(character).image_src,
            candidates: candidates::load_image_candidates(review_dir, &character.id)?,
        });
    }
    characters.sort_by(|left, right| left.name.cmp(&right.name));

    let allowed = state
        .access
        .entries()
        .map(|(email, user)| AllowedUserView {
            email: email.to_string(),
            admin: user.admin,
        })
        .collect();

    let logins = user_repo::recent_logins(&state.db, RECENT_LOGIN_LIMIT)
        .await?
        .into_iter()
        .map(|user| LoginView {
            email: user.email,
            display_name: user.display_name,
            last_login_at: user.last_login_at.unwrap_or_else(|| "never".into()),
        })
        .collect();

    let tpl = AdminTemplate {
        csrf_token: csrf::token(&session).await?,
        characters,
        allowed,
        logins,
    };
    Ok(render(&tpl)?.into_response())
}

/// Serve one unreviewed candidate image.
///
/// These deliberately do not live under the unauthenticated `/static` mount, so
/// this gated route is the only way to fetch them.
pub async fn serve_candidate(
    State(state): State<AppState>,
    session: Session,
    Path(file): Path<String>,
) -> AppResult<Response> {
    if require_admin(&state.access, &session).await?.is_none() {
        return Ok(Redirect::to("/login").into_response());
    }

    let path = candidates::candidate_path(FsPath::new(IMAGE_CANDIDATE_DIR), &file)
        .ok_or(AppError::NotFound)?;
    let bytes = std::fs::read(&path)
        .with_context(|| format!("reading candidate image {}", path.display()))?;

    Ok(([(header::CONTENT_TYPE, "image/png")], bytes).into_response())
}

/// Exists so the archive can be captured before anything that resets the dev
/// database — notably the sqlx checksum failure that follows editing an applied
/// migration, whose only local fix is deleting `stuffy-council.sqlite*`.
pub async fn export_stories(
    State(state): State<AppState>,
    session: Session,
) -> AppResult<Response> {
    if require_admin(&state.access, &session).await?.is_none() {
        return Ok(Redirect::to("/login").into_response());
    }

    let stories: Vec<ExportedStory> = story_repo::export_all(&state.db).await?;
    Ok(Json(stories).into_response())
}

fn render<T: Template>(tpl: &T) -> AppResult<Html<String>> {
    let body = tpl
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("template render: {e}")))?;
    Ok(Html(body))
}

#[cfg(test)]
mod tests {
    // Template projection only; the gate and the DB reads are covered by the
    // full-router tests in tests/router_smoke.rs, and candidate discovery by
    // web::candidates. Nothing here mutates state, so state-transition is N/A.
    use super::*;

    fn facts(candidates: Vec<ImageCandidate>) -> CharacterFacts {
        CharacterFacts {
            id: "ruff-ruff".into(),
            name: "Ruff Ruff".into(),
            kind: "stuffy".into(),
            species: "plush dog".into(),
            title: "The OG".into(),
            role: "self-declared leader".into(),
            faction: String::new(),
            on_council: false,
            speech_style: "Confident".into(),
            catchphrase: String::new(),
            traits: Vec::new(),
            loves: Vec::new(),
            fears: Vec::new(),
            relationships: Vec::new(),
            lore: String::new(),
            portrait: None,
            candidates,
        }
    }

    fn template(characters: Vec<CharacterFacts>, logins: Vec<LoginView>) -> AdminTemplate {
        AdminTemplate {
            csrf_token: "token".into(),
            characters,
            allowed: vec![AllowedUserView {
                email: "boss@example.com".into(),
                admin: true,
            }],
            logins,
        }
    }

    #[test]
    fn admin_template_renders_candidate_art_that_the_public_page_no_longer_shows() {
        let tpl = template(
            vec![facts(vec![ImageCandidate {
                src: "/admin/candidates/ruff-ruff--candidate-clean.png".into(),
                label: "Clean".into(),
            }])],
            Vec::new(),
        );

        let body = tpl.render().expect("render admin template");

        assert!(body.contains("/admin/candidates/ruff-ruff--candidate-clean.png"));
        assert!(body.contains("alt=\"Ruff Ruff art candidate: Clean\""));
        assert!(
            !body.contains("/static/stuffies/review/"),
            "candidate art must not be addressed through the unauthenticated static mount"
        );
    }

    #[test]
    fn admin_template_without_candidates_omits_the_gallery() {
        let tpl = template(vec![facts(Vec::new())], Vec::new());

        let body = tpl.render().expect("render admin template");

        assert!(body.contains("Ruff Ruff"));
        assert!(!body.contains("sc-candidate-grid"));
    }

    #[test]
    fn admin_template_shows_never_signed_in_accounts() {
        let tpl = template(
            Vec::new(),
            vec![LoginView {
                email: "ghost@example.com".into(),
                display_name: "Ghost".into(),
                last_login_at: "never".into(),
            }],
        );

        let body = tpl.render().expect("render admin template");

        assert!(body.contains("ghost@example.com"));
        assert!(body.contains("never"));
    }

    #[test]
    fn admin_template_escapes_character_facts_rather_than_trusting_them() {
        let mut hostile = facts(Vec::new());
        hostile.lore = "<script>alert(1)</script>".into();

        let body = template(vec![hostile], Vec::new())
            .render()
            .expect("render admin template");

        // Asserts the property, not the entity style: Askama emits numeric
        // entities (`&#60;`), so matching `&lt;` would pass for the wrong reason.
        assert!(
            !body.contains("<script"),
            "unescaped markup in body: {body}"
        );
        assert!(
            !body.contains("</script>"),
            "unescaped markup in body: {body}"
        );
        assert!(body.contains("alert(1)"), "lore text should still display");
    }
}
