//! Character listing + detail pages. Route paths remain `/council` and
//! `/council/{id}` because "the Council" is Lennon's in-world label.

use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use tower_sessions::Session;

use crate::cast::{CastRegistry, Character};
use crate::error::{AppError, AppResult};
use crate::routes::require_user;
use crate::state::AppState;
use crate::web::portrait::{self, CharacterPortrait};

#[derive(Template)]
#[template(path = "council.html")]
struct CouncilTemplate<'a> {
    characters: Vec<CharacterPortrait<'a>>,
}

#[derive(Template)]
#[template(path = "character.html")]
struct CharacterTemplate<'a> {
    character: &'a Character,
    /// Relationships resolved to the target's display name (+ id for the
    /// link). Built here because the template only holds one `Character` and
    /// can't reach the registry to turn a `with` id into a name.
    relationships: Vec<RelationshipView>,
    image_src: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct RelationshipView {
    id: String,
    name: String,
    bond: String,
}

pub async fn list_characters(
    State(state): State<AppState>,
    session: Session,
) -> AppResult<Response> {
    if require_user(&state.access, &session).await?.is_none() {
        return Ok(Redirect::to("/login").into_response());
    }

    let mut characters: Vec<CharacterPortrait<'_>> =
        state.cast.all().map(portrait::for_character).collect();
    characters.sort_by(|a, b| a.character.name.cmp(&b.character.name));

    let tpl = CouncilTemplate { characters };
    Ok(render(&tpl)?.into_response())
}

pub async fn show_character(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> AppResult<Response> {
    if require_user(&state.access, &session).await?.is_none() {
        return Ok(Redirect::to("/login").into_response());
    }

    let character = state.cast.get(&id).ok_or(AppError::NotFound)?;

    let relationships = relationship_views(character, &state.cast);
    let tpl = CharacterTemplate {
        character,
        relationships,
        image_src: portrait::for_character(character).image_src,
    };
    Ok(render(&tpl)?.into_response())
}

fn relationship_views(character: &Character, cast: &CastRegistry) -> Vec<RelationshipView> {
    character
        .relationships
        .iter()
        .map(|r| RelationshipView {
            id: r.with.clone(),
            name: cast
                .get(&r.with)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| r.with.clone()),
            bond: r.bond.clone(),
        })
        .collect()
}

fn render<T: Template>(tpl: &T) -> AppResult<Html<String>> {
    let body = tpl
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("template render: {e}")))?;
    Ok(Html(body))
}

#[cfg(test)]
mod tests {
    // Relationship resolution and template projection. Candidate-art discovery
    // moved to `web::candidates` and is tested there.
    use crate::cast::Relationship;

    use super::*;

    fn build_character() -> Character {
        Character {
            id: "ruff-ruff".into(),
            name: "Ruff Ruff".into(),
            species: "plush dog".into(),
            title: "The OG".into(),
            kind: "stuffy".into(),
            image: Some("ruff-ruff.png".into()),
            color_palette: Vec::new(),
            traits: Vec::new(),
            speech_style: "Confident".into(),
            fears: Vec::new(),
            loves: Vec::new(),
            catchphrase: None,
            role: "self-declared leader".into(),
            faction: Some("The OG".into()),
            faction_role: Some("leader".into()),
            on_council: false,
            relationships: Vec::new(),
            lore: None,
        }
    }

    #[test]
    fn relationship_views_known_target_uses_display_name() {
        let cast = CastRegistry::load_from_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("cast"),
        )
        .expect("load committed cast");
        let mut character = build_character();
        character.relationships = vec![Relationship {
            with: "lennon".into(),
            bond: "best friends".into(),
        }];

        let views = relationship_views(&character, &cast);

        assert_eq!(
            views,
            vec![RelationshipView {
                id: "lennon".into(),
                name: "Lennon".into(),
                bond: "best friends".into(),
            }]
        );
    }

    #[test]
    fn relationship_views_missing_target_uses_stable_id_as_name() {
        let cast = CastRegistry::default();
        let mut character = build_character();
        character.relationships = vec![Relationship {
            with: "missing-friend".into(),
            bond: "mysterious acquaintance".into(),
        }];

        let views = relationship_views(&character, &cast);

        assert_eq!(
            views,
            vec![RelationshipView {
                id: "missing-friend".into(),
                name: "missing-friend".into(),
                bond: "mysterious acquaintance".into(),
            }]
        );
    }

    /// Candidate art is unapproved and admin-only; the public character page
    /// must never surface it, whatever is sitting in the review directory.
    #[test]
    fn character_template_never_renders_candidate_art() {
        let character = build_character();
        let template = CharacterTemplate {
            character: &character,
            relationships: Vec::new(),
            image_src: Some("/static/stuffies/ruff-ruff.png".into()),
        };

        let body = template.render().expect("render character template");

        assert!(!body.contains("art-candidates-heading"));
        assert!(!body.contains("sc-candidate-grid"));
        assert!(!body.contains("/static/stuffies/review/"));
        assert!(body.contains("src=\"/static/stuffies/ruff-ruff.png\" alt=\"Ruff Ruff\""));
    }

    #[test]
    fn character_template_without_canonical_portrait_falls_back() {
        let character = build_character();
        let template = CharacterTemplate {
            character: &character,
            relationships: Vec::new(),
            image_src: None,
        };

        let body = template.render().expect("render character template");

        assert!(body.contains("sc-portrait__ph"));
    }

    #[test]
    fn council_template_renders_canonical_and_fallback_portraits() {
        let canonical = build_character();
        let mut fallback = build_character();
        fallback.id = "woofy".into();
        fallback.name = "Woofy".into();
        fallback.image = Some("woofy.png".into());
        let template = CouncilTemplate {
            characters: vec![
                CharacterPortrait {
                    character: &canonical,
                    image_src: Some("/static/stuffies/ruff-ruff.png".into()),
                },
                CharacterPortrait {
                    character: &fallback,
                    image_src: None,
                },
            ],
        };

        let body = template.render().expect("render council template");

        assert!(body.contains("src=\"/static/stuffies/ruff-ruff.png\" alt=\"\""));
        assert!(body.contains("Open Ruff Ruff&rsquo;s page"));
        assert!(body.contains("Open Woofy&rsquo;s page"));
        assert_eq!(body.matches("sc-portrait__ph").count(), 1);
    }
}
