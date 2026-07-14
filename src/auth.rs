//! Google OAuth 2.0 sign-in.
//!
//! Flow:
//!   1. GET  /auth/google           — mint state + PKCE, stash in session, 302 to Google
//!   2. GET  /auth/google/callback  — verify state, exchange code, fetch userinfo,
//!                                    check allowlist, upsert user, cycle session id,
//!                                    stash SessionUser, redirect home
//!
//! We deliberately do NOT validate the ID token JWT ourselves; we treat
//! Google's TLS + a successful `userinfo` fetch (which requires the access
//! token we just received over TLS from token_uri) as the trust anchor.
//! `email_verified` on the userinfo response is enforced.

use anyhow::{Context, Result, anyhow};
use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenUrl,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::config::Config;

pub const SESSION_USER_KEY: &str = "user";
pub const OAUTH_STATE_KEY: &str = "oauth_state";

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://openidconnect.googleapis.com/v1/userinfo";

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

/// Transient values that must survive the redirect to Google and back.
/// Stashed in the session before we hand the browser off to Google.
#[derive(Debug, Serialize, Deserialize)]
pub struct PendingOAuth {
    pub csrf: String,
    pub pkce_verifier: String,
}

/// Fully-configured oauth2 client (all four endpoints set).
///
/// The type-state on `BasicClient` reflects which endpoints are populated;
/// we always populate auth, token, client-secret, and redirect, so the
/// alias below is the only shape callers should see.
pub type GoogleOAuthClient = BasicClient<
    EndpointSet,      // auth_uri
    EndpointNotSet,   // device_authorization_uri (unused)
    EndpointNotSet,   // introspection_uri (unused)
    EndpointNotSet,   // revocation_uri (unused)
    EndpointSet,      // token_uri
>;

pub fn google_client(cfg: &Config) -> Result<GoogleOAuthClient> {
    let client = BasicClient::new(ClientId::new(cfg.google_client_id.clone()))
        .set_client_secret(ClientSecret::new(cfg.google_client_secret.clone()))
        .set_auth_uri(
            AuthUrl::new(GOOGLE_AUTH_URL.to_string()).context("hardcoded auth_url invalid")?,
        )
        .set_token_uri(
            TokenUrl::new(GOOGLE_TOKEN_URL.to_string()).context("hardcoded token_url invalid")?,
        )
        .set_redirect_uri(
            RedirectUrl::new(cfg.google_redirect_url.clone())
                .context("GOOGLE_REDIRECT_URL is not a valid URL")?,
        );
    Ok(client)
}

/// Build the authorize URL + the state we need to remember while the
/// browser is off talking to Google.
pub fn begin_authorization(client: &GoogleOAuthClient) -> (url::Url, PendingOAuth) {
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();
    let pending = PendingOAuth {
        csrf: csrf_token.secret().clone(),
        pkce_verifier: pkce_verifier.secret().clone(),
    };
    (auth_url, pending)
}

/// Exchange the authorization code from the callback for an access token.
///
/// The `http_client` MUST be configured with `redirect(reqwest::redirect::Policy::none())`
/// per the oauth2 crate's security guidance — otherwise a malicious token endpoint
/// could redirect us to an attacker-controlled host.
pub async fn exchange_code(
    client: &GoogleOAuthClient,
    http_client: &reqwest::Client,
    code: String,
    pkce_verifier: String,
) -> Result<BasicTokenResponse> {
    client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
        .request_async(http_client)
        .await
        .context("Google token exchange failed")
}

/// Google's OIDC userinfo response subset we actually use.
#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub sub: String,
    pub email: String,
    #[serde(default)]
    pub email_verified: bool,
    /// Google returns `name` as the display name (concatenated given + family
    /// with locale awareness). Falls back to email if for some reason absent.
    #[serde(default)]
    pub name: Option<String>,
}

pub async fn fetch_userinfo(
    http_client: &reqwest::Client,
    access_token: &str,
) -> Result<GoogleUserInfo> {
    let info: GoogleUserInfo = http_client
        .get(GOOGLE_USERINFO_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .context("calling Google userinfo endpoint")?
        .error_for_status()
        .context("Google userinfo returned non-2xx")?
        .json()
        .await
        .context("decoding Google userinfo body")?;
    if !info.email_verified {
        return Err(anyhow!(
            "Google reports email `{}` as unverified; refusing to sign in",
            info.email
        ));
    }
    Ok(info)
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
    info: &GoogleUserInfo,
    admin: bool,
) -> Result<SessionUser> {
    let display = info.name.clone().unwrap_or_else(|| info.email.clone());
    let email_lower = info.email.to_ascii_lowercase();

    sqlx::query(
        "INSERT INTO users (email, google_sub, display_name, last_login_at)
         VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(google_sub) DO UPDATE SET
             email = excluded.email,
             display_name = excluded.display_name,
             last_login_at = excluded.last_login_at",
    )
    .bind(&email_lower)
    .bind(&info.sub)
    .bind(&display)
    .execute(pool)
    .await
    .context("upserting user on Google login")?;

    // Fetch the row to get the id; we cannot rely on `last_insert_rowid`
    // because the ON CONFLICT path doesn't produce one.
    let row: (i64, String, String) = sqlx::query_as(
        "SELECT id, email, display_name FROM users WHERE google_sub = ?1",
    )
    .bind(&info.sub)
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
