# AGENTS.md — The Stuffy Council

Canonical, tool-agnostic notes for any coding agent (GitHub Copilot,
Claude, Cursor, etc.) working in this repo. Written for agents; humans
will ask an agent to interpret — see
[.github/instructions/agent-authoring.instructions.md](.github/instructions/agent-authoring.instructions.md).
Small on purpose — grow carefully, prefer deleting stale rules over
piling on new ones.

## What this is

A private, family-only website that generates one bedtime story per day
starring characters (**stuffies** plus Lennon and Dad) defined in
`cast/*.toml`. The story is produced by a locally-hosted open-source
LLM (Ollama by default), cached by date in SQLite, and rendered as
server-side HTML.

## Authoring policy (read before writing anything)

All artifacts in this repo — code, docs, config — are written for AI
agents only. Human readability is not a goal; every change must pass a
rotating subagent review. Details and the review protocol are in
[.github/instructions/agent-authoring.instructions.md](.github/instructions/agent-authoring.instructions.md).
The running review log lives at
[.github/agent-review-log.md](.github/agent-review-log.md).

## Stack (do not change without a good reason)

| Concern            | Choice                                     |
| ------------------ | ------------------------------------------ |
| Language           | Rust (edition 2024, pinned via `rust-toolchain.toml`) |
| Web framework      | `axum` 0.8 + `tokio`                       |
| Templates          | `askama` 0.14 (compile-time, escapes by default) |
| Storage            | SQLite via `sqlx` 0.8 (WAL, foreign keys on) |
| Sessions           | `tower-sessions` + `tower-sessions-sqlx-store` (server-side) |
| Auth               | Google OAuth 2.0 (`oauth2` 5) + allowlist   |
| LLM                | Ollama HTTP API (`/api/generate`, `stream=false`) |
| Rate limiting      | `tower_governor` per client IP             |
| Styling            | Tailwind (CDN for now — vendor before production) |
| Deploy target      | Azure Container Apps (Ollama runs external) |

## Layout

```
src/
  main.rs           # boot: config, DB, generator, layers, serve
  config.rs         # env parsing, fails loud
  db.rs             # SQLite pool + migrations
  auth.rs           # Google OAuth client, SessionUser, upsert_user
  error.rs          # single AppError enum + IntoResponse
  state.rs          # AppState passed to handlers
  cast.rs           # CastRegistry (loads cast/*.toml — stuffies + humans)
  story_repo.rs     # cache read/write for the per-day story
  stories/
    mod.rs          # StoryGenerator trait, StoryService, prompt builder
    ollama.rs       # Ollama impl
  routes/
    mod.rs          # Router assembly
    auth.rs         # /login, /auth/google (+ /callback), /logout
    home.rs         # /, /story/today
    characters.rs   # /council, /council/{id}
  web/
    csrf.rs         # per-session CSRF token (double-submit)
    security.rs     # CSP + other security headers

templates/          # Askama .html templates (extend base.html)
cast/               # One TOML per character (stuffies + humans); filename = stable id
migrations/         # sqlx migrations, applied on startup
```

## Ground rules

1. **Every state-changing route MUST verify a CSRF token.** Use
   `crate::web::csrf::verify(&session, submitted).await?` at the top of any
   `POST` handler. New form templates MUST include `<input type="hidden"
   name="_csrf" value="{{ csrf_token }}">`.
2. **Every protected route MUST call `require_user(&session)` and redirect
   to `/login` on `None`.** Do not sprinkle auth checks; use the helper.
3. **Never build SQL with `format!`. Always use `sqlx::query(...).bind(...)`**
   or `query_as::<_, T>(...).bind(...)`.
4. **Never send user-provided values through Askama with `{{ x|safe }}`.**
   Askama auto-escapes by default; the `safe` filter is banned unless the
   value came from us and cannot be influenced by any request.
5. **Model output IS untrusted input.** When we display a story we split on
   `\n\n` and render each paragraph as text; do not switch to Markdown
   rendering without a hardened renderer + strict allowlist.
6. **Secrets live in env vars, not the repo.** `.env` is git-ignored;
   `.env.example` documents the surface. `SESSION_SECRET` must be 64+
   chars in production and must not match the example value.
7. **Prompt input to the LLM comes only from our own TOML.** If we later
   accept any user-controllable prompt input, treat it as untrusted and
   sanitize.
8. **Character filenames are stable ids.** Renaming a character file
   is a data migration — historical `stories.cast_json` rows will
   contain the old id.

## Local dev

```bash
cp .env.example .env
# Generate a real session secret:
#   Windows PowerShell:  -join ((48..57)+(65..90)+(97..122) | Get-Random -Count 96 | % {[char]$_})
#   Bash:                openssl rand -hex 64
# Paste it into SESSION_SECRET.

# Google OAuth setup (one-time):
#   1. https://console.cloud.google.com/ → pick a project (or create one).
#   2. APIs & Services → Credentials → Create Credentials → OAuth client ID.
#   3. Application type: Web application.
#   4. Authorized redirect URIs: http://127.0.0.1:8080/auth/google/callback
#      (add your production URL too when you deploy).
#   5. Copy the client ID + secret into GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET.
#   6. Put your Gmail(s) into ALLOWED_EMAILS — anyone else is rejected after the
#      Google round-trip.

# Start Ollama (separately) and pull a model:
ollama serve                              # in another terminal
ollama pull llama3.1:8b-instruct-q4_K_M

# Run the app:
cargo run
# → http://127.0.0.1:8080 → sign in with a Google account on the allowlist
```

## Deploy target — Azure Container Apps

* Build image locally or via ACR build; push to ACR.
* Container App reads config from env vars (map from Container App secrets
  where sensitive).
* Mount a persistent Azure Files share at `/data` for the SQLite DB.
* `OLLAMA_URL` points to wherever Ollama is running — for now, an on-prem
  machine reachable over a private endpoint or Cloudflare-tunnel-style
  bridge. Do NOT expose Ollama to the public internet.
* Front the app with the built-in Container Apps ingress (TLS terminated
  there). Do not disable HTTPS.
* Set `APP_ENV=production` so cookies get `Secure` and HSTS is emitted.

## Common tasks

**Add a new character** (stuffy or human): drop a
`cast/<kebab-id>.toml` file that matches the schema in
[cast/README.md](cast/README.md). Set `kind = "stuffy"` (default) or
`kind = "human"`. Restart. `CastRegistry` validates on load —
malformed TOML or dangling `relationships[].with` references will
fail the boot loudly.

**Add a new user**: append their Gmail address to `ALLOWED_EMAILS` in
the environment and restart. The `users` row is created on their first
successful Google sign-in.

**Change the model**: set `OLLAMA_MODEL` in the environment and restart.
The `model` column on cached stories records which model produced each,
for future comparisons.

**Swap Ollama for something else** (hosted API, another local runtime):
add an `impl StoryGenerator for MyNewThing` and wire it up in
`main.rs`. Nothing else changes.

## Non-goals (for now)

* No public signup — this is a closed roster.
* No JS build pipeline — HTMX + a Tailwind CDN link. Introduce a real
  build only when we outgrow this.
* No public API — only server-rendered HTML routes.
* No image generation in-process — image links point at pre-generated
  files under `/static/stuffies/` if present.
