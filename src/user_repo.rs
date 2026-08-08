//! Read side of the `users` table. Writes live in `auth::upsert_user`.

use anyhow::{Context, Result};
use sqlx::SqlitePool;

/// A signed-in account as shown on the admin page. `last_login_at` is `None`
/// only for rows written before that column started being populated.
#[derive(sqlx::FromRow)]
pub struct UserLogin {
    pub email: String,
    pub display_name: String,
    pub last_login_at: Option<String>,
}

/// Most recently seen accounts first. Rows that have never recorded a login
/// sort last rather than being hidden, so a stale account stays visible.
pub async fn recent_logins(pool: &SqlitePool, limit: i64) -> Result<Vec<UserLogin>> {
    sqlx::query_as::<_, UserLogin>(
        "SELECT email, display_name, last_login_at
         FROM users
         ORDER BY last_login_at IS NULL, last_login_at DESC
         LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("listing recent logins")
}

#[cfg(test)]
mod tests {
    // Ordering, the null-login boundary, the limit, and the empty case.
    use super::*;

    async fn pool() -> (tempfile::TempDir, SqlitePool) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let url = format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("t.sqlite").display()
        );
        let pool = crate::db::connect(&url).await.expect("connect");
        (tmp, pool)
    }

    async fn insert(pool: &SqlitePool, email: &str, last_login: Option<&str>) {
        sqlx::query(
            "INSERT INTO users (email, google_sub, display_name, last_login_at)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(email)
        .bind(format!("sub-{email}"))
        .bind(email)
        .bind(last_login)
        .execute(pool)
        .await
        .expect("insert user");
    }

    #[tokio::test]
    async fn recent_logins_returns_most_recent_first() {
        let (_tmp, pool) = pool().await;
        insert(&pool, "old@example.com", Some("2026-01-01T00:00:00.000Z")).await;
        insert(&pool, "new@example.com", Some("2026-08-01T00:00:00.000Z")).await;
        insert(&pool, "mid@example.com", Some("2026-04-01T00:00:00.000Z")).await;

        let emails: Vec<_> = recent_logins(&pool, 10)
            .await
            .expect("list")
            .into_iter()
            .map(|u| u.email)
            .collect();

        assert_eq!(
            emails,
            vec!["new@example.com", "mid@example.com", "old@example.com"]
        );
    }

    #[tokio::test]
    async fn recent_logins_sorts_never_logged_in_last_without_hiding_them() {
        let (_tmp, pool) = pool().await;
        insert(&pool, "never@example.com", None).await;
        insert(&pool, "seen@example.com", Some("2026-04-01T00:00:00.000Z")).await;

        let users = recent_logins(&pool, 10).await.expect("list");

        assert_eq!(users.len(), 2, "an account with no login must stay visible");
        assert_eq!(users[0].email, "seen@example.com");
        assert_eq!(users[1].email, "never@example.com");
        assert!(users[1].last_login_at.is_none());
    }

    #[tokio::test]
    async fn recent_logins_honors_the_limit() {
        let (_tmp, pool) = pool().await;
        insert(&pool, "a@example.com", Some("2026-01-01T00:00:00.000Z")).await;
        insert(&pool, "b@example.com", Some("2026-02-01T00:00:00.000Z")).await;

        let users = recent_logins(&pool, 1).await.expect("list");

        assert_eq!(users.len(), 1);
        assert_eq!(users[0].email, "b@example.com");
    }

    #[tokio::test]
    async fn recent_logins_of_empty_table_is_empty_rather_than_an_error() {
        let (_tmp, pool) = pool().await;

        assert!(recent_logins(&pool, 10).await.expect("list").is_empty());
    }
}
