//! Character listing + detail pages. Route paths remain `/council` and
//! `/council/{id}` because "the Council" is Lennon's in-world label.

use std::path::Path as FsPath;

use anyhow::Context;
use askama::Template;
use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use tower_sessions::Session;

use crate::auth::{SESSION_USER_KEY, SessionUser};
use crate::cast::Character;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::web::portrait::{self, CharacterPortrait};

const IMAGE_CANDIDATE_DIR: &str = "static/stuffies/review";

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
    image_candidates: Vec<ImageCandidate>,
    image_src: Option<String>,
}

struct RelationshipView {
    id: String,
    name: String,
    bond: String,
}

#[derive(Debug, PartialEq, Eq)]
struct ImageCandidate {
    src: String,
    label: String,
}

pub async fn list_characters(
    State(state): State<AppState>,
    session: Session,
) -> AppResult<Response> {
    if require_user(&session).await?.is_none() {
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
    let image_candidates = load_image_candidates(FsPath::new(IMAGE_CANDIDATE_DIR), &character.id)?;

    let tpl = CharacterTemplate {
        character,
        relationships,
        image_candidates,
        image_src: portrait::for_character(character).image_src,
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

fn load_image_candidates(
    review_dir: &FsPath,
    character_id: &str,
) -> anyhow::Result<Vec<ImageCandidate>> {
    let entries = match std::fs::read_dir(review_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!("reading candidate image directory {}", review_dir.display())
            });
        }
    };
    let prefix = format!("{character_id}--candidate-");
    let mut candidates = Vec::new();

    for entry in entries {
        let entry = entry.with_context(|| {
            format!("reading candidate image entry in {}", review_dir.display())
        })?;
        if !entry
            .file_type()
            .with_context(|| {
                format!(
                    "reading candidate image type for {}",
                    entry.path().display()
                )
            })?
            .is_file()
        {
            continue;
        }

        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let Some(label_slug) = file_name
            .strip_prefix(&prefix)
            .and_then(|name| name.strip_suffix(".png"))
        else {
            continue;
        };
        if !valid_candidate_label_slug(label_slug) {
            continue;
        }

        candidates.push(ImageCandidate {
            src: format!("/static/stuffies/review/{file_name}"),
            label: candidate_label(label_slug),
        });
    }

    candidates.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(candidates)
}

fn valid_candidate_label_slug(label_slug: &str) -> bool {
    label_slug.split('-').all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
    })
}

