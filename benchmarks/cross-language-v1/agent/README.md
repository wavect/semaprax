# Agent-driver seam (v1)

`benchmarks/cross-language-v1/run.py` scores a fixed, human-written source
tree against each language's official toolchain. It has no model, provider,
sampling, or budget concept anywhere in it, and no code path invokes a
model — it is a cross-language toolchain-conformance harness, not an Agent
harness. This directory is the missing seam: it lets a *solver* (a real
model, or — the only thing actually exercised here — a deterministic
recorded script standing in for one) produce the candidate file(s) for a
(task, language) pair, and scores what it produces through `run.py`'s own
build/test/leak-check/provenance machinery, unmodified.

**Nothing here has ever called a real model.** See [Non-claims](#non-claims).

## Layout

| Path | Role |
| --- | --- |
| `contracts.py` | Explicit, recorded data shapes: `ModelIdentity`, `SamplingParams`, `Budget`, `PricingRates`, `SolverRequest`/`SolverResponse`, transcript entries, and the versioned literal-redaction policy. No field has an ambient/environment fallback. |
| `budget.py` | `BudgetLedger`: charges token/retry/cost usage against a `Budget` and raises the instant a ceiling would be crossed, before the attempt is applied. |
| `transport.py` | The abstract `SolverTransport` seam (`complete(request) -> response`) plus its exceptions. |
| `replay_transport.py` | Deterministic, offline transport. Reads a committed JSON fixture (`benchmark.cross_language.agent.replay_fixture.v1`) and replays it verbatim — no network, no credentials, no clock, no RNG. |
| `live_transport.py` | A real-provider transport. Declared, never exercised: refuses to construct without an explicit `api_key` (never `os.environ`), and refuses to `complete()` even when one is supplied. See its module docstring. |
| `prompts.py` | Deterministic prompt construction from a task's `EQUIVALENCE.md` and public scaffold files only — never from `hidden/`. |
| `orchestrator.py` | `evaluate_agent_pair`: calls a transport, enforces its budget, and (only on a produced candidate) writes it into the same two-phase scratch-tree scoring `run.py::evaluate_pair` already performs, by importing `run.py`'s own functions rather than reimplementing them. |
| `run_agent.py` | CLI entry point. Every model/sampling/budget/transport parameter is a flag; none has an environment-variable fallback. |
| `fixtures/` | Committed replay fixtures. Each one is a hand-authored script, not a recorded model transcript (see its own `_non_claim` field). |
| `tests/test_agent_driver.py` | Offline, credential-free self-tests (`python3 -m unittest discover -s benchmarks/cross-language-v1/agent/tests`). |
| `specialization_protocol.py` | Input-only protocol validator and held-out schedule builder for #147. It never creates a transport or an outcome: all emitted rows remain `not_authorized` / `not_attempted`. |
| `specialization_accounting.py` | Read-only #326 accounting of the original 81 planned cells and nine later additions. Emits no observations or approvals; exits 3 while blocked. |
| `provenance/specialization-held-out-v1.{protocol,tasks}.json` | Exact original public JSON input bytes from `ebe4235e`, not a new protocol, adapted model, frozen executable corpus or independent review. |

## External baseline admission is not execution

`baseline_admission.py` is an offline, pure validator for a future Zero or
mainstream coding-agent baseline's *input provenance*. Its caller supplies the
canonical owner `tasks.json` bytes and a descriptor binding the baseline's
official source, immutable revision and artifact digest, license, exact agent
guidance/invocation-contract digests, installation receipt digest, explicit model identity, and each
independently reviewed port tree, oracle, equivalence review, and candidate
path. The gate recomputes the owner inventory SHA-256, requires its reviewed
pin, and derives the ordered closed task set itself; it accepts no detached
task-ID/digest pair. It rejects missing, extra, reordered, mutable, malformed,
or undeclared inputs deterministically.

The only currently owner-pinned external identity is Zero:
`https://github.com/vercel-labs/zerolang` at
`eb2ed6c22fe3f6e3152efa0c0d05ffcf1ff4a2c7`, the exact subject reserved by
`agent-task-comparison-v1`. A descriptor for a different source, a near-miss
or moving revision, or any mainstream baseline is unavailable until its owner
records an equivalent immutable pin; this gate does not invent one.

Even a descriptor that passes every check returns `status: "unavailable"`.
The gate neither reads those inputs from disk nor provisions or invokes a
toolchain, agent, model, or oracle; it intentionally rejects an `executed`
claim.  It therefore does not enable the reserved `zero-graph-native` v1 lane,
does not change any `adapters.json` availability, and is not evidence that a
Zero or mainstream baseline ran.  An independently reviewed port and official
toolchain execution record remain required before any comparison outcome can
be admitted.

## Why model identity and budgets are explicit constructor/CLI arguments

`AGENTS.md`'s non-negotiable invariants state: "Capabilities are explicit.
Compiler and generated code gain no ambient filesystem, process, network,
home, secret, key, wallet, or signing authority." A benchmark harness that
read an API key, a model name, or a token budget from an environment
variable deep inside its own call stack would be exactly that pattern,
applied to model authority instead of filesystem/network authority: a
config value with real consequences (cost, what gets sent where) determined
by whatever happened to be set in the calling shell, not by anything visible
in the command that ran. Every one of `contracts.py`'s dataclasses is
therefore a plain constructor call with required fields, `run_agent.py`'s
CLI has one flag per field with no default that reaches for `os.environ`,
and `live_transport.LiveTransport` refuses to even construct without an
explicit `api_key` argument. `tests/test_agent_driver.py`'s
`test_no_environment_variable_can_satisfy_construction` pins this: setting
`ANTHROPIC_API_KEY` in the process environment and passing an empty
`api_key` argument still raises.

## Quick start (deterministic replay — no network, no credentials)

```sh
python3 benchmarks/cross-language-v1/agent/run_agent.py \
  --task structured-input-error-handling-v1 --language rust \
  --candidate-path candidate.rs \
  --transport replay \
  --fixture benchmarks/cross-language-v1/agent/fixtures/structured-input-error-handling-v1-rust-ok.json \
  --model-provider anthropic --model-name claude-mock --model-revision fixture-2026-09-18 \
  --temperature 0.0 --top-p 1.0 --seed 1 --max-output-tokens 2000 \
  --max-prompt-tokens 100000 --max-completion-tokens 100000 --max-total-tokens 100000 \
  --max-retries 3 --max-cost-usd 1.0 \
  --output /tmp/agent-result.json
```

This replays a hand-authored script (one transient-error attempt, then a
final attempt) against `structured-input-error-handling-v1`'s real,
committed, unmodified `hidden/rust/` overlay, using the real `rustc`
toolchain `adapters.json` already declares — exactly the same build/test/
leak-check path `run.py` uses for a human-written candidate. It writes a
`benchmark.cross_language.agent.v1` document with `status: "ok"`,
`leak_check: "ok"`, both build/run phases `passed: true`, and a `provenance`
block binding the task's digest, the adapter's observed version, the
model identity, the sampling seed, the prompt digest, and the transcript
digest together.

The result retains every transcript entry and every candidate artifact in
deterministic order. Each retains its original content digest. An authorized
operator preparing a result for publication can pass an explicit JSON policy
with exact keys `{"schema":"benchmark.cross_language.agent.redactions.v1",
"literals":[...]}` through `--redaction-file`; matching literals are replaced
by digest-tagged markers while the policy's literal values themselves are
never copied into the result. This is a projection, not a heuristic secret
scanner or ambient credential lookup. With no policy, the locally written
evidence is complete and unredacted.

The transport may write **only** the exact `--candidate-path` set. Traversal,
platform-root paths, non-text artifacts, duplicate declarations, and any
extra path (including a public test or scaffold) are terminal failures before
the scorer creates a scratch write. This keeps a candidate from making its
own oracle pass by rewriting it.

## Budget enforcement fails closed

```sh
# Same command as above, but --max-prompt-tokens 10 instead of 100000.
python3 benchmarks/cross-language-v1/agent/run_agent.py ... --max-prompt-tokens 10 --output /tmp/agent-budget.json
```

exits `1` and writes a document with `"status": "budget_exceeded"`,
`"reason": "prompt_tokens 410 exceeds max_prompt_tokens=10"`, zero committed
usage, the rejected attempt still present in the transcript, and **no**
`public`/`hidden` key — the pair never reaches a build or test step. See
`tests/test_agent_driver.py`'s `OrchestratorBudgetFailureTests` and
`RealToolchainEndToEndTests.test_cli_budget_exceeded_exits_nonzero_and_still_records_an_outcome`.

## Non-claims

- **No real model has ever been called from this repository, by this seam
  or any other.** Every fixture under `fixtures/` is a hand-authored replay
  script (see each fixture's own `_non_claim` field), the offline analogue
  of `tests/documentation/cross_language_benchmark_suite.rs`'s
  `MockLanguage` mock adapters — built to exercise the harness's own logic,
  not to report a model's behavior.
- **`LiveTransport` is declared and inert.** It refuses to construct without
  an explicit, non-placeholder-checked `api_key` (there is no environment
  fallback to fall back to), and its `complete()` refuses to run even when a
  key is supplied, because no HTTP request/response mapping in it has ever
  been verified against a real provider response from this environment (no
  network access, no credentials). Implementing and verifying that mapping
  is `HUMAN_BLOCKED: model budget and credentials`, same as issue #211's
  live two-model pilot requirement already was before this change.
- **This does not widen what any of the nine languages' adapters claim.**
  The six `"implemented": false` rows in `adapters.json` (Zero, NTNT, Aver,
  Vera, Hale, MoonBit) are unchanged; this seam calls the same `adapters.json`
  and the same `run.py::stage` a human-driven run would, so a
  candidate for one of those languages is still `blocked`, never scored as a
  pass or a fail.
- **No result produced through this seam is committed as a benchmark
  result.** `results/README.md`'s reasoning (this host measures contention,
  not the subject) applies here unchanged, and doubly so: nothing here has
  even measured a real model's *output quality*, only the harness's own
  plumbing against a synthetic script.

## Frozen specialization protocol (no spend)

Issue #147 needs a controlled comparison of the same exact base model with
no semantic guidance, versioned guidance, and versioned guidance plus a
schema-constrained action surface. `specialization_protocol.py` freezes that
comparison before it can run: its public task-inventory digest, model identity,
resource ceiling, independent oracle digest, complete metric inventory, and
every held-out task row are all digest-pinned in one canonical plan. A proposed
adapter is optional, but may name only owner-declared `development` tasks and
remains `proposed`; it cannot include a held-out task or claim to have trained
anything.

```sh
python3 benchmarks/cross-language-v1/agent/specialization_protocol.py \
  --protocol benchmarks/cross-language-v1/agent/specialization-protocol.example.json \
  --tasks benchmarks/cross-language-v1/tasks.json \
  --output /tmp/specialization-plan.json
```

This only validates supplied JSON and writes a plan. It does not load
`LiveTransport`, a provider SDK, credentials, a candidate, hidden task bytes,
or a toolchain. The output deliberately says `status: not_authorized` and
`execution: not_attempted`; it is neither a training artifact nor a model
evaluation. An actual run remains blocked on the explicit authorization list,
and any outcome must still be scored by the same independent hidden-oracle
path used by `run.py`.


## Issue #326: frozen-cell accounting (blocked)

Status: authorization, real provider execution and independent reviews remain
outstanding. This accounting is not a completed model experiment and does not
close [#326](https://github.com/wavect/semaprax/issues/326).

The original [#147 protocol commit](https://github.com/wavect/semaprax/commit/ebe4235ebc2b9621f7fd174f8bf7647744dc88c5)
contains three controls (`base`, `guided`, `constrained`), nine held-out
`semaprax-project` tasks and three repeats: **81 cells**. The exact original
protocol and public task inventory are retained in `agent/provenance/`:

| Original file | Original Git blob | SHA-256 of retained bytes |
| --- | --- | --- |
| `agent/specialization-protocol.example.json` | `457e56e77d68155c562068ede4e1f9c5fecd6a6f` | `3165375ffa9ce3b20e650be8dd261263d6d59bcb105868364580cf9b1c35c62a` |
| `tasks.json` | `ae30aba4943a2e52bf056edcd7bb5f7d680114a8` | `f5cd390280bbdd82533fa6953d114ecf66a67d65c66d6571ccc6acb05a1113f1` |

These bytes reproduce the original schedule through the **unchanged**
`specialization_protocol.build_plan`. They are historical fixture inputs,
not a newly approved model, guidance set or adaptation method. Acquisition from
GitHub and hash checks are source provenance, not independent experiment review.
The original complete task metadata has 12 tasks: the development and validation
tasks are not evaluated, and `bounded-counter-repair-v1` has no declared
`semaprax-project` port. Each omission has an explicit selection reason.

The current example at `6a7a341273c30b81cf53f3fa9ef94dc7930b4c72` instead
selects ten held-out tasks and **90 cells**, because
`iterative-repair-workflow-v1` was added after the original freeze. The accounting
retains all original 81 rows and separately lists its nine extra cells as
`outside_original_frozen_matrix`. It does not delete any current task, alter the
13-task/182-slot comparison corpus, or authorize a 90-cell replacement study.
No `adapted` arm is added to the three-control matrix.

The frozen proposal's oracle SHA-256 starts `4fda2976`; current `run.py` starts
`29b99551`. This byte mismatch is reported, not silently repaired. Retaining two
metadata files is **not** reconstruction or approval of the original executable
source/oracle. Actual execution requires separately reviewed corpus, guidance,
action-schema and oracle artifacts, including any known semantic corrections;
new bytes need explicit versioned review rather than inherited approval.

### Emit the complete blocked record

```sh
python3 benchmarks/cross-language-v1/agent/specialization_accounting.py \
  > /tmp/specialization-accounting.json
status=$?
test "$status" -eq 3
```

Exit **3** means the complete report was emitted and the experiment remains
blocked. Exit **2** means source/capability admission failed; stdout is empty
and stderr contains the refusal. There is no execute, approval, credentials,
source-root, task-filter or result-import flag. The CLI uses the existing
bounded no-follow acquisition primitives and fails closed where unavailable.
Its selected source pins describe the checked branch revision, not a promise
that later changes can reuse this report without review. Standard repository
Python modules remain trusted code; this utility is not a new execution sandbox.

The report retains all required metrics for each row as **null**, not false,
zero tokens, zero cost or zero latency. Every row is `not_attempted` with explicit
blocker IDs. The three pairwise comparisons are `not_estimable`: no effect,
confidence interval, winner, equivalence or performance claim can be inferred
from zero real observations. The original preview budget is not spend authority.
The report reads public metadata and selected source files, never public/hidden
task trees; this is not evidence that an eventual model experiment is leak-free.

### Outstanding authorization and independent records

Before a real run, retain an exact plan/source digest and the actual approving
person's decision for the real base model/revision, the three control artifacts,
shared sampling/tool/token/retry/latency/cost limits, and any development-only
adaptation method and dataset. Keep all held-out material outside adaptation and
tuning; an adapted arm would need its own explicitly approved study amendment.
Record the frozen split/oracle, allowed provider endpoint and egress content,
explicit credential-use authority, separate training/inference spend caps and
cancellation conditions. Do not place secrets in protocol or evidence files.

An independently assigned reviewer/data custodian must retain a dated leakage
review and a separate control-comparison review, bound to the plan, data and
outcome artifacts. Missing reviewer identities and record digests remain null.
A checklist, source-code audit, replay fixture or this report is not an
independent review or an attestation of the author's own experiment.

Finally, the existing `LiveTransport.complete()` is an inert refusal seam even
when credentials are supplied. After authority is granted, a real provider
integration still needs implementation and actual wire/budget/egress verification
through the existing orchestration and scorer boundaries. No caller may use the
predecessor `not_authorized` protocol for live training or inference. Local
provider execution can qualify; a hosted CI receipt is not required by itself.
