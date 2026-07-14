-- 0002: replace password auth with Google OAuth.
--
-- The `users` table pivots from (username, password_hash) to
-- (email, google_sub). Because this repo has never seen production
-- traffic there are no rows to migrate; we drop the old table and
-- recreate it. If this file changes AFTER the first real deployment,
-- write a proper ALTER TABLE migration instead — do NOT re-run this
-- DROP against a live database.

DROP TABLE IF EXISTS users;

CREATE TABLE users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Gmail address as returned by Google's `userinfo` endpoint. NOCASE so
    -- lookups are case-insensitive; the allowlist check in config.rs
    -- lowercases both sides too as defense-in-depth.
    email           TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    -- Google's stable OpenID Connect subject id. Never changes for a
    -- given Google account; the email address CAN change on their side.
    google_sub      TEXT    NOT NULL UNIQUE,
    display_name    TEXT    NOT NULL,
    created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_login_at   TEXT
);
