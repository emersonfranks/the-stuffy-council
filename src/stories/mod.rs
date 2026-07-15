//! Story generation.
//!
//! Layered so the rest of the app never talks to a model directly:
//!
//!   [handler] → [StoryService] → [StoryGenerator trait] → [OllamaGenerator]
//!
//! Swapping models — or moving from local Ollama to a hosted endpoint —
//! becomes a single-line change in `main.rs`.

pub mod ollama;

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use time::Date;

use crate::cast::{CastRegistry, Character};

/// Bounds for how many stuffies appear in a single story.
///
/// Kept small because more characters means noisier prose from small local
/// models. Tune as we grow the cast.
pub const MIN_CAST_SIZE: usize = 2;
pub const MAX_CAST_SIZE: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedStory {
    pub title: String,
    pub body: String,
    pub cast: Vec<String>,   // stuffy ids
    pub model: String,
    pub prompt: String,
}

#[async_trait]
pub trait StoryGenerator: Send + Sync + 'static {
    /// Model identifier we persist alongside each story.
    fn model_id(&self) -> &str;

    /// Given a fully-built prompt, return the raw model output.
    async fn generate(&self, prompt: &str) -> Result<String>;
}

/// Coordinates prompt building, cast selection, and calling the underlying model.
#[derive(Clone)]
pub struct StoryService {
    generator: Arc<dyn StoryGenerator>,
    cast: Arc<CastRegistry>,
}

impl StoryService {
    pub fn new(generator: Arc<dyn StoryGenerator>, cast: Arc<CastRegistry>) -> Self {
        Self { generator, cast }
    }

    /// Pick a deterministic cast for a given date, so re-runs of the same
    /// day produce the same characters (before caching kicks in). Only
    /// stuffies rotate; humans (Lennon, Dad) are always in the prompt as
    /// narrative frame and are added by `build_prompt`.
    pub fn pick_cast_for(&self, date: Date) -> Vec<&Character> {
        let mut all: Vec<&Character> = self.cast.all().filter(|c| c.is_stuffy()).collect();
        all.sort_by(|a, b| a.id.cmp(&b.id));

        if all.is_empty() {
            return Vec::new();
        }

        // Seed on the date so a given day picks a stable cast unless the
        // roster changes.
        let seed = date_seed(date);
        let mut rng = StdRng::seed_from_u64(seed);

        let max = MAX_CAST_SIZE.min(all.len());
        let min = MIN_CAST_SIZE.min(max);
        let target = if min == max {
            max
        } else {
            rng.gen_range(min..=max)
        };

        all.shuffle(&mut rng);
        all.into_iter().take(target).collect()
    }

    pub fn build_prompt(&self, date: Date, cast: &[&Character]) -> String {
        let names = cast
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        // Humans always frame the story regardless of the daily rotation.
        let humans: Vec<&Character> = self.cast.all().filter(|c| c.is_human()).collect();

        let mut prompt = String::new();
        prompt.push_str(SYSTEM_PREAMBLE);
        prompt.push_str("\n\n");

        if !humans.is_empty() {
            prompt.push_str("## The World\n\n");
            prompt.push_str(
                "The stuffies live in Lennon's home. Lennon's imagination is \
                 what animates them. Dad gives Ruff Ruff his literal voice, \
                 interprets every other stuffy's sounds for the audience, and \
                 narrates their adventures. These characters are ALWAYS \
                 present in the world; they may or may not be central to today's \
                 story but their personalities and dynamics shape everything.\n\n",
            );
            for h in &humans {
                prompt.push_str(&h.to_prompt_brief());
                prompt.push('\n');
            }
        }

        prompt.push_str("## Today's Cast (stuffies)\n\n");
        for s in cast {
            prompt.push_str(&s.to_prompt_brief());
            prompt.push('\n');
        }

        prompt.push_str(&format!(
            "\n## Today\n\nThe date is {date}. Today's story centers on these stuffies: {names}. \
             They are the ones on-screen; other named stuffies from the world may be \
             MENTIONED in passing but should not have speaking parts. Faction leaders \
             may bring along one or two unnamed crew members (e.g. Woofy with a couple \
             of Avocatts, Bar Bar with a TeeTurtle or two) if it serves the story.\n\n\
             Tone: playful and a little chaotic. Stuffies may bicker, boast, form \
             short-lived alliances, or stage absurd mock-conflicts. Lennon may stir \
             the pot on purpose — that's canon. Ruff Ruff will insist he's the real \
             leader; the council will disagree. Keep it warm underneath the mischief \
             and end on a note that feels good to hear before bed. No genuinely \
             scary or adult content.\n\n\
             Length: 300–500 words. Give the story a short evocative title.\n\n",
        ));
        prompt.push_str(OUTPUT_FORMAT_INSTRUCTIONS);
        prompt
    }

