//! Google Identity Services (GIS) sign-in.
//!
//! Flow:
//!   1. GET  /login              — page embeds Google's GIS button configured
//!      with our `client_id` (public) and a `data-login_uri` pointing at
//!      /auth/google/verify.
//!   2. User clicks the button; Google handles the entire sign-in UI (password,
//!      MFA, passkey) inside a Google-controlled iframe/popup. On success,
//!      Google's JS lib POSTs the browser to our /auth/google/verify endpoint
//!      with a form body containing:
//!        - `credential`     — signed ID token JWT
//!        - `g_csrf_token`   — random string; Google also sets it as a cookie
//!   3. The verify handler double-submits the g_csrf_token, verifies the JWT
//!      signature against Google's JWKS, checks `iss`/`aud`/`exp`, looks the
//!      email up in `AccessList`, upserts the user, and cycles the session id.
//!
//! We NEVER hold a client_secret. Google's public keys are the only trust
//! anchor. The `client_id` is embedded in the login-page HTML and safe to
//! read by anyone.
//!
//! The JwkCache refreshes lazily on unknown-`kid` misses because Google
//! rotates keys roughly every fortnight; on-miss refresh keeps us correct
//! without a background task. To keep an attacker who floods
//! /auth/google/verify with random-`kid` tokens from turning us into a
//! JWKS-fetch amplifier against Google, on-miss refreshes are rate-limited
//! by `JWKS_REFRESH_COOLDOWN` and coalesced by `refresh_mutex` — concurrent
//! misses across many workers only produce at most one outbound fetch per
//! cooldown window. The cooldown is keyed on refresh ATTEMPT time (recorded
//! before the outbound send), so a failing upstream cannot be repeatedly
//! re-hit inside the same window.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::{Mutex, RwLock};

pub const SESSION_USER_KEY: &str = "user";

const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const GOOGLE_ISSUERS: &[&str] = &["accounts.google.com", "https://accounts.google.com"];

/// Minimum wall-clock gap between two on-miss JWKS refreshes. Any unknown-`kid`
/// miss inside this window returns "kid not found" without contacting Google.
/// Google rotates roughly every fortnight; five minutes is generous for
/// legitimate rotation while denying attackers the ability to force repeated
/// outbound fetches. `pub` so integration tests can reason about the bound.
pub const JWKS_REFRESH_COOLDOWN: Duration = Duration::from_secs(300);

/// Session-stored identity. Kept minimal — cookie header size matters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUser {
    pub id: i64,
    pub email: String,
    pub display_name: String,
    /// True when the signed-in user has `admin = true` in
    /// `authorized-users.toml`. Persisted on the session, not the DB row —
    /// the source of truth is the committed file, checked at each sign-in.
    #[serde(default)]
    pub admin: bool,
}

/// Cached Google JWKS keyed by JWT header `kid`.
///
/// Google rotates roughly every fortnight. On an unknown-kid verify we
/// refresh at most once per `JWKS_REFRESH_COOLDOWN` window (measured from
/// refresh ATTEMPT, so upstream failures also count) and coalesce concurrent
/// refreshers through `refresh_mutex`; if the kid is still absent after a
/// permitted refresh — or the cooldown blocked the refresh entirely —
/// verification fails without a fresh fetch. This bounds outbound traffic
/// to Google under a flood of tokens with attacker-chosen random `kid`s
/// even when Google itself is returning errors.
pub struct JwkCache {
    keys: RwLock<HashMap<String, DecodingKey>>,
    http: reqwest::Client,
    jwks_url: String,
    /// `Some(t)` iff a `refresh()` has been *attempted* (successful or not) at
    /// least once; `t` is when the attempt began. Governs the on-miss cooldown
    /// gate. Tracking attempt time (not success time) is what makes upstream
    /// failures fall under the same amplification cap as successes.
    last_refresh_attempt: RwLock<Option<Instant>>,
    /// Held for the duration of a single `refresh()` call to prevent a
    /// stampede: N concurrent misses across workers turn into at most one
    /// outbound fetch. Must never be held across a `keys` write lock.
    refresh_mutex: Mutex<()>,
}

