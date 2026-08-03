//! Discovery of unreviewed character art.
//!
//! Candidates live OUTSIDE `static/` on purpose: that directory is mounted with
//! `ServeDir` and is NOT authenticated, so anything under it is anonymously
//! downloadable whether or not a template links it. Drafts are served instead
//! through the admin-gated `/admin/candidates/{file}` route.

use std::path::{Path as FsPath, PathBuf};

use anyhow::Context;

/// Where candidate art is dropped for review. Files are named
/// `<character-id>--candidate-<label-slug>.png`; anything else is ignored.
/// Must never move under `static/` — see the module note.
pub const IMAGE_CANDIDATE_DIR: &str = "art-review";

/// Resolve a requested candidate file name to a path on disk.
///
/// `None` unless the name matches the candidate pattern exactly, which rejects
/// traversal because the pattern admits no separators and no dots beyond the
/// `.png` suffix. The result must also be a regular file directly inside
/// `review_dir`.
pub fn candidate_path(review_dir: &FsPath, file_name: &str) -> Option<PathBuf> {
    let (character_id, label_slug) = file_name.strip_suffix(".png")?.split_once("--candidate-")?;
    if !valid_slug(character_id) || !valid_slug(label_slug) {
        return None;
    }

    let path = review_dir.join(file_name);
    if path.file_name()? != std::ffi::OsStr::new(file_name) || !path.is_file() {
        return None;
    }
    Some(path)
}

#[derive(Debug, PartialEq, Eq)]
pub struct ImageCandidate {
    pub src: String,
    pub label: String,
}

pub fn load_image_candidates(
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
            src: format!("/admin/candidates/{file_name}"),
            label: candidate_label(label_slug),
        });
    }

    candidates.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(candidates)
}

fn valid_candidate_label_slug(label_slug: &str) -> bool {
    valid_slug(label_slug)
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.split('-').all(|part| {
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
                    src: "/admin/candidates/ruff-ruff--candidate-clean.png".into(),
                    label: "Clean".into(),
                },
                ImageCandidate {
                    src: "/admin/candidates/ruff-ruff--candidate-well-loved.png".into(),
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
                    src: "/admin/candidates/woofy--candidate-alpha.png".into(),
                    label: "Alpha".into(),
                },
                ImageCandidate {
                    src: "/admin/candidates/woofy--candidate-beta.png".into(),
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
                src: "/admin/candidates/ruff-ruff--candidate-v2.png".into(),
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
    fn candidate_path_accepts_a_real_candidate_file() {
        let temp = tempdir().expect("temp dir");
        touch(&temp.path().join("ruff-ruff--candidate-clean.png"));

        let resolved = candidate_path(temp.path(), "ruff-ruff--candidate-clean.png");

        assert_eq!(
            resolved,
            Some(temp.path().join("ruff-ruff--candidate-clean.png"))
        );
    }

    #[test]
    fn candidate_path_rejects_traversal_and_non_candidate_names() {
        let temp = tempdir().expect("temp dir");
        touch(&temp.path().join("ruff-ruff--candidate-clean.png"));
        touch(&temp.path().join("secret.png"));
        std::fs::create_dir(temp.path().join("nested")).expect("nested dir");
        touch(&temp.path().join("nested/ruff-ruff--candidate-clean.png"));

        for hostile in [
            "../secret.png",
            "..\\secret.png",
            "nested/ruff-ruff--candidate-clean.png",
            "/etc/passwd",
            "secret.png",
            "ruff-ruff--candidate-clean.png.exe",
            "ruff-ruff--candidate-WIP.png",
            "ruff-ruff--candidate-.png",
            "--candidate-clean.png",
        ] {
            assert_eq!(
                candidate_path(temp.path(), hostile),
                None,
                "`{hostile}` must not resolve to a servable file"
            );
        }
    }

    #[test]
    fn candidate_path_rejects_a_well_formed_name_that_does_not_exist() {
        let temp = tempdir().expect("temp dir");

        assert_eq!(
            candidate_path(temp.path(), "woofy--candidate-ghost.png"),
            None
        );
    }

    #[test]
    fn discovered_candidates_are_served_through_the_admin_route_not_static() {
        let temp = tempdir().expect("temp dir");
        touch(&temp.path().join("woofy--candidate-alpha.png"));

        let candidates = load_image_candidates(temp.path(), "woofy").expect("load candidates");

        assert!(candidates[0].src.starts_with("/admin/candidates/"));
        assert!(
            !candidates[0].src.contains("/static/"),
            "candidate art must never be addressed through the unauthenticated static mount"
        );
    }

    /// Guards the reason candidates moved: `static/` is mounted with `ServeDir`
    /// and is not authenticated, so anything under it is anonymously fetchable.
    #[test]
    fn review_dir_stays_outside_the_unauthenticated_static_mount() {
        assert!(
            !IMAGE_CANDIDATE_DIR.starts_with("static"),
            "moving candidates back under static/ would expose them without auth"
        );
    }
}
