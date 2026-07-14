//! CSRF protection using the double-submit pattern against the session store.
//!
//! Flow:
//!   * `token(session)` mints (or reuses) a per-session random token and stores it in the session.
//!   * Every rendered form embeds that token in a hidden `_csrf` field.
//!   * On POST, `verify(session, submitted)` compares in constant time.
//!
//! Since our sessions are server-side (tower-sessions with the SQLx store,
//! sending only an opaque session id in the cookie), an attacker on another
//! origin cannot read the session's stored token to forge a matching form.

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use subtle::ConstantTimeEq;
use tower_sessions::Session;

use crate::error::AppError;

const CSRF_KEY: &str = "csrf_token";
const TOKEN_BYTES: usize = 32;

/// Get or create the CSRF token for the current session.
pub async fn token(session: &Session) -> Result<String, AppError> {
    if let Some(t) = session
        .get::<String>(CSRF_KEY)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("session get: {e}")))?
    {
        return Ok(t);
    }
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = URL_SAFE_NO_PAD.encode(bytes);
    session
        .insert(CSRF_KEY, &token)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("session insert: {e}")))?;
    Ok(token)
}

/// Verify a submitted token against the session's stored token in constant time.
pub async fn verify(session: &Session, submitted: &str) -> Result<(), AppError> {
    let stored = session
        .get::<String>(CSRF_KEY)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("session get: {e}")))?
        .ok_or(AppError::CsrfMismatch)?;

    if stored.as_bytes().ct_eq(submitted.as_bytes()).unwrap_u8() == 1 {
        Ok(())
    } else {
        Err(AppError::CsrfMismatch)
    }
}
