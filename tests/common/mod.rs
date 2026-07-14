//! Shared helpers for integration tests.
//!
//! Every test that boots the app calls [`build_test_app`] to produce an
//! `AppState` wired the same way `main.rs` does, but backed by scratch
//! resources (a temp SQLite file, a temp allowlist, an empty cast, and a
//! no-op story generator). Tests bind their own ephemeral port and hand
//! the state to [`stuffy_council::serve`].

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use tempfile::TempDir;

use stuffy_council::access::AccessList;
use stuffy_council::auth::JwkCache;
use stuffy_council::cast::CastRegistry;
use stuffy_council::config::{Config, Environment};
use stuffy_council::state::AppState;
use stuffy_council::stories::{StoryGenerator, StoryService};

/// A ready-to-serve `AppState` plus the tempdir backing its on-disk state.
///
/// Callers MUST keep `TestApp` alive for the lifetime of the server —
/// dropping it deletes the SQLite database file the server is talking to.
pub struct TestApp {
    pub state: AppState,
    _tmp: TempDir,
}

pub async fn build_test_app() -> Result<TestApp> {
    let tmp = tempfile::tempdir()?;

    let db_path = tmp.path().join("test.sqlite");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let db = stuffy_council::db::connect(&db_url).await?;

    // Empty cast: enough for the auth surface + council/story routes to be
    // wired up. Individual tests that need characters can extend later.
    let cast = Arc::new(CastRegistry::default());

    let allow_path = tmp.path().join("authorized-users.toml");
    std::fs::write(
        &allow_path,
        "[[users]]\nemail = \"test@example.com\"\nadmin = true\n",
    )?;
    let access = Arc::new(AccessList::load_from_file(
        &allow_path,
        Environment::Development,
    )?);

    let generator: Arc<dyn StoryGenerator> = Arc::new(NoopGenerator);
    let stories = StoryService::new(generator, cast.clone());

    // Empty JWKS cache. Tests that exercise sign-in verification will
    // populate it themselves; the auth-gate smoke tests do not sign in.
    let http = reqwest::Client::new();
    let jwks = Arc::new(JwkCache::new(http));

    let config = Arc::new(Config {
        env: Environment::Development,
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        public_origin: "http://localhost".to_string(),
        session_secret: vec![0u8; 64],
        database_url: db_url,
        google_client_id: "test-client-id.apps.googleusercontent.com".to_string(),
        ollama_url: "http://127.0.0.1:11434".to_string(),
        ollama_model: "test-model".to_string(),
        ollama_timeout: Duration::from_secs(5),
        // Permissive limits so the rate limiter doesn't reject legitimate
        // test traffic. Rate-limit behavior is tested elsewhere.
        rate_limit_per_second: 1000,
        rate_limit_burst: 1000,
    });

    Ok(TestApp {
        state: AppState {
            config,
            db,
            cast,
            stories,
            access,
            jwks,
        },
        _tmp: tmp,
    })
}

/// StoryGenerator stub. Never called in the currently-written tests, but
/// required to construct `AppState`.
struct NoopGenerator;

#[async_trait]
impl StoryGenerator for NoopGenerator {
    fn model_id(&self) -> &str {
        "test-noop"
    }

    async fn generate(&self, _prompt: &str) -> Result<String> {
        Ok(String::new())
    }
}
