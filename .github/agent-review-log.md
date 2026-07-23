Last reviewer: GPT-5.5 (copilot)

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

## 2026-07-14 — disposition-copilot-pr1-review

- Author model:   Claude Opus 4.7 (copilot)
- Reviewer model: GPT-5.6 Sol (copilot), then GPT-5.5 (copilot) after BLOCK fix
- Delegated:      no
- Files:
  - migrations/0002_google_auth.sql
  - src/auth.rs
  - src/web/security.rs

Change dispositions four inline findings that GitHub Copilot's automated
reviewer left on PR #1 (`feat/google-auth`) just before merge:

- **Copilot F1** (MINOR, migrations/0002_google_auth.sql:16) — column comment
  referenced the pre-GIS `userinfo` flow + non-existent `config.rs`
  allowlist check. **status: Fixed** — comment now points at
  `verify_id_token` + `AccessList`.
- **Copilot F2 + F4** (src/web/security.rs:18 and :35) — claimed browsers
  ignore path components in CSP source expressions. **status: Rejected
  (factually incorrect per W3C CSP L3 §6.7.2.7 step 4.6 and §6.7.2.12;
  paths ARE enforced on initial requests, ignored ONLY after redirect per
  §7.6).** Kept the path pinning; tightened the doc comment to describe
  what browsers actually enforce plus the redirect caveat.
- **Copilot F3** (MAJOR, src/auth.rs:107, `JwkCache::get_or_refresh`) —
  amplification: unknown-`kid` POSTs triggered an outbound Google fetch
  per miss. **status: Fixed** — added `JWKS_REFRESH_COOLDOWN`,
  `last_refresh_attempt`, `refresh_mutex`, and `refresh_if_stale`, and
  reworked `get_or_refresh` to route through them.

### Findings

#### F1 — BLOCK | security | src/auth.rs:126 | Failed JWKS refreshes bypass the cooldown
- what: `last_refresh` was updated only on refresh success, so a returning-500
  upstream let every queued unknown-`kid` request perform another outbound
  fetch after acquiring the mutex.
- why:  agent-authoring.instructions.md permits BLOCK for security holes and
  requires safety comments to preserve true invariants — the documented "at
  most one outbound fetch per cooldown window" invariant was false during
  upstream failure.
- fix:  Rename `last_refresh` → `last_refresh_attempt` and update it at the
  START of `refresh()`, before the outbound send. Failed attempts now also
  count against the cooldown budget.
- status: Fixed (src/auth.rs; verified by new test
  `upstream_500_still_counts_against_cooldown`)

#### F2 — BLOCK | tests | src/auth.rs:329 | Tests didn't reproduce the amplification path
- what: The initial tier-1 tests only drove `should_refresh` and
  `refresh_if_stale` directly. Reverting `get_or_refresh` to call
  unconditional `refresh()` would have left every new test green.
- why:  agent-authoring.instructions.md requires every bug fix to land with
  a regression test reproducing the original failure path; absence is BLOCK.
  test-quality.instructions.md also mandates state-transition and dependency-
  failure coverage.
- fix:  Add a fake JWKS server (`FakeJwks` in `src/auth.rs`
  `#[cfg(test)] mod tests`) with a hit counter and configurable response
  (200 empty-keys or 500). New tests drive `get_or_refresh` end-to-end and
  assert the hit count matches the amplification cap.
- status: Fixed (src/auth.rs; new tests
  `flood_of_unknown_kids_produces_one_upstream_fetch`,
  `upstream_500_still_counts_against_cooldown`,
  `concurrent_misses_collapse_to_one_upstream_fetch`,
  `miss_after_cooldown_expiry_permits_second_attempt`)

#### F3 — MAJOR | correctness | src/auth.rs:169 | Cooldown check ran outside mutex; late misser could race in-flight refresh
- what: With the cooldown check outside the mutex, a late-arriving misser
  could observe `last_refresh_attempt` being set (recorded at the start of
  an in-flight `refresh()`), skip the mutex, and read the still-empty keys
  map for a spurious "kid not found" while the refresh was mid-flight.
- why:  agent-authoring.instructions.md elevates missing test coverage on
  `src/auth.rs` to MAJOR; test-quality.instructions.md requires state-
  transition coverage for the concurrent-refresh path.
- fix:  Acquire `refresh_mutex` unconditionally on cache miss, THEN check
  the cooldown inside the mutex. Late arrivers now wait for any in-flight
  refresh to publish its keys before re-reading the cache.
- status: Fixed (src/auth.rs; new test `late_arriver_waits_for_in_flight_refresh`
  uses a `Notify`-gated fake to prove task B blocks on the mutex until task A
  releases)

#### F4 — MINOR | tests | src/auth.rs:460 | Regression tests discarded `get_or_refresh` results with `let _ = …`
- what: Four new regression tests only asserted the upstream hit count and
  ignored the per-call return value, silently tolerating unexpected `Ok`
  results.
- why:  test-quality.instructions.md explicitly bans "Silent failure
  tolerance — `let _ = sut.do();` without asserting" and requires dependency-
  failure paths to validate observable propagation.
- fix:  Replace `let _ = …` with explicit `assert!(result.is_err(), …)`
  (and, for the spawned-task variant, return a bool from the task and
  assert after join).
- status: Fixed (src/auth.rs; applied to
  `upstream_500_still_counts_against_cooldown`,
  `concurrent_misses_collapse_to_one_upstream_fetch`,
  `miss_after_cooldown_expiry_permits_second_attempt`, and the new
  `late_arriver_waits_for_in_flight_refresh`)

#### F5 — NIT | docs | src/auth.rs:166 | `refresh_if_stale` return-value comment overstated success semantics
- what: The comment said `Ok(true)` covered "whether it succeeded or
  failed", but `self.refresh().await?` returns `Err` on failure.
- why:  agent-authoring.instructions.md says comments must help future
  agents preserve behavior; inaccurate comments should be corrected or
  removed.
- fix:  Rewrote the comment to enumerate Ok(true) / Ok(false) / Err(_)
  explicitly and to state that `Err` still consumed the cooldown budget
  because `refresh()` records attempt time before the outbound send.
- status: Fixed (src/auth.rs)

## 2026-07-14 — dev-setup-runbook

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: GPT-5.6 Sol → GPT-5.5 → Claude Opus 4.7 (copilot)
  (two BLOCK-triggered follow-ups; final review NO FINDINGS)
