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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use reqwest::StatusCode;
use reqwest::header::{COOKIE, LOCATION, SET_COOKIE};
use reqwest::redirect::Policy;
use tokio::net::TcpListener;
use tower_sessions::Session;
use tower_sessions_sqlx_store::SqliteStore;

use common::jwt::GoogleJwtFixture;
use common::{build_test_app, build_test_app_with_jwks_url};
use stuffy_council::stories::{StoryGenerationError, StoryGenerationResult, StoryGenerator};

use common::build_test_app_with_story_generator_and_jwks_url;

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

    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(5))
        .build()?;

    // Poll rather than sleep a fixed interval: the listener is already bound, so
    // connections queue while `serve` runs session-store migrations, and a loaded
    // machine can push that past any blind wait.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        match client.get(format!("http://{addr}/healthz")).send().await {
            Ok(response) if response.status().is_success() => break,
            _ if tokio::time::Instant::now() >= deadline => {
                anyhow::bail!("test app did not become ready within 10s")
            }
            _ => tokio::time::sleep(Duration::from_millis(10)).await,
        }
    }

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

fn session_cookie_from(response: &reqwest::Response) -> String {
    response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("stuffy_session="))
        .expect("response sets session cookie")
        .split(';')
        .next()
        .expect("session cookie pair")
        .to_string()
}

async fn sign_in_allowed(
    addr: SocketAddr,
    client: &reqwest::Client,
    jwt: &GoogleJwtFixture,
) -> Result<String> {
    sign_in_as(addr, client, jwt, "test@example.com").await
}

async fn sign_in_as(
    addr: SocketAddr,
    client: &reqwest::Client,
    jwt: &GoogleJwtFixture,
    email: &str,
) -> Result<String> {
    let response = post_google_verify(
        addr,
        client,
        &jwt.issue(email),
        "matching-token",
        "matching-token",
        None,
    )
    .await?;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    Ok(session_cookie_from(&response))
}

enum StubStoryFailure {
    Unavailable,
    Internal,
}

struct StubStoryGenerator;

#[async_trait]
impl StoryGenerator for StubStoryGenerator {
    fn model_id(&self) -> &str {
        "stub-test-generator"
    }

    async fn generate(&self, _prompt: &str) -> StoryGenerationResult<String> {
        Ok("The Council convened.\n\nThen it adjourned for snacks.".to_string())
    }
}

struct FailingStoryGenerator {
    failure: StubStoryFailure,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl StoryGenerator for FailingStoryGenerator {
    fn model_id(&self) -> &str {
        "failing-test-generator"
    }

    async fn generate(&self, _prompt: &str) -> StoryGenerationResult<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.failure {
            StubStoryFailure::Unavailable => Err(StoryGenerationError::Unavailable(
                anyhow::anyhow!("test generator unavailable"),
            )),
            StubStoryFailure::Internal => Err(StoryGenerationError::Internal(anyhow::anyhow!(
                "sensitive internal test failure"
            ))),
        }
    }
}

#[tokio::test]
async fn get_story_today_generator_unavailable_returns_friendly_200() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let app = build_test_app_with_story_generator_and_jwks_url(
        Arc::new(FailingStoryGenerator {
            failure: StubStoryFailure::Unavailable,
            calls: Arc::clone(&calls),
        }),
        Some(&jwt.jwks_url),
    )
    .await?;
    let (addr, client, _app) = spawn_test_app(app).await?;
    let session_cookie = sign_in_allowed(addr, &client, &jwt).await?;

    let response = client
        .get(format!("http://{addr}/story/today"))
        .header(COOKIE, &session_cookie)
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await?;
    assert!(body.contains("The story elf is offline. Try again shortly."));
    assert!(!body.contains("Ollama"));
    assert!(!body.contains("sensitive"));

    let retry = client
        .get(format!("http://{addr}/story/today"))
        .header(COOKIE, session_cookie)
        .send()
        .await?;
    assert_eq!(retry.status(), StatusCode::OK);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "temporary failures must not be cached"
    );
    Ok(())
}