impl JwkCache {
    pub fn new(http: reqwest::Client) -> Self {
        Self::with_jwks_url(http, GOOGLE_JWKS_URL.to_string())
    }

    /// Test-only endpoint override; production MUST use [`Self::new`].
    #[doc(hidden)]
    pub fn with_jwks_url(http: reqwest::Client, jwks_url: String) -> Self {
        Self {
            keys: RwLock::new(HashMap::new()),
            http,
            jwks_url,
            last_refresh_attempt: RwLock::new(None),
            refresh_mutex: Mutex::new(()),
        }
    }

    /// Unconditional fetch from the configured JWKS URL. Records the attempt
    /// timestamp BEFORE the outbound send so that failed attempts still count
    /// against the on-miss cooldown — without this, a flood of unknown `kid`s
    /// against a returning-500 upstream would produce one outbound fetch per
    /// misser, defeating the amplification cap. Callers should prefer
    /// `refresh_if_stale` on the on-miss path; this is exposed only for the
    /// eager warm-up at boot in main.rs, where we want a hard failure if
    /// Google is unreachable so the operator sees it.
    pub async fn refresh(&self) -> Result<()> {
        *self.last_refresh_attempt.write().await = Some(Instant::now());

        let set: JwkSet = self
            .http
            .get(&self.jwks_url)
            .send()
            .await
            .context("fetching Google JWKS")?
            .error_for_status()
            .context("Google JWKS returned non-2xx")?
            .json()
            .await
            .context("decoding Google JWKS body")?;

        let mut map = HashMap::with_capacity(set.keys.len());
        for jwk in &set.keys {
            let Some(kid) = jwk.common.key_id.clone() else {
                continue;
            };
            let key = DecodingKey::from_jwk(jwk).context("building DecodingKey from JWK")?;
            map.insert(kid, key);
        }
        if map.is_empty() {
            return Err(anyhow!("Google JWKS response contained no usable keys"));
        }

        *self.keys.write().await = map;
        Ok(())
    }

    async fn get(&self, kid: &str) -> Option<DecodingKey> {
        self.keys.read().await.get(kid).cloned()
    }

    /// True when a fresh outbound refresh attempt is permitted (never
    /// attempted, or the last attempt was at least `JWKS_REFRESH_COOLDOWN`
    /// ago). Split out so tests can assert the decision without hitting HTTP.
    async fn should_refresh(&self) -> bool {
        match *self.last_refresh_attempt.read().await {
            None => true,
            Some(t) => t.elapsed() >= JWKS_REFRESH_COOLDOWN,
        }
    }

    /// Coalesced + cooldown-gated refresh used by `get_or_refresh`. Acquires
    /// `refresh_mutex` on EVERY cache miss (before checking the cooldown)
    /// so a late-arriving misser is forced to wait for any in-flight refresh
    /// to publish its keys before it re-checks the cache. Checking the
    /// cooldown *outside* the mutex would let a late arriver observe
    /// `last_refresh_attempt` being set — recorded at the start of the
    /// in-flight `refresh()` — skip the mutex, and then read the still-empty
    /// keys map for a spurious "kid not found" while the refresh is
    /// mid-flight.
    ///
    /// Returns:
    ///  * `Ok(true)`  — a fresh outbound attempt ran AND succeeded (keys
    ///    published, `last_refresh_attempt` updated).
    ///  * `Ok(false)` — no attempt ran because a recent attempt is already
    ///    on file (either observed before or after taking the mutex).
    ///  * `Err(_)`    — a fresh attempt ran and failed; `last_refresh_attempt`
    ///    was still updated (by `refresh()` before the outbound send), so
    ///    subsequent callers inside the cooldown window will skip.
    async fn refresh_if_stale(&self) -> Result<bool> {
        let _guard = self.refresh_mutex.lock().await;
        if !self.should_refresh().await {
            return Ok(false);
        }
        self.refresh().await?;
        Ok(true)
    }

