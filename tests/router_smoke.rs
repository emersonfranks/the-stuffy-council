//! Router smoke tests — the tier-2 integration harness.
//!
//! These tests bind an ephemeral TCP port, spawn the full
//! [`stuffy_council::serve`] loop, and hit routes with `reqwest`. They
//! exist to catch bugs in the wire-up between the layered router, the
//! session store, the rate limiter, and `axum::serve` — bugs that unit
//! tests over the router alone would miss.
//!
//! Guards these invariants:
//!
//! 1. **Requests get a ConnectInfo extension.**
//!    `axum::serve` must be given the router wrapped in
//!    `into_make_service_with_connect_info::<SocketAddr>` because
//!    `tower_governor`'s default key extractor reads it. If we ever
//!    stop wrapping, every request 500s with "Unable to extract key!".
//!
//! 2. **Public routes render.** `/login` returns 200 and contains the
//!    Google Identity Services markup.
//!
//! 3. **Protected routes redirect anonymous users to `/login`, never
//!    200 (accidental leak) or 500 (bad auth wire-up).** Covers the
//!    "Rule 3: Auth flows" requirement in
//!    [`.github/instructions/test-quality.instructions.md`](../.github/instructions/test-quality.instructions.md).
//!
//! Deferred until we have a signed-JWT test harness: full
//! `POST /auth/google/verify` end-to-end coverage. Its allowlist-reject
//! branch is currently exercised only by manual QA.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Result;
use reqwest::StatusCode;
use reqwest::redirect::Policy;
use tokio::net::TcpListener;

use common::build_test_app;

/// Spin up the real app on an ephemeral port and return a `reqwest::Client`
/// pre-configured to NOT follow redirects (so tests observe the 3xx).
async fn spawn() -> Result<(SocketAddr, reqwest::Client, common::TestApp)> {
    let app = build_test_app().await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    // Clone state for the spawned server. `AppState: Clone`.
    let state_for_server = app.state.clone();
    tokio::spawn(async move {
        let _ = stuffy_council::serve(state_for_server, listener).await;
    });

    // Small readiness wait: axum::serve takes a few ms to accept connections.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(5))
        .build()?;
    Ok((addr, client, app))
}

/// Regression test: without `into_make_service_with_connect_info`, this
/// endpoint returns 500 "Unable to extract key!" because `tower_governor`
/// cannot key the rate limiter.
#[tokio::test]
async fn get_login_returns_200_for_anonymous_visitor() -> Result<()> {
    let (addr, client, _app) = spawn().await?;

    let resp = client.get(format!("http://{addr}/login")).send().await?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    assert_eq!(
        status,
        StatusCode::OK,
        "GET /login should render successfully; got {status}. body: {body}"
    );
    assert!(
        body.contains("g_id_signin") || body.contains("Sign in with Google"),
        "login page missing Google Identity Services markup. body: {body}"
    );
    Ok(())
}

/// Regression for the visual-identity work: the login page must link our
/// self-hosted stylesheet and must NOT reference the Tailwind Play CDN (a
/// `<script>` the CSP never allowed, so it never loaded).
#[tokio::test]
async fn login_links_local_css_and_drops_tailwind_cdn() -> Result<()> {
    let (addr, client, _app) = spawn().await?;
    let body = client
        .get(format!("http://{addr}/login"))
        .send()
        .await?
        .text()
        .await?;
    assert!(
        body.contains("/static/app.css"),
        "login page should link the self-hosted stylesheet. body: {body}"
    );
    assert!(
        !body.contains("cdn.tailwindcss.com"),
        "login page still references the Tailwind CDN. body: {body}"
    );
    Ok(())
}

/// Regression: the `/static` mount actually serves our stylesheet through the
/// full middleware stack (it was never mounted before the visual-identity work).
#[tokio::test]
async fn static_stylesheet_is_served() -> Result<()> {
    let (addr, client, _app) = spawn().await?;
    let resp = client
        .get(format!("http://{addr}/static/app.css"))
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "GET /static/app.css should 200"
    );
    let body = resp.text().await?;
    assert!(
        body.contains("--brand-council"),
        "served /static/app.css should be our stylesheet; got: {}",
        body.chars().take(120).collect::<String>()
    );
    Ok(())
}