#[tokio::test]
async fn get_story_today_internal_generator_error_returns_generic_500() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let app = build_test_app_with_story_generator_and_jwks_url(
        Arc::new(FailingStoryGenerator {
            failure: StubStoryFailure::Internal,
            calls: Arc::new(AtomicUsize::new(0)),
        }),
        Some(&jwt.jwks_url),
    )
    .await?;
    let (addr, client, _app) = spawn_test_app(app).await?;
    let session_cookie = sign_in_allowed(addr, &client, &jwt).await?;

    let response = client
        .get(format!("http://{addr}/story/today"))
        .header(COOKIE, session_cookie)
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response.text().await?;
    assert_eq!(body, "Something went sideways backstage. Please try again.");
    assert!(!body.contains("sensitive internal test failure"));
    Ok(())
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
    let session_cookie = session_cookie_from(&response);
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
    let headers = resp.headers().clone();
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
    assert!(headers.contains_key("content-security-policy"));
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
    assert!(!headers.contains_key("strict-transport-security"));
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

/// Regression for #9: the shared layout must pull no third-party script. HTMX
/// was loaded from unpkg but never used, and its presence forced `unpkg.com`
/// into `script-src` for every page.
#[tokio::test]
async fn shared_layout_loads_no_third_party_script() -> Result<()> {
    let (addr, client, _app) = spawn().await?;
    let body = client
        .get(format!("http://{addr}/login"))
        .send()
        .await?
        .text()
        .await?;
    assert!(
        !body.contains("unpkg.com"),
        "layout still loads a script from unpkg. body: {body}"
    );
    assert!(
        !body.contains("htmx"),
        "layout still references HTMX; drop the dependency or re-add it to script-src. body: {body}"
    );
    Ok(())
}

/// #9: `/login` is the ONLY route allowed to name a third-party origin, and
/// the only one carrying `'unsafe-inline'`, because Google Identity Services
/// requires both. Every relaxation GIS depends on is asserted — dropping any
/// one of them breaks sign-in without breaking any other test.
#[tokio::test]
async fn login_response_carries_the_google_scoped_csp() -> Result<()> {
    let (addr, client, _app) = spawn().await?;
    let resp = client.get(format!("http://{addr}/login")).send().await?;
    let csp = resp
        .headers()
        .get("content-security-policy")
        .expect("login CSP")
        .to_str()?
        .to_owned();

    for required in [
        "script-src 'self' https://accounts.google.com/gsi/client",
        "style-src 'self' 'unsafe-inline' https://accounts.google.com/gsi/style",
        "img-src 'self' data: https://*.googleusercontent.com",
        "connect-src 'self' https://accounts.google.com/gsi/",
        "frame-src https://accounts.google.com/gsi/",
        "frame-ancestors 'none'",
    ] {
        assert!(
            csp.contains(required),
            "login CSP lost a GIS requirement `{required}`. got: {csp}"
        );
    }
    assert!(
        !csp.contains("unpkg.com"),
        "login CSP still admits unpkg. got: {csp}"
    );
    Ok(())
}

fn assert_strict_csp(path: &str, csp: &str) {
    // Allowlist every source token rather than banning known-bad origins, so a
    // newly introduced third party fails here instead of slipping through.
    for directive in csp.split("; ") {
        let mut tokens = directive.trim_end_matches(';').split_whitespace();
        let name = tokens.next().unwrap_or_default();
        for source in tokens {
            assert!(
                matches!(source, "'self'" | "'none'" | "data:"),
                "{path} {name} admits non-same-origin source `{source}` outside /login. got: {csp}"
            );
        }
    }
    assert!(
        csp.contains("script-src 'self';"),
        "{path} widened script-src. got: {csp}"
    );
    assert!(
        csp.contains("frame-ancestors 'none'"),
        "{path} lost clickjacking protection. got: {csp}"
    );
}

fn csp_of(response: &reqwest::Response, path: &str) -> Result<String> {
    Ok(response
        .headers()
        .get("content-security-policy")
        .unwrap_or_else(|| panic!("{path} sent no CSP"))
        .to_str()?
        .to_owned())
}

