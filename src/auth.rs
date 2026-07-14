//! Authentication: argon2id password hashing + session-backed login.

use anyhow::{Context, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use password_hash::{
    PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Serialized into the session cookie. Keep small — cookie header size matters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUser {
    pub id: i64,
    pub username: String,
    pub display_name: String,
}

/// Session key used consistently across the app.
pub const SESSION_USER_KEY: &str = "user";

/// Argon2id with parameters strong enough for a low-QPS family site.
///
/// (~19 MiB memory, 2 iterations, 1 lane — ~50–150ms on a modern CPU.)
fn argon2() -> Argon2<'static> {
    let params = Params::new(19_456, 2, 1, None).expect("valid argon2 params");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("argon2 hash failure: {e}"))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, encoded_hash: &str) -> bool {
    // Any parse error → treat as invalid credentials (constant-ish time).
    let Ok(parsed) = PasswordHash::new(encoded_hash) else {
        return false;
    };
    argon2()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[derive(Debug, sqlx::FromRow)]
struct UserRow {
    id: i64,
    username: String,
    display_name: String,
    password_hash: String,
}

/// Look up a user and verify their password in one shot.
///
/// Returns `Ok(None)` for both "no such user" and "wrong password" so callers
/// can render a single generic error message — never leak which case it was.
pub async fn authenticate(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> Result<Option<SessionUser>> {
    let row: Option<UserRow> = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, display_name, password_hash FROM users WHERE username = ?1",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .context("querying user")?;

    let Some(row) = row else {
        // Waste a hash to keep timing similar to the found-user path.
        let _ = verify_password(password, dummy_hash());
        return Ok(None);
    };

    if !verify_password(password, &row.password_hash) {
        return Ok(None);
    }

    Ok(Some(SessionUser {
        id: row.id,
        username: row.username,
        display_name: row.display_name,
    }))
}

/// Cached argon2 hash of a throwaway string, used only to burn similar CPU
/// on the "no such user" branch so a timing side channel cannot enumerate
/// valid usernames. Computed once, on first miss.
fn dummy_hash() -> &'static str {
    use std::sync::LazyLock;
    static DUMMY: LazyLock<String> = LazyLock::new(|| {
        hash_password("invalid-placeholder-not-a-real-password")
            .expect("argon2 hashing should never fail with valid params")
    });
    DUMMY.as_str()
}

/// Insert or update a user with the given plaintext password.
///
/// This is used by the bootstrap CLI/env-var seed flow — never by public
/// HTTP endpoints.
pub async fn upsert_user(
    pool: &SqlitePool,
    username: &str,
    display_name: &str,
    password: &str,
) -> Result<()> {
    let hash = hash_password(password)?;
    sqlx::query(
        "INSERT INTO users (username, display_name, password_hash)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(username) DO UPDATE SET
             display_name = excluded.display_name,
             password_hash = excluded.password_hash",
    )
    .bind(username)
    .bind(display_name)
    .bind(hash)
    .execute(pool)
    .await
    .context("upserting user")?;
    Ok(())
}
