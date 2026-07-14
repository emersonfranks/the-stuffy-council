Last reviewer: GPT-5.6 Sol (copilot)

# Agent Review Log

Append-only record of subagent reviews required by
[instructions/agent-authoring.instructions.md](instructions/agent-authoring.instructions.md).
Before appending a new entry, scan earlier entries whose `Files` list
overlaps this change; disposition every not-yet-resolved finding using
the status values in the template (Fixed / Deferred / Rejected).

## Entry template

```
## <UTC date> — <short change slug>

- Author model:   <model that wrote the change (self or delegated subagent)>
- Reviewer model: <different from Author, different from previous entry's Reviewer>
- Delegated:      yes | no
- Files:
  - <workspace-relative path>
  - ...

### Findings

#### F1 — SEVERITY | AREA | FILE:LINE | summary
- what: ...
- why:  ...
- fix:  ...
- status: Fixed | Deferred (<condition>; owner: <owner>) | Rejected (<specific reason>)
```

If the reviewer returned `NO FINDINGS`, replace the `### Findings` body
with the single line `NO FINDINGS`.

After appending, update the `Last reviewer:` line at the top to the
reviewer model just used.

---

## 2026-07-13 — reset-to-lightweight-policy

Prior log entries deleted along with the heavyweight review machinery
they were auditing. The last reviewer under the previous policy was
`Claude Opus 4.8 (copilot)`; the rotation rule carries forward from
that point so the next review must not use Claude Opus 4.8 and must
not use whichever model authors the change.

- Author model:   Claude Opus 4.7 (copilot)
- Reviewer model: GPT-5.5 (copilot)
- Delegated:      no
- Files:
  - .github/instructions/agent-authoring.instructions.md
  - .github/agent-review-log.md

### Findings

#### F1 — MINOR | agent-authoring | .github/agent-review-log.md:7 | prior-open-finding scan wording was too vague
- what: "Scan recent entries" left the search boundary undefined and
  did not require Fixed/Deferred/Rejected wording for older findings.
- why:  Policy step 5 requires dispositioning open findings from
  earlier entries the change touches with the template's status values.
- fix:  Reworded to "scan earlier entries whose `Files` list overlaps
  this change; disposition every not-yet-resolved finding using the
  status values in the template."
- status: Fixed

#### F2 — NIT | agent-authoring | .github/instructions/agent-authoring.instructions.md:170 | "required model" was a dangling reference
- what: "If the required model is unavailable" implied a
  model-selection rule that no longer exists — the simplified policy
  just says pick any eligible reviewer.
- why:  Documentation rule (rules without enforcement paths get
  deleted); rewrite is meant to remove residual heavyweight-policy
  language.
- fix:  Changed to "If the chosen model is unavailable..."
- status: Fixed

## 2026-07-14 — replace-placeholders-with-real-cast

- Author model:   Claude Opus 4.7 (copilot)
- Reviewer model: Claude Opus 4.8 (copilot)
- Delegated:      no
- Files:
  - src/stuffies.rs
  - src/stories/mod.rs
  - templates/stuffy.html
  - templates/council.html
  - stuffies/README.md
  - stuffies/lennon.toml (new)
  - stuffies/dad.toml (new)
  - stuffies/ruff-ruff.toml (new)
  - stuffies/woofy.toml (new)
  - stuffies/bar-bar.toml (new)
  - stuffies/bramble.toml (deleted)
  - stuffies/captain-whiskers.toml (deleted)
  - stuffies/luna.toml (deleted)

Change summary: replaced the three placeholder stuffies with the real
family cast (Lennon, Dad, Ruff Ruff, Woofy, Bar Bar). Extended the
`Stuffy` struct with optional `kind` (`stuffy`|`human`), `faction`,
`faction_role`, and `on_council`; renamed `role_in_council` -> `role`.
Story service now filters `pick_cast_for` to stuffies only and always
injects humans as a "The World" framing section in the prompt. Tone
guidance rewritten for the family voice (playful chaos, bickering,
Ruff-Ruff-vs-council running theme). Length bumped to 300-500 words.

### Findings

#### F1 — MINOR | security | src/stories/mod.rs SYSTEM_PREAMBLE + stuffies/woofy.toml | "AK collection" reached the model without a toy/pretend-prop reframing
- what: Woofy's loves line and lore reached `to_prompt_brief` with only
  a weak "(mostly) ceremonial" softening. A small local model could
  render literal AK-47 content.
- why:  AGENTS.md rule 5 (model output is untrusted input) + agent-
  authoring policy on safety assertions that must not be relaxed.
- fix:  Added one clause to SYSTEM_PREAMBLE explicitly framing any
  in-world "weapons" (Woofy's AKs, Ruff Ruff's wooden-spoon surgical
  instrument, etc.) as plush toys / pretend props for dramatic
  entrances and slapstick, never real firearms, never causing real
  harm. Preserves the canonical texture, pins the interpretation.
- status: Fixed

