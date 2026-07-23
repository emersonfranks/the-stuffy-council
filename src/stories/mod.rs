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

use anyhow::anyhow;
use async_trait::async_trait;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use thiserror::Error;
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
    pub cast: Vec<String>, // stuffy ids
    pub model: String,
    pub prompt: String,
}

#[derive(Debug, Error)]
pub enum StoryGenerationError {
    #[error("story generator temporarily unavailable")]
    Unavailable(#[source] anyhow::Error),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type StoryGenerationResult<T> = std::result::Result<T, StoryGenerationError>;

#[async_trait]
pub trait StoryGenerator: Send + Sync + 'static {
    /// Model identifier we persist alongside each story.
    fn model_id(&self) -> &str;

    /// Given a fully-built prompt, return the raw model output.
    async fn generate(&self, prompt: &str) -> StoryGenerationResult<String>;
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
             temporary transactional alliances, or stage absurd mock-conflicts. Lennon may stir \
             the pot on purpose — that's canon. Ruff Ruff's leadership grievance is \
             available when it gives the scene useful friction, but do not force it. \
             Warmth comes from Lennon's love and the shared home, NOT from rivals \
             becoming friends, equals, or validating one another's claims. End on a \
             note that feels good \
             to hear before bed. No genuinely \
             scary or adult content.\n\n\
             Length: 220–350 words. Favor a few vivid scenes over exhaustive \
             dialogue or explanation. Give the story a short evocative title \
             whose central promise is fulfilled before the final paragraph.\n\n",
        ));
        prompt.push_str(OUTPUT_FORMAT_INSTRUCTIONS);
        prompt
    }

    /// Generate a fresh story for the given date. Does not touch the cache;
    /// the caller (e.g. the daily-story service) decides when to persist.
    pub async fn generate_for(&self, date: Date) -> StoryGenerationResult<GeneratedStory> {
        let cast = self.pick_cast_for(date);
        if cast.len() < MIN_CAST_SIZE {
            return Err(StoryGenerationError::Internal(anyhow!(
                "need at least {MIN_CAST_SIZE} stuffies to write a story (have {})",
                cast.len()
            )));
        }
        let cast_ids: Vec<String> = cast.iter().map(|s| s.id.clone()).collect();
        let prompt = self.build_prompt(date, &cast);

        let raw = self.generator.generate(&prompt).await?;
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
* Write polished children's fiction, not a demonstration of a character \
    database. Treat each character brief as a PALETTE, NOT A CHECKLIST. Choose \
    only the few details that serve this particular scene; leave most listed \
    traits, loves, fears, titles, props, sounds, lore, and running jokes unused. \
    Let canon shape choices, reactions, subtext, and comic timing instead of \
    reciting or explaining it to the reader.
* Build scenes through cause and effect: characters want something, act, \
    misread one another, interrupt, adapt, and reveal themselves. Use natural, \
    varied dialogue. Trust the reader to understand jokes and relationships \
    from behavior; avoid stiff role labels or phrases like `playing the role \
    of...` unless characters are literally casting a performance.
* Every daily story is a SELF-CONTAINED EPISODE with a beginning, central \
    action, and resolution. The title is a promise: the debate, contest, rescue, \
    performance, mystery, or other event it names MUST happen on the page and \
    reach a concrete outcome. Do not spend the whole story gathering characters \
    or announcing what they are about to do.
* Before writing, silently plan roughly the first quarter for setup, the \
    middle half for the promised event and complications, and the final quarter \
    for outcome and aftermath. Resolve today's central dramatic question even \
    when relationships, jokes, or consequences carry into future stories.
* Never end at the threshold of the promised event. Lines such as `the debate \
    was off`, `the meeting had officially begun`, or `and then the door opened` \
    are setup beats, not endings, when the central action remains unresolved. \
    End after readers learn what happened and see its comic or emotional \
    consequence. No cliffhangers and no `to be continued` endings.
* Ruff Ruff is the ONLY stuffy with literal voiced English dialogue. Dad \
  performs his voice slightly higher and cracked.
* Every other stuffy makes only the sounds or language in their character \
  brief. Woofy hums; Bar Bar says variations of his own name. They NEVER \
  speak English aloud.
* Reserve quoted English dialogue for humans and Ruff Ruff. When Dad is not \
    physically in the scene, convey every other stuffy's intended meaning \
    through FREE INDIRECT DISCOURSE: weave Dad's interpretation into the \
    narrator's prose in that stuffy's viewpoint. Do not call it a thought \
    bubble or translation, do not write `he meant`, and do not quote the \
    interpreted English as spoken dialogue. For example: `Woofy gave an \
    imperious hum. The agenda was obvious, and frankly it was embarrassing \
    that everyone else needed so long to see it.` Dad's interpretation is the \
    narrative mechanism; it does NOT place him in the scene.
* Use a stuffy's native sound cue only when it adds character, emotion, or \
    comic timing. Do not attach a hum or `Bar bar` to every line of meaning.
* When Dad is physically in the scene, he may translate a stuffy's sounds \
    aloud, sparingly, instead of using free indirect discourse.
* \"The OG\" is Lennon's label for Ruff Ruff because he is her oldest stuffy. \
    It is NOT his catchphrase. Do not make him repeatedly say \"As the OG\" or \
    use it to prefix his dialogue; express his pride in seniority naturally \
    and with varied wording.
* Lennon talks naturally like a bright, mischievous 10-year-old. She does \
    not have to initiate every premise; let her react, tease, participate, or \
    stir trouble through action. Do not turn any one phrase into her ritual \
    opening.
* Props, native sounds, and recurring bits are opportunities, never \
    requirements. Use an optional hook only when the scene earns it; omit most \
    hooks from every story. Character briefs own the specific examples.
* Woofy sees himself as the Supreme Leader. He NEVER serves as security, a \
    guard, an underling, or someone else's supporting detail. His Avocatt crew \
    may provide those services for him.
* Council politics are HARD CANON. An election for President of Lennon's room \
    ended in a tie. As her compromise, Lennon assigned the tied candidates the \
    same formal council office — co-president — except Ruff Ruff, whom she \
    deliberately excluded to create chaos. The candidates reject that this \
    compromise makes their authority equal. Ruff Ruff had one self-cast vote \
    like the other tied candidates and insists one vote means first place, so \
    he won anyway.
* Formal co-presidency does NOT mean the stuffies consider one another equal. \
    Each believes his own basis for power outranks the others: Woofy claims the \
    universe, Bar Bar claims royal authority over his pride, and Ruff Ruff claims \
    seniority plus his one-vote victory. They coexist and may cooperate \
    transactionally, but they do NOT consider one another friends or equals.
* No character other than Woofy — including Lennon, Dad, council members, and \
    Woofy's Avocatt crew — ever addresses or refers to him as Supreme Leader or \
    validates that self-appointed title. No stuffy ever validates another \
    stuffy's rank, leadership, supremacy, or claim to Lennon. Ruff Ruff never \
    says Woofy is in charge, would make a good leader, or deserves help leading. \
    Woofy never accepts Bar Bar or Ruff Ruff as an equal or superior. A truce, \
    shared success, or resolved daily plot must NOT settle this rivalry.
* Lennon may stop a fight, redirect it, impose a ruling, or enjoy the chaos, \
    but she never erases their political reality by declaring `We're all \
    friends here` or claiming the rival stuffies consider one another friends.
* \"The OG\" belongs only to Ruff Ruff as Lennon's label for her oldest \
    stuffy. No character applies it to Bar Bar, Woofy, or anyone else. Lennon \
    loves every stuffy; Ruff Ruff is her actual number one, but no council \
    member will ever acknowledge that fact.
* Playful chaos is welcome: bickering, boasting, silly power grabs, \
    absurd mock-conflicts. Ruff Ruff's claim to leadership is an available \
    running theme, not an obligation.
* Mock conflict stays theatrical and harmless. Never name or show a firearm; \
    no prop causes real harm.
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

    if let Some(rest) = cleaned
        .strip_prefix("TITLE:")
        .or_else(|| cleaned.strip_prefix("Title:"))
    {
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
    // regression dimensions verify voice, free indirect discourse, optional
    // hooks, and hard role invariants in the fully composed prompt. Boundary,
    // dependency-error, and state-transition dimensions are N/A.
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

        async fn generate(&self, _prompt: &str) -> StoryGenerationResult<String> {
            panic!("prompt composition must not invoke the generator")
        }
    }

    fn write_committed_cast(dir: &FsPath) {
        let files = [
            ("bar-bar.toml", include_str!("../../cast/bar-bar.toml")),
            ("dad.toml", include_str!("../../cast/dad.toml")),
            ("lennon.toml", include_str!("../../cast/lennon.toml")),
            ("ruff-ruff.toml", include_str!("../../cast/ruff-ruff.toml")),
            ("woofy.toml", include_str!("../../cast/woofy.toml")),
        ];
        for (name, contents) in files {
            fs::write(dir.join(name), contents).expect("write committed cast fixture");
        }
    }

    #[test]
    fn build_prompt_encodes_stuffy_voice_and_interpretation_canon() {
        let temp = tempdir().expect("temp dir");
        write_committed_cast(temp.path());
        let cast = Arc::new(CastRegistry::load_from_dir(temp.path()).expect("load cast"));
        let service = StoryService::new(Arc::new(UnusedGenerator), cast.clone());
        let woofy = cast.get("woofy").expect("Woofy fixture");
        let bar_bar = cast.get("bar-bar").expect("Bar Bar fixture");
        let ruff_ruff = cast.get("ruff-ruff").expect("Ruff Ruff fixture");

        let prompt = service.build_prompt(
            Date::from_calendar_date(2026, Month::July, 15).expect("valid date"),
            &[woofy, bar_bar, ruff_ruff],
        );

        assert!(
            prompt.contains("Ruff Ruff is the ONLY stuffy with literal voiced English dialogue")
        );
        assert!(prompt.contains("They NEVER speak English aloud"));
        assert!(prompt.contains("Reserve quoted English dialogue for humans and Ruff Ruff"));
        assert!(prompt.contains("FREE INDIRECT DISCOURSE"));
        assert!(prompt.contains("Do not call it a thought bubble or translation"));
        assert!(prompt.contains("do not write `he meant`"));
        assert!(prompt.contains("The agenda was obvious"));
        assert!(prompt.contains("it does NOT place him in the scene"));
        assert!(prompt.contains("Do not attach a hum or `Bar bar` to every line"));
        assert!(prompt.contains("When Dad is physically in the scene, he may translate"));
        assert!(prompt.contains("It is NOT his catchphrase"));
        assert!(prompt.contains("Do not make him repeatedly say \"As the OG\""));
        assert!(prompt.contains(
            "Dad gives Ruff Ruff his literal voice, interprets every other stuffy's sounds"
        ));
        assert!(prompt.contains("Makes pseudo-humming sounds aloud; never speaks English"));
        assert!(prompt.contains("Makes only variations of his own name aloud"));
        assert!(prompt.contains("The only stuffy with literal voiced English dialogue"));
        assert!(prompt.contains("Length: 220–350 words"));
        assert!(prompt.contains("Favor a few vivid scenes"));
        assert_eq!(woofy.catchphrase, None);
        assert_eq!(bar_bar.catchphrase, None);
        assert_eq!(ruff_ruff.catchphrase, None);
        assert!(!prompt.contains("*imperious hum*"));
        assert!(!prompt.contains("Bar. Bar. BAR."));
        assert!(!prompt.contains("As the OG, I say"));
        assert!(!prompt.contains("thought-bubble dialogue"));
        assert!(!prompt.contains("His thought bubble read"));
        assert!(!prompt.contains("Bar Bar speaks by repeating his own name"));
    }

    #[test]
    fn ruff_ruff_cast_treats_og_as_lennons_label_not_catchphrase() {
        let source = include_str!("../../cast/ruff-ruff.toml");
        let mut ruff_ruff: Character = toml::from_str(source).expect("parse Ruff Ruff");
        ruff_ruff.id = "ruff-ruff".into();

        assert_eq!(ruff_ruff.catchphrase, None);
        assert!(source.contains("Lennon calls Ruff Ruff \"the OG\""));
        assert!(source.contains("not a phrase he habitually says"));
        assert!(!ruff_ruff.to_prompt_brief().contains("Catchphrase:"));
    }

    #[test]
    fn build_prompt_treats_character_hooks_as_optional_and_roles_as_invariants() {
        let temp = tempdir().expect("temp dir");
        write_committed_cast(temp.path());
        let cast = Arc::new(CastRegistry::load_from_dir(temp.path()).expect("load cast"));
        let service = StoryService::new(Arc::new(UnusedGenerator), cast.clone());
        let woofy = cast.get("woofy").expect("Woofy fixture");
        let ruff_ruff = cast.get("ruff-ruff").expect("Ruff Ruff fixture");
        let lennon = cast.get("lennon").expect("Lennon fixture");

        let prompt = service.build_prompt(
            Date::from_calendar_date(2026, Month::July, 16).expect("valid date"),
            &[woofy, ruff_ruff],
        );

        assert!(prompt.contains("PALETTE, NOT A CHECKLIST"));
        assert!(prompt.contains("leave most listed"));
        assert!(prompt.contains("available when it gives the scene useful friction"));
        assert!(prompt.contains("an available running theme, not an obligation"));
        assert!(prompt.contains(
            "Props, native sounds, and recurring bits are opportunities, never requirements"
        ));
        assert!(prompt.contains("omit most hooks from every story"));
        assert!(prompt.contains("Woofy sees himself as the Supreme Leader"));
        assert!(prompt.contains("He NEVER serves as security"));
        assert!(prompt.contains("His Avocatt crew may provide those services"));
        assert!(prompt.contains("Lennon talks naturally like a bright, mischievous 10-year-old"));
        assert!(prompt.contains("avoid stiff role labels"));
        assert!(prompt.contains("chk-chk"));
        let normalized_words = prompt
            .to_ascii_lowercase()
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert!(
            !normalized_words
                .iter()
                .any(|word| word == "ak" || word == "aks")
        );
        assert!(!prompt.to_ascii_lowercase().contains("what if we"));
        assert_eq!(lennon.catchphrase, None);
        assert!(
            woofy
                .role
                .contains("never a guard, security detail, or subordinate")
        );
        assert!(
            !ruff_ruff
                .loves
                .iter()
                .any(|love| love.contains("wooden spoon"))
        );
        assert!(!prompt.contains("a permanent grievance for this character"));
        assert!(prompt.contains("Council status: NOT on the council."));
        assert!(!include_str!("../../cast/lennon.toml").contains("permanent grievance"));
    }

    #[test]
    fn build_prompt_requires_a_self_contained_title_promise_and_resolution() {
        let temp = tempdir().expect("temp dir");
        write_committed_cast(temp.path());
        let cast = Arc::new(CastRegistry::load_from_dir(temp.path()).expect("load cast"));
        let service = StoryService::new(Arc::new(UnusedGenerator), cast.clone());
        let woofy = cast.get("woofy").expect("Woofy fixture");
        let bar_bar = cast.get("bar-bar").expect("Bar Bar fixture");

        let prompt = service.build_prompt(
            Date::from_calendar_date(2026, Month::July, 16).expect("valid date"),
            &[woofy, bar_bar],
        );

        assert!(prompt.contains("SELF-CONTAINED EPISODE"));
        assert!(prompt.contains("The title is a promise"));
        assert!(prompt.contains("MUST happen on the page and reach a concrete outcome"));
        assert!(prompt.contains("first quarter for setup"));
        assert!(prompt.contains("final quarter for outcome and aftermath"));
        assert!(prompt.contains("Resolve today's central dramatic question"));
        assert!(prompt.contains("Never end at the threshold of the promised event"));
        assert!(prompt.contains("the debate was off"));
        assert!(prompt.contains("the meeting had officially begun"));
        assert!(prompt.contains("and then the door opened"));
        assert!(prompt.contains("No cliffhangers"));
        assert!(prompt.contains("central promise is fulfilled before the final paragraph"));
    }

    #[test]
    fn build_prompt_preserves_council_rivalry_and_rejects_mutual_validation() {
        let temp = tempdir().expect("temp dir");
        write_committed_cast(temp.path());
        let cast = Arc::new(CastRegistry::load_from_dir(temp.path()).expect("load cast"));
        let service = StoryService::new(Arc::new(UnusedGenerator), cast.clone());
        let woofy = cast.get("woofy").expect("Woofy fixture");
        let bar_bar = cast.get("bar-bar").expect("Bar Bar fixture");
        let ruff_ruff = cast.get("ruff-ruff").expect("Ruff Ruff fixture");

        let prompt = service.build_prompt(
            Date::from_calendar_date(2026, Month::July, 16).expect("valid date"),
            &[woofy, bar_bar, ruff_ruff],
        );

        assert!(prompt.contains("election for President of Lennon's room ended in a tie"));
        assert!(prompt.contains("same formal council office — co-president — except Ruff Ruff"));
        assert!(prompt.contains("reject that this compromise makes their authority equal"));
        assert!(prompt.contains("deliberately excluded to create chaos"));
        assert!(prompt.contains("one self-cast vote"));
        assert!(prompt.contains("one vote means first place"));
        assert!(prompt.contains("do NOT consider one another friends or equals"));
        assert!(prompt.contains("cooperate transactionally"));
        assert!(prompt.contains("No character other than Woofy"));
        assert!(prompt.contains("including Lennon, Dad, council members"));
        assert!(prompt.contains("Woofy's Avocatt crew"));
        assert!(prompt.contains("ever addresses or refers to him as Supreme Leader"));
        assert!(prompt.contains("Ruff Ruff never says Woofy is in charge"));
        assert!(prompt.contains("would make a good leader"));
        assert!(prompt.contains("deserves help leading"));
        assert!(prompt.contains("Woofy claims the universe"));
        assert!(prompt.contains("Bar Bar claims royal authority over his pride"));
        assert!(prompt.contains("Ruff Ruff claims seniority plus his one-vote victory"));
        assert!(prompt.contains("A truce, shared success, or resolved daily plot must NOT settle"));
        assert!(prompt.contains("she never erases their political reality"));
        assert!(prompt.contains("We're all friends here"));
        assert!(prompt.contains("The OG\" belongs only to Ruff Ruff"));
        assert!(prompt.contains("No character applies it to Bar Bar"));
        assert!(prompt.contains("Ruff Ruff is her actual number one"));
        assert!(prompt.contains("no council member will ever acknowledge that fact"));
        assert!(prompt.contains("never calls Bar Bar the OG"));
        assert!(prompt.contains("never acknowledges him as leader or Lennon's number one"));
        assert!(!prompt.contains("leader-to-leader respect"));
        assert!(!prompt.contains("forgives afterward"));
        assert!(!prompt.contains("like old friends"));
        assert!(!prompt.contains("Each of them refers to him as"));
        assert!(!prompt.contains("being addressed as Supreme Leader"));
        assert!(!include_str!("../../cast/README.md").contains("forgives afterward"));
        assert!(!include_str!("../../cast/README.md").contains("Each of them refers to him as"));
    }
}
