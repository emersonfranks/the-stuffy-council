---
applyTo: "**"
description: "Always-on authoring policy for the-stuffy-council. Sets the audience for all repo artifacts (agents only), the acceptable-comment rule, modularity expectations, and the mandatory rotating subagent-review protocol that gates every change."
---

# Agent Authoring Policy (Always-On)

Applies to every artifact in this repo: Rust source, HTML templates, SQL,
TOML, Dockerfile, markdown docs, `.env.example`, CI config — everything.

Repo baseline rules (CSRF, SQL binding, auto-escape, secrets handling,
stuffy-id stability, etc.) live in [../../AGENTS.md](../../AGENTS.md).
This file layers *authoring* rules on top of those *behavioral* rules.

## Audience

Everything in this repo is written for AI coding agents. That is the only
audience that matters.

- No human is expected to read the code, comments, or docs directly. If a
  human needs to understand something, they will ask an agent to explain
  it — the agent is the interpreter.
- Optimize every artifact for:
  1. an agent's ability to understand the current state in one read,
  2. an agent's ability to modify safely without breaking invariants,
  3. an agent's ability to hand off to a different agent mid-task.
- Do NOT optimize for: aesthetic preferences, Clean-Code ideals as ends in
  themselves, human onboarding, tutorial-style narration, or "someday a
  developer will thank us."

Clean-Code / SOLID / DRY / etc. apply only when they measurably serve the
three optimization targets above. If a "best practice" makes the code
harder for an agent to modify — skip it, and note why in a comment if the
skip is non-obvious.

## Comments and docstrings

Delete every comment that does not help a future agent modify the code
without regressing behavior.

**Do NOT write:**
- Narration of what the next line does (`// increments counter`).
- Preamble that restates the function name or signature.
- Doc comments on private items unless they capture a real gotcha.
- Aesthetic dividers, ASCII art, or section headers separating trivially
  distinct blocks.
- Tutorial-tone prose ("Now we…", "First, we'll…").
- Change-log entries inside source (`// added by X on Y`) — that's what
  git is for.

