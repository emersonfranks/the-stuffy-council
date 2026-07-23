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
//! 4. **Google sign-in traverses the real auth stack.** Signed local JWTs
//!    exercise CSRF, JWKS verification, allowlist, DB upsert, session rotation,
//!    and the resulting authenticated request without contacting Google.

mod common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use reqwest::StatusCode;
use reqwest::header::{COOKIE, LOCATION, SET_COOKIE};
use reqwest::redirect::Policy;
use tokio::net::TcpListener;
use tower_sessions::Session;
use tower_sessions_sqlx_store::SqliteStore;

use common::jwt::GoogleJwtFixture;
use common::{build_test_app, build_test_app_with_jwks_url};

/// Spin up the real app on an ephemeral port and return a `reqwest::Client`
/// pre-configured to NOT follow redirects (so tests observe the 3xx).
async fn spawn() -> Result<(SocketAddr, reqwest::Client, common::TestApp)> {
    let app = build_test_app().await?;
    spawn_test_app(app).await
}

async fn spawn_with_jwks_url(
    jwks_url: &str,
) -> Result<(SocketAddr, reqwest::Client, common::TestApp)> {
    let app = build_test_app_with_jwks_url(Some(jwks_url)).await?;
    spawn_test_app(app).await
}

async fn spawn_test_app(
    app: common::TestApp,
) -> Result<(SocketAddr, reqwest::Client, common::TestApp)> {
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

async fn post_google_verify(
    addr: SocketAddr,
    client: &reqwest::Client,
    credential: &str,
    cookie_token: &str,
    form_token: &str,
    session_cookie: Option<&str>,
) -> Result<reqwest::Response> {
    let cookie_header = match session_cookie {
        Some(session_cookie) => format!("g_csrf_token={cookie_token}; {session_cookie}"),
        None => format!("g_csrf_token={cookie_token}"),
    };
    Ok(client
        .post(format!("http://{addr}/auth/google/verify"))
        .header(COOKIE, cookie_header)
        .form(&[("credential", credential), ("g_csrf_token", form_token)])
        .send()
        .await?)
}

async fn seed_anonymous_session(app: &common::TestApp) -> Result<String> {
    let store = Arc::new(SqliteStore::new(app.state.db.clone()));
    let session = Session::new(None, store, None);
    session.insert("anonymous_marker", true).await?;
    session.save().await?;
    let id = session.id().expect("saved session has id");
    Ok(format!("stuffy_session={id}"))
}

#[tokio::test]
async fn post_google_verify_valid_allowed_token_sets_authenticated_session() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let (addr, client, app) = spawn_with_jwks_url(&jwt.jwks_url).await?;
    let anonymous_cookie = seed_anonymous_session(&app).await?;

    let response = post_google_verify(
        addr,
        &client,
        &jwt.issue("test@example.com"),
        "matching-token",
        "matching-token",
        Some(&anonymous_cookie),
    )
    .await?;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers().get(LOCATION).unwrap(), "/");
    assert_eq!(
        jwt.hit_count(),
        1,
        "sign-in should fetch the local JWKS once"
    );
    let session_cookie = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("stuffy_session="))
        .expect("successful sign-in sets session cookie")
        .split(';')
        .next()
        .expect("session cookie pair")
        .to_string();
    assert_ne!(
        session_cookie, anonymous_cookie,
        "sign-in must rotate session id"
    );

    let authenticated = client
        .get(format!("http://{addr}/"))
        .header(COOKIE, session_cookie)
        .send()
        .await?;
    assert_eq!(authenticated.status(), StatusCode::OK);

    let stale_session = client
        .get(format!("http://{addr}/"))
        .header(COOKIE, anonymous_cookie)
        .send()
        .await?;
    assert_eq!(stale_session.status(), StatusCode::SEE_OTHER);
    assert_eq!(stale_session.headers().get(LOCATION).unwrap(), "/login");
    Ok(())
}

#[tokio::test]
async fn post_google_verify_valid_off_allowlist_token_redirects_denied() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let (addr, client, _app) = spawn_with_jwks_url(&jwt.jwks_url).await?;

    let response = post_google_verify(
        addr,
        &client,
        &jwt.issue("not-allowed@example.com"),
        "matching-token",
        "matching-token",
        None,
    )
    .await?;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get(LOCATION).unwrap(),
        "/login?error=denied"
    );
    assert!(
        response
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .all(|value| !value
                .to_str()
                .unwrap_or_default()
                .starts_with("stuffy_session=")),
        "denied sign-in must not create a session"
    );
    Ok(())
}

#[tokio::test]
async fn post_google_verify_mismatched_csrf_redirects_csrf() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let (addr, client, _app) = spawn_with_jwks_url(&jwt.jwks_url).await?;

    let response = post_google_verify(
        addr,
        &client,
        &jwt.issue("test@example.com"),
        "cookie-token",
        "form-token",
        None,
    )
    .await?;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get(LOCATION).unwrap(),
        "/login?error=csrf"
    );
    Ok(())
}

#[tokio::test]
async fn post_google_verify_token_signed_by_wrong_key_redirects_google_error() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let (addr, client, _app) = spawn_with_jwks_url(&jwt.jwks_url).await?;

    let response = post_google_verify(
        addr,
        &client,
        &jwt.issue_with_wrong_key("test@example.com"),
        "matching-token",
        "matching-token",
        None,
    )
    .await?;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get(LOCATION).unwrap(),
        "/login?error=google"
    );
    Ok(())
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
async fn static_ruff_ruff_portrait_is_served_as_png() -> Result<()> {
    assert_static_png_is_served("/static/stuffies/ruff-ruff.png").await
}

#[tokio::test]
async fn static_bar_bar_portrait_is_served_as_png() -> Result<()> {
    assert_static_png_is_served("/static/stuffies/bar-bar.png").await
}

#[tokio::test]
async fn static_bar_bar_angry_variant_is_served_as_png() -> Result<()> {
    assert_static_png_is_served("/static/stuffies/bar-bar--angry.png").await
}

#[tokio::test]
async fn static_woofy_portrait_is_served_as_png() -> Result<()> {
    assert_static_png_is_served("/static/stuffies/woofy.png").await
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
