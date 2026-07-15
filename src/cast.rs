//! Load and validate the character cast from the `cast/` directory.
//! Contains both stuffies (`kind = "stuffy"`) and humans (`kind = "human"`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub with: String,
    pub bond: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    /// Stable id — filename minus `.toml`. Populated by the loader, not from the file.
    #[serde(default)]
    pub id: String,

    pub name: String,
    pub species: String,
    pub title: String,

    /// `"stuffy"` (default) or `"human"`. Humans are the narrative frame (Lennon,
    /// Dad); stuffies rotate through the daily cast.
    #[serde(default = "default_kind")]
    pub kind: String,

    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub color_palette: Vec<String>,

    #[serde(default)]
    pub traits: Vec<String>,
    pub speech_style: String,
    #[serde(default)]
    pub fears: Vec<String>,
    #[serde(default)]
    pub loves: Vec<String>,
    #[serde(default)]
    pub catchphrase: Option<String>,

    pub role: String,

    /// Freeform faction label (`"Avocatts"`, `"TeeTurtles"`, `"The OG"`, ...).
    #[serde(default)]
    pub faction: Option<String>,
    /// `"leader"` or `"member"` within a faction. Rendered into the prompt so
    /// the model knows whose crew is whose.
    #[serde(default)]
    pub faction_role: Option<String>,
    /// Default true. When `false`, `to_prompt_brief` emits an explicit
    /// "NOT on the council" line so the model can play the grievance.
    #[serde(default = "default_on_council")]
    pub on_council: bool,

    #[serde(default)]
    pub relationships: Vec<Relationship>,

    #[serde(default)]
    pub lore: Option<String>,
}

fn default_kind() -> String {
    "stuffy".to_string()
}

fn default_on_council() -> bool {
    true
}

impl Character {
    pub fn is_stuffy(&self) -> bool {
        self.kind == "stuffy"
    }

    pub fn is_human(&self) -> bool {
        self.kind == "human"
    }

    /// Faction → design-system accent token, matching the `[data-accent="…"]`
    /// selectors in static/app.css. Deterministic so a character's card color
    /// is stable across renders. Known factions match exactly (normalized:
    /// trimmed + lowercased) to avoid substring collisions — e.g. "Dog Squad"
    /// must NOT read as "The OG". Any other named faction gets `blossom`;
    /// factionless characters (the humans) get the default `mint`. Keep the
    /// returned set (`mint`/`lavender`/`peach`/`blossom`) in sync with that CSS.
    pub fn accent(&self) -> &'static str {
        let normalized = self.faction.as_deref().map(|f| f.trim().to_ascii_lowercase());
        match normalized.as_deref() {
            Some("avocatts") => "mint",
            Some("teeturtles") => "lavender",
            Some("the og") => "peach",
            Some(_) => "blossom",
            None => "mint",
        }
    }

    /// Renders this character as a compact brief for the LLM prompt.
    ///
    /// We do not include raw model output back into prompts, and this method is
    /// only ever fed our own hand-authored TOML — so no injection surface here.
    pub fn to_prompt_brief(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# {} — {}\n", self.name, self.title));
        out.push_str(&format!(
            "Species: {}. Role: {}.\n",
            self.species, self.role
        ));
        if let Some(faction) = &self.faction {
            let role = self.faction_role.as_deref().unwrap_or("member");
            out.push_str(&format!("Faction: {faction} ({role}).\n"));
        }
        if !self.on_council {
            out.push_str("Council status: NOT on the council (Lennon left them off — a permanent grievance for this character).\n");
        }
        if !self.traits.is_empty() {
            out.push_str(&format!("Traits: {}.\n", self.traits.join(", ")));
        }
        out.push_str(&format!("Speech style: {}\n", self.speech_style));
        if !self.loves.is_empty() {
            out.push_str(&format!("Loves: {}.\n", self.loves.join(", ")));
        }
        if !self.fears.is_empty() {
            out.push_str(&format!("Fears: {}.\n", self.fears.join(", ")));
        }
        if let Some(cp) = &self.catchphrase {
            out.push_str(&format!("Catchphrase: \"{}\"\n", cp));
        }
        if !self.relationships.is_empty() {
            out.push_str("Relationships:\n");
            for r in &self.relationships {
                out.push_str(&format!("  - with {}: {}\n", r.with, r.bond));
            }
        }
        if let Some(lore) = &self.lore {
            out.push_str("Lore:\n");
            for line in lore.trim().lines() {
                out.push_str(&format!("  {line}\n"));
            }
        }
        out
    }
}