    /// Return the key for `kid`, refreshing from Google on miss subject to
    /// the cooldown + singleflight gate. If the kid is still absent after
    /// a permitted refresh, or if the cooldown blocked the refresh entirely,
    /// verification fails — an attacker cannot force repeated outbound
    /// fetches by sending tokens with random `kid`s.
    async fn get_or_refresh(&self, kid: &str) -> Result<DecodingKey> {
        if let Some(k) = self.get(kid).await {
            return Ok(k);
        }
        self.refresh_if_stale().await?;
        self.get(kid)
            .await
            .ok_or_else(|| anyhow!("kid `{kid}` not found in Google JWKS after refresh"))
    }

    pub async fn len(&self) -> usize {
        self.keys.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.keys.read().await.is_empty()
    }
}

/// The subset of Google ID-token claims we care about.
///
/// `iss` / `aud` / `exp` are validated by `jsonwebtoken` via `Validation`;
/// they are not read from the struct.
#[derive(Debug, Deserialize)]
pub struct GoogleIdClaims {
    pub sub: String,
    pub email: String,
    #[serde(default)]
    pub email_verified: bool,
    #[serde(default)]
    pub name: Option<String>,
}

/// Verify a Google ID token end-to-end: signature, issuer, audience,
/// expiry, and `email_verified`. Returns the decoded claims on success.
pub async fn verify_id_token(
    jwks: &JwkCache,
    client_id: &str,
    id_token: &str,
) -> Result<GoogleIdClaims> {
    let header = decode_header(id_token).context("decoding JWT header")?;
    let Some(kid) = header.kid else {
        return Err(anyhow!("Google ID token header missing `kid`"));
    };
    let key = jwks.get_or_refresh(&kid).await?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[client_id]);
    validation.set_issuer(GOOGLE_ISSUERS);
    // 60s of clock skew tolerance; validate_exp is on by default.
    validation.leeway = 60;

    let data = decode::<GoogleIdClaims>(id_token, &key, &validation)
        .context("verifying Google ID token")?;

    if !data.claims.email_verified {
        return Err(anyhow!(
            "Google reports email `{}` as unverified; refusing to sign in",
            data.claims.email
        ));
    }
    Ok(data.claims)
}

/// Insert or update the user row keyed by `google_sub`, and return the
/// SessionUser payload the caller stashes in the session.
///
/// `admin` comes from the caller's `AccessList` lookup, not from Google.
///
/// Keying on `google_sub` (rather than email) means an allowed user who
/// legitimately changes their Gmail address on Google's side stays the
/// same row; the email column is updated in place.
pub async fn upsert_user(
    pool: &SqlitePool,
    claims: &GoogleIdClaims,
    admin: bool,
) -> Result<SessionUser> {
    let display = claims.name.clone().unwrap_or_else(|| claims.email.clone());
    let email_lower = claims.email.to_ascii_lowercase();

    sqlx::query(
        "INSERT INTO users (email, google_sub, display_name, last_login_at)
         VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(google_sub) DO UPDATE SET
             email = excluded.email,
             display_name = excluded.display_name,
             last_login_at = excluded.last_login_at",
    )
    .bind(&email_lower)
    .bind(&claims.sub)
    .bind(&display)
    .execute(pool)
    .await
    .context("upserting user on Google sign-in")?;

    // Fetch the row to get the id; the ON CONFLICT path doesn't produce
    // one via last_insert_rowid.
    let row: (i64, String, String) =
        sqlx::query_as("SELECT id, email, display_name FROM users WHERE google_sub = ?1")
            .bind(&claims.sub)
            .fetch_one(pool)
            .await
            .context("re-reading user row after upsert")?;

    Ok(SessionUser {
        id: row.0,
        email: row.1,
        display_name: row.2,
        admin,
    })
}