async fn assert_static_png_is_served(path: &str) -> Result<()> {
    const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

    let (addr, client, _app) = spawn().await?;
    let resp = client.get(format!("http://{addr}{path}")).send().await?;

    assert_eq!(resp.status(), StatusCode::OK, "GET {path} should 200");
    assert_eq!(
        resp.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("image/png"),
        "GET {path} should return image/png"
    );
    let body = resp.bytes().await?;
    assert!(
        body.starts_with(PNG_SIGNATURE),
        "GET {path} should return PNG bytes"
    );
    Ok(())
}

#[tokio::test]
async fn static_clean_art_candidate_is_served_as_png() -> Result<()> {
    assert_static_png_is_served("/static/stuffies/review/ruff-ruff--candidate-clean.png").await
}

#[tokio::test]
async fn static_well_loved_art_candidate_is_served_as_png() -> Result<()> {
    assert_static_png_is_served("/static/stuffies/review/ruff-ruff--candidate-well-loved.png").await
}

/// The off-allowlist denial renders its in-voice message on the login page.
#[tokio::test]
async fn login_denied_error_renders_in_voice_copy() -> Result<()> {
    let (addr, client, _app) = spawn().await?;
    let resp = client
        .get(format!("http://{addr}/login?error=denied"))
        .send()
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await?;
    assert!(
        body.contains("Google account"),
        "the denied message (distinct from the base login copy) should render on /login?error=denied. body: {body}"
    );
    Ok(())
}

/// `/healthz` is public. If this 500s the wire-up is broken globally.
#[tokio::test]
async fn get_healthz_returns_200() -> Result<()> {
    let (addr, client, _app) = spawn().await?;

    let resp = client.get(format!("http://{addr}/healthz")).send().await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

/// Protected root redirects anonymous callers to `/login` — not 200 (leak)
/// and not 500 (bad wire-up). Covers Rule 3 (auth flow smoke).
#[tokio::test]
async fn get_root_redirects_anonymous_to_login() -> Result<()> {
    let (addr, client, _app) = spawn().await?;

    let resp = client.get(format!("http://{addr}/")).send().await?;
    assert!(
        resp.status().is_redirection(),
        "GET / should redirect anonymous callers; got {}",
        resp.status()
    );
    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        location.ends_with("/login"),
        "GET / should redirect to /login; got Location: {location}"
    );
    Ok(())
}

/// Protected character listing redirects anonymous callers to `/login`.
#[tokio::test]
async fn get_council_redirects_anonymous_to_login() -> Result<()> {
    let (addr, client, _app) = spawn().await?;

    let resp = client.get(format!("http://{addr}/council")).send().await?;
    assert!(
        resp.status().is_redirection(),
        "GET /council should redirect anonymous callers; got {}",
        resp.status()
    );
    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        location.ends_with("/login"),
        "GET /council should redirect to /login; got Location: {location}"
    );
    Ok(())
}

/// Protected story-of-the-day route redirects anonymous callers to `/login`.
#[tokio::test]
async fn get_story_today_redirects_anonymous_to_login() -> Result<()> {
    let (addr, client, _app) = spawn().await?;

    let resp = client
        .get(format!("http://{addr}/story/today"))
        .send()
        .await?;
    assert!(
        resp.status().is_redirection(),
        "GET /story/today should redirect anonymous callers; got {}",
        resp.status()
    );
    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        location.ends_with("/login"),
        "GET /story/today should redirect to /login; got Location: {location}"
    );
    Ok(())
}

/// Protected character-detail route redirects anonymous callers to `/login`.
#[tokio::test]
async fn get_council_detail_redirects_anonymous_to_login() -> Result<()> {
    let (addr, client, _app) = spawn().await?;

    let resp = client
        .get(format!("http://{addr}/council/lennon"))
        .send()
        .await?;
    assert!(
        resp.status().is_redirection(),
        "GET /council/{{id}} should redirect anonymous callers; got {}",
        resp.status()
    );
    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        location.ends_with("/login"),
        "GET /council/{{id}} should redirect to /login; got Location: {location}"
    );
    Ok(())
}

/// POST /logout without a valid CSRF token returns 403 Forbidden.
/// Guards the CSRF rule (AGENTS.md ground rule 1): every state-changing
/// route must verify the token before performing side effects.
#[tokio::test]
async fn post_logout_without_csrf_returns_403() -> Result<()> {
    let (addr, client, _app) = spawn().await?;

    let resp = client
        .post(format!("http://{addr}/logout"))
        .form(&[("_csrf", "not-a-real-token")])
        .send()
        .await?;

    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "POST /logout without a valid CSRF token must be rejected"
    );
    Ok(())
}
