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
| Auth               | Google Identity Services (client-side JWT) + committed allowlist |
| LLM                | Ollama HTTP API (`/api/generate`, `stream=false`) |
| Rate limiting      | `tower_governor` per client IP             |
| Styling            | Self-hosted `static/app.css` (design tokens + Tailwind-compatible utility subset) |
| Testing            | Built-in `cargo test`; unit tests inline, integration in `tests/*.rs` |
| Deploy target      | Azure Container Apps (Ollama runs external) |

## Layout

```
src/
  main.rs           # CLI wrapper: parses env, builds AppState, calls serve
  lib.rs            # public serve(state, listener) + module declarations
  config.rs         # env parsing, fails loud
  db.rs             # SQLite pool + migrations
  auth.rs           # Google Identity Services: JwkCache, verify_id_token, SessionUser, upsert_user
  error.rs          # single AppError enum + IntoResponse
  state.rs          # AppState passed to handlers
  cast.rs           # CastRegistry (loads cast/*.toml — stuffies + humans)
  access.rs         # AccessList (loads authorized-users.toml)
  story_repo.rs     # cache read/write for the per-day story
  stories/
    mod.rs          # StoryGenerator trait, StoryService, prompt builder
    ollama.rs       # Ollama impl
  routes/
    mod.rs          # Router assembly
    auth.rs         # /login, /auth/google/verify, /logout
    home.rs         # /, /story/today
    characters.rs   # /council, /council/{id}
  web/
    csrf.rs         # per-session CSRF token (double-submit)
    security.rs     # CSP + other security headers

templates/          # Askama .html templates (extend base.html)
cast/               # One TOML per character (stuffies + humans); filename = stable id
authorized-users.toml  # Committed allowlist (email + admin flag)
migrations/         # sqlx migrations, applied on startup
tests/              # tier-2 integration tests (see docs/testing/README.md)
  common/mod.rs     # shared TestApp harness
  router_smoke.rs   # boots app on ephemeral port, hits real routes
docs/
  testing/README.md # tier map + pointers to instruction files
```

## Ground rules

1. **Every state-changing route MUST verify a CSRF token.** Use
   `crate::web::csrf::verify(&session, submitted).await?` at the top of any
   `POST` handler. New form templates MUST include `<input type="hidden"
   name="_csrf" value="{{ csrf_token }}">`.
   *Exception:* `POST /auth/google/verify` uses Google's `g_csrf_token`
   double-submit (cookie value must equal the form field) instead, because
   the request originates from a Google-hosted form and no prior page of
   ours minted the CSRF token. That check must run before the ID token is
   verified.
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

Fresh machine → running story: follow the full runbook at
[docs/dev-setup.md](docs/dev-setup.md). It covers Rust, Ollama, the
Google OAuth client id, `.env`, the allowlist, and a per-step verify +
troubleshooting table.

Already set up (toolchain + Ollama present, `.env` filled)? Just:

```bash
cargo run   # → http://localhost:8080 (use `localhost`, NOT 127.0.0.1 —
            #   Google GIS only allows plain HTTP on the literal `localhost`)
```

Key facts the runbook expands on:

* `SESSION_SECRET` must be 64+ chars (`openssl rand -hex 64`).
* `GOOGLE_CLIENT_ID` is the PUBLIC OAuth Web-application client id; there
  is NO client secret in this project.
* Sign in with a Gmail listed in `authorized-users.toml` (edit + PR).
* On Windows the Ollama installer runs the server as a background service
  — do NOT also run `ollama serve` (port clash).
* Never edit an applied migration; if you hit
  `migration N ... has been modified`, reset the dev DB per the runbook.

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

**Add a new user**: add a `[[users]]` block to `authorized-users.toml`
at the repo root, commit, open a PR. The row in the `users` table is
created on their first successful Google sign-in.

**Change the model**: set `OLLAMA_MODEL` in the environment and restart.
The `model` column on cached stories records which model produced each,
for future comparisons.

**Swap Ollama for something else** (hosted API, another local runtime):
add an `impl StoryGenerator for MyNewThing` and wire it up in
`main.rs`. Nothing else changes.

**Add a test.** Unit test → inline `#[cfg(test)] mod tests` at the
bottom of the module. Integration test → new file under `tests/`; use
`mod common;` and call `common::build_test_app().await` to get an
`AppState`, then `tokio::spawn(stuffy_council::serve(state, listener))`
on an ephemeral port. Rules in
[.github/instructions/test-quality.instructions.md](.github/instructions/test-quality.instructions.md)
and
[.github/instructions/test-style.instructions.md](.github/instructions/test-style.instructions.md);
orientation in [docs/testing/README.md](docs/testing/README.md).

## Non-goals (for now)

* No public signup — this is a closed roster.
* No JS build pipeline — HTMX (unpkg) + a self-hosted `static/app.css`
  (the Tailwind Play CDN was removed; #9 may vendor the full Tailwind
  build). Introduce a real build only when we outgrow this.
* No public API — only server-rendered HTML routes.
* No image generation in-process — image links point at pre-generated
  files under `/static/stuffies/` if present.