#[cfg(test)]
// Shared with tests/common/mod.rs; update both #[path] includes if moved.
#[path = "../tests/support/google_jwt.rs"]
mod google_jwt;

#[cfg(test)]
mod tests {
    // Coverage dimensions here (see .github/instructions/test-quality.instructions.md):
    //   * signed-token verification — signature, claims, issuer, audience,
    //     expiry/leeway, and verified-email policy
    //   * decision logic  — should_refresh_* (functional + state-transition)
    //   * amplification cap — flood_of_unknown_kids_produces_one_upstream_fetch
    //     (regression for the finding)
    //   * upstream failure — upstream_500_still_counts_against_cooldown
    //     (error / dependency-failure)
    //   * concurrency     — concurrent_misses_collapse_to_one_upstream_fetch
    //     (singleflight state-transition)
    //   * cooldown expiry — miss_after_cooldown_expiry_permits_second_attempt
    //
    use super::*;
    use axum::Router;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::get;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use time::OffsetDateTime;
    use tokio::net::TcpListener;
    use tokio::sync::Notify;

    use super::google_jwt::{GoogleJwtFixture, GoogleTokenClaims, TEST_CLIENT_ID};

    #[derive(Clone)]
    enum FakeResponse {
        /// 200 OK with `{"keys": []}` — well-formed but useless, which is
        /// exactly what "unknown kid" looks like from the cache's POV.
        EmptyKeys,
        /// 500 to simulate Google unavailable.
        Status500,
    }

    struct FakeJwks {
        url: String,
        hits: Arc<AtomicUsize>,
        release: Arc<Notify>,
        // Server runs in a spawned task; dropping the handle aborts it.
        _server: tokio::task::JoinHandle<()>,
    }

    impl FakeJwks {
        async fn spawn(response: FakeResponse) -> Self {
            Self::spawn_inner(response, /* gated */ false).await
        }

        /// Variant used by `late_arriver_waits_for_in_flight_refresh`: the
        /// handler increments `hits` immediately (so the test can observe
        /// task A has entered `refresh()`), then blocks on `release` until
        /// the test explicitly notifies it. Response body is `EmptyKeys`
        /// because the test doesn't need a positive verification.
        async fn spawn_gated() -> Self {
            Self::spawn_inner(FakeResponse::EmptyKeys, /* gated */ true).await
        }

        async fn spawn_inner(response: FakeResponse, gated: bool) -> Self {
            let hits = Arc::new(AtomicUsize::new(0));
            let release = Arc::new(Notify::new());
            let hits_for_handler = Arc::clone(&hits);
            let release_for_handler = Arc::clone(&release);
            let app = Router::new().route(
                "/certs",
                get(move || {
                    let hits = Arc::clone(&hits_for_handler);
                    let release = Arc::clone(&release_for_handler);
                    let response = response.clone();
                    async move {
                        hits.fetch_add(1, Ordering::SeqCst);
                        if gated {
                            release.notified().await;
                        }
                        match response {
                            FakeResponse::EmptyKeys => (
                                StatusCode::OK,
                                [("content-type", "application/json")],
                                r#"{"keys":[]}"#,
                            )
                                .into_response(),
                            FakeResponse::Status500 => {
                                StatusCode::INTERNAL_SERVER_ERROR.into_response()
                            }
                        }
                    }
                }),
            );
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind fake JWKS");
            let addr = listener.local_addr().expect("local_addr");
            let server = tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
            Self {
                url: format!("http://{addr}/certs"),
                hits,
                release,
                _server: server,
            }
        }

        fn hit_count(&self) -> usize {
            self.hits.load(Ordering::SeqCst)
        }

        /// Unblock the gated handler so it produces a response. No-op for
        /// non-gated fakes (nothing is waiting on the notify).
        fn release(&self) {
            self.release.notify_waiters();
        }
    }

