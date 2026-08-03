//! Shared view model for `templates/story.html`.
//!
//! Both the live `/story/today` path and the archived `/story/{date}` path
//! render the same page, so the paragraph split and cast-name resolution live
//! here rather than being duplicated per handler.

use askama::Template;
use time::Date;

use crate::cast::CastRegistry;

#[derive(Template)]
#[template(path = "story.html")]
pub(crate) struct StoryTemplate {
    pub csrf_token: String,
    pub title: String,
    pub is_unavailable: bool,
    pub body_paragraphs: Vec<String>,
    pub cast_names: Vec<String>,
    pub date_display: String,
    pub model: String,
}

impl StoryTemplate {
    pub(crate) fn from_story(
        csrf_token: String,
        date: Date,
        title: String,
        body: &str,
        cast_ids: &[String],
        model: String,
        cast: &CastRegistry,
    ) -> Self {
        Self {
            csrf_token,
            title,
            is_unavailable: false,
            body_paragraphs: paragraphs(body),
            // Ids that no longer resolve are dropped: a renamed cast file is a
            // data migration (AGENTS.md ground rule 8) and historical rows keep
            // the old id, which must not break rendering the archive.
            cast_names: cast_ids
                .iter()
                .filter_map(|id| cast.get(id).map(|c| c.name.clone()))
                .collect(),
            date_display: date.to_string(),
            model,
        }
    }

    pub(crate) fn unavailable(csrf_token: String, date: Date, title: &str, message: &str) -> Self {
        Self {
            csrf_token,
            title: title.to_string(),
            is_unavailable: true,
            body_paragraphs: vec![message.to_string()],
            cast_names: Vec::new(),
            date_display: date.to_string(),
            model: String::new(),
        }
    }
}

/// Model output is untrusted (AGENTS.md ground rule 5); it is split on blank
/// lines and rendered as text, never as markup.
fn paragraphs(body: &str) -> Vec<String> {
    body.split("\n\n")
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    // Functional and edge dimensions for the projection. Negative/error and
    // state-transition are N/A: this is a total, stateless value mapping.
    use super::*;

    fn cast() -> CastRegistry {
        CastRegistry::load_from_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("cast"))
            .expect("load current cast")
    }

    fn date() -> Date {
        Date::from_calendar_date(2026, time::Month::July, 31).expect("date")
    }

    #[test]
    fn paragraphs_splits_on_blank_lines_and_drops_whitespace_only_entries() {
        assert_eq!(
            paragraphs("One.\n\nTwo.\n\n   \n\n  Three.  "),
            vec!["One.", "Two.", "Three."]
        );
    }

    #[test]
    fn paragraphs_of_blank_body_is_empty_rather_than_one_empty_paragraph() {
        assert!(paragraphs("   \n\n  ").is_empty());
    }

    #[test]
    fn from_story_resolves_cast_ids_to_display_names() {
        let tpl = StoryTemplate::from_story(
            "t".into(),
            date(),
            "T".into(),
            "Body.",
            &["ruff-ruff".to_string(), "woofy".to_string()],
            "m".into(),
            &cast(),
        );

        assert_eq!(tpl.cast_names, vec!["Ruff Ruff", "Woofy"]);
        assert!(!tpl.is_unavailable);
        assert_eq!(tpl.date_display, "2026-07-31");
    }

    #[test]
    fn from_story_drops_cast_ids_that_no_longer_exist() {
        let tpl = StoryTemplate::from_story(
            "t".into(),
            date(),
            "T".into(),
            "Body.",
            &["ruff-ruff".to_string(), "retired-stuffy".to_string()],
            "m".into(),
            &cast(),
        );

        assert_eq!(
            tpl.cast_names,
            vec!["Ruff Ruff"],
            "a renamed or removed cast file must not break archived stories"
        );
    }

    #[test]
    fn unavailable_carries_no_cast_or_model() {
        let tpl = StoryTemplate::unavailable("t".into(), date(), "Not ready", "Try later.");

        assert!(tpl.is_unavailable);
        assert!(tpl.cast_names.is_empty());
        assert!(tpl.model.is_empty());
        assert_eq!(tpl.body_paragraphs, vec!["Try later."]);
    }
}