/// In-memory, read-only registry of all characters loaded at startup.
#[derive(Debug, Clone, Default)]
pub struct CastRegistry {
    by_id: BTreeMap<String, Character>,
}

impl CastRegistry {
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            return Err(anyhow!(
                "cast directory {} does not exist or is not a directory",
                dir.display()
            ));
        }

        let mut by_id = BTreeMap::new();
        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("reading {}", dir.display()))?
        {
            let entry = entry?;
            let path: PathBuf = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str());
            if ext != Some("toml") {
                continue;
            }
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow!("unreadable character filename: {}", path.display()))?
                .to_string();
            validate_id(&id)
                .with_context(|| format!("invalid character id from filename `{}`", path.display()))?;

            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let mut character: Character = toml::from_str(&text)
                .with_context(|| format!("parsing {}", path.display()))?;
            character.id = id.clone();
            by_id.insert(id, character);
        }

        // Validate cross-references — every `relationships[].with` must point to a real character.
        for character in by_id.values() {
            for r in &character.relationships {
                if !by_id.contains_key(&r.with) {
                    return Err(anyhow!(
                        "character `{}` references unknown character `{}` in relationships",
                        character.id,
                        r.with
                    ));
                }
            }
        }

        Ok(CastRegistry { by_id })
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn all(&self) -> impl Iterator<Item = &Character> {
        self.by_id.values()
    }

    pub fn get(&self, id: &str) -> Option<&Character> {
        self.by_id.get(id)
    }
}

/// Character ids must be filesystem-safe kebab-case so they round-trip cleanly
/// as filenames, URL slugs, and JSON strings.
fn validate_id(id: &str) -> Result<()> {
    if id.is_empty() {
        return Err(anyhow!("empty id"));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow!(
            "id `{id}` must be lowercase ASCII letters, digits, and hyphens only"
        ));
    }
    if id.starts_with('-') || id.ends_with('-') {
        return Err(anyhow!("id `{id}` must not start or end with a hyphen"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // Coverage dimensions (see .github/instructions/test-quality.instructions.md):
    // accent() is a pure, total faction→token mapping. Functional (each known
    // faction), edge (case/whitespace, substring collision), and negative
    // (unknown, none) dimensions are covered below. Error-handling and
    // state-transition dimensions are N/A — the function is infallible and
    // holds no state.
    use super::*;

    fn character_in_faction(faction: Option<&str>) -> Character {
        Character {
            id: "x".into(),
            name: "X".into(),
            species: "s".into(),
            title: "t".into(),
            kind: "stuffy".into(),
            image: None,
            color_palette: vec![],
            traits: vec![],
            speech_style: "s".into(),
            fears: vec![],
            loves: vec![],
            catchphrase: None,
            role: "r".into(),
            faction: faction.map(str::to_string),
            faction_role: None,
            on_council: true,
            relationships: vec![],
            lore: None,
        }
    }

    #[test]
    fn accent_maps_each_known_faction_to_a_distinct_token() {
        assert_eq!(character_in_faction(Some("Avocatts")).accent(), "mint");
        assert_eq!(character_in_faction(Some("TeeTurtles")).accent(), "lavender");
        assert_eq!(character_in_faction(Some("The OG")).accent(), "peach");
    }

    #[test]
    fn accent_ignores_case_and_surrounding_whitespace() {
        assert_eq!(character_in_faction(Some("  avocatts ")).accent(), "mint");
    }

    #[test]
    fn accent_unknown_named_faction_falls_back_to_blossom() {
        assert_eq!(character_in_faction(Some("Sparkle Squad")).accent(), "blossom");
        // Regression: a name merely CONTAINING "og" (e.g. "Dog Squad") must not
        // be misread as "The OG" — exact matching, not substring.
        assert_eq!(character_in_faction(Some("Dog Squad")).accent(), "blossom");
    }

    #[test]
    fn accent_factionless_character_uses_default_mint() {
        assert_eq!(character_in_faction(None).accent(), "mint");
    }
}
