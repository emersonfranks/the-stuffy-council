//! Stuffy Council — daily generative bedtime stories.
//!
//! CLI wrapper. All wiring lives in [`stuffy_council`]; this file only
//! parses env, builds `AppState`, binds the listener, and hands off.

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use stuffy_council::access::AccessList;
use stuffy_council::auth;
use stuffy_council::cast::CastRegistry;
use stuffy_council::config::Config;
use stuffy_council::state::AppState;
use stuffy_council::stories;
use stuffy_council::stories::StoryService;
use stuffy_council::stories::ollama::OllamaGenerator;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    init_tracing();

    let config = Arc::new(Config::from_env().context("loading config")?);
    tracing::info!(env = ?config.env, addr = %config.bind_addr, "starting stuffy council");

    let db = stuffy_council::db::connect(&config.database_url)
        .await
        .context("connecting to database")?;

    let cast = Arc::new(CastRegistry::load_from_dir("cast").context("loading cast")?);
    tracing::info!(count = cast.len(), "loaded cast");
    let stuffy_count = cast.all().filter(|c| c.is_stuffy()).count();
    if stuffy_count < stories::MIN_CAST_SIZE {
        tracing::warn!(
            need = stories::MIN_CAST_SIZE,
            have = stuffy_count,
            "not enough stuffies to generate a story yet"
        );
    }

    let access = Arc::new(
        AccessList::load_from_file("authorized-users.toml", config.env)
            .context("loading authorized-users.toml")?,
    );
    tracing::info!(count = access.len(), "loaded authorized users");

    let generator = Arc::new(OllamaGenerator::new(
        &config.ollama_url,
        &config.ollama_model,
        config.ollama_timeout,
    )?);
    let stories = StoryService::new(generator, cast.clone());

    let jwks_http = reqwest::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(concat!("stuffy-council/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("building HTTP client for Google JWKS")?;
    let jwks = Arc::new(auth::JwkCache::new(jwks_http));
    jwks.refresh().await.context("initial Google JWKS fetch")?;
    tracing::info!(count = jwks.len().await, "loaded Google JWKS");

    let state = AppState {
        config: config.clone(),
        db,
        cast,
        stories,
        access,
        jwks,
    };

    let listener = TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("binding {}", config.bind_addr))?;
    tracing::info!(addr = %config.bind_addr, "listening");

    stuffy_council::serve(state, listener).await
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,stuffy_council=debug,tower_http=info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
