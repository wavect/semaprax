# Agent context economics v1

Status: deterministic offline evidence for the current context contract. It is
not a model-token, latency, answer-quality, or repository-scale benchmark.

`semaprax context-benchmark <manifest>` evaluates checked maintenance questions
from a strict tab-separated `semaprax.agent-context-benchmark.v1` manifest.
Sources are canonical regular files beneath the manifest directory; absolute,
parent, current-directory, repeated/trailing separator, backslash,
control-character, case-mismatched, Windows forbidden-character,
trailing-dot/trailing-space/alternate-data-stream/reserved-device, missing, and
symlink aliases fail as `SPX-G005`. Malformed integers, unknown or duplicate
filters/cases/IDs, unavailable Graph v6 facets, and evidence outside the
declared relevant set also fail closed. The command performs no network access
and emits canonical `semaprax.agent-context-economics.v1` JSON.

## Measurements

Each case freezes its question, source, root, context limits, supported filters,
relevant function IDs, and required evidence IDs. Results bind the exact
manifest SHA-256, repeat the exact relevant/evidence ID arrays, bind the source
revision and exact context SHA-256, and record:

- UTF-8 source, question, and context bytes;
- emitted function nodes;
- `semaprax.lexical-token.v1` units, which count maximal ASCII
  alphanumeric/underscore runs and each non-whitespace punctuation or non-ASCII
  scalar as one unit;
- relevance precision as relevant emitted facts divided by all emitted facts;
- evidence recall as emitted required-evidence facts divided by declared
  required evidence.

Ratios are reduced exact fractions, never floating-point estimates. The lexical
unit is repository-defined and explicitly carries `model_tokens: false`; it is
not compatible with any model tokenizer and cannot support a token-cost or
model-context savings claim. Relevant/evidence IDs are reviewed corpus labels,
not automatically proven answer correctness.

The v1 corpus contains four maintenance questions over one checked module. Its
golden aggregate is seven emitted nodes, `1/1` relevance, `2/3` evidence recall
(one intentionally node-truncated case), context/source bytes `1817/616`, and
context/source lexical units `43/11`. The context is larger than repeatedly
reading this small source. That result is retained rather than described as a
saving; larger representative repositories and model-specific evaluation are
future gates.

Two cases also freeze the complete `context` CLI JSON, while the economics
golden freezes every case digest, budget, score, and aggregate. Mutation,
manifest rejection, independent JSON parsing, and deterministic replay are
executable tests.

## Quality routing

`scripts/quality.sh` accepts `quick`, `changed`, or `full` (the no-argument
default remains `full`). `quick` is advisory inner-loop feedback only.
`changed` derives the exact union of a reviewed base-to-`HEAD` committed diff,
staged, unstaged, untracked, deleted, and both rename-side paths directly from
Git. The base is the canonical ancestor commit from `SEMAPRAX_QUALITY_BASE`.
Otherwise, the planner resolves `SEMAPRAX_QUALITY_TARGET_REF` or the symbolic
`refs/remotes/origin/HEAD`, computes `git merge-base --all`, and requires
exactly one base. It never consults the current branch upstream. Non-UTF-8
environment values, absence, abbreviation, non-ancestor, or ambiguity reject
planning. Optional explicit paths must equal that complete set. Traversal,
absolute, backslash, repeated/trailing separator, control-character,
case-mismatched, Windows forbidden-character/trailing-dot/trailing-space/ADS/
reserved-device, symlink, and nonexistent non-deletion aliases are rejected.
Only documentation and the economics files below are narrow; unknown,
workflow, platform, native, router, or core semantic paths escalate to `full`.
`full` is the complete local workspace baseline, not hosted OS, sanitizer,
simulator, emulator, signing, or packaging evidence.

| Path family | Invariant | Focused changed evidence |
| --- | --- | --- |
| `docs/**`, `README.md`, `CHANGELOG.md` | Documentation truth | documentation, examples, Rustdoc |
| `src/agent_economics.rs` | Offline economics contract | agent-context, economics, documentation, examples |
| `benchmarks/agent-context-v1/**` and its snapshots/tests | Corpus, exact goldens, scores | agent-context, economics, documentation, examples |
| `src/graph.rs`, `src/main.rs`, and every other path including router changes | Broad compiler/graph dispatch, unknown, wide, or self-modifying routing impact | Full workspace baseline |

`scripts/quality-route.sh` prints the versioned
`semaprax.quality-route.v2` path and gate plan. The executor validates its
schema, complete envelope, canonical base, and the exact ordered gate sequence
for the effective profile, then dispatches only those declared gates. Missing,
duplicate, reordered, or wrong-profile gates reject the plan. An unmapped path
never means “no tests.”

Graphify remains deferred under [ADR 0001](decisions/0001-graphify.md). No new
pinned temporary Graphify run was available for this tranche, so no adoption or
comparative token claim was made.
