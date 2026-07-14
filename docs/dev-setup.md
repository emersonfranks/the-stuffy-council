# Dev setup runbook — zero to a running story

Follow this top to bottom on a fresh machine and you will end at a
signed-in browser showing a generated bedtime story. Every step has a
**Verify** line; if it fails, jump to [Troubleshooting](#troubleshooting).

This is the one artifact in the repo written to be followed directly by a
person onboarding the project (the rest is agent-facing — see
[../.github/instructions/agent-authoring.instructions.md](../.github/instructions/agent-authoring.instructions.md)).
Keep it terse and copy-pasteable; if a step stops being true, fix the step.

## What you end up with

- Rust 1.97 toolchain (pinned by [../rust-toolchain.toml](../rust-toolchain.toml)).
- Ollama running locally on `127.0.0.1:11434` with one model pulled.
- A `.env` with a real session secret + your Google client id.
- Your Gmail in the committed allowlist.
- The app on `http://localhost:8080`, and a story cached in SQLite.

## Prerequisites

| Need | Why | Get it |
| --- | --- | --- |
| `git` | clone the repo | <https://git-scm.com> / `winget install Git.Git` |
| `rustup` | installs the pinned Rust toolchain | <https://rustup.rs> / `winget install Rustlang.Rustup` |
| Ollama | local LLM that writes the story | step 3 below |
| A Google account | sign-in is Google-only | you have one |
| Internet at boot | app fetches Google's JWKS on startup | — |

Windows only: on first build `rustup` may prompt for the **Visual Studio
C++ build tools** (the MSVC linker `link.exe`). Accept it — the build
fails without a linker.

Windows shells: run every **unlabeled** command below in **Git Bash**
(ships with Git for Windows) — they use POSIX tools (`curl`, `grep`,
`rm`, `\` line continuations). Only commands explicitly marked
*PowerShell* run in PowerShell.

## Step 1 — Clone

```bash
git clone https://github.com/emersonfranks/the-stuffy-council.git
cd the-stuffy-council
```

**Verify:** `ls AGENTS.md` prints `AGENTS.md`.

## Step 2 — Rust toolchain

`rust-toolchain.toml` pins `1.97.0`, so the first `cargo` command inside
the repo auto-installs that exact toolchain via `rustup`. You do not pick
a version.

```bash
cargo --version
```

**Verify:** prints `cargo 1.97.0 ...`. If `cargo: command not found` on
Windows Git Bash, see [Troubleshooting](#cargo-command-not-found-git-bash).

## Step 3 — Ollama

Install, then confirm the server is listening, then pull the model.

### 3a. Install + start

| OS | Install |
| --- | --- |
| Windows | `winget install --id Ollama.Ollama` |
| macOS | the app from <https://ollama.com/download>, or `brew install ollama` |
| Linux | `curl -fsSL https://ollama.com/install.sh \| sh` |

The Windows service, the macOS app, and the Linux systemd unit each start
Ollama for you on `127.0.0.1:11434`. **Verify it's listening (just below);
run `ollama serve` yourself only if that check fails** — starting a second
copy conflicts on port 11434.

**Verify:**

```bash
curl -s http://127.0.0.1:11434/api/version
```

prints `{"version":"..."}`. If it hangs on Windows right after install,
give the service a moment; do not launch `ollama serve` manually (see
[Troubleshooting](#only-one-usage-of-each-socket-address-windows)).

### 3b. Pull the model

The default model is `llama3.1:8b-instruct-q4_K_M` (~5 GB). It fits in
~6 GB of VRAM or runs on CPU (slower).

```bash
ollama pull llama3.1:8b-instruct-q4_K_M
```

Alternatives (set `OLLAMA_MODEL` in `.env` to match): `mistral-nemo:12b-instruct`
(better prose, ~7 GB), `qwen2.5:7b-instruct` (strong instruction following).

**Verify:** a real generation through the exact endpoint the app uses:

```bash
curl -s http://127.0.0.1:11434/api/generate \
  -H 'Content-Type: application/json' \
  -d '{"model":"llama3.1:8b-instruct-q4_K_M","prompt":"Say goodnight to a stuffed animal in one sentence.","stream":false,"options":{"num_predict":40}}'
```

returns JSON with a non-empty `"response"`. The first call is slow while
the model loads (tens of seconds on a GPU; CPU-only can take minutes);
later calls skip the model-load delay.

## Step 4 — Google OAuth client id

Sign-in is delegated to Google Identity Services. The server holds only a
**public** client id — there is no client secret anywhere in this project.

1. <https://console.cloud.google.com/> → pick or create a project.
2. Configure the consent screen first — Google won't issue a client id
   without it: **APIs & Services → OAuth consent screen** → User type
   **External** → set an app name, your email as the user support email,
   and your email as the developer contact → save.
3. While the app stays in **Testing** status, add the Gmail you'll sign in
   with as a test user (**Audience → Test users**, or **OAuth consent
   screen → Test users** depending on console version). An External app in
   **Testing** only lets test users through — skip this and the Step 8
   sign-in is blocked.
4. **APIs & Services → Credentials → Create Credentials → OAuth client ID**.
5. Application type: **Web application**.
6. **Authorized JavaScript origins** → add `http://localhost:8080`.
7. **Authorized redirect URIs** → add `http://localhost:8080/auth/google/verify`.
8. Create. Copy the **Client ID** (ends in `.apps.googleusercontent.com`).
   Ignore the client secret — GIS does not use it and we never store it.

**Verify:** you have a client id string in your clipboard.

## Step 5 — `.env`

```bash
cp .env.example .env
```

Then edit `.env` and set two values (leave the rest at defaults):

- `SESSION_SECRET` — 64+ random chars. Generate one:
  - macOS / Linux / Git Bash: `openssl rand -hex 64`
  - Windows PowerShell (CSPRNG, 128 hex chars): `$b = New-Object byte[] 64; [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($b); -join ($b | ForEach-Object { $_.ToString('x2') })`
- `GOOGLE_CLIENT_ID` — paste the id from step 4.

Do **not** commit `.env` (it is git-ignored). The full config surface is
documented inline in [../.env.example](../.env.example).

**Verify:** `grep -E '^(SESSION_SECRET|GOOGLE_CLIENT_ID)=' .env` shows both
filled in, and `SESSION_SECRET` is not the placeholder from the example.

## Step 6 — Add yourself to the allowlist

The allowlist ([../authorized-users.toml](../authorized-users.toml)) is the
sole gate after a successful Google sign-in. Add the Gmail you will sign in
with:

```toml
[[users]]
email = "you@gmail.com"
admin = true
```

For teammates this is a PR to that file; for your own local dev you can
just edit it. Duplicate or empty entries fail boot loudly.

**Verify:** your email appears in `authorized-users.toml`.

## Step 7 — Run

```bash
cargo run
```

First build takes a couple of minutes; migrations apply automatically on
boot (no `sqlx-cli` needed).

**Verify:** the log ends with lines like:

```
INFO stuffy_council: loaded cast count=5
INFO stuffy_council: loaded authorized users count=1
INFO stuffy_council: loaded Google JWKS count=4
INFO stuffy_council: listening addr=127.0.0.1:8080
```

and, in another terminal, `curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:8080/healthz`
prints `200`.

## Step 8 — Sign in and see a story

Open **`http://localhost:8080`** — use `localhost`, **not** `127.0.0.1`
(Google GIS only permits plain HTTP on the literal `localhost` host).
Click the Google button and sign in with the allowlisted account, then
visit `/story/today`.

**Verify:** a story renders. The first generation is slow while the model
warms up (tens of seconds on a GPU; CPU-only can take minutes — bump
`OLLAMA_TIMEOUT_SECS` in `.env` if it times out). It is then cached by
date in SQLite and instant on reload.

You're done.

## Troubleshooting

### `Only one usage of each socket address` (Windows)

The Ollama background service is already bound to `11434`. Do not run
`ollama serve` yourself on Windows — the installer runs it for you. Just
`curl http://127.0.0.1:11434/api/version` to confirm it's up.

### `cargo: command not found` (Git Bash)

`rustup` adds `~/.cargo/bin` to the Windows PATH for cmd/PowerShell but
Git Bash sometimes doesn't pick it up until a shell restart. Quick fix:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

### `migration N was previously applied but has been modified`

Someone edited a committed migration file; `sqlx` checksums each one and
refuses to run against a DB that applied the old version. This repo is
pre-production, so the local fix is to drop the dev DB and let migrations
re-run (see [Resetting local state](#resetting-local-state)). Going
forward, never edit an applied migration — add a new `migrations/000N_*.sql`
instead.

### Blank page or connection refused during sign-in

You opened `127.0.0.1:8080`. Use `http://localhost:8080` — Google GIS
rejects `127.0.0.1` for plain-HTTP local dev.

### Boot error: `SESSION_SECRET`

It must be 64+ characters; in production it must also differ from the
example value. Regenerate with the step 5 command.

### Boot error: authorized users list is empty

In production an empty allowlist is a hard boot error (nobody could sign
in). Add at least one `[[users]]` entry.

### Boot error: `initial Google JWKS fetch`

The app fetches Google's public keys at startup and needs outbound HTTPS
to `googleapis.com`. Check your network / proxy / firewall.

### `/story/today` first load is slow

Expected: the first generation loads the model before writing (tens of
seconds on a GPU, potentially minutes on CPU). It's cached by date
afterward, so that date is instant forever. On CPU, raise
`OLLAMA_TIMEOUT_SECS` in `.env` if the request times out.

## Resetting local state

The dev database is `stuffy-council.sqlite` (plus `-wal` / `-shm`
sidecars) in the repo root, git-ignored. To wipe it and start clean —
losing any cached stories, which regenerate on demand:

```bash
# stop the app first
rm -f stuffy-council.sqlite stuffy-council.sqlite-shm stuffy-council.sqlite-wal
cargo run   # migrations recreate the schema on boot
```

Your `users` row is disposable — it's re-created from `authorized-users.toml`
on your next sign-in.