    /// Generate a fresh story for the given date. Does not touch the cache;
    /// the caller (e.g. the daily-story service) decides when to persist.
    pub async fn generate_for(&self, date: Date) -> Result<GeneratedStory> {
        let cast = self.pick_cast_for(date);
        if cast.len() < MIN_CAST_SIZE {
            return Err(anyhow!(
                "need at least {MIN_CAST_SIZE} stuffies to write a story (have {})",
                cast.len()
            ));
        }
        let cast_ids: Vec<String> = cast.iter().map(|s| s.id.clone()).collect();
        let prompt = self.build_prompt(date, &cast);

        let raw = self
            .generator
            .generate(&prompt)
            .await
            .context("model call failed")?;
        let (title, body) = parse_titled_output(&raw);

        Ok(GeneratedStory {
            title,
            body,
            cast: cast_ids,
            model: self.generator.model_id().to_string(),
            prompt,
        })
    }
}

/// Deterministic seed from a date; changes each day, stable within a day.
fn date_seed(date: Date) -> u64 {
    let y = date.year() as u64;
    let d = date.ordinal() as u64;
    y.wrapping_mul(1_000).wrapping_add(d)
}

const SYSTEM_PREAMBLE: &str = "\
You are the storyteller for a household of stuffed animals belonging to a \
young girl named Lennon. Each day one small adventure or quiet moment \
unfolds among them. Your job is to write today's story.

Rules:
* Ruff Ruff is the ONLY stuffy with literal voiced English dialogue. Dad \
  performs his voice slightly higher and cracked.
* Every other stuffy makes only the sounds or language in their character \
  brief. Woofy hums; Bar Bar says variations of his own name. They NEVER \
  speak English aloud.
* When Dad is not physically in the scene, show a non-Ruff-Ruff stuffy's \
  intended words as thought-bubble-equivalent prose attributed directly to \
  that stuffy. Include a small audible sound cue, then write, for example: \
  `Woofy gave an imperious hum. His thought bubble read, \"The agenda is \
  obvious.\"` Dad's interpretation is the narrative mechanism; it does NOT \
  place him in the scene.
* When Dad is physically in the scene, he may translate a stuffy's sounds \
  aloud instead of using thought-bubble prose.
* Playful chaos is welcome: bickering, boasting, silly power grabs, \
  absurd mock-conflicts. Ruff Ruff insisting he is the real leader is \
  a running theme.
* Any 'weapons' in this world (Woofy's AKs, Ruff Ruff's wooden-spoon \
  'surgical instrument', etc.) are plush toys or pretend props used for \
  dramatic entrances and slapstick. They are NEVER real firearms and \
  never cause real harm.
* No genuinely scary or adult content. Rough-and-tumble is fine; real \
  harm is not.
* Prefer specific sensory details (the couch, a blanket fort, the \
  hallway, weather, the sound of Lennon giggling) over generic prose.
* End on a warm, hopeful, or wryly funny note that leaves the family \
  smiling before bed.
";

const OUTPUT_FORMAT_INSTRUCTIONS: &str = "\
Return ONLY the story, formatted exactly like this and nothing else:

TITLE: <a short evocative title, no quotes>

