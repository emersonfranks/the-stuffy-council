Last reviewer: Claude Opus 4.8 (copilot)

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

## 2026-07-14 — replace-oauth-code-flow-with-gis

- Author model:   Claude Opus 4.7 (copilot)
- Reviewer model: GPT-5.5 (copilot)
- Delegated:      no
- Files:
  - Cargo.toml
  - src/auth.rs (fully rewritten)
  - src/routes/auth.rs (fully rewritten)
  - src/routes/mod.rs
  - src/state.rs
  - src/main.rs
  - src/config.rs
  - src/web/security.rs
  - templates/base.html
  - templates/login.html
  - .env.example
  - AGENTS.md
  - README.md

Change summary: replaced the Google OAuth 2.0 authorization-code flow
(with client_secret, PKCE, and server-side token exchange) with
Google Identity Services (GIS). Server no longer holds any Google
secret. New shape: browser embeds GIS JS lib, Google renders the
button and handles auth on their side, POSTs a signed ID token JWT
to `/auth/google/verify`. The verify handler double-submits GIS's
`g_csrf_token`, verifies the JWT against Google's public JWKS (cached
with on-miss refresh), enforces `email_verified`, gates on the
allowlist, upserts the user, and cycles the session id. Dropped
`oauth2` crate + `GOOGLE_CLIENT_SECRET`/`GOOGLE_REDIRECT_URL` env vars;
added `jsonwebtoken`. Public `GOOGLE_CLIENT_ID` sits in the login-page
HTML source. Booted end-to-end with a fake client_id and verified
JWKS fetch from `https://www.googleapis.com/oauth2/v3/certs`.

### Findings

#### F1 — MINOR | correctness | src/config.rs:61 | PUBLIC_ORIGIN fallback still emits 127.0.0.1, which GIS rejects for local HTTP
- what: When PUBLIC_ORIGIN is omitted, the login URI fell back to
  `http://{bind_addr}` → `http://127.0.0.1:8080/auth/google/verify`,
  contradicting the docs that say GIS requires literal `localhost`
  for plain-HTTP dev.
- why:  AGENTS.md local-dev guidance; safe-modification invariant.
- fix:  Changed the fallback to `http://localhost:{bind_addr.port()}`
  with a comment explaining why it's not `bind_addr`.
- status: Fixed

#### F2 — MINOR | security | src/web/security.rs:31 | GIS CSP sources wider than the documented GIS surface
- what: CSP allowed whole-origin `https://accounts.google.com`,
  added `https://apis.google.com`, and included Google in
  `form-action` \u2014 wider than what GIS documents.
- why:  agent-authoring policy: safety/security assertions must not
  be relaxed without careful review.
