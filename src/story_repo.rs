//! Story cache — one row per calendar date.

use anyhow::{Context, Result};
use serde::Serialize;
use sqlx::SqlitePool;
use time::Date;
use time::format_description::well_known::Iso8601;

use crate::stories::GeneratedStory;

pub struct CachedStory {
    pub date: Date,
    pub title: String,
    pub body: String,
    pub cast: Vec<String>,
    pub model: String,
}

/// One archive-index entry. Deliberately not `CachedStory`: the index renders
/// hundreds of rows and must not pull every story body out of SQLite.
pub struct StorySummary {
    pub date: Date,
    pub title: String,
}

/// A verbatim row mirror for backups. Field names and shapes match the columns
/// in `migrations/0001_init.sql` so a dump can be restored without a decoder;
/// `cast_json` therefore stays an encoded string rather than an array.
#[derive(Serialize, sqlx::FromRow)]
pub struct ExportedStory {
    pub story_date: String,
    pub title: String,
    pub body: String,
    pub cast_json: String,
    pub model: String,
    pub prompt: String,
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
struct StoryRow {
    story_date: String,
    title: String,
    body: String,
    cast_json: String,
    model: String,
}

pub async fn get(pool: &SqlitePool, date: Date) -> Result<Option<CachedStory>> {
    let key = date.format(&Iso8601::DATE).context("formatting date")?;
    let row: Option<StoryRow> = sqlx::query_as::<_, StoryRow>(
        "SELECT story_date, title, body, cast_json, model FROM stories WHERE story_date = ?1",
    )
    .bind(&key)
    .fetch_optional(pool)
    .await
    .context("querying story cache")?;

    let Some(row) = row else {
        return Ok(None);
    };

    let cast: Vec<String> =
        serde_json::from_str(&row.cast_json).context("decoding cached cast_json")?;
    let parsed_date =
        Date::parse(&row.story_date, &Iso8601::DATE).context("parsing cached story date")?;

    Ok(Some(CachedStory {
        date: parsed_date,
        title: row.title,
        body: row.body,
        cast,
        model: row.model,
    }))
}

/// Most recent stories first, capped at `limit`.
pub async fn list_recent(pool: &SqlitePool, limit: i64) -> Result<Vec<StorySummary>> {
    #[derive(sqlx::FromRow)]
    struct SummaryRow {
        story_date: String,
        title: String,
    }

    // story_date is a zero-padded ISO-8601 string, so lexical DESC is chronological.
    let rows: Vec<SummaryRow> = sqlx::query_as::<_, SummaryRow>(
        "SELECT story_date, title FROM stories ORDER BY story_date DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("listing recent stories")?;

    rows.into_iter()
        .map(|row| {
            Ok(StorySummary {
                date: Date::parse(&row.story_date, &Iso8601::DATE)
                    .context("parsing archived story date")?,
                title: row.title,
            })
        })
        .collect()
}

/// Every row, oldest first, for backup. Unbounded on purpose — a partial
/// backup that looks complete is worse than a slow one.
pub async fn export_all(pool: &SqlitePool) -> Result<Vec<ExportedStory>> {
    sqlx::query_as::<_, ExportedStory>(
        "SELECT story_date, title, body, cast_json, model, prompt, created_at
         FROM stories ORDER BY story_date ASC",
    )
    .fetch_all(pool)
    .await
    .context("exporting stories")
}

pub async fn put(pool: &SqlitePool, date: Date, story: &GeneratedStory) -> Result<()> {
    let key = date.format(&Iso8601::DATE).context("formatting date")?;
    let cast_json = serde_json::to_string(&story.cast).context("encoding cast for cache")?;

    sqlx::query(
        "INSERT INTO stories (story_date, title, body, cast_json, model, prompt)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(story_date) DO NOTHING",
    )
    .bind(&key)
    .bind(&story.title)
    .bind(&story.body)
    .bind(&cast_json)
    .bind(&story.model)
    .bind(&story.prompt)
    .execute(pool)
    .await
    .context("caching story")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    // Query-contract coverage for the archive reads: ordering, the limit
    // boundary, column fidelity, and the empty case. Uses a real temp SQLite
    // file through `db::connect` so migrations and types are the production ones.
    use super::*;
    use crate::stories::GeneratedStory;

    async fn pool() -> (tempfile::TempDir, SqlitePool) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let url = format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("t.sqlite").display()
        );
        let pool = crate::db::connect(&url).await.expect("connect");
        (tmp, pool)
    }

    fn date(day: u8) -> Date {
        Date::from_calendar_date(2026, time::Month::July, day).expect("date")
    }

    fn story(title: &str) -> GeneratedStory {
        GeneratedStory {
            title: title.into(),
            body: "Body one.\n\nBody two.".into(),
            cast: vec!["ruff-ruff".into()],
            model: "test-model".into(),
            prompt: "the full prompt".into(),
        }
    }

    async fn seed(pool: &SqlitePool, days: &[(u8, &str)]) {
        for (day, title) in days {
            put(pool, date(*day), &story(title)).await.expect("put");
        }
    }

    #[tokio::test]
    async fn list_recent_returns_newest_first() {
        let (_tmp, pool) = pool().await;
        seed(&pool, &[(1, "First"), (3, "Third"), (2, "Second")]).await;

        let titles: Vec<_> = list_recent(&pool, 10)
            .await
            .expect("list")
            .into_iter()
            .map(|s| s.title)
            .collect();

        assert_eq!(titles, vec!["Third", "Second", "First"]);
    }

    #[tokio::test]
    async fn list_recent_honors_the_limit_and_keeps_the_newest() {
        let (_tmp, pool) = pool().await;
        seed(&pool, &[(1, "First"), (2, "Second"), (3, "Third")]).await;

        let summaries = list_recent(&pool, 2).await.expect("list");

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].date, date(3));
        assert_eq!(summaries[1].date, date(2));
    }

    #[tokio::test]
    async fn list_recent_of_empty_archive_is_empty_rather_than_an_error() {
        let (_tmp, pool) = pool().await;

        assert!(list_recent(&pool, 10).await.expect("list").is_empty());
    }

    #[tokio::test]
    async fn export_all_returns_every_row_oldest_first_with_all_columns() {
        let (_tmp, pool) = pool().await;
        seed(&pool, &[(3, "Third"), (1, "First"), (2, "Second")]).await;

        let rows = export_all(&pool).await.expect("export");

        let dates: Vec<_> = rows.iter().map(|r| r.story_date.as_str()).collect();
        assert_eq!(dates, vec!["2026-07-01", "2026-07-02", "2026-07-03"]);
        let first = &rows[0];
        assert_eq!(first.title, "First");
        assert_eq!(first.body, "Body one.\n\nBody two.");
        assert_eq!(first.cast_json, r#"["ruff-ruff"]"#);
        assert_eq!(first.model, "test-model");
        // Without the prompt a backup cannot reproduce or audit the story.
        assert_eq!(first.prompt, "the full prompt");
        assert!(!first.created_at.is_empty());
    }

    #[tokio::test]
    async fn put_keeps_the_first_story_written_for_a_date() {
        let (_tmp, pool) = pool().await;

        put(&pool, date(1), &story("Original")).await.expect("put");
        put(&pool, date(1), &story("Replacement"))
            .await
            .expect("put again");

        let cached = get(&pool, date(1)).await.expect("get").expect("row");
        assert_eq!(
            cached.title, "Original",
            "stories are immutable once written, so a revisit must be identical"
        );
    }
}