fn candidate_label(label_slug: &str) -> String {
    label_slug
        .split('-')
        .map(|word| {
            let mut characters = word.chars();
            let first = characters
                .next()
                .expect("candidate label words are non-empty");
            format!("{}{}", first.to_ascii_uppercase(), characters.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    // Candidate discovery is stateless filesystem projection. Functional,
    // edge, negative, and dependency-error dimensions are covered below.
    // State-transition is N/A because discovery never mutates files or memory.
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn touch(path: &FsPath) {
        fs::write(path, []).expect("write candidate fixture");
    }

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
    fn load_image_candidates_matching_files_returns_sorted_labels_and_urls() {
        let temp = tempdir().expect("temp dir");
        touch(&temp.path().join("ruff-ruff--candidate-well-loved.png"));
        touch(&temp.path().join("ruff-ruff--candidate-clean.png"));
        touch(&temp.path().join("woofy--candidate-clean.png"));
        touch(&temp.path().join("ruff-ruff--candidate-draft.jpg"));
        fs::create_dir(temp.path().join("ruff-ruff--candidate-directory.png"))
            .expect("candidate-like directory");

        let candidates = load_image_candidates(temp.path(), "ruff-ruff").expect("load candidates");

        assert_eq!(
            candidates,
            vec![
                ImageCandidate {
                    src: "/static/stuffies/review/ruff-ruff--candidate-clean.png".into(),
                    label: "Clean".into(),
                },
                ImageCandidate {
                    src: "/static/stuffies/review/ruff-ruff--candidate-well-loved.png".into(),
                    label: "Well Loved".into(),
                },
            ]
        );
    }

    #[test]
    fn load_image_candidates_returns_requested_character_and_ignores_other_characters() {
        let temp = tempdir().expect("temp dir");
        touch(&temp.path().join("woofy--candidate-alpha.png"));
        touch(&temp.path().join("woofy--candidate-beta.png"));
        touch(&temp.path().join("bar-bar--candidate-alpha.png"));

        let candidates = load_image_candidates(temp.path(), "woofy").expect("load candidates");

        assert_eq!(
            candidates,
            vec![
                ImageCandidate {
                    src: "/static/stuffies/review/woofy--candidate-alpha.png".into(),
                    label: "Alpha".into(),
                },
                ImageCandidate {
                    src: "/static/stuffies/review/woofy--candidate-beta.png".into(),
                    label: "Beta".into(),
                },
            ]
        );
    }

    #[test]
    fn load_image_candidates_missing_directory_returns_empty() {
        let temp = tempdir().expect("temp dir");
        let missing = temp.path().join("missing");

        let candidates = load_image_candidates(&missing, "ruff-ruff").expect("missing is empty");

        assert!(candidates.is_empty());
    }

    #[test]
    fn load_image_candidates_invalid_label_slugs_are_ignored() {
        let temp = tempdir().expect("temp dir");
        touch(&temp.path().join("ruff-ruff--candidate-.png"));
        touch(&temp.path().join("ruff-ruff--candidate-WIP.png"));
        touch(&temp.path().join("ruff-ruff--candidate-well--loved.png"));
        touch(&temp.path().join("ruff-ruff--candidate-v2.png"));

        let candidates = load_image_candidates(temp.path(), "ruff-ruff").expect("load candidates");

        assert_eq!(
            candidates,
            vec![ImageCandidate {
                src: "/static/stuffies/review/ruff-ruff--candidate-v2.png".into(),
                label: "V2".into(),
            }]
        );
    }

    #[test]
    fn load_image_candidates_path_is_file_returns_contextual_error() {
        let temp = tempdir().expect("temp dir");
        let file = temp.path().join("not-a-directory");
        touch(&file);

        let error = load_image_candidates(&file, "ruff-ruff").expect_err("file is not directory");

        assert!(
            error
                .to_string()
                .contains("reading candidate image directory"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn character_template_with_candidates_renders_review_gallery() {
        let character = build_character();
        let template = CharacterTemplate {
            character: &character,
            relationships: Vec::new(),
            image_candidates: vec![
                ImageCandidate {
                    src: "/static/stuffies/review/ruff-ruff--candidate-clean.png".into(),
                    label: "Clean".into(),
                },
                ImageCandidate {
                    src: "/static/stuffies/review/ruff-ruff--candidate-well-loved.png".into(),
                    label: "Well Loved".into(),
                },
            ],
            image_src: Some("/static/stuffies/ruff-ruff.png".into()),
        };

        let body = template.render().expect("render character template");

        assert!(body.contains("id=\"art-candidates-heading\""));
        assert!(body.contains("src=\"/static/stuffies/review/ruff-ruff--candidate-clean.png\""));
        assert!(body.contains("alt=\"Ruff Ruff art candidate: Well Loved\""));
        assert!(body.contains("src=\"/static/stuffies/ruff-ruff.png\" alt=\"Ruff Ruff\""));
    }

    #[test]
    fn character_template_without_candidates_omits_review_gallery() {
        let character = build_character();
        let template = CharacterTemplate {
            character: &character,
            relationships: Vec::new(),
            image_candidates: Vec::new(),
            image_src: None,
        };

        let body = template.render().expect("render character template");

        assert!(!body.contains("art-candidates-heading"));
        assert!(!body.contains("sc-candidate-grid"));
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