    fn cache_pointed_at(url: &str) -> JwkCache {
        JwkCache::with_jwks_url(reqwest::Client::new(), url.to_string())
    }

    // ---- decision-logic tests (no HTTP) --------------------------------

    #[tokio::test]
    async fn verify_id_token_with_valid_signed_token_returns_claims() {
        let jwt = GoogleJwtFixture::spawn().await;
        let cache = cache_pointed_at(&jwt.jwks_url);
        assert!(cache.is_empty().await);

        let claims = verify_id_token(&cache, TEST_CLIENT_ID, &jwt.issue("test@example.com"))
            .await
            .expect("valid token verifies");

        assert_eq!(claims.sub, "subject-test@example.com");
        assert_eq!(claims.email, "test@example.com");
        assert!(claims.email_verified);
        assert_eq!(claims.name.as_deref(), Some("Test User"));
        assert!(!cache.is_empty().await);
        assert_eq!(cache.len().await, 1);
        assert_eq!(jwt.hit_count(), 1, "unknown kid should refresh once");
    }

    async fn verify_with_test_jwks(claims: &GoogleTokenClaims) -> Result<GoogleIdClaims> {
        let jwt = GoogleJwtFixture::spawn().await;
        let cache = cache_pointed_at(&jwt.jwks_url);
        verify_id_token(&cache, TEST_CLIENT_ID, &jwt.issue_claims(claims)).await
    }

    #[tokio::test]
    async fn verify_id_token_with_wrong_audience_returns_verification_error() {
        let mut claims = GoogleTokenClaims::valid("test@example.com");
        claims.aud = "other-client-id".into();

        let error = verify_with_test_jwks(&claims)
            .await
            .expect_err("wrong audience must fail");

        assert_eq!(error.to_string(), "verifying Google ID token");
    }

    #[tokio::test]
    async fn verify_id_token_with_wrong_issuer_returns_verification_error() {
        let mut claims = GoogleTokenClaims::valid("test@example.com");
        claims.iss = "https://attacker.example".into();

        let error = verify_with_test_jwks(&claims)
            .await
            .expect_err("wrong issuer must fail");

        assert_eq!(error.to_string(), "verifying Google ID token");
    }

    #[tokio::test]
    async fn verify_id_token_expired_beyond_leeway_returns_verification_error() {
        let mut claims = GoogleTokenClaims::valid("test@example.com");
        claims.exp = (OffsetDateTime::now_utc() - time::Duration::minutes(2)).unix_timestamp();

        let error = verify_with_test_jwks(&claims)
            .await
            .expect_err("expired token must fail");

        assert_eq!(error.to_string(), "verifying Google ID token");
    }

    #[tokio::test]
    async fn verify_id_token_expired_within_leeway_returns_claims() {
        let mut claims = GoogleTokenClaims::valid("test@example.com");
        claims.exp = (OffsetDateTime::now_utc() - time::Duration::seconds(30)).unix_timestamp();

        let verified = verify_with_test_jwks(&claims)
            .await
            .expect("clock skew within leeway verifies");

        assert_eq!(verified.sub, "subject-test@example.com");
    }

    #[tokio::test]
    async fn verify_id_token_signed_by_wrong_key_returns_verification_error() {
        let jwt = GoogleJwtFixture::spawn().await;
        let cache = cache_pointed_at(&jwt.jwks_url);

        let error = verify_id_token(
            &cache,
            TEST_CLIENT_ID,
            &jwt.issue_with_wrong_key("test@example.com"),
        )
        .await
        .expect_err("wrong signature must fail");

        assert_eq!(error.to_string(), "verifying Google ID token");
    }

    #[tokio::test]
    async fn verify_id_token_with_unverified_email_returns_named_error() {
        let mut claims = GoogleTokenClaims::valid("test@example.com");
        claims.email_verified = false;

        let error = verify_with_test_jwks(&claims)
            .await
            .expect_err("unverified email must fail");

        assert_eq!(
            error.to_string(),
            "Google reports email `test@example.com` as unverified; refusing to sign in"
        );
    }