- fix:  Narrowed script-src / style-src / connect-src / frame-src
  to the specific `/gsi/...` paths GIS actually loads; dropped
  `apis.google.com`; reverted `form-action` to `'self'` (Google's
  POST is initiated on Google's own page, governed by their CSP).
- status: Fixed

#### F3 — MINOR | docs | AGENTS.md rule 1 | CSRF rule did not document the intentional GIS exception
- what: Rule 1 said every state-changing route must call
  `crate::web::csrf::verify`, but `/auth/google/verify` intentionally
  uses GIS's `g_csrf_token` cookie/form double-submit instead.
- why:  Documentation-rule enforceability + current-state accuracy.
- fix:  Added an *Exception* clause under rule 1 naming the GIS
  handler and requiring the double-submit before the JWT verify.
- status: Fixed

#### F4 — NIT | agent-authoring | src/routes/mod.rs:20 | route comment referred to a nonexistent SessionUser extractor
- what: Comment claimed auth is applied "via the SessionUser
  extractor"; actually each handler calls a local `require_user`
  helper and returns Redirect::to("/login") on `None`.
- why:  agent-authoring comments rule: inaccurate boundary comments
  are worse than none.
- fix:  Rewrote the comment to describe the actual pattern.
- status: Fixed

## 2026-07-14 — testing-infrastructure

- Author model:   Claude Opus 4.7 (copilot)
- Reviewer model: Claude Opus 4.8 (copilot)
- Delegated:      no
- Files:
  - Cargo.toml (dev-dependencies)
  - src/lib.rs (new) + src/main.rs (slimmed)
  - src/access.rs (13 inline unit tests)
  - src/routes/auth.rs (9 inline unit tests)
  - tests/common/mod.rs (new)
  - tests/router_smoke.rs (new, 7 integration tests)
  - .github/instructions/test-quality.instructions.md (new)
  - .github/instructions/test-style.instructions.md (new)
  - .github/instructions/agent-authoring.instructions.md (Tests section + AREA enum)
  - docs/testing/README.md (new)
  - AGENTS.md (stack table + layout tree + Common tasks)

Change summary: adopted a testing methodology mirroring rad-service and
built the harness to enforce it. Split the crate into `lib.rs` + thin
`main.rs` so integration tests boot the same wiring production uses.
Added `tests/common/mod.rs::build_test_app` that produces an `AppState`
backed by temp SQLite + tempfile allowlist + empty cast + no-op
`StoryGenerator`. `tests/router_smoke.rs` spawns the real
`stuffy_council::serve` on an ephemeral port and hits routes via
`reqwest` — including the regression test for the `tower_governor`
`ConnectInfo` bug from the prior commit. Wrote 22 inline unit tests
covering `AccessList` and `parse_g_csrf_cookie` across the five
coverage dimensions. New instruction files codify Rule 1 / Rule 2 /
Rule 3 (cross-component / incident-linked / auth-flow) discipline.
Updated `agent-authoring.instructions.md` to make missing tests a
reviewer finding.

### Findings

#### F1 — MINOR | tests | tests/router_smoke.rs | Rule 3.1 route-metadata coverage was incomplete
- what: 4 of 8 routes in `src/routes/mod.rs` had no anonymous-callers
  smoke test — `/story/today`, `/council/{id}`, `/logout` all missing.
- why:  test-quality.instructions.md Rule 3 §1 requires each route
  to have an access-behavior smoke; shipping the rule and the tests
  together with half-coverage undercuts the rule.
- fix:  Added `get_story_today_redirects_anonymous_to_login`,
  `get_council_detail_redirects_anonymous_to_login`, and
  `post_logout_without_csrf_returns_403`. `/auth/google/verify`
  remains deferred — needs a signed-JWT harness that's out of scope
  here; deferred is noted in the module-level doc comment.
- status: Fixed (with `/auth/google/verify` explicitly deferred)

#### F2 — MINOR | tests | src/access.rs + src/routes/auth.rs | Omitted coverage dimensions not annotated with the required N/A note
- what: `AccessList` tests omitted state-transition dimension
  (read-only registry); `parse_g_csrf_cookie` tests omitted
  state-transition AND error-handling (pure stateless parser).
  Neither module's `mod tests` carried the N/A note the new rule
  requires.
- why:  test-quality.instructions.md Coverage dimensions rule
  requires an explicit N/A note per omitted dimension.
- fix:  Added a top-of-`mod tests` comment in each module listing
  the covered dimensions and naming the N/A dimensions with reason.
- status: Fixed

#### F3 — MINOR | docs | test-quality.instructions.md frontmatter + docs/testing/README.md | Declared out-of-repo rad-service docs "authoritative"
- what: Both files told agents that rad-service's framework doc was
  authoritative and to "propagate" changes over — an unenforceable
  cross-repo instruction since agents here can't reach rad-service.
- why:  agent-authoring.instructions.md Documentation section:
  cross-links must use workspace-relative paths agents can follow,
  and rules without an enforcement path get deleted.
- fix:  Reworded both docs to make this repo's instruction files
  authoritative for this repo, demoting the rad-service mention to
  a plain provenance note.
- status: Fixed

#### F4 — NIT | docs | tests/router_smoke.rs header | Referenced rad-service's section numbers and a not-yet-existent log entry slug
- what: The module doc mentioned "Section 3.7 Rule 3" (rad-service's
  numbering, absent from this repo's docs) and a specific log-entry
  slug that hadn't been written yet.
- why:  agent-authoring.instructions.md Comments / Documentation
  rules on followable cross-links.
- fix:  Cite "Rule 3: Auth flows" by heading name with a link to the
  local instruction file; drop the pre-emptive log-slug reference.
- status: Fixed

#### F5 — NIT | agent-authoring | src/access.rs + src/routes/auth.rs | ASCII dividers between test blocks
- what: `// ---- Functional ----`, etc. inside both `mod tests`.
- why:  agent-authoring.instructions.md Comments rule explicitly
  bans aesthetic dividers / section headers.
- fix:  Deleted the dividers; folded the intent into the top-of-mod
  N/A note (which also resolves F2).
- status: Fixed

#### F6 — NIT | tests | test-quality Rule 5 vs test-style scaffolding step 4 | Internal contradiction on the readiness sleep
- what: Rule 5 forbade wall-clock dependence; test-style step 4
  prescribed a fixed 50ms readiness sleep. On paper the two rules
  contradicted, even though the sleep itself is stable in practice.
- why:  Q5 self-consistency; the two always-on files should not
  appear to disagree with each other.
- fix:  Added a "sanctioned exception" clause to Rule 5 naming the
  spawn readiness sleep in `tests/router_smoke.rs::spawn` and
  requiring written justification for any other timing-dependent
  construct.
- status: Fixed

(Log grows from here.)

