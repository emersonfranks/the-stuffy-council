# The Stuffy Council

A tiny, private website that writes a new bedtime story every day
starring a small cast of characters — the stuffies plus Lennon (the
kid who owns them and drives the imagination) and Dad (who narrates
and voices most of them). One story per calendar day, cached forever.
Family login only — no public accounts.

Built in Rust (Axum + Askama + SQLite), talking to a locally-hosted
open-source LLM (Ollama) for the story text.

> Detailed conventions for anyone editing this repo (including AI agents)
> live in [AGENTS.md](AGENTS.md). Start there before making changes.

## Requirements

* **Rust** 1.97+ (see `rust-toolchain.toml` — will auto-install via
  `rustup` if you have it).
* **Ollama** running locally: <https://ollama.com/download>.
  Pull a model once:
  ```bash
  ollama pull llama3.1:8b-instruct-q4_K_M
  ```
  Alternatives worth trying: `mistral-nemo:12b-instruct`,
  `qwen2.5:7b-instruct`.

## Quick start

```bash
# 1. Config
cp .env.example .env
# Edit .env — at minimum:
#  * SESSION_SECRET to 64+ random bytes (`openssl rand -hex 64`).
#  * GOOGLE_CLIENT_ID from a Google Cloud Console OAuth 2.0 Web-application
#    credential. That value is PUBLIC (safe to embed in HTML); there is
#    NO client secret in this project. In the credential add:
#      Authorized JavaScript origins: http://localhost:8080
#      Authorized redirect URIs:      http://localhost:8080/auth/google/verify
# Then edit `authorized-users.toml` at the repo root and add your Gmail
# address (commit + PR for teammates; ship as one). No env-var allowlist.

# 2. Start Ollama in another terminal
ollama serve

# 3. Run the app
cargo run

# 4. Open http://localhost:8080 (NOT 127.0.0.1 — Google GIS only allows
#    plain HTTP on the literal `localhost` hostname). Click the button.
```

## What's here

* `cast/` — one TOML file per character (stuffies + humans). Add your
  own; the loader validates on startup. See [cast/README.md](cast/README.md)
  for the schema. Five characters ship in the box (Lennon, Dad, Ruff
  Ruff, Woofy, Bar Bar) — tune to taste.
* `src/stories/` — generator abstraction. `StoryService` builds the
  prompt from the day's cast; `OllamaGenerator` calls Ollama. Swap in
  another backend by implementing `StoryGenerator`.
* `src/web/security.rs` — CSP + baseline security headers.
* `src/web/csrf.rs` — session-backed CSRF tokens; used on every form.
* `migrations/` — SQL schema, applied automatically on boot.

## Security posture (day one)

* **Google Identity Services** for sign-in. Google hosts the entire
  auth UI (password / MFA / passkey) and hands us a signed ID token
  JWT. Our server verifies the JWT against Google's public JWKS. We
  hold only the PUBLIC `GOOGLE_CLIENT_ID`; there is no client secret
  in this project, nowhere on disk, nowhere in Azure secrets.
* **Email allowlist** (`authorized-users.toml`, committed at the repo
  root) is the sole gate after a successful Google sign-in. Google
  reports `email_verified` — we require it. Adding or removing a user
  is a PR.
* **Server-side sessions** (SQLite store); cookie carries only an opaque id.
* **Session id rotation on login** to defeat session fixation.
* **CSRF tokens** on every POST (double-submit against the session);
  GIS's own `g_csrf_token` double-submit covers the sign-in POST.
* **CSP + `X-Content-Type-Options` + `X-Frame-Options` + `Referrer-Policy`
  + `Permissions-Policy`** on every response.
* **HSTS** in production only (never over plain HTTP).
* **Rate limiting** per client IP via `tower_governor`.
* **Cookies**: `HttpOnly`, `SameSite=Lax`, `Secure` in production.
* **TLS**: terminated at Azure Container Apps ingress; the app itself
  speaks plain HTTP on the container port.

Known caveats to close before "real" production:

* Tailwind is loaded from the CDN. Vendor it locally (or swap for a
  pre-built stylesheet) and drop `'unsafe-inline'` from `style-src`.
* HTMX is loaded from unpkg.com. Same story — self-host and pin an SRI
  hash, then remove `https://unpkg.com` from `script-src`.

## Deploy

See [AGENTS.md](AGENTS.md#deploy-target--azure-container-apps) for the
Azure Container Apps notes.

```bash
docker build -t stuffy-council:local .
docker run --rm -p 8080:8080 \
  -e SESSION_SECRET=... \
  -e OLLAMA_URL=http://host.docker.internal:11434 \
  -v stuffy-data:/data \
  stuffy-council:local
```

## Roadmap (short)

* Per-stuffy generated images (hook a text-to-image pipeline behind a
  similar trait).
* Story history browser (`/story/YYYY-MM-DD`).
* Small admin page for editing stuffies from the browser.
* Vendor Tailwind + HTMX; tighten CSP.
