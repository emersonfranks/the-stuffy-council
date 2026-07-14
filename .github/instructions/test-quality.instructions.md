---
applyTo: "**/*.rs"
description: "Test scope, coverage, and failure-pattern discipline for the-stuffy-council. Rules here are authoritative for this repo; the shape is adapted from rad-service's testing methodology."
---

# Test Discipline (Always-On)

Applies to every `.rs` file in this repo. Tests exist so future agents can
refactor safely; they are not human bloat. Every non-trivial change must
add or update tests along the coverage dimensions below.

Style rules (naming, structure, dev-dep list) live in
[test-style.instructions.md](test-style.instructions.md). Both apply.

## Stack

- **Built-in `#[test]` / `#[tokio::test]`** in the standard `cargo test`
  runner. No third-party test-runner.
- **Unit tests** live in `#[cfg(test)] mod tests { … }` at the bottom of
  the module they test. Same file as the code they exercise.
- **Integration tests (tier-2)** live in `tests/*.rs` at the crate root
  and use the fully-layered app via `stuffy_council::serve` on an ephemeral
  TCP port. Shared setup: [tests/common/mod.rs](../../tests/common/mod.rs).
- **End-to-end tests (tier-3) do not exist yet.** Absence is not an
  exemption — flag any scenario that requires an external system
  (real Google, real Ollama, deployed image) as a deferred gap.

Run per-crate: `cargo test`. No solution/workspace layer.

## Coverage dimensions

For every public function, handler, route, or serialized type you add or
modify, derive scenarios across all five dimensions before writing tests.
If a dimension is genuinely N/A for that unit, say so in the module test's
top comment; do not silently omit it.

1. **Functional** — correct output for valid input on the happy path.
2. **Edge / boundary** — empty, zero, whitespace, unicode, max size,
   regime transitions (e.g. `Environment::Development` vs
   `Environment::Production`).
3. **Negative** — invalid input, malformed data, unauthorized callers,
   violated preconditions. Assert on the *rejection* (specific error
   variant, HTTP status, `AppError` type), not "didn't panic."
4. **Error handling** — every dependency failure the caller must observe:
   HTTP 4xx/5xx from external calls, SQLite errors, missing files,
   template render failures. Validate propagation and the user-visible
   result.
5. **State transition** — for anything with persisted or in-memory state:
   valid transitions, rejected invalid ones, behavior after
   partial/interrupted writes (session, `story_repo`, `AccessList` reload).

## Mandatory test categories

Three scenario classes cannot be satisfied by unit tests. Mocks encode
each side's assumptions and miss cross-component contract mismatches.

- **Rule 1: Cross-component workflows** — anything that spans multiple
  modules end-to-end (e.g. GIS POST → JWT verify → `AccessList::check` →
  `upsert_user` → `session.cycle_id`). Requires an integration test in
  `tests/*.rs` against the real router via `stuffy_council::serve`.
- **Rule 2: Incident-linked scenarios** — anything reproducing a bug we
  hit in dev or production. Every bug fix MUST land with a regression
  test that reproduces the original failure path. The current canonical
  example: `tests/router_smoke.rs::get_login_returns_200_for_anonymous_visitor`
  guards the `tower_governor` / `ConnectInfo` outage.
- **Rule 3: Auth flows** — every state-changing route and every route
  behind `require_user`. Two things are required:
  1. **Route-metadata coverage** — for each route in `src/routes/mod.rs`,
     assert its access behavior with the real middleware stack: public
     routes return 200; protected routes redirect anonymous callers to
     `/login` (not 200, not 500). See
     `tests/router_smoke.rs::get_root_redirects_anonymous_to_login`.
  2. **Real-middleware smoke** — at least one integration test per
     auth-affecting route change must exercise the request through the
     full layer stack (session, CSRF, rate limiter, security headers).
  Pure handler unit tests do not satisfy either part.

