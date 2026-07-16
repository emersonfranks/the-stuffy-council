use std::path::Path;

use crate::cast::Character;

const PORTRAIT_DIR: &str = "static/stuffies";

pub(crate) struct CharacterPortrait<'a> {
    pub character: &'a Character,
    pub image_src: Option<String>,
}

pub(crate) fn for_character(character: &Character) -> CharacterPortrait<'_> {
    CharacterPortrait {
        character,
        image_src: canonical_src(Path::new(PORTRAIT_DIR), character),
    }
}

fn canonical_src(portrait_dir: &Path, character: &Character) -> Option<String> {
    let expected_name = format!("{}.png", character.id);
    if character.image.as_deref() != Some(expected_name.as_str()) {
        return None;
    }
    portrait_dir
        .join(&expected_name)
        .is_file()
        .then(|| format!("/static/stuffies/{expected_name}"))
}

#[cfg(test)]
mod tests {
    // Portrait resolution is stateless filesystem projection. Functional,
    // edge, and negative dimensions are covered. Dependency errors collapse
    // to the documented silhouette fallback; state-transition is N/A.
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn character(image: Option<&str>) -> Character {
        Character {
            id: "ruff-ruff".into(),
            name: "Ruff Ruff".into(),
            species: "plush Pog".into(),
            title: "The OG".into(),
            kind: "stuffy".into(),
            image: image.map(str::to_owned),
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
    fn canonical_src_matching_declared_file_returns_public_url() {
        let temp = tempdir().expect("temp dir");
        fs::write(temp.path().join("ruff-ruff.png"), b"png").expect("write portrait");

        let src = canonical_src(temp.path(), &character(Some("ruff-ruff.png")));

        assert_eq!(src.as_deref(), Some("/static/stuffies/ruff-ruff.png"));
    }

    #[test]
    fn canonical_src_missing_file_returns_silhouette_fallback() {
        let temp = tempdir().expect("temp dir");

        let src = canonical_src(temp.path(), &character(Some("ruff-ruff.png")));

        assert_eq!(src, None);
    }

    #[test]
    fn canonical_src_noncanonical_declaration_returns_silhouette_fallback() {
        let temp = tempdir().expect("temp dir");
        fs::write(temp.path().join("other.png"), b"png").expect("write other image");

        let src = canonical_src(temp.path(), &character(Some("other.png")));

        assert_eq!(src, None);
    }

    #[test]
    fn canonical_src_without_declaration_returns_silhouette_fallback() {
        let temp = tempdir().expect("temp dir");
        fs::write(temp.path().join("ruff-ruff.png"), b"png").expect("write portrait");

        let src = canonical_src(temp.path(), &character(None));

        assert_eq!(src, None);
    }
}