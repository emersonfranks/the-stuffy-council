//! Stuffy Council — daily generative bedtime stories.

mod auth;
mod cast;
mod config;
mod db;
mod error;
mod routes;
mod state;
mod stories;
mod story_repo;
mod web;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::http::StatusCode;
use tokio::net::TcpListener;
use tokio::signal;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tower_sessions::cookie::SameSite;
use tower_sessions::cookie::time as ttime;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::SqliteStore;

use crate::cast::CastRegistry;
use crate::config::Config;
use crate::state::AppState;
use crate::stories::StoryService;
use crate::stories::ollama::OllamaGenerator;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env for local dev; a no-op if the file is absent.
    let _ = dotenvy::dotenv();

    init_tracing();

    let config = Arc::new(Config::from_env().context("loading config")?);
    tracing::info!(env = ?config.env, addr = %config.bind_addr, "starting stuffy council");

    let db = db::connect(&config.database_url)
        .await
        .context("connecting to database")?;

    maybe_bootstrap_admin(&db).await?;

    let cast = Arc::new(
        CastRegistry::load_from_dir("cast").context("loading cast")?,
    );
    tracing::info!(count = cast.len(), "loaded cast");
    let stuffy_count = cast.all().filter(|c| c.is_stuffy()).count();
    if stuffy_count < stories::MIN_CAST_SIZE {
        tracing::warn!(
            need = stories::MIN_CAST_SIZE,
            have = stuffy_count,
            "not enough stuffies to generate a story yet"
        );
    }

    let generator = Arc::new(OllamaGenerator::new(
        &config.ollama_url,
        &config.ollama_model,
        config.ollama_timeout,
    )?);
    let stories = StoryService::new(generator, cast.clone());

    let session_store = SqliteStore::new(db.clone());
    session_store
        .migrate()
        .await
        .context("session store migrations")?;

    let cookie_secure = config.env.cookies_require_secure();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_name("stuffy_session")
        .with_secure(cookie_secure)
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(ttime::Duration::days(30)));

    let governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(config.rate_limit_per_second)
            .burst_size(config.rate_limit_burst)
            .finish()
            .context("building rate limiter config")?,
    );
    let rate_limit_layer = GovernorLayer::new(governor_config);

    let state = AppState {
        config: config.clone(),
        db,
        cast,
        stories,
    };

    // Build the router and stack the security-header layers on top.
    let mut app = routes::router(state);
    for layer in web::security::header_layers(config.env) {
        app = app.layer(layer);
    }

    let app = app
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(session_layer)
        .layer(rate_limit_layer)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("binding {}", config.bind_addr))?;
    tracing::info!(addr = %config.bind_addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum serve")?;

    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,stuffy_council=debug,tower_http=info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// If `BOOTSTRAP_ADMIN_USER` + `BOOTSTRAP_ADMIN_PASSWORD` are set on startup,
/// upsert that account. Meant strictly for first-boot seeding — remove the env
/// vars after the first successful login.
async fn maybe_bootstrap_admin(db: &sqlx::SqlitePool) -> Result<()> {
    let (Ok(user), Ok(pw)) = (
        std::env::var("BOOTSTRAP_ADMIN_USER"),
        std::env::var("BOOTSTRAP_ADMIN_PASSWORD"),
    ) else {
        return Ok(());
    };
    let display = std::env::var("BOOTSTRAP_ADMIN_DISPLAY_NAME").unwrap_or_else(|_| user.clone());
    if pw.len() < 12 {
        return Err(anyhow::anyhow!(
            "BOOTSTRAP_ADMIN_PASSWORD must be at least 12 characters"
        ));
    }
    auth::upsert_user(db, &user, &display, &pw).await?;
    tracing::warn!(user = %user, "bootstrapped/updated user from env — unset BOOTSTRAP_ADMIN_* after first login");
    Ok(())
}

/// Wait for Ctrl+C or SIGTERM so containers shut down cleanly.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("install ctrl_c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