/// #9: every route other than `/login` gets the strict policy — no inline
/// execution and no third-party origin at all.
#[tokio::test]
async fn anonymous_non_login_routes_carry_the_strict_base_csp() -> Result<()> {
    let (addr, client, _app) = spawn().await?;

    for path in [
        "/healthz",
        "/",
        "/council",
        "/council/ruff-ruff",
        "/story/today",
        "/static/app.css",
        "/no-such-route",
    ] {
        let resp = client.get(format!("http://{addr}{path}")).send().await?;
        assert_strict_csp(path, &csp_of(&resp, path)?);
    }

    let logout = client.post(format!("http://{addr}/logout")).send().await?;
    assert_strict_csp("/logout", &csp_of(&logout, "/logout")?);

    // Google's cross-origin POST lands here; it redirects rather than rendering,
    // so it needs no GIS relaxation of its own.
    let verify =
        post_google_verify(addr, &client, "unused", "cookie-token", "form-token", None).await?;
    assert_strict_csp(
        "/auth/google/verify",
        &csp_of(&verify, "/auth/google/verify")?,
    );
    Ok(())
}

/// Regression for the layer ordering in `lib::serve`: the security-header
/// layers must sit OUTSIDE the rate limiter, or responses the middleware
/// synthesizes itself never pass through them and ship with no CSP at all.
#[tokio::test]
async fn rate_limited_response_still_carries_the_strict_base_csp() -> Result<()> {
    let app = common::build_test_app_rejecting_burst_traffic().await?;
    let (addr, client, _app) = spawn_test_app(app).await?;

    let mut rejected = None;
    for _ in 0..10 {
        let resp = client.get(format!("http://{addr}/healthz")).send().await?;
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            rejected = Some(resp);
            break;
        }
    }
    let rejected = rejected.expect("burst traffic should trip the rate limiter");
    assert_strict_csp("429", &csp_of(&rejected, "429")?);
    Ok(())
}

/// data must carry the strict policy, so an escape past Askama's auto-escaping
/// still cannot execute. The anonymous variant above only ever sees redirects.
#[tokio::test]
async fn authenticated_rendered_pages_carry_the_strict_base_csp() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let app = build_test_app_with_story_generator_and_jwks_url(
        Arc::new(StubStoryGenerator),
        Some(&jwt.jwks_url),
    )
    .await?;
    let (addr, client, _app) = spawn_test_app(app).await?;
    let session = sign_in_allowed(addr, &client, &jwt).await?;

    for path in ["/", "/council", "/council/ruff-ruff", "/story/today"] {
        let resp = client
            .get(format!("http://{addr}{path}"))
            .header(COOKIE, session.clone())
            .send()
            .await?;
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "{path} did not render an authenticated page"
        );
        assert_strict_csp(path, &csp_of(&resp, path)?);
    }
    Ok(())
}

/// Boots the app with the real cast and a stub generator, signs in, and
/// materializes today's story so the archive has a row to serve.
async fn spawn_with_one_archived_story(
    jwt: &GoogleJwtFixture,
    email: &str,
) -> Result<(SocketAddr, reqwest::Client, common::TestApp, String, String)> {
    let app = build_test_app_with_story_generator_and_jwks_url(
        Arc::new(StubStoryGenerator),
        Some(&jwt.jwks_url),
    )
    .await?;
    let (addr, client, app) = spawn_test_app(app).await?;
    let session = sign_in_as(addr, &client, jwt, email).await?;

    let today = time::OffsetDateTime::now_utc().date().to_string();
    let generated = client
        .get(format!("http://{addr}/story/today"))
        .header(COOKIE, session.clone())
        .send()
        .await?;
    assert_eq!(generated.status(), StatusCode::OK, "seeding today's story");

    Ok((addr, client, app, session, today))
}

#[tokio::test]
async fn get_story_archive_lists_a_generated_story_with_a_link_to_its_date() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let (addr, client, _app, session, today) =
        spawn_with_one_archived_story(&jwt, "test@example.com").await?;

    let body = client
        .get(format!("http://{addr}/story"))
        .header(COOKIE, session)
        .send()
        .await?
        .text()
        .await?;

    assert!(
        body.contains(&format!("/story/{today}")),
        "archive should link today's story. body: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn get_story_by_date_renders_the_archived_story() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let (addr, client, _app, session, today) =
        spawn_with_one_archived_story(&jwt, "test@example.com").await?;

    let resp = client
        .get(format!("http://{addr}/story/{today}"))
        .header(COOKIE, session)
        .send()
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await?;

    assert!(
        body.contains("The Council convened"),
        "archived story body should render. body: {body}"
    );
    Ok(())
}