    #[tokio::test]
    async fn should_refresh_true_when_never_attempted() {
        // Placeholder URL is never contacted — should_refresh does not fetch.
        let cache = cache_pointed_at("http://127.0.0.1:1/certs");
        assert!(cache.should_refresh().await);
    }

    #[tokio::test]
    async fn should_refresh_false_immediately_after_attempt() {
        let cache = cache_pointed_at("http://127.0.0.1:1/certs");
        *cache.last_refresh_attempt.write().await = Some(Instant::now());
        assert!(!cache.should_refresh().await);
    }

    #[tokio::test]
    async fn should_refresh_true_after_cooldown_elapsed() {
        let cache = cache_pointed_at("http://127.0.0.1:1/certs");
        let past = Instant::now()
            .checked_sub(JWKS_REFRESH_COOLDOWN + Duration::from_secs(1))
            .expect("test clock predates process start");
        *cache.last_refresh_attempt.write().await = Some(past);
        assert!(cache.should_refresh().await);
    }

    #[tokio::test]
    async fn cooldown_bounds_worst_case_fetch_rate() {
        // Regression guard on the constant itself: dropping this below one
        // minute would make the DoS gate porous under sustained flooding.
        assert!(JWKS_REFRESH_COOLDOWN >= Duration::from_secs(60));
    }

    // ---- amplification / regression tests (HTTP) ----------------------

    /// Regression for PR #1 Copilot finding F3: a flood of unknown-`kid`
    /// lookups must NOT translate to a corresponding flood of outbound
    /// fetches. Reverting `get_or_refresh` to call `refresh()`
    /// unconditionally makes this test fail (hit count == 5 instead of 1).
    #[tokio::test]
    async fn flood_of_unknown_kids_produces_one_upstream_fetch() {
        let fake = FakeJwks::spawn(FakeResponse::EmptyKeys).await;
        let cache = cache_pointed_at(&fake.url);

        for _ in 0..5 {
            // We expect an error (empty JWKS + unknown kid); the SHAPE of
            // the error is not the point of this test, the hit count is.
            let result = cache.get_or_refresh("attacker-kid").await;
            assert!(result.is_err(), "unknown kid must not verify");
        }
        assert_eq!(
            fake.hit_count(),
            1,
            "cooldown must cap outbound fetches to one per window"
        );
    }

    /// Copilot F3 explicitly called out amplification, but a naive fix that
    /// only records `last_refresh` on success would still leak under an
    /// upstream that returns 500. This test locks that in: even when the
    /// fake JWKS is down, only ONE outbound attempt happens per window.
    #[tokio::test]
    async fn upstream_500_still_counts_against_cooldown() {
        let fake = FakeJwks::spawn(FakeResponse::Status500).await;
        let cache = cache_pointed_at(&fake.url);

        for _ in 0..5 {
            let result = cache.get_or_refresh("attacker-kid").await;
            assert!(
                result.is_err(),
                "unknown kid against 500-upstream must not verify"
            );
        }
        assert_eq!(
            fake.hit_count(),
            1,
            "failed refreshes must still consume the cooldown budget"
        );
    }

    /// Singleflight coverage: N concurrent misses across tasks must collapse
    /// to a single outbound fetch, not N. Without `refresh_mutex` this test
    /// non-deterministically produces hit counts up to 10.
    #[tokio::test]
    async fn concurrent_misses_collapse_to_one_upstream_fetch() {
        let fake = FakeJwks::spawn(FakeResponse::EmptyKeys).await;
        let cache = Arc::new(cache_pointed_at(&fake.url));

        let mut joins = Vec::new();
        for _ in 0..10 {
            let cache = Arc::clone(&cache);
            joins.push(tokio::spawn(async move {
                cache.get_or_refresh("attacker-kid").await.is_err()
            }));
        }
        for j in joins {
            let was_err = j.await.expect("task panicked");
            assert!(
                was_err,
                "unknown kid must not verify from any concurrent task"
            );
        }
        assert_eq!(
            fake.hit_count(),
            1,
            "singleflight must coalesce concurrent refreshers"
        );
    }

