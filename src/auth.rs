//! Google Identity Services (GIS) sign-in.
//!
//! Flow:
//!   1. GET  /login              — page embeds Google's GIS button configured
//!                                 with our `client_id` (public) and a
//!                                 `data-login_uri` pointing at /auth/google/verify.
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
//! without a background task.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::RwLock;

pub const SESSION_USER_KEY: &str = "user";

const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const GOOGLE_ISSUERS: &[&str] = &["accounts.google.com", "https://accounts.google.com"];

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
/// Google rotates roughly every fortnight. On an unknown-kid verify, we
/// refresh once and retry; if the kid is still absent, verification fails.
pub struct JwkCache {
    keys: RwLock<HashMap<String, DecodingKey>>,
    http: reqwest::Client,
}

impl JwkCache {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            keys: RwLock::new(HashMap::new()),
            http,
        }
    }

    pub async fn refresh(&self) -> Result<()> {
        let set: JwkSet = self
            .http
            .get(GOOGLE_JWKS_URL)
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

    /// Return the key for `kid`, refreshing from Google on miss.
    async fn get_or_refresh(&self, kid: &str) -> Result<DecodingKey> {
        if let Some(k) = self.get(kid).await {
            return Ok(k);
        }
        self.refresh().await?;
        self.get(kid)
            .await
            .ok_or_else(|| anyhow!("kid `{kid}` not found in Google JWKS after refresh"))
    }

    pub async fn len(&self) -> usize {
        self.keys.read().await.len()
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
    let row: (i64, String, String) = sqlx::query_as(
        "SELECT id, email, display_name FROM users WHERE google_sub = ?1",
    )
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
