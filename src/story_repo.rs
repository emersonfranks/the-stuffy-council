//! Story cache — one row per calendar date.

use anyhow::{Context, Result};
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
        cast: cast,
        model: row.model,
    }))
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
