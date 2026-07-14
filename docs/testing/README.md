# Testing — orientation

This repo mirrors the testing methodology from `rad-service`
(`.github/instructions/test-{quality,style}.instructions.md` +
`docs/testing/test-assessment-framework.md`), adapted to Rust idioms.

Testing is agent infrastructure: it lets any future agent refactor the
code without regressing invariants a human would otherwise have to
remember. It's not human bloat.

## Where the rules live

Enforcement rules are in [.github/instructions/](../../.github/instructions/):

- [test-quality.instructions.md](../../.github/instructions/test-quality.instructions.md)
  — coverage dimensions, mandatory categories, quality criteria,
  failure patterns to avoid. What every test must satisfy.
- [test-style.instructions.md](../../.github/instructions/test-style.instructions.md)
  — layout, naming, integration-test scaffolding pattern. How every test
  is written.

Both are `applyTo: "**/*.rs"` so any agent editing Rust picks them up.

## Test tiers

| Tier | Kind | Where it lives | How it runs |
|------|------|----------------|-------------|
| 1 | Unit | `#[cfg(test)] mod tests { ... }` at the bottom of each `src/*.rs` module | `cargo test` |
| 2 | Integration | `tests/*.rs` at the crate root; shared setup in `tests/common/mod.rs` | `cargo test` |
| 3 | End-to-end | Does not exist yet. Anything requiring real Google JWKS, real Ollama, or a deployed image is deferred. | — |

Tier-1 tests use standard Rust unit tests and small helpers. Tier-2
tests boot the actual app via `stuffy_council::serve` on an ephemeral
`TcpListener` and hit routes with `reqwest`. See
[tests/router_smoke.rs](../../tests/router_smoke.rs) for the canonical
example, including the `ConnectInfo` regression that motivated the
harness.

## When you're modifying an existing module

Before writing tests, list the entry points and the branches they contain.
For each, apply the five coverage dimensions
(functional / edge / negative / error / state-transition) and write down
each specific verifiable claim. That's your test list. Then write one
`#[test]` per item, red-green-refactor.

For a full assessment of an existing component's coverage vs its testable
surface, follow the enumeration procedure in
[.github/instructions/test-quality.instructions.md](../../.github/instructions/test-quality.instructions.md)
under "TDD workflow": list entry points → list journeys per entry point →
derive scenarios across the five dimensions → write one test per scenario.
This maps to rad-service's `test-assess` skill but does not require that
skill to be reachable.

## Provenance

Methodology adapted from `rad-service`
(`.github/instructions/test-{quality,style}.instructions.md`,
`docs/testing/test-assessment-framework.md`, `docs/testing/test-philosophy.md`).
The rules in **this** repo's `.github/instructions/test-*.instructions.md`
are authoritative for this repo — they are self-contained on purpose so
agents don't need to reach across repositories to enforce them. When you
discover a methodology gap here that rad-service handles better, port the
specific rule over rather than linking out.
