//! Library entry point.
//!
//! `main.rs` is a thin CLI wrapper that constructs `AppState` and calls
//! [`serve`]. Everything else lives here so integration tests can build the
//! exact same wiring against a test `AppState`.

pub mod access;
pub mod auth;
pub mod cast;
pub mod config;
pub mod db;
pub mod error;
pub mod routes;
pub mod state;
pub mod stories;
pub mod story_repo;
pub mod web;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::http::StatusCode;
use tokio::net::TcpListener;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tower_sessions::cookie::SameSite;
use tower_sessions::cookie::time as ttime;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::SqliteStore;

use crate::state::AppState;

/// Assemble every middleware layer around the router and serve on `listener`
/// until Ctrl+C / SIGTERM. `main.rs` and integration tests both call this so
/// the wire-up under test is identical to production.
///
/// The `into_make_service_with_connect_info::<SocketAddr>` at the bottom is
/// required by `tower_governor`'s default key extractor; without it every
/// request returns 500 "Unable to extract key!". `tests/router_smoke.rs`
/// contains the regression test.
pub async fn serve(state: AppState, listener: TcpListener) -> Result<()> {
    let session_store = SqliteStore::new(state.db.clone());
    session_store
        .migrate()
        .await
        .context("session store migrations")?;

    let cookie_secure = state.config.env.cookies_require_secure();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_name("stuffy_session")
        .with_secure(cookie_secure)
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(ttime::Duration::days(30)));

    let governor_config = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(state.config.rate_limit_per_second)
            .burst_size(state.config.rate_limit_burst)
            .finish()
            .context("building rate limiter config")?,
    );
    let rate_limit_layer = GovernorLayer::new(governor_config);

    let env = state.config.env;
    let mut app = routes::router(state);
    for layer in web::security::header_layers(env) {
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

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("axum serve")
}

/// Waits for Ctrl+C (any platform) or SIGTERM (unix). Container SIGTERM
/// arrives on shutdown so this lets the graceful-shutdown path run.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