When a required test can't yet exist (harness missing, external system
required), flag it in the PR as a deferred gap and disposition it in the
subagent review log entry. Do not pretend unit coverage substitutes.

## Test quality criteria

1. **Assertion robustness** — specific enough that a subtle defect fails
   them. `assert!(result.is_ok())` is insufficient; assert on the
   inner value / status / variant.
2. **Condition validation** — set up a known initial state, verify the
   final state, and verify that state which should not change did not.
3. **Regression detection** — if the behavior under test changes, the
   test must fail. Never assert what the code just happens to do; assert
   what the requirement is.
4. **No false positives** — no broad `catch_unwind`, no swallowing
   `Result`s with `.ok()` in the assert. Use `assert!(matches!(err,
   AppError::X(_)))` for typed errors.
5. **No flakes** — no wall-clock dependence, no live external network,
   no reliance on iteration order of unordered collections. Seed any
   randomness. Freeze time via injected clock, not `SystemTime::now`.
   *Sanctioned exception:* the ~50ms readiness sleep after
   `tokio::spawn(serve(...))` in
   [tests/router_smoke.rs](../../tests/router_smoke.rs) — documented in
   [test-style.instructions.md](test-style.instructions.md) scaffolding
   step 4. Any new timing-dependent construct outside that helper
   needs a written justification.
6. **Clarity of intent** — the name states the scenario and the expected
   outcome (`load_from_file_defaults_admin_to_false_when_omitted`). One
   behavioral claim per test.
7. **Determinism** — for input-varying suites, use a helper or one test
   per case; do not loop inside a `#[test]` in a way that hides which
   case failed. Parameterization crates (e.g. `rstest`) are permitted
   when the variants share a real assertion.

## Failure patterns to avoid

- **Assertion-free execution** — calling code under test with no
  `assert*!` following it.
- **Tautological assertions** — asserting that a mock returned what it
  was configured to return.
- **Overly broad scope** — `test_all_paths_at_once`. Split.
- **Implementation coupling** — asserting on private-field values or
  internal call sequences. Assert on observable outputs and side effects.
- **Missing negative validation** — happy-path-only suites. Every route
  needs ≥1 4xx / redirect test; every parser needs ≥1 rejection test.
- **Missing boundary validation** — no zero/empty/max tests on
  bounded inputs (e.g. `SESSION_SECRET` min-length, allowlist empty).
- **Environmental dependency** — live external HTTP, real Ollama, real
  Google JWKS in a unit test. Use `tempfile` for on-disk state; use
  ephemeral `TcpListener` + `reqwest` for integration; feature-gate any
  test that requires the outside world.
- **Silent failure tolerance** — `let _ = sut.do();` without asserting.
- **Incomplete state verification** — asserting the return value but
  ignoring the DB row, cookie, or log line the contract also requires.
- **Untraceable magic values** — every expected value must trace to a
  requirement, a spec, or test-data setup declared in the same file.
- **Happy-path-inflated coverage** — ten tests varying only valid
  inputs while edge / negative / error / state dimensions are empty.

## TDD workflow

1. **Read the spec / the change ticket.** Identify the entry points and
   the decision branches through them.
2. **Test list before test code.** Each item is a specific verifiable
   claim (e.g. "`AccessList::check` with mixed-case input returns the
   matched entry"). Not "validates input."
3. **Red-green-refactor per item.** One failing test at a time; commit
   green increments where sensible.
4. **Flag mandatory-category gaps explicitly** if the harness doesn't
   exist (integration surface for the module, live Google, etc.).
5. **Do not declare a change done with zero-ratio dimensions** without
   an explicit "N/A because …" note in the module test comment.

## Review-time expectations

Subagent reviewers enforce this file against every diff. A change that
adds new production logic or fixes a bug must include tests, and the
absence is a MINOR finding by default — MAJOR when the change touches
`auth`, `access`, `web/csrf`, `web/security`, or `error`, and BLOCK when
it lands without a regression test for a bug we can reproduce.
