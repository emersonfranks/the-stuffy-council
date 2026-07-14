-- Initial schema for the Stuffy Council.
--
-- Two concerns live here:
--   * Users (private family login)
--   * Story cache (one story per calendar day)
--
-- Sessions have their own table managed by `tower-sessions-sqlx-store`; we
-- don't declare it here — the store creates it on startup.

CREATE TABLE IF NOT EXISTS users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    username        TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    password_hash   TEXT    NOT NULL,           -- argon2id encoded string
    display_name    TEXT    NOT NULL,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS stories (
    -- ISO-8601 date in the council's local timezone; one story per day.
    story_date      TEXT    PRIMARY KEY,
    title           TEXT    NOT NULL,
    body            TEXT    NOT NULL,
    -- JSON array of stuffy ids that appeared, so we can show cast + rotate later.
    cast_json       TEXT    NOT NULL,
    -- Which model produced it, for future comparisons.
    model           TEXT    NOT NULL,
    -- Full prompt used, kept for auditing / prompt-tuning iteration.
    prompt          TEXT    NOT NULL,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS stories_created_at_idx ON stories(created_at);