**DO write:**
- Non-obvious invariants and constraints
  (e.g. "callers must hold the pool lock", "runs before session rotation
  or CSRF breaks").
- Cross-file coupling that a text search won't obviously find
  (e.g. "when you rename this column, also update `migrations/0002_*` and
  `story_repo::StoryRow`").
- The *why* of a deviation from the obvious approach, when the deviation
  is deliberate (e.g. "using `query_as` runtime binding instead of the
  `query!` macro so `cargo check` works without a live DB").
- Safety/security assertions that must not be relaxed without careful
  review (e.g. "prompt input comes only from our own TOML; do not
  concatenate request data here").
- `TODO(<condition>)` / `FIXME(<condition>)` where `<condition>` is a
  concrete, checkable trigger — not "someday."

Prefer richer names, narrower types, or a targeted test over a comment.
Only fall back to a comment when it is the cheapest reliable way to keep
a future agent from re-breaking something.

Rustdoc (`///`) counts as a comment for these rules. Use it on public
items only when its presence prevents a real regression risk. Never
duplicate a signature in prose.

## Names

Names carry the load. A type / function / variable name must convey what
it does *and* the scope or boundary it covers. If an agent has to open
the definition to learn the semantics, the name is wrong — rename it,
do not annotate it.

- Rename when scope changes. Renames are mandatory, not optional.
- Do not use a comment or doc-string to make an imprecise name accurate.
- Test names must state the surface under test and the scenario. Words
  like `smoke`, `basic`, `misc` are only acceptable when the rest of the
  name pins the scope.

## Modularity

Code must remain easy to modify. Agents decide file/module boundaries
using this heuristic:

- Split a file when it exceeds what a single agent turn can read and
  reason about without losing precision. There is no fixed line count;
  the trigger is "I had to re-read the same section twice to answer a
  question about it."
- Put behavior behind a trait as soon as a second implementation, mock,
  or swap becomes plausible — not before. First use: concrete. Second
  use anywhere on the horizon: extract.
- Prefer many small `mod` files inside a folder over one large `mod.rs`.
- Avoid speculative extension points, generics-for-one-caller, and
  configurability without a concrete second caller. YAGNI is a hard
  default.

State ownership rules (SRP-like) still apply because they make code
easier to modify, not because a textbook says so.

## Tests

Tests are agent infrastructure — they let future agents refactor without
regressing invariants the previous agent knew about. Rules for what tests
must cover and how they are written live in the pair of always-on files:

- [test-quality.instructions.md](test-quality.instructions.md)
- [test-style.instructions.md](test-style.instructions.md)

For the review protocol below, that means:

- **New production logic** must land with tests along the applicable
  coverage dimensions (functional / edge / negative / error /
  state-transition). Absence is a MINOR finding by default.
- **Bug fixes** must land with a regression test that reproduces the
  original failure path via the tier that would have caught the bug in
  the first place (tier-2 for wire-up bugs, tier-1 for pure-logic bugs).
  Absence is a BLOCK finding — do not merge a fix without the guard.
- **Changes to `src/auth.rs`, `src/access.rs`, `src/web/csrf.rs`,
  `src/web/security.rs`, or `src/error.rs`** carry an elevated bar: any
  missing test coverage becomes a MAJOR finding, not MINOR.

## Documentation

`AGENTS.md`, `README.md`, `.github/instructions/**`, `cast/README.md`,
and any other markdown in this repo are all agent-facing docs and follow
the same rules as source.

- Run **Markdownlint: Lint Workspace** in VS Code after changing Markdown.
  The installed extension and its workspace command both consume
  [../../.markdownlint-cli2.jsonc](../../.markdownlint-cli2.jsonc).
- `.github/agent-review-log.md` is the only lint exclusion. Its line-1 reviewer
  marker and repeated headings are protocol data; do not rewrite historical
  entries or broaden the exclusion.

- Terse, factual, scannable. Tables and bullet lists over paragraphs.
- No marketing copy, no rationale for its own sake, no "welcome!" tone.
- Every rule must be enforceable — by a test, a lint, a review checklist
  item, or a specific tool invocation. Rules without enforcement paths
  get deleted.
- Cross-link with workspace-relative paths (not URLs) so agents can
  follow them.

The `.env.example`, `Dockerfile`, and CI configuration follow the same
policy: comments are for the *next agent to modify this file*, not for a
theoretical operator.

## Rotating subagent review

Every change — code, docs, config, this file — gets ONE subagent review
before it's declared done. Follow-up review is required only if that
review returns `BLOCK` findings.

### Steps

1. Finish drafting the change (all files written, `cargo check` green
   for Rust changes).
2. Look up the previous reviewer at the top of
   [../agent-review-log.md](../agent-review-log.md) (`Last reviewer:`
   line).
3. Pick a reviewer from the pool below, excluding:
   - the model that authored this change (may be you or a delegated
     subagent — see "Delegated authoring" below), and
   - the previous reviewer.
4. Invoke `runSubagent` with `agentName: "Explore"` and the chosen
   `model`. Prompt MUST include: what changed and why, the file list
   with absolute paths, a link to this file and to `AGENTS.md`, and the
   instruction to return findings in the format below (or `NO FINDINGS`).
5. Disposition every returned finding — plus any not-yet-resolved
   finding from earlier log entries the change touches — as one of:
   - **Fixed** — apply the fix in this same change.
   - **Deferred (<condition>; owner: <owner>)** — `<condition>` is a
     concrete checkable trigger (e.g. "next change touching
     src/routes/**"), not "someday." `<owner>` is a named user or
     "next agent to modify <path>".
   - **Rejected (<specific reason>)** — why the finding doesn't apply.
6. Append one entry to the review log using the template that lives in
   that file. Update the `Last reviewer:` line at the top.
7. **Follow-up review is required only if step 4 returned any `BLOCK`
   finding.** In that case, after fixing, restart from step 3 with a
   reviewer different from both the author and the reviewer just used.
   For `MAJOR` / `MINOR` / `NIT` findings, disposition them and the
   change is done — no chained re-reviews.

### Reviewer pool

- `GPT-5.5 (copilot)`
- `GPT-5.6 Sol (copilot)`
- `Claude Opus 4.8 (copilot)`
- `Claude Opus 4.7 (copilot)`
- `Gemini 3.6 Flash (copilot)`

Never invoke the same model in two consecutive reviews. If the chosen
model is unavailable, pick another that still satisfies the "different
from author, different from previous reviewer" constraint.

### Findings format

```
SEVERITY | AREA | FILE:LINE | one-line summary
  what:   <what is wrong>
  why:    <cite a specific rule from this file or AGENTS.md>
  fix:    <smallest change that resolves it>
```

- `SEVERITY` ∈ `{BLOCK, MAJOR, MINOR, NIT}`. Use `BLOCK` only for
  things that MUST be fixed before the change ships (security holes,
  broken invariants, AGENTS.md ground-rule violations). Everything
  else is safe to disposition and move on.
- `AREA` ∈ `{security, correctness, tests, agent-authoring, modularity,
  naming, docs, perf, other}`.

A review with no findings returns the literal line `NO FINDINGS`.

## Delegated authoring (optional, dev-lead pattern)

The orchestrator (the agent driving the conversation) MAY delegate the
actual file writing to a subagent using a different model, then act as
coordinator instead of author. This mixes model perspectives into the
codebase.

To delegate:

1. Pick an authoring model from the reviewer pool.
2. Invoke `runSubagent` with `agentName: "Explore"`, that `model`, and a
   prompt that includes the task, the current contents of files that
   will change, and this contract: "return the full final contents of
   each file to be created or modified, one fenced block per file
   labeled with the absolute path — not a diff, not partial contents."
3. Apply the returned contents mechanically. If output is incomplete or
   invalid, re-invoke with the deficiency spelled out. If delegation
   can't complete after a couple of tries (response too big, malformed
   output), self-author and note it in the log entry.
4. Proceed with the review rule above. The `Author model` in the log
   is the delegated subagent's model; the reviewer must differ from it
   and from the previous reviewer.

Aim for a mix over time — sometimes yourself, sometimes someone else.
No arithmetic gate. If the reviewer notices the log shows a long run of
same-authored iterations, they should raise it as a soft finding.

## When rules conflict

If an authoring rule here conflicts with a behavioral rule in AGENTS.md,
AGENTS.md wins. Raise the conflict as a finding in the next review.

