---
applyTo: "**/*.rs"
description: "Always-on test style for the-stuffy-council: naming, structure, dev-dep list, harness patterns."
---

# Test Style (Always-On)

Applies to every `.rs` file. Covers **how** tests are written. For **what**
they must cover, see [test-quality.instructions.md](test-quality.instructions.md).
Both apply.

## Frameworks and dev-deps

- Built-in `#[test]` / `#[tokio::test]` from `tokio = { features = ["full"] }`.
- Standard `assert!`, `assert_eq!`, `assert_ne!`, `assert!(matches!(x, …))`.
- `tempfile` for scratch dirs/files (on-disk state).
- `reqwest` (already a runtime dep) for integration-test HTTP clients;
  build with `.redirect(reqwest::redirect::Policy::none())` so tests
  observe 3xx responses.
- `async-trait` for implementing `StoryGenerator` and similar traits in
  tests.

New test-only crates go in `[dev-dependencies]` in `Cargo.toml`. Do not
bring in a mocking framework speculatively; introduce one only when a
concrete second use is on the horizon (YAGNI, per
[agent-authoring.instructions.md](agent-authoring.instructions.md)).

## Layout

- **Unit tests** — inline at the bottom of the module they exercise:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      // …
  }
  ```
  Same file as the code under test. No parallel `src/foo_test.rs` file.

- **Integration tests** — files under `tests/`, one per surface:
  `tests/router_smoke.rs`, `tests/story_service.rs`, …
  Shared helpers go under `tests/common/mod.rs` and are declared with
  `mod common;` at the top of each integration test file. Each `tests/*.rs`
  compiles as its own crate; `common` is the standard Rust convention
  for sharing setup across them.

## Naming

Snake-case three-part `method_condition_expected_result`, mirroring
rad-service's `MethodName_Condition_ExpectedResult`:

```rust
#[test]
fn load_from_file_defaults_admin_to_false_when_omitted() { ... }

#[tokio::test]
async fn get_login_returns_200_for_anonymous_visitor() { ... }
```

Two-part (`function_result`) is acceptable for tiny pure-logic tests
(`parse_g_csrf_cookie_empty_header_returns_none`). Don't flip conventions
mid-module.

## Structure

Use Arrange / Act / Assert visible as three logical blocks (comments are
fine but not required when the shape is obvious). Prefer:

```rust
#[test]
fn some_function_with_input_returns_expected_output() {
    // Arrange
    let input = build_input();

    // Act
    let got = function_under_test(input);

    // Assert
    assert_eq!(got, EXPECTED);
}
```

When setup grows past ~5 lines, factor it into a private helper in the
same `mod tests` (e.g. `write_allow_file` in `src/access.rs::tests`).
Helpers named `build_*`, `write_*`, or `spawn_*`.

## Integration-test scaffolding

Every integration test that boots the app follows this template
(implemented in [tests/common/mod.rs](../../tests/common/mod.rs) and
[tests/router_smoke.rs](../../tests/router_smoke.rs)):

1. `common::build_test_app().await` — builds an `AppState` backed by a
   temp SQLite file, an empty cast, a one-user `authorized-users.toml`,
   and a no-op `StoryGenerator`.
2. Bind `TcpListener` on `127.0.0.1:0` (ephemeral port).
3. `tokio::spawn` the server via `stuffy_council::serve(state, listener)`.
4. Small readiness sleep (~50ms) before hitting the port. `tokio::spawn`
   returns immediately; `axum::serve` needs a moment to start accepting.
5. Build a `reqwest::Client` with `redirect(Policy::none)` and hit the
   real HTTP endpoint. Assert on status, headers, and body substrings.
6. `TestApp` (returned by `build_test_app`) owns the `TempDir`; hold it
   for the lifetime of the test so the SQLite file survives.

## What NOT to do

- Do NOT construct a `Router` and call `.oneshot()` for regression tests
  that need to exercise `axum::serve` layers (like the `ConnectInfo`
  extension `tower_governor` reads). `oneshot` bypasses the make-service
  layer where the bug lived. Use the full `serve` path.
- Do NOT hit `googleapis.com`, real Ollama, or any external service in
  any test in this repo. If a scenario requires it, feature-gate it
  with `#[ignore]` or a `#[cfg(feature = "live")]` guard and note in
  the PR that it is a tier-3 scenario deferred until we have a live-test
  harness.
- Do NOT enable request logging in tests (they produce noisy output that
  masks real test failures). If you need logs while debugging, invoke
  `tracing_subscriber::fmt().try_init()` in the specific test — never
  from a shared helper that other tests share.
- Do NOT assert on log output. Assert on the observable state instead
  (return value, HTTP body, DB row, cookie, `Result` variant).