/// `/story/today` is a static segment and must keep winning over the
/// `/story/{date}` parameter, which would otherwise reject it as a bad date.
#[tokio::test]
async fn get_story_today_still_routes_to_the_generating_handler() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let app = build_test_app_with_story_generator_and_jwks_url(
        Arc::new(StubStoryGenerator),
        Some(&jwt.jwks_url),
    )
    .await?;
    let (addr, client, _app) = spawn_test_app(app).await?;
    let session = sign_in_allowed(addr, &client, &jwt).await?;

    let resp = client
        .get(format!("http://{addr}/story/today"))
        .header(COOKIE, session)
        .send()
        .await?;

    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "/story/today must not fall through to the date parameter"
    );
    assert!(resp.text().await?.contains("The Council convened"));
    Ok(())
}

#[tokio::test]
async fn get_story_by_date_with_no_cached_story_returns_404() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let (addr, client, _app, session, _today) =
        spawn_with_one_archived_story(&jwt, "test@example.com").await?;

    let resp = client
        .get(format!("http://{addr}/story/1999-01-01"))
        .header(COOKIE, session)
        .send()
        .await?;

    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "an uncached date must 404, never generate"
    );
    Ok(())
}

#[tokio::test]
async fn get_story_by_date_with_unparseable_date_returns_400() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let (addr, client, _app, session, _today) =
        spawn_with_one_archived_story(&jwt, "test@example.com").await?;

    for bad in ["not-a-date", "2026-13-45", "2026-02-30", "%27%20OR%201%3D1"] {
        let resp = client
            .get(format!("http://{addr}/story/{bad}"))
            .header(COOKIE, session.clone())
            .send()
            .await?;
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "`{bad}` must be rejected before reaching SQL"
        );
    }
    Ok(())
}

#[tokio::test]
async fn get_story_archive_redirects_anonymous_to_login() -> Result<()> {
    let (addr, client, _app) = spawn().await?;

    for path in ["/story", "/story/2026-07-31"] {
        let resp = client.get(format!("http://{addr}{path}")).send().await?;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{path} must be gated");
        assert_eq!(resp.headers().get(LOCATION).unwrap(), "/login");
    }
    Ok(())
}

#[tokio::test]
async fn get_admin_export_returns_every_stored_column_for_an_admin() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let (addr, client, _app, session, today) =
        spawn_with_one_archived_story(&jwt, "test@example.com").await?;

    let resp = client
        .get(format!("http://{addr}/admin/stories.json"))
        .header(COOKIE, session)
        .send()
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let rows: serde_json::Value = resp.json().await?;

    let row = rows.get(0).expect("exported story row");
    assert_eq!(row["story_date"], today);
    // A backup missing prompt or model cannot reproduce or audit the story.
    for column in [
        "story_date",
        "title",
        "body",
        "cast_json",
        "model",
        "prompt",
        "created_at",
    ] {
        assert!(
            row.get(column).is_some_and(|v| !v.is_null()),
            "export dropped `{column}`, so the backup is not restorable: {row}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn get_admin_export_forbidden_for_signed_in_non_admin() -> Result<()> {
    let jwt = GoogleJwtFixture::spawn().await;
    let (addr, client, _app, session, _today) =
        spawn_with_one_archived_story(&jwt, "viewer@example.com").await?;

    let resp = client
        .get(format!("http://{addr}/admin/stories.json"))
        .header(COOKIE, session)
        .send()
        .await?;

    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "a signed-in non-admin must not read the archive dump"
    );
    assert!(!resp.text().await?.contains("The Council convened"));
    Ok(())
}

#[tokio::test]
async fn get_admin_export_redirects_anonymous_to_login() -> Result<()> {
    let (addr, client, _app) = spawn().await?;

    let resp = client
        .get(format!("http://{addr}/admin/stories.json"))
        .send()
        .await?;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(LOCATION).unwrap(), "/login");
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