#### F2 — MINOR | other | src/routes/stuffies.rs + templates/council.html | Council listing showed humans and off-council characters undifferentiated
- what: The listing page titled "The Council" showed Lennon/Dad
  (kind=human) and Ruff Ruff (on_council=false) with no visual
  distinction — the new fields were honored on the detail page but
  dropped on the listing they were added for.
- why:  agent-authoring names/scope rule; the label asserted a
  membership the data contradicts.
- fix:  Retitled the page "The Crew" (subhead: "Everyone who might
  turn up in a story") and added inline badges: `human` for
  kind=human, the faction label when set, and `off-council` when
  `on_council=false`. Left the route path `/council` alone since it
  is public URL surface — retitling the page is enough.
- status: Fixed

#### F3 — NIT | correctness | stuffies/ (deleted bramble/captain-whiskers/luna) | undocumented data migration
- what: Removing the placeholder ids leaves any pre-existing
  `stories.cast_json` rows dangling. Behavior is graceful
  (`filter_map` drops unknown ids) but AGENTS.md rule 8 flags stuffy
  id changes as data migrations.
- why:  AGENTS.md rule 8.
- fix:  Rejected (specific reason): pre-launch scaffold cleanup. No
  live Ollama has been run against this repo; the three placeholder
  ids have never been persisted to any real `stories` row. The
  graceful `filter_map` in `home::today` prevents any surface
  regression even in a dev DB. This log entry itself is the record
  of the id change.
- status: Rejected (pre-launch scaffold cleanup; no persisted rows reference these ids and behavior degrades gracefully)

#### F4 — NIT | agent-authoring | src/stuffies.rs `on_council` doc comment | flavor prose in a doc comment
- what: The doc comment on `on_council` narrated "Ruff Ruff is
  famously false; the fight over that fact is his whole thing" —
  aesthetic prose rather than a constraint.
- why:  agent-authoring policy: no tutorial-tone / aesthetic comments;
  comments should describe constraints or behavior a future agent
  needs.
- fix:  Trimmed to the functional statement: "Default true. When
  `false`, `to_prompt_brief` emits an explicit 'NOT on the council'
  line so the model can play the grievance."
- status: Fixed

## 2026-07-14 — rename-stuffies-to-cast

- Author model:   Claude Opus 4.7 (copilot)
- Reviewer model: GPT-5.6 Sol (copilot)
- Delegated:      no
- Files:
  - stuffies/ -> cast/ (folder renamed)
  - src/stuffies.rs -> src/cast.rs (struct Stuffy -> Character, StuffyRegistry -> CastRegistry)
  - src/routes/stuffies.rs -> src/routes/characters.rs (handlers renamed)
  - templates/stuffy.html -> templates/character.html (variable renamed)
  - src/main.rs (module, imports, load path, tracing)
  - src/state.rs (field: stuffies -> cast)
  - src/stories/mod.rs (imports, field, type refs)
  - src/routes/mod.rs (module + handler wiring)
  - src/routes/home.rs (state.cast)
  - templates/council.html (template struct field + loop variable)
  - Dockerfile (COPY path)
  - .dockerignore (whitelist)
  - AGENTS.md (layout, ground rule 8, common tasks)
  - README.md (opening + cast path)
  - cast/README.md (path pointer)
  - .github/instructions/agent-authoring.instructions.md (docs list)

Product-name phrases ("The Stuffy Council", `stuffy-council` crate,
session cookie `stuffy_session`, DB filename default) left intact.
The `kind = "stuffy"` field value in TOMLs and its Rust checks left
intact (semantic subgroup label). Public URL paths `/council` and
`/council/{id}` left intact (in-world label + public URL surface).
`.github/agent-review-log.md` historical entries left intact per
append-only rule; they still reference the old paths as they existed
at review time.

### Findings

#### F1 — NIT | naming | templates/council.html | loop variable `s` still encoded the old stuffy-only scope
- what: `{% for s in characters %}` — variable name `s` (stuffy) no
  longer matches the widened iteration scope.
- why:  Names rule (scope changed, rename mandatory).
- fix:  Renamed loop variable to `character` and updated the six
  in-loop references.
- status: Fixed

#### F2 — NIT | docs | cast/README.md | documented image path drifted from AGENTS.md
- what: Doc example said `# file under /static/cast/` while AGENTS.md
  still says `/static/stuffies/`.
- why:  Documentation rule requires factual, single-source-of-truth
  agent-facing docs.
- fix:  Reverted cast/README.md example to `/static/stuffies/` — that
  path is semantically accurate (only stuffies have images; no `image`
  field on Lennon or Dad) and matches AGENTS.md.
- status: Fixed

#### F3 — NIT | docs | README.md | opening summary described stuffies only, omitting Lennon and Dad
- what: The one-line summary said "starring a small cast of
  stuffed-animal characters (**stuffies**)" — no mention of the
  humans that now live in the registry too.
- why:  Documentation rule; current-state accuracy.
- fix:  Reworded to name Lennon and Dad explicitly alongside the
  stuffies.
- status: Fixed

## 2026-07-14 — feat/google-auth

- Author model:   Claude Opus 4.7 (copilot)
- Reviewer model: Claude Opus 4.8 (copilot)
- Delegated:      no
- Files:
  - Cargo.toml
  - migrations/0002_google_auth.sql (new)
  - src/config.rs
  - src/auth.rs (rewritten)
  - src/routes/auth.rs (rewritten)
  - src/routes/mod.rs
  - src/state.rs
  - src/main.rs
  - templates/login.html
  - .env.example
  - AGENTS.md
  - README.md

Change summary: replaced argon2 password auth with Google OAuth 2.0
(PKCE + state) gated on a Gmail allowlist (`ALLOWED_EMAILS`). Users
table pivots to `(email, google_sub, display_name)` via a DROP+CREATE
migration (pre-launch, no live rows). `AppState` now carries a shared
`reqwest::Client` (redirect Policy::none) and a typed
`GoogleOAuthClient`. Session id rotated on successful sign-in;
`email_verified` enforced.

### Findings

#### F1 — MINOR | docs | .env.example | RATE_LIMIT_PER_SECOND comment falsely claimed a stricter login limit
- what: Comment said "Login gets a stricter limit internally" but no
  per-login limiter exists — only the global GovernorLayer.
- why:  Documentation rule; false security claims mislead future agents.
- fix:  Dropped the claim. If we later want throttled login, we'll add
  a real per-route limiter.
- status: Fixed

#### F2 — MINOR | other | src/routes/auth.rs google_callback | transient Google/network failures dumped users to a 500
- what: `exchange_code` and `fetch_userinfo` errors mapped to
  `AppError::Internal` (500), inconsistent with the `/login?error=google`
  UX used for Google-returned consent errors.
- why:  Design-intent consistency.
- fix:  Match on each result; log the error via `tracing::warn!` and
  redirect to `/login?error=google`. Genuine internal issues (session
  writes, DB) still bubble up as 500.
- status: Fixed

#### F3 — MINOR | agent-authoring | src/config.rs | misplaced comment above `google_redirect_url`
- what: A comment about `.env.example` shipping empty secret values
  sat above `google_redirect_url`, which has no empty-check and is
  not what the comment described.
- why:  Comments-and-docstrings rule (comments must aid modification;
  mis-attribution is worse than none).
- fix:  Moved the comment above `google_client_secret` where the
  empty-check it describes lives.
- status: Fixed

#### F4 — NIT | docs | AGENTS.md layout tree | routes/auth.rs comment stale
- what: Layout tree showed `routes/auth.rs # /login, /logout` without
  the new Google endpoints.
- why:  Documentation accuracy.
- fix:  Updated to `# /login, /auth/google (+ /callback), /logout`.
- status: Fixed

(Log grows from here.)

## 2026-07-14 — file-based-allowlist

- Author model:   Claude Opus 4.7 (copilot)
- Reviewer model: GPT-5.6 Sol (copilot)
- Delegated:      no
- Files:
  - authorized-users.toml (new)
  - src/access.rs (new)
  - src/config.rs
  - src/auth.rs
  - src/routes/auth.rs
  - src/state.rs
  - src/main.rs
  - .env.example
  - .dockerignore
  - Dockerfile
  - AGENTS.md
  - README.md

Change summary: folded backlog issue #2 into PR #1. Replaced the
`ALLOWED_EMAILS` env-var allowlist with a committed
`authorized-users.toml` at the repo root (add/remove is a PR). New
`AccessList` type (`src/access.rs`) parses the file, lowercases +
trims emails, rejects duplicates, and errors on empty in production
only. `SessionUser` gains a persisted-on-session-only `admin: bool`
sourced from the file (never from Google); `upsert_user` takes it as
a parameter. Google callback swaps the `state.config.allowed_emails`
membership check for `state.access.check(&info.email)`. Container
image now COPYs `authorized-users.toml` into `/app/`.

### Findings

#### F1 — MAJOR | correctness | Dockerfile:44 | Docker build cannot copy the allowlist because `.dockerignore` excludes it
- what: The runtime stage COPYs `authorized-users.toml`, but
  `.dockerignore` starts with `*` and never re-includes that file,
  so the source is missing from the build context.
- why:  Broke the documented Azure Container Apps image build; safe-
  modification invariant.
- fix:  Added `!authorized-users.toml` to `.dockerignore`.
- status: Fixed

#### F2 — NIT | agent-authoring | src/access.rs:3 | module preamble recorded change history instead of a current invariant
- what: The module doc-comment said the env-var allowlist "is gone"
  and pointed at issue #2 for the design rationale — describes how
  the code changed, not what future agents must preserve.
- why:  agent-authoring policy: comments must NOT include change-log
  entries; git is the record.
- fix:  Rewrote to state the current-state invariants only
  (case-insensitive gate, PR-to-modify, duplicates + prod-empty
  boot errors).
- status: Fixed