- Delegated:      no
- Files:
  - docs/dev-setup.md (new)
  - AGENTS.md (## Local dev slimmed to a pointer)
  - README.md (runbook pointer + OS-agnostic Ollama-start step)

Adds a zero-to-running dev runbook (backlog #19, scoped up to a full
machine-setup doc at the owner's request) and repoints README + AGENTS.md
at it instead of duplicating the steps. The runbook is the one deliberate
human-facing artifact in the repo; noted as such in its header so the
agents-only audience rule isn't misread as a violation.

### Findings

#### F1 — BLOCK | security | docs/dev-setup.md | PowerShell SESSION_SECRET was 62 chars + non-CSPRNG
- what: `Get-Random -Count 96` on the 62-char alphabet returns all 62
  (PowerShell caps -Count at collection size), failing the 64-char
  minimum; `Get-Random` is also not a CSPRNG.
- why:  AGENTS.md ground rule 6 (real 64+ char SESSION_SECRET); the
  command both fails boot and is unfit for key material.
- fix:  Replaced with a CSPRNG hex one-liner
  (`RandomNumberGenerator.Create().GetBytes` → 128 hex chars), verified
  to run on PS 5.1 + 7.
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F2 — BLOCK | correctness | docs/dev-setup.md | Google setup omitted consent screen + test users
- what: The client-id steps skipped OAuth-consent-screen config and the
  test-user enumeration an External+Testing app needs, which blocks the
  Step 8 sign-in on a fresh project.
- why:  Concrete setup omission that breaks the runbook's end-to-end promise.
- fix:  Expanded Step 4 to configure the consent screen (External, app
  name, support/dev email) and add the sign-in Gmail as a test user before
  creating the Web-application client.
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F3 — BLOCK | correctness | README.md | condensed quick start ran `ollama serve` unconditionally
- what: Windows already runs Ollama as a service, so a bare `ollama serve`
  errors on the socket and contradicts the runbook.
- why:  Factually wrong command on a supported path.
- fix:  README step 2 became verify-first (`/api/version`), serve only if
  down. Superseded by F6's uniform cross-OS wording.
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F4 — BLOCK | docs | docs/dev-setup.md | Windows shell unspecified for POSIX commands
- what: Most commands use `curl` flags, `\` continuations, `grep`, `rm -f`
  that break in Windows PowerShell.
- why:  Runbook promises copy-pasteable Windows setup.
- fix:  Prerequisites now states unlabeled commands run in Git Bash on
  Windows; only PowerShell-labeled ones run in PowerShell.
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F5 — MINOR | correctness | docs/dev-setup.md | timing/VRAM claims were CPU-unaware
- what: "~30–60 s / loads into VRAM" ignored the documented CPU fallback,
  which can exceed the default 120 s OLLAMA_TIMEOUT_SECS.
- why:  Authoring policy requires factual docs.
- fix:  All three timing spots now qualify GPU vs CPU and point at
  OLLAMA_TIMEOUT_SECS.
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F6 — BLOCK | correctness | README.md + docs/dev-setup.md | Linux `ollama serve` could re-create the port clash
- what: After F3, README still told Linux users to run `ollama serve`, but
  the Linux installer starts a systemd unit → same 11434 conflict; and the
  runbook gave no non-systemd fallback.
- why:  Factual-doc rule; both systemd and non-systemd Linux were wrong.
- fix:  Unified the guidance everywhere: the OS service/app/unit starts
  Ollama; verify `/api/version` first; run `ollama serve` only if that
  fails. Runbook 3a dropped its per-OS "Server start" column for that rule.
- status: Fixed (reviewer: GPT-5.5)

#### F7 — MINOR | correctness | docs/dev-setup.md | test-user note tied to verification not Testing status
- what: "unverified External app" conflated verification with the Testing
  publishing status that actually imposes the test-user gate.
- why:  Google separates the two axes.
- fix:  Reworded to "an External app in **Testing** only lets test users
  through."
- status: Fixed (reviewer: GPT-5.5)

#### F8 — MINOR | correctness | docs/dev-setup.md | "later calls are fast" absolute
- what: Skipping model load doesn't make CPU inference fast.
- why:  Factual-doc rule.
- fix:  Changed to "later calls skip the model-load delay."
- status: Fixed (reviewer: GPT-5.5)

Third review (Claude Opus 4.7) independently re-verified all fixes, links,
anchors, the PowerShell one-liner, and whole-doc accuracy against config.rs
/ db.rs / main.rs → NO FINDINGS.

## 2026-07-14 — visual-identity (#22)

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: GPT-5.6 Sol → GPT-5.5 (copilot) (one BLOCK-triggered
  follow-up; follow-up returned no BLOCK)
- Delegated:      no
- Files:
  - static/app.css (new), static/fonts/* (woff2 + OFL), static/img/*.svg
  - src/cast.rs (accent() + tests), src/error.rs (in-voice + tests)
  - src/web/security.rs (CSP: drop dead Tailwind origin)
  - src/routes/{mod,home,characters,auth}.rs, templates/*.html
  - Dockerfile, .dockerignore, README.md, AGENTS.md, tests/router_smoke.rs

Full visual identity for #22: design tokens (light + dark), self-hosted
Fredoka/Nunito, component primitives, collectible-card council grid,
character sheet, reading treatment, in-voice states. Ships the design
standalone (portraits are silhouette placeholders pending #8). Notable
deviation from #22's non-goal: the Tailwind Play CDN was REMOVED (it is a
`<script>` the CSP `script-src` never allowed, so it never loaded); a
self-hosted Tailwind-compatible utility subset replaces it. No CSP
loosening; no build pipeline. #9 may vendor full Tailwind later.

### Findings

#### F1 — BLOCK | correctness | Dockerfile + .dockerignore | prod image omitted static/
- what: `.dockerignore` allowlist lacked `static/**` and the runtime stage
  never copied it — every asset would 404 in Azure.
- fix:  `!static/**` in .dockerignore; `COPY --chown=app:app static /app/static`.
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F2 — BLOCK | accessibility | static/app.css | a:hover overrode button text (contrast) + underline leak
- what: global `a`/`a:hover` color (specificity 0,1,1) beat `.sc-btn--primary`
  (0,1,0) on hover → lavender text on teal; links also underlined.
- fix:  `a { color: inherit; text-decoration: none; }`; every `<a>` carries a
  component class that owns its color; `.sc-navlink:hover` added.
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F3 — BLOCK | accessibility | static/app.css | council card had no visible focus ring
- what: the whole-card overlay link is zero-size; `.sc-toon:focus-within`
  only changed transform/shadow.
- fix:  added `.sc-toon:focus-within { outline: 3px solid var(--focus); }`.
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F4 — BLOCK | tests | tests/router_smoke.rs | Tailwind/static fix had no regression test
- fix:  added `login_links_local_css_and_drops_tailwind_cdn`,
  `static_stylesheet_is_served` (GET /static/app.css → 200 + `--brand-council`),
  and `login_denied_error_renders_in_voice_copy`.
- status: Fixed (reviewer: GPT-5.6 Sol; test strengthened after GPT-5.5, see F11)

#### F5 — MAJOR | tests | src/error.rs | elevated-bar copy change, no coverage
- fix:  added tests asserting 404/500 status + public body and that Internal
  never leaks the underlying anyhow message.
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F6 — MINOR | correctness | src/cast.rs | accent() substring collision ("Dog Squad"→og)
- fix:  exact normalized match; regression test `Dog Squad`→blossom; module
  coverage-dimension N/A note added.
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F7 — MINOR | css | static/app.css | missing baseline reset (<ul> bullets, box-sizing)
- fix:  minimal reset (box-sizing, margin strips, `ul[class]` list-style none,
  `img/svg` block, `button { font: inherit }`).
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F8 — MINOR | security | src/web/security.rs | dead Tailwind origin left in CSP
- fix:  removed `https://cdn.tailwindcss.com` from style-src; doc + README
  updated; `'unsafe-inline'` now documented as GIS-only.
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F9 — NIT | css | static/app.css | unused primitives
- fix:  removed `.sc-chip-user`, `.sc-btn--ghost`, `.sc-display`, and the
  unused `--on-lavender/-danger/-peach` tokens.
- status: Fixed (reviewer: GPT-5.6 Sol)

#### F10 — MINOR | tests | src/routes/home.rs + characters.rs | authed view-model logic uncovered
- what: home `spotlight` (on-council filter + sort) and character
  `relationships` name-resolution + fallback have no functional test.
- status: Deferred (blocked on #15 signed-JWT test harness — the same reason
  tests/router_smoke.rs already defers all authenticated rendering; the logic
  is a trivial filter/sort and a map-with-fallback). owner: #15

#### F11 — MINOR | tests | tests/router_smoke.rs | denied-login assertion was tautological
- what: asserted `body.contains("Council")`, which the base login page already
  contains → would pass even if the denied branch stopped rendering.
- fix:  assert the distinctive `"Google account"` substring (only in the
  denial message).
- status: Fixed (reviewer: GPT-5.5)

#### F12 — MINOR | docs | AGENTS.md | canonical guidance still said Tailwind CDN
- fix:  updated the tech-stack row + the non-goals bullet to the self-hosted
  `static/app.css` state.
- status: Fixed (reviewer: GPT-5.5)

(Log grows from here.)

## 2026-07-15 — pr24-copilot-review-fixes

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: GPT-5.6 Sol (copilot)
- Delegated:      no
- Files:
  - static/app.css
  - templates/council.html

Change summary: addressed the three GitHub Copilot automated review
comments on PR #24. (1) Removed `border-radius: var(--radius-sm)` from
the global `:focus-visible` / `.sc-card:focus-within` rule so the focus
outline follows each control's own radius instead of reshaping focused
pills and cards; added a do-not-re-add invariant comment. (2) Rewrote
the `static/app.css` header comment, which was both truncated mid-
sentence and contained a personal identifier (named child + age), into
a complete agent-facing tone description with no personal data and no
dangling reference. (3) Fixed the `council.html` empty-state hint from
`cast/<name>.toml` to `cast/<kebab-id>.toml` to match the stable-id
rule in AGENTS.md (ground rule 8). Prior overlapping-file finding F10
(visual-identity entry, authed view-model coverage) stays Deferred on
#15 — unchanged, this change does not touch home.rs/characters.rs.
`cargo check` clean; full `cargo test` green (37 unit + 10 integration).

### Findings

NO FINDINGS

## 2026-07-15 — character-art-bible

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: GPT-5.5 (copilot)
- Delegated:      no
- Files:
  - docs/character-art.md (new)
  - cast/README.md

Change summary: added a durable art bible (`docs/character-art.md`) that
defines how per-character image assets are generated — asset spec, a
fixed house-style block and global negatives to reuse verbatim, a
prompt formula whose only variable is the `Shot:` line, consistency
rules, per-character identity locks (Bar Bar / Woofy / Ruff Ruff built
from their reference photos), variant naming, and operational notes
(#10 `.gitattributes` binary guard before committing PNGs; AGENTS.md
rule 5 no-weapons steer for Woofy; templates don't render `image` yet).
Cross-linked from `cast/README.md` (schema comment + Editing bullet).
No code touched.

### Findings

#### F1 — MINOR | agent-authoring | docs/character-art.md:41 | rule named specific real people where a generic rule suffices
- what: The "Humans get no portrait" bullet named "Lennon and Dad" as
  the real people, when the rule holds for any `kind = "human"`.
- why:  agent-authoring compliance (avoid embedding personal
  identifiers where a generic, more maintainable rule works); the
  generic form also survives adding a third human without an edit.
- fix:  Reworded to "Human characters (`kind = \"human\"`) are real
  people; they keep the silhouette placeholder and carry no `image`
  field."
- status: Fixed

## 2026-07-15 — Ruff Ruff candidate gallery

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: GPT-5.6 Sol (copilot)
- Delegated:      no
- Files:
  - .gitattributes (new)
  - static/stuffies/review/ruff-ruff--candidate-clean.png (new)
  - static/stuffies/review/ruff-ruff--candidate-well-loved.png (new)
  - docs/character-art.md
  - src/routes/characters.rs
  - templates/character.html
  - static/app.css
  - tests/router_smoke.rs
  - .github/agent-review-log.md

Change summary: added two noncanonical Ruff Ruff design candidates and a
conditional character-detail gallery. Candidate discovery matches
`<stable-id>--candidate-<lowercase-kebab-label>.png` in
`static/stuffies/review/`, filters invalid/non-file entries, sorts display
labels, treats a missing directory as no candidates, and propagates other
filesystem errors. The gallery renders semantic figures with accessible alt
text only when candidates exist. Added the `*.png binary` guard, documented
temporary public-static scope and promotion/deletion, and corrected Ruff
Ruff's identity lock to the round-left/X-right stitched face. Six unit and
template tests cover discovery/rendering; two tier-2 tests fetch both PNGs
through the real static service. `cargo check` green; 43 unit + 12 integration
tests green; touched Rust files rustfmt-clean; clippy has only six pre-existing
warnings outside this slice. The earlier visual-identity F10 authenticated
view-model gap remains Deferred on #15: this change tests gallery rendering
directly and static delivery end-to-end, but cannot yet exercise an
authenticated `/council/{id}` request through the full router.

### Findings

#### F1 — NIT | docs | docs/character-art.md | binary-guard wording described future work after the guard landed
- what: The PNG rule still said to add `.gitattributes` and framed it as open
  backlog work after this change created the file.
- why:  Agent-facing docs must describe current state accurately.
- fix:  Reworded it as a standing invariant linked to `../.gitattributes`.
- status: Fixed

#### F2 — NIT | docs | docs/character-art.md | candidate face contradicted the Ruff Ruff identity lock
- what: Both candidates use the recognizable asymmetric stitched face while
  the lock still specified two closed eyes.
- why:  The art bible requires one byte-stable identity lock for variants.
- fix:  Locked the notched round viewer-left eye and X-shaped viewer-right
  eye; promotion now requires updating the lock if a selected design differs.
- status: Fixed

#### F3 — NIT | perf | docs/character-art.md | temporary review assets' public deployment was undisclosed
- what: Review PNGs are copied into the container and served publicly under
  `/static` until deletion.
- why:  Accurate scope matters even though the toy illustrations contain no
  private reference photos.
- fix:  Documented direct public reachability, confirmed reference photos are
  absent, and required prompt deletion after selection.
- status: Fixed

#### F4 — MINOR | tests | tests/router_smoke.rs | candidate URLs lacked real static-service coverage
- what: Unit tests asserted generated URL strings but could not catch a
  mount, packaging, MIME, or payload mismatch.
- why:  Test-discipline Rule 1 requires cross-component behavior to use the
  tier-2 real-server harness when independently testable.
- fix:  Added one integration test per candidate asserting HTTP 200,
  `image/png`, and the PNG byte signature through `stuffy_council::serve`.
- status: Fixed

## 2026-07-15 — Ruff Ruff Pog label

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: GPT-5.5 (copilot)
- Delegated:      no
- Files:
  - cast/ruff-ruff.toml
  - .github/agent-review-log.md

Change summary: changed Ruff Ruff's displayed species from "worn beige plush
dog" to "worn beige plush Pog", matching the existing lore that he is a Pog
(Person Dog). `cargo check` green.

### Findings

NO FINDINGS

## 2026-07-15 — stuffy voice and thought-bubble canon

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: GPT-5.6 Sol (copilot)
- Delegated:      no
- Files:
  - cast/woofy.toml
  - cast/bar-bar.toml
  - cast/ruff-ruff.toml
  - cast/dad.toml
  - cast/lennon.toml
  - cast/README.md
  - src/stories/mod.rs
  - docs/character-art.md
  - .github/agent-review-log.md

Change summary: encoded the canon that Ruff Ruff is the only stuffy with
literal voiced English; Dad performs that voice. Every other stuffy makes
only its native sounds/language aloud. When Dad is off-scene, his interpreted
English appears as thought-bubble-equivalent prose attributed to the stuffy
without placing Dad in the scene; when physically present, Dad may translate
aloud. Story-scene art uses post-generation typed overlays rather than
model-rendered text. Updated all affected cast briefs/relationships, the
story prompt, schema example, and art guidance. A temporary-cast unit test
asserts the complete composed prompt contract. `cargo check` green; 44 unit +
12 integration tests green; clippy has only six pre-existing warnings.

### Findings

#### F1 — MINOR | correctness | cast/dad.toml | title implied Dad voices the entire crew
- what: "The Voice and Interpreter Behind the Crew" could scope "Voice" to
  every stuffy and steer the small model toward voiced English.
- why:  Ruff Ruff alone has Dad-performed literal dialogue.
- fix:  Changed the title to "Ruff Ruff's Voice and the Crew's Interpreter."
- status: Fixed

#### F2 — MINOR | correctness | cast/woofy.toml + cast/bar-bar.toml | relationship subjects were ambiguous
- what: "interprets his hums" / "interprets each tonal Bar" could make the
  stuffy appear to interpret itself.
- why:  Dad is the interpreter; relationship briefs go directly to the model.
- fix:  Began both bonds with the explicit subject "Dad" and named what he
  translates.
- status: Fixed

#### F3 — MINOR | tests | src/stories/mod.rs | regression test bypassed build_prompt and off-scene invariants
- what: Constant-fragment assertions could pass while the composed World or
  character-brief prompt regressed.
- why:  Test-quality policy requires robust coverage of the modified public
  prompt builder.
- fix:  Replaced it with a temporary three-character registry and asserted
  the complete final prompt: native cue, off-scene non-presence, thought
  bubble, Dad-present translation, World block, and injected Woofy brief.
- status: Fixed

#### F4 — MINOR | docs | docs/character-art.md | overlay metadata did not preserve bubble semantics
- what: Generic dialogue/sound metadata could not deterministically choose
  native text, a stuffy thought bubble, or Dad's speech bubble.
- why:  Visual semantics must survive when text is overlaid after generation.
- fix:  Required typed overlay records with kind (`native_sound`, `thought`,
  `speech`), attributed character id, text, anchor point, and placement
  rectangle.
- status: Fixed

## 2026-07-16 — free indirect discourse and OG canon

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: GPT-5.5 (copilot)
- Delegated:      no
- Files:
  - cast/woofy.toml
  - cast/bar-bar.toml
  - cast/dad.toml
  - cast/ruff-ruff.toml
  - cast/README.md
  - src/stories/mod.rs
  - docs/character-art.md
  - .github/agent-review-log.md

Change summary: replaced literal thought-bubble narration in prose with free
indirect discourse. Quoted English remains exclusive to humans/Ruff Ruff;
other stuffies' native sounds are optional cues used only for character or
timing, while interpreted meaning is woven into viewpoint prose without
naming thought bubbles, translations, or "he meant." Visual thought bubbles
remain typed post-generation overlays only. Removed Woofy/Bar Bar native-sound
catchphrases and Ruff Ruff's "As the OG" catchphrase; clarified that "the OG"
is Lennon's label for her oldest stuffy. Shortened stories from 300–500 to
220–350 words. Regression tests compose the prompt from all five committed
cast TOMLs and assert the prose, sound, length, and catchphrase contracts.
`cargo check` green; 45 unit + 12 integration tests green; clippy has only six
pre-existing warnings.

### Findings

#### F1 — MINOR | correctness | src/stories/mod.rs | story length remained 300–500 words
- what: The prompt did not encode the requested direction toward less text.
- why:  The current prose should move toward the future image-led format.
- fix:  Changed the target to 220–350 words, favored a few vivid scenes over
  exhaustive dialogue/explanation, and asserted both directives.
- status: Fixed

#### F2 — MINOR | correctness | cast/woofy.toml + cast/bar-bar.toml | native sounds remained catchphrases
- what: `Catchphrase:` strongly encouraged the 8B model to repeat Woofy's hum
  and Bar Bar's native sounds despite the optional-cue rule.
- why:  Native sounds are language/character cues, not obligatory slogans.
- fix:  Removed both catchphrase fields and asserted Woofy, Bar Bar, and Ruff
  Ruff have no catchphrase or former catchphrase text in the final prompt.
- status: Fixed

#### F3 — MINOR | tests | src/stories/mod.rs | regression used synthetic cast briefs
- what: Handwritten fixtures could pass while committed Woofy, Bar Bar, Dad,
  or Ruff Ruff TOMLs reintroduced conflicting guidance.
- why:  Incident-linked regression coverage must guard the production
  contract that generated the failure.
- fix:  The test now copies all five committed TOMLs into a temporary registry,
  composes a Woofy/Bar Bar/Ruff Ruff prompt, and verifies production briefs,
  free indirect discourse, optional sounds, shorter length, and absence of
  former stuffy catchphrases/literal bubble wording.
- status: Fixed

## 2026-07-16 — selective character hooks and hard invariants

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: GPT-5.6 Sol (copilot)
- Delegated:      no
- Files:
  - cast/lennon.toml
  - cast/ruff-ruff.toml
  - cast/woofy.toml
  - cast/dad.toml
  - cast/README.md
  - src/cast.rs
  - src/stories/mod.rs
  - docs/character-art.md
  - .github/agent-review-log.md

Change summary: reworked the prompt from fact recitation to selective
children's-fiction guidance. Character briefs are a palette, not a checklist;
scenes advance through wants, action, interruption, adaptation, and subtext.
Lennon now speaks naturally as a mischievous 10-year-old without a ritual
phrase. Ruff Ruff's spoon is one optional doctor-tangent hook. Woofy's sole
optional threat cue is an implied `chk-chk`, never a named/visible weapon;
hard canon says he is Supreme Leader and never security/guard/subordinate,
while his crew may protect him. Ruff Ruff's council grievance is optional
friction, and generic off-council rendering is factual only. Art guidance
also treats sounds/props as scarce optional panel elements. Production-cast
tests guard the direct incidents and distinguish optional hooks from hard
role invariants. `cargo check` green; 46 unit + 12 integration tests green;
clippy has only six pre-existing warnings.

### Findings

#### F1 — MAJOR | correctness | src/stories/mod.rs | repeated optional hooks remained checklist-salient
- what: Spoon and `chk-chk` details were repeated across the system prompt,
  owning brief, lore, and Dad's anecdote, overwhelming "rare/omit" qualifiers
  for the 8B model.
- why:  Repetition was the root cause of the reported checklist behavior.
- fix:  Kept each hook exactly once in its owning speech style; made the
  system optionality rule generic; removed duplicate lore and Dad-anecdote
  references while preserving prop-free medical chaos.
- status: Fixed

#### F2 — MINOR | correctness | src/cast.rs | generic off-council brief forced a permanent grievance
- what: Every off-council character received an unconditional emotional
  consequence instead of a factual status.
- why:  Ruff Ruff's grievance is available scene friction, not a required
  defining beat.
- fix:  Renderer now emits only `Council status: NOT on the council.`;
  Lennon source lore also describes the dispute as scene-dependent.
- status: Fixed

#### F3 — MINOR | tests | src/stories/mod.rs | incident bans used fragile exact casing/punctuation
- what: Exact `AK` and `What if we...?` checks missed case, plural, and
  phrase-prefix variants.
- why:  Incident regressions must fail if the prohibited pattern returns in
  any committed production brief.
- fix:  Lowercase tokenization rejects standalone `ak`/`aks`, the composed
  prompt rejects the full `what if we` stem, and committed Lennon lore rejects
  restored permanent-grievance wording.
- status: Fixed

## 2026-07-16 — self-contained daily story arcs

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: GPT-5.5 (copilot)
- Delegated:      no
- Files:
  - src/stories/mod.rs
  - src/stories/ollama.rs
  - .github/agent-review-log.md

Change summary: fixed an incident where a 282-word story called "The Great
Debate" ended as the debate began. The prompt now requires every daily story
to fulfill its title promise on-page, allocate space for setup/action/outcome,
resolve the central dramatic question, and end after its consequence rather
than at an event threshold. Continuity may carry forward, but no daily
cliffhangers. Separately, Ollama completion metadata is validated so
`done=false`, token-limit stops, and unknown completion reasons cannot be
cached as stories; legacy responses without metadata remain accepted. Focused
tests cover the exact incident endings and protocol branches. `cargo check`
green; 53 unit + 12 integration tests green; clippy has only six pre-existing
warnings.

### Findings

#### F1 — MINOR | tests | src/stories/mod.rs | incident regression omitted two reported threshold endings
- what: The test guarded generic closure and "the meeting had officially
  begun" but not "the debate was off" or the second threshold example.
- why:  Incident-linked coverage must fail if the exact failure steers are
  removed.
- fix:  Asserted all three prompt examples: `the debate was off`, `the meeting
  had officially begun`, and `and then the door opened`.
- status: Fixed

#### F2 — MINOR | tests | src/stories/ollama.rs | legacy omitted completion metadata was not deserialized in a test
- what: Direct struct construction did not prove older Ollama JSON without
  `done_reason`/`eval_count` remained compatible.
- why:  Compatibility depends on Serde defaults for omitted fields.
- fix:  Deserialized `{"response":"complete","done":true}` and asserted the
  exact completed text is accepted.
- status: Fixed

#### F3 — MINOR | correctness | src/stories/ollama.rs | unknown completion reasons failed open
- what: Any non-`length` reason with `done=true` was accepted into the daily
  cache.
- why:  Ambiguous future/malformed completion states should not be cached.
- fix:  Accepted only `stop` and legacy absent reason; retained the specific
  length error and rejected every other reason with its value in the error.
- status: Fixed

## 2026-07-16 — council election and rival claimants

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: GPT-5.6 Sol (copilot)
- Delegated:      no
- Files:
  - cast/README.md
  - cast/lennon.toml
  - cast/bar-bar.toml
  - cast/ruff-ruff.toml
  - cast/woofy.toml
  - src/stories/mod.rs
  - .github/agent-review-log.md

Change summary: established the council's political origin and non-recognition
canon. The election for President of Lennon's room tied; Lennon's compromise
assigned tied candidates the same formal co-president office, but they reject
equal authority. She deliberately excluded tied candidate Ruff Ruff to create
chaos. Woofy claims universal supremacy, Bar Bar royal authority over his
pride, and Ruff Ruff seniority plus a one-vote victory. They coexist and may
cooperate transactionally, but never consider one another friends/equals or
validate rank. `The OG` belongs only to Ruff Ruff; he is Lennon's actual number
one, which council members never acknowledge. Lennon may resolve daily action
without dissolving the rivalry. Production-cast tests guard all reported
incidents. `cargo check` green; 54 unit + 12 integration tests green; clippy
has only six pre-existing warnings.

### Findings

#### F1 — BLOCK | correctness | src/stories/mod.rs + cast/woofy.toml | Supreme-Leader ban was not universal and lore contradicted it
- what: The system banned only Bar Bar from using the title while Woofy's lore
  said every Avocatt called him Supreme Leader.
- why:  Nobody but Woofy validates or uses his self-appointed title; the
  local brief would override the narrower global rule for the 8B model.
- fix:  Universally banned use/validation by Lennon, Dad, council members, and
  Avocatt crew; rewrote Woofy motivations/lore and the README example so he
  demands a title nobody honors.
- status: Fixed

#### F2 — MINOR | tests | src/stories/mod.rs | hard election and rank rules were incompletely guarded
- what: The regression omitted universal title exclusivity, Ruff Ruff's
  refusal to help Woofy lead, self-cast-vote theory, and the three power bases.
- why:  Incident-linked production-data tests must fail when corrected canon
  regresses.
- fix:  Added assertions for universal title scope, full Ruff refusal, tied
  one-vote/first-place theory, and universe/royal/seniority power bases.
- status: Fixed

#### F3 — MINOR | docs | cast/README.md | schema example preserved forbidden Avocatt validation
- what: Copyable example lore still said every Avocatt used Woofy's title.
- why:  Agent-facing examples must match hard canon.
- fix:  Rewrote the example to say Woofy demands the title and nobody,
  including his crew, uses or validates it; added negative regression guards.
- status: Fixed

#### F4 — MINOR | docs | cast/README.md | non-friend/equal rule excluded Ruff Ruff
- what: The rule was scoped to formal co-presidents, but excluded Ruff Ruff is
  also a rival claimant.
- why:  Transactional coexistence and unresolved rivalry apply to every
  claimant.
- fix:  Scoped the rule to all rival claimants: formal co-presidents plus
  excluded Ruff Ruff.
- status: Fixed

## 2026-07-16 — council political canon follow-up

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: Claude Opus 4.7 (copilot)
- Delegated:      no
- Files:
  - cast/README.md
  - cast/lennon.toml
  - cast/bar-bar.toml
  - cast/ruff-ruff.toml
  - cast/woofy.toml
  - src/stories/mod.rs

Change summary: follow-up review after the BLOCK verified universal Supreme
Leader non-recognition, the tied-election compromise, all three power bases,
pairwise rivalry, exclusive OG label, unacknowledged Lennon-number-one fact,
and Lennon's ability to resolve daily action without political reconciliation.

### Findings

NO FINDINGS

## 2026-07-16 — promote Ruff Ruff Well Loved portrait

- Author model:   Claude Opus 4.8 (copilot)
- Reviewer model: GPT-5.5 (copilot)
- Delegated:      no
- Files:
  - static/stuffies/ruff-ruff.png (new)
  - static/stuffies/review/ruff-ruff--candidate-clean.png (deleted)
  - static/stuffies/review/ruff-ruff--candidate-well-loved.png (deleted)
  - src/web/portrait.rs (new)
  - src/web/mod.rs
  - src/routes/characters.rs
  - src/routes/home.rs
  - templates/character.html
  - templates/council.html
  - templates/home.html
  - static/app.css
  - tests/router_smoke.rs
  - docs/character-art.md
  - .github/agent-review-log.md

Change summary: promoted the family-selected Well Loved design to Ruff Ruff's
canonical 512×512 transparent RGBA portrait, preserving the cream cutout,
sticker edge, stitched face, spoon, and repair marks. Removed both temporary
review candidates. A shared resolver renders only an existing exact
`<stable-id>.png` declaration; missing/noncanonical assets keep the permanent
silhouette fallback. Wired detail, council-grid, and home-spotlight portraits
with accessible/redundant alt handling and stable dimensions. Added resolver,
template, and real-static-service tests. Asset SHA256:
`b2ac4e21894b36a31fb4a240dec5d76a3f389eb487beeb03e6580fdb53be3f1d`.
`cargo check` green; 61 unit + 11 integration tests green; clippy has only six
pre-existing warnings.

### Findings

#### F1 — NIT | docs | docs/character-art.md | sizing guidance still said object-fit cover
- what: The asset table warned about cropping after CSS switched to contain.
- why:  Future portrait framing guidance must match runtime behavior.
- fix:  Documented `object-fit: contain` and transparent padding/faction-tint
  behavior.
- status: Fixed

#### F2 — NIT | agent-authoring | static/app.css | portrait comments described permanent fallback as temporary
- what: Comments still called the shared frame and silhouette placeholders
  pending issue #8.
- why:  Current invariant is canonical art when available and permanent
  fallback for humans/missing files.
- fix:  Reworded both comments to describe the shared frame and permanent
  fallback.
- status: Fixed

## 2026-07-18 — issue-8 phase-one portraits and landing roster

- Author model:   GitHub Copilot (current session)
- Reviewer model: Claude Opus 4.8 + GPT-5.6 Sol; Claude Opus 4.7 after BLOCK fix (copilot)
- Delegated:      no
- Files:
  - static/stuffies/review/bar-bar--candidate-faithful.png (new)
  - static/stuffies/review/bar-bar--candidate-storybook.png (new)
  - static/stuffies/review/woofy--candidate-regal.png (new)
  - static/stuffies/review/woofy--candidate-commanding.png (new)
  - src/routes/characters.rs
  - src/routes/home.rs
  - templates/home.html
  - tests/router_smoke.rs
  - .github/agent-review-log.md

Change summary: added two transparent review portraits each for Bar Bar and
Woofy without promoting canonical art before family selection. The landing
spotlight now includes all five cast members in name order, including
off-council Ruff Ruff, while missing canonical portraits retain silhouettes.
Generic gallery and real-static-route tests cover all candidate files.
Touched-file rustfmt and `cargo check` pass; 66 unit + 15 integration tests
pass; clippy reports only the six baseline warnings. The final follow-up
review returned `NO FINDINGS`.

### Findings

#### F1 — NIT | tests | src/routes/characters.rs | test name described the wrong filesystem scenario
- what: `load_image_candidates_unreadable_directory_returns_contextual_error`
  passed a regular file rather than an unreadable directory.
- why:  Test names must state the actual scenario so future agents do not
  infer permission-error coverage that does not exist.
- fix:  Renamed it to
  `load_image_candidates_path_is_file_returns_contextual_error`.
- status: Fixed

#### F2 — BLOCK | correctness | static/stuffies/review/bar-bar--candidate-storybook.png | floor-shadow antialiasing remained after initial cutout
- what: The generated gray oval was transparent internally, but a faint
  antialiased ellipse perimeter remained visible on a contrasting background.
- why:  The issue specification and art bible prohibit baked floor shadows;
  family selection must compare compliant candidates.
- fix:  Removed the residual neutral component while preserving Bar Bar's
  colored core and sticker edge, normalized hidden transparent RGB to black,
  and verified the result on a fresh teal composite. Claude Opus 4.7's
  required post-BLOCK review returned `NO FINDINGS`.
- status: Fixed

## 2026-07-18 — Woofy selection and four-choice Bar Bar gallery

- Author model:   GitHub Copilot (current session)
- Reviewer model: GPT-5.5 (copilot)
- Delegated:      no
- Files:
  - static/stuffies/woofy.png (new)
  - static/stuffies/review/woofy--candidate-regal.png (deleted)
  - static/stuffies/review/woofy--candidate-commanding.png (deleted)
  - static/stuffies/review/bar-bar--candidate-faithful.png (deleted)
  - static/stuffies/review/bar-bar--candidate-storybook.png (deleted)
  - static/stuffies/review/bar-bar--candidate-well-loved-happy.png (new)
  - static/stuffies/review/bar-bar--candidate-well-loved-angry.png (new)
  - static/stuffies/review/bar-bar--candidate-bold-happy.png (new)
  - static/stuffies/review/bar-bar--candidate-bold-angry.png (new)
  - src/routes/characters.rs
  - tests/router_smoke.rs
  - docs/character-art.md
  - .github/agent-review-log.md

Change summary: promoted the family-selected Woofy Regal portrait to the
canonical 512×512 transparent asset and removed Woofy's review candidates.
Replaced Bar Bar's phase-one drafts with four 1024×1024 transparent review
choices: Well Loved Happy, Well Loved Angry, Bold Happy, and Bold Angry.
Candidate-discovery coverage asserts all four sorted labels and URLs, and
real-static-service tests cover every review asset. Touched-file rustfmt,
focused tests, and `cargo check` pass.

### Findings

#### F1 — MINOR | other | static/stuffies/review/bar-bar--candidate-bold-happy.png | initial Scruffier label overstated the visible wear difference
- what: The second happy candidate was not visibly scruffier than Well Loved
  Happy at gallery size; its actual distinction was a bolder, more saturated
  treatment.
- why:  Artifact names must match their scope and semantics so agents and
  family reviewers can understand the current state in one read.
- fix:  Renamed the matched pair and files from Scruffier Happy/Angry to Bold
  Happy/Angry, then updated discovery and real-static-route tests.
- status: Fixed

#### F2 — NIT | tests | src/routes/characters.rs | Woofy phase-one test name implied removed candidates remained active
- what: The candidate-filtering test retained phase-one Woofy names after the
  family selected Regal and both review files were removed.
- why:  Test names must state the behavior and scenario rather than preserve a
  stale project phase.
- fix:  Reframed the test as generic requested-character filtering with neutral
  Alpha/Beta fixtures.
- status: Fixed

## 2026-07-18 — finalize issue-8 selected portraits

- Author model:   GitHub Copilot (current session)
- Reviewer model: GPT-5.6 Sol (copilot)
- Delegated:      no
- Files:
  - static/stuffies/woofy.png (new)
  - static/stuffies/bar-bar.png (new)
  - static/stuffies/bar-bar--angry.png (new)
  - src/routes/characters.rs
  - src/routes/home.rs
  - templates/home.html
  - tests/router_smoke.rs
  - docs/character-art.md
  - .github/agent-review-log.md

Change summary: finalized family selections for issue #8. Woofy Regal and Bar
Bar Well Loved Happy are canonical 512×512 transparent portraits. Bar Bar Well
Loved Angry remains a 512×512 named variant for future scenes. Temporary review
files are removed, the landing spotlight shows all five cast members with
canonical portraits for all three stuffies, and Dad/Lennon retain silhouette
fallbacks. Touched-file rustfmt and `cargo check` pass; 65 unit + 14 integration
tests pass. Strict clippy retains only the six documented baseline warnings.

### Findings

NO FINDINGS

## 2026-07-22 — remove unused session secret

- Author model:   GitHub Copilot (current session)
- Reviewer model: Claude Opus 4.8 (copilot)
- Delegated:      no
- Files:
  - .env.example
  - .github/instructions/test-quality.instructions.md
  - AGENTS.md
  - README.md
  - cast/ruff-ruff.toml
  - docs/dev-setup.md
  - src/config.rs
  - tests/common/mod.rs
  - .github/agent-review-log.md

Change summary: verified tower-sessions uses an opaque cookie id backed by the
SQLite session store and consumes no signing key. Removed the unused session
secret from configuration, test fixtures, setup instructions, and deployment
examples. Extracted deterministic config lookup and added minimum-environment
plus required-client-id tests. Included the user-required Ruff Ruff canon
correction from "stuffed dog" to "stuffed pog." Touched-file rustfmt passes;
67 unit + 14 integration tests pass. Strict clippy retains only the six
documented baseline warnings.

### Findings

#### F1 — MAJOR | docs | docs/dev-setup.md | setup outcome still promised a session secret
- what: The runbook's outcome list retained a prose-only session-secret
  requirement after the variable and Step 5 instructions were removed.
- why:  Agent-facing documentation must remain factual when configuration
  requirements change.
- fix:  Changed the outcome to promise only a `.env` with the Google client id
  and rescanned active docs for both token and prose variants.
- status: Fixed

#### F2 — MINOR | tests | src/config.rs | injected config seam had happy-path-only coverage
- what: The new deterministic lookup test proved valid defaults but did not
  exercise a parser rejection branch.
- why:  Parsers require negative validation under the repository test-quality
  policy.
- fix:  Added a missing-Google-client-id test asserting the exact required
  error through the same injected lookup seam.
- status: Fixed

## 2026-07-22 — complete config validation coverage

- Author model:   GitHub Copilot (current session)
- Reviewer model: GPT-5.5 (copilot)
- Delegated:      no
- Files:
  - src/config.rs
  - .github/agent-review-log.md

Change summary: completed deterministic `Config` validation coverage through
the injected lookup seam. Tests cover minimum defaults, every configured
field, production/development regimes, origin fallback/trimming, required and
blank client ids, unknown environments, invalid bind addresses, malformed
numeric values, and zero boundaries. The boundary tests exposed that timeout
and rate-limit fields accepted zero despite their positive-integer contract;
typed parsers now reject malformed and zero values with the existing errors.
Obsolete session-secret scenarios were excluded because PR #27 removed that
unused configuration. Touched-file rustfmt passes; 80 unit + 14 integration
tests pass. Strict clippy retains only six documented baseline warnings.

### Findings

NO FINDINGS

## 2026-07-23 — signed Google JWT auth harness

- Author model:   GitHub Copilot (current session)
- Reviewer model: Claude Opus 4.7 (copilot)
- Delegated:      no
- Files:
  - Cargo.toml
  - Cargo.lock
  - src/auth.rs
  - tests/common/mod.rs
  - tests/support/google_jwt.rs
  - tests/router_smoke.rs
  - .github/agent-review-log.md

Change summary: added a shared generated-RSA/JWKS fixture and deterministic
Google-format ID-token signing. Unit tests cover valid claims, issuer,
audience, expiry/leeway, verified email, and wrong signatures. Full-router
tests cover successful allowlisted sign-in, session-id rotation and stale-id
rejection, allowlist denial, Google CSRF mismatch, and wrong-key rejection.
The alternate JWKS constructor is hidden and documented as test-only. Touched
files are rustfmt-clean; 87 unit + 18 integration tests pass. Strict clippy
retains only three findings in untouched modules.

### Findings

#### F1 — MAJOR | tests | tests/router_smoke.rs | success test did not prove session-id rotation
- what: The initial success test began without an existing session, so deleting
  `session.cycle_id()` would not make it fail.
- why:  Security regression tests must assert the fixation-defense transition,
  not only that some authenticated cookie exists.
- fix:  Seeded an anonymous session through the same SQLite store, sent its real
  cookie on sign-in, asserted the returned id differs, authenticated with the
  new id, and confirmed the stale id redirects to `/login`.
- status: Fixed

#### F2 — MINOR | tests | tests/common/mod.rs | Google client id duplicated fixture constant
- what: The AppState fixture repeated the JWT fixture audience literal.
- why:  Duplicated test contract values can drift into misleading failures.
- fix:  Configured AppState from `jwt::TEST_CLIENT_ID`.
- status: Fixed

#### F3 — MINOR | agent-authoring | prior visual-identity F10 trigger was satisfied
- what: F10 deferred authenticated home spotlight and character relationship
  rendering until #15 supplied a signed-JWT harness.
- why:  Review policy requires a fresh disposition when a prior finding's
  concrete trigger becomes true.
- fix:  The harness blocker is removed, but those view-model scenarios do not
  overlap this auth-route change.
- status: Deferred (next change touching authenticated rendering in
  `src/routes/home.rs` or `src/routes/characters.rs`; owner: next agent to
  modify either path)

#### F4 — NIT | agent-authoring | shared fixture path coupling was undocumented
- what: Unit and integration test crates include the same source file through
  separate `#[path]` attributes.
- why:  Moving the fixture requires updating both non-obvious includes.
- fix:  Added reciprocal one-line coupling comments at both include sites.
- status: Fixed

#### F5 — NIT | security | alternate JWKS constructor looked production-ready
- what: The public constructor could be mistaken for an approved production
  trust-anchor override.
- why:  Production verification must remain pinned to Google's JWKS endpoint.
- fix:  Marked it `#[doc(hidden)]` and documented that production MUST use
  `JwkCache::new`.
- status: Fixed

## 2026-07-23 — PR 29 Copilot comment fixes

- Author model:   GitHub Copilot (current session)
- Reviewer model: GPT-5.6 Sol (copilot)
- Delegated:      no
- Files:
  - src/auth.rs
  - tests/common/mod.rs
  - tests/support/google_jwt.rs
  - .github/agent-review-log.md

Change summary: addressed both Copilot PR comments. Local JWKS fixture servers
now retain their task handles and abort them on fixture drop, including the
pre-existing unit fake. Production construction remains private and pinned to
Google; the hidden public integration escape hatch is explicitly named
`with_test_jwks_url`. Focused verifier and Google POST matrices pass, and the
full 87 unit + 18 integration suite remains green.

### Findings

#### F1 — NIT | agent-authoring | src/auth.rs | fixture lifecycle comment repeated the old misconception
- what: The field comment claimed dropping a Tokio `JoinHandle` aborts its
  task, although dropping detaches it and the fixture's `Drop` calls abort.
- why:  Comments must preserve the actual non-obvious lifecycle invariant.
- fix:  Reworded the comment to state that dropping `FakeJwks` aborts the
  spawned server task.
- status: Fixed

## 2026-07-23 — graceful story-generator outage

- Author model:   GitHub Copilot (current session)
- Reviewer model: Claude Opus 4.8 (copilot)
- Delegated:      no
- Files:
  - src/stories/mod.rs
  - src/stories/ollama.rs
  - src/routes/home.rs
  - src/routes/characters.rs
  - templates/story.html
  - tests/common/mod.rs
  - tests/router_smoke.rs
  - .github/agent-review-log.md

Change summary: added typed temporary-versus-internal story generation
failures. Ollama connection and timeout errors render a friendly authenticated
story page with HTTP 200 and warn-level diagnostics; HTTP, JSON, protocol, and
other failures retain the generic non-leaking 500. Local adapter tests pin
classification, and signed-auth full-router tests pin both HTTP outcomes and
prove temporary failures are retried rather than cached. The previously
deferred visual-identity F10 is now fixed: existing tests cover spotlight
ordering and portrait fallbacks, while new tests cover relationship display
name resolution and missing-target fallback. Touched files are rustfmt-clean;
92 unit + 20 integration tests pass. Strict clippy retains only three findings
in untouched modules.

### Findings

#### F1 — NIT | modularity | templates/story.html | paragraph markup duplicated across availability branches
- what: Both branches rendered the same reading container and paragraph loop.
- why:  Duplicate template markup can silently diverge when future agents edit
  only one branch.
- fix:  Hoisted the shared reading block and gated only Featuring/model
  metadata on `!is_unavailable`.
- status: Fixed

#### F2 — NIT | tests | tests/router_smoke.rs | unavailable regression did not prove failure was uncached
- what: A persisted friendly placeholder would still satisfy the initial HTTP
  200/body assertions.
- why:  State-transition tests must assert state that must remain unchanged.
- fix:  Counted generator calls and repeated the request with the same
  authenticated session; the second call must invoke generation again.
- status: Fixed

#### F3 — MINOR | tests | prior visual-identity F10 | authenticated view-model coverage deferral matured
- what: #15 removed the auth-harness blocker, and this change touches
  authenticated home rendering, satisfying the deferred trigger.
- why:  Earlier overlapping findings require a fresh disposition when their
  trigger becomes true.
- fix:  Existing home tests cover all-cast spotlight ordering and portrait
  fallbacks. Extracted `relationship_views` and tested known display-name
  resolution plus missing-target stable-id fallback.
- status: Fixed

## 2026-07-23 — deterministic LF line endings

- Author model:   GitHub Copilot (current session)
- Reviewer model: GPT-5.5 (copilot)
- Delegated:      no
- Files:
  - .gitattributes
  - .github/agent-review-log.md

Change summary: enforced LF for all text through `text=auto eol=lf`, with
explicit Rust, TOML, HTML, Markdown, SQL, and Dockerfile rules. PNG and WOFF2
assets remain binary. `git add --renormalize .` changed no existing content
because every tracked text blob was already LF. Effective-attribute checks,
repository-wide staged-blob CR scanning, and diff/binary audits pass; 92 unit
+ 20 integration tests remain green.

### Findings

NO FINDINGS