<the story body, plain prose, no headings, no bullet points>
";

/// Parse `TITLE: ...\n\n<body>` out of the model's raw output.
///
/// We are lenient: if the model doesn't obey the format, we fall back to a
/// generated title and use the whole output as the body.
fn parse_titled_output(raw: &str) -> (String, String) {
    let cleaned = raw.trim();

    if let Some(rest) = cleaned.strip_prefix("TITLE:").or_else(|| cleaned.strip_prefix("Title:")) {
        // Title is the first line after `TITLE:`; body is everything after the first blank line.
        let mut parts = rest.splitn(2, '\n');
        let title = parts
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches('"')
            .to_string();
        let body = parts.next().unwrap_or("").trim().to_string();
        if !title.is_empty() && !body.is_empty() {
            return (title, body);
        }
    }

    // Fallback — use a stable placeholder title, keep full body.
    ("A Council Story".to_string(), cleaned.to_string())
}

#[cfg(test)]
mod tests {
    // The prompt contract is stateless canonical text. Functional and
    // regression dimensions verify the literal-voice/thought-bubble rule in
    // the fully composed prompt. Boundary, dependency-error, and
    // state-transition dimensions are N/A.
    use std::fs;
    use std::path::Path as FsPath;

    use tempfile::tempdir;
    use time::Month;

    use super::*;

    struct UnusedGenerator;

    #[async_trait]
    impl StoryGenerator for UnusedGenerator {
        fn model_id(&self) -> &str {
            "unused"
        }

        async fn generate(&self, _prompt: &str) -> Result<String> {
            panic!("prompt composition must not invoke the generator")
        }
    }

    fn write_character(dir: &FsPath, id: &str, name: &str, kind: &str, speech_style: &str) {
        let toml = format!(
            "name = {name:?}\n\
             species = \"test character\"\n\
             title = \"test title\"\n\
             kind = {kind:?}\n\
             role = \"test role\"\n\
             speech_style = {speech_style:?}\n"
        );
        fs::write(dir.join(format!("{id}.toml")), toml).expect("write cast fixture");
    }

    #[test]
    fn build_prompt_encodes_stuffy_voice_and_interpretation_canon() {
        let temp = tempdir().expect("temp dir");
        write_character(
            temp.path(),
            "dad",
            "Dad",
            "human",
            "Ruff Ruff's voice and interpreter of every other stuffy's sounds.",
        );
        write_character(
            temp.path(),
            "ruff-ruff",
            "Ruff Ruff",
            "stuffy",
            "The only stuffy with literal voiced English dialogue.",
        );
        write_character(
            temp.path(),
            "woofy",
            "Woofy",
            "stuffy",
            "Makes pseudo-humming sounds aloud and never speaks English.",
        );
        let cast = Arc::new(CastRegistry::load_from_dir(temp.path()).expect("load cast"));
        let service = StoryService::new(Arc::new(UnusedGenerator), cast.clone());
        let woofy = cast.get("woofy").expect("Woofy fixture");

        let prompt = service.build_prompt(
            Date::from_calendar_date(2026, Month::July, 15).expect("valid date"),
            &[woofy],
        );

        assert!(prompt.contains(
            "Ruff Ruff is the ONLY stuffy with literal voiced English dialogue"
        ));
        assert!(prompt.contains("They NEVER speak English aloud"));
        assert!(prompt.contains("When Dad is not physically in the scene"));
        assert!(prompt.contains("small audible sound cue"));
        assert!(prompt.contains("His thought bubble read"));
        assert!(prompt.contains("it does NOT place him in the scene"));
        assert!(prompt.contains("When Dad is physically in the scene, he may translate"));
        assert!(prompt.contains(
            "Dad gives Ruff Ruff his literal voice, interprets every other stuffy's sounds"
        ));
        assert!(prompt.contains("Makes pseudo-humming sounds aloud and never speaks English"));
        assert!(!prompt.contains("Bar Bar speaks by repeating his own name"));
    }
}