    /// After the cooldown window elapses, a subsequent miss IS allowed to
    /// try again — the gate is a rate limiter, not a permanent seal. Fakes
    /// the passage of time by rewriting `last_refresh_attempt` (real sleep
    /// would push the test past its 60s wallclock budget in CI).
    #[tokio::test]
    async fn miss_after_cooldown_expiry_permits_second_attempt() {
        let fake = FakeJwks::spawn(FakeResponse::EmptyKeys).await;
        let cache = cache_pointed_at(&fake.url);

        let first = cache.get_or_refresh("kid-a").await;
        assert!(first.is_err(), "unknown kid must not verify");
        assert_eq!(fake.hit_count(), 1);

        // Simulate cooldown expiry.
        *cache.last_refresh_attempt.write().await = Some(
            Instant::now()
                .checked_sub(JWKS_REFRESH_COOLDOWN + Duration::from_secs(1))
                .expect("test clock predates process start"),
        );

        let second = cache.get_or_refresh("kid-b").await;
        assert!(second.is_err(), "unknown kid must not verify");
        assert_eq!(
            fake.hit_count(),
            2,
            "cooldown expiry must re-enable outbound refresh"
        );
    }

    /// Regression for the mutex-vs-cooldown race that GPT-5.5 flagged in the
    /// first follow-up review of PR #1: without acquiring `refresh_mutex`
    /// BEFORE checking the cooldown, a late arriver can observe
    /// `last_refresh_attempt` being set (recorded at the start of an
    /// in-flight `refresh()`), skip the mutex, and read the still-empty
    /// keys map for a spurious "kid not found" while the refresh is
    /// mid-flight.
    ///
    /// The test gates the fake JWKS on a `Notify`, starts one caller
    /// (task A) which blocks inside `refresh()`, and asserts that a second
    /// caller (task B) arriving while task A is mid-flight does NOT complete
    /// until we explicitly release the fake. The buggy pre-fix flow would
    /// let task B race past the mutex and complete promptly with an error.
    #[tokio::test]
    async fn late_arriver_waits_for_in_flight_refresh() {
        let fake = FakeJwks::spawn_gated().await;
        let cache = Arc::new(cache_pointed_at(&fake.url));

        let cache_a = Arc::clone(&cache);
        let task_a = tokio::spawn(async move { cache_a.get_or_refresh("kid").await.is_err() });

        // Wait until task A has actually reached the fake handler; using
        // yield_now avoids a wallclock-sensitive sleep.
        while fake.hit_count() == 0 {
            tokio::task::yield_now().await;
        }

        let cache_b = Arc::clone(&cache);
        let mut task_b = tokio::spawn(async move { cache_b.get_or_refresh("kid").await.is_err() });

        // If the mutex-first fix is missing, task B races past the singleflight
        // and completes within microseconds while task A is still gated.
        tokio::select! {
            biased;
            r = &mut task_b => panic!(
                "task B raced past in-flight refresh; join result = {:?}",
                r
            ),
            _ = tokio::time::sleep(Duration::from_millis(50)) => {}
        }

        // Release the fake so task A can finish; task B, previously blocked
        // on the mutex, will then be scheduled and complete too.
        fake.release();

        let a_err = task_a.await.expect("task A panicked");
        let b_err = task_b.await.expect("task B panicked");
        assert!(a_err && b_err, "both callers must see unknown-kid error");
        assert_eq!(
            fake.hit_count(),
            1,
            "only one outbound attempt allowed under singleflight"
        );
    }
}
