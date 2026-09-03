# Agent task comparison v1

Status: reproducible framework and three-task corpus authored, unrun. There are no
comparative observations or productivity results. The Zero lane is an external,
unrun reservation rather than an implementation or parity claim.

Audience: benchmark operators, agent integrators, and reviewers of comparative
evidence.

This contract closes the measurement-design gap identified by
[Agent Task Economics v1](AGENT-TASK-ECONOMICS-V1.md). That earlier report
records exact compiler-protocol traffic for one scripted graph workflow. It
does not compare agents. This framework defines paired, externally observed
agent trials over identical task and fixture bytes:

- `semaprax-graph-operational`, using a verified image and typed intentions;
- `semaprax-source-first`, using canonical source and conventional compiler or
  language-server feedback; and
- `zero-graph-native`, reserved at exact external subject
  `vercel-labs/zerolang@eb2ed6c22fe3f6e3152efa0c0d05ffcf1ff4a2c7`.

Only the two Semaprax lanes are available in v1. The manifest marks Zero
`external_unrun`; the validator rejects observations for it. A later Zero
comparison requires a separately reviewed semantic port, provisioned official
toolchain and adapter, and exact observations. Semaprax results cannot be
silently reused, translated, or estimated for that lane.

## Corpus and pairing

[`manifest.json`](../benchmarks/agent-task-comparison-v1/manifest.json) binds
three cold-state repetitions of three tasks. Each paired trial starts from the
same task-specific checked-in fixture bytes and uses the same user prompt.
Lane instructions restrict the available work surface without changing the
requested outcome.

`signature-migration-v1` requests the bounded scalar signature migration used
by the product workflow, caller migration, identity preservation, validation,
review material, explicit analysis blind spots and no publication.
`stale-signature-recovery-v1` adds one exact checked-in sibling-body patch after
the first identifying inspection. Both lanes must detect or encounter the
drift, retain the unrelated edit and recover without overwriting source.
`owned-signature-migration-v1` reorders two `Bytes` owners and one borrowed
slice view. It requires original left-to-right call evaluation, exact-once owner
and view retention, rebuilt loan/cleanup admission, ownership-aware review and
explicit runtime/deployment/generated/API/consumer blind spots. Its separate
owned-data fixture prevents the scalar result from being reused as ownership
evidence.

The task JSON freezes the prompt, setup, drift point, ordered acceptance rubric
and blinded review protocol. A generated plan additionally authenticates every
task, fixture and drift-patch byte and binds the exact Semaprax repository HEAD.
It deliberately does not infer a task result from existing scripted workflow
tests.

Generate the canonical plan with:

```sh
python3 scripts/agent-task-comparison.py plan \
  --manifest benchmarks/agent-task-comparison-v1/manifest.json \
  --output /absolute/path/to/plan.json
```

The plan is a protocol artifact, not execution evidence. Its claims remain
`not_observed` and `not_claimed`.

Generate the canonical complete available execution matrix with:

```sh
python3 scripts/agent-task-comparison.py matrix \
  --manifest benchmarks/agent-task-comparison-v1/manifest.json \
  --output /absolute/path/to/matrix.json
```

`semaprax.agent-task-comparison-matrix.v1` binds one plan/head and enumerates
every available task/lane/trial tuple in deterministic order. Each row carries
the SHA-256 digest of the exact trial contract produced by the command below,
so an external dispatcher can verify what it ran without embedding repeated
prompts and fixtures in the matrix. External lanes remain separately listed as
unrun. Matrix generation invokes no agent, validator or publication host and
contains no observations or comparative result.

Generate one canonical external-harness trial contract with:

```sh
python3 scripts/agent-task-comparison.py trial \
  --manifest benchmarks/agent-task-comparison-v1/manifest.json \
  --task owned-signature-migration-v1 \
  --lane semaprax-graph-operational \
  --trial 1 \
  --output /absolute/path/to/trial.json
```

`semaprax.agent-task-comparison-trial.v1` binds the plan, exact repository
subject, task/prompt/fixture/drift bytes, lane instructions, state, acceptance
rubric, review protocol and required metric inventory for one available tuple.
It resolves the Semaprax subject to the plan's exact Git head and rejects the
external-unrun Zero lane. It invokes no model, tool, compiler, validator,
reviewer or publication host; all outcome and comparison claims remain
unobserved. The external harness must still archive actual evidence and produce
the separate observation object accepted by the report command.

## Typed event-ledger derivation

An external harness can derive the observation metrics from a canonical typed
event ledger instead of hand-entering aggregate totals:

```sh
python3 scripts/agent-task-comparison.py observation \
  --manifest benchmarks/agent-task-comparison-v1/manifest.json \
  --ledger evidence/task-lane-trial/ledger.json \
  --output /absolute/repository/path/evidence/task-lane-trial/observation.json
```

The output must be a distinct file beside the repository-relative ledger so
all artifact paths keep the same meaning. The bounded
`semaprax.agent-task-comparison-ledger.v1` object carries the same exact
plan/task/lane/trial/model/harness bindings as an observation, 1 through 63
authenticated external artifacts, complete stream-evidence references, typed
events, acceptance rows and outcome. It uses canonical compact JSON with one
terminal LF and is capped at 8 MiB and 65,536 events.

Closed events record provider token usage, context presentation, tool calls,
failed attempts, stale failures and recovery actions, validation/review
intervals and human interventions. The derivation sums byte/token/time values
and counts action events. Complete stream evidence is mandatory even when a
stream has no events, so a zero still has an auditable basis. The resulting
observation automatically authenticates the complete ledger itself as
`typed-event-ledger` in addition to every referenced provider, tool,
validation, drift, review or intervention artifact.

Derivation does not make the external recorder trustworthy, infer tokens from
bytes, time work internally, invoke an agent, or validate task correctness. It
turns event-level assertions into reproducible totals and lets the existing
observation/report validator re-hash the ledger and all supporting evidence.

## Observation contract

An external harness owns model invocation, isolation, deterministic drift
injection, validation timing and reviewer timing. One compact JSON observation
is required for every `(task, available lane, trial)` tuple. Each uses schema
`semaprax.agent-task-comparison-observation.v1` and contains exactly:

- the canonical plan SHA-256, task, lane, one-based trial and cold/warm state;
- exact model, tokenizer, model configuration, harness, host and toolchain
  revisions plus the task prompt SHA-256;
- an ordered inventory of single-link regular evidence artifacts with relative
  paths, exact bytes, SHA-256 and kind;
- all required metrics as an observed nonnegative value, measurement method and
  one or more authenticated artifact references;
- every acceptance criterion in task order with `passed` or `failed` and
  authenticated evidence references; and
- an overall `completed`, `failed` or `aborted` outcome.

The required metric inventory is:

| Metric | Required observation |
| --- | --- |
| `model_input_tokens`, `model_output_tokens` | Provider/tokenizer usage counters; never inferred from UTF-8 bytes or lexical units |
| `presented_context_bytes` | Exact bytes made visible to the model, including repeated presentation |
| `tool_calls` | External agent-tool invocations, not internal compiler protocol calls |
| `tool_request_bytes`, `tool_response_bytes` | Exact serialized external tool traffic |
| `failed_attempts` | Agent-proposed actions rejected by the harness or acceptance validator |
| `stale_failures`, `stale_recovery_actions` | Explicit stale detection and subsequent recovery operations |
| `validation_wall_ms` | Monotonic elapsed time for the fixed validation protocol |
| `review_wall_ms` | Blinded reviewer's active time under the task protocol |
| `human_interventions` | Harness-recorded interventions beyond the frozen prompt and drift injection |

Zero values still require evidence. Missing token accounting, timing, tool
traffic, acceptance rows or artifacts makes an observation ineligible instead
of turning the field into zero. Evidence-file authentication establishes which
bytes were supplied; it does not independently prove that a provider counter,
timer or reviewer was honest. The benchmark operator must archive the provider
usage response, ordered tool transcript, validation transcript, drift record,
review record and rubric decisions needed to audit each metric.

Paths are repository-relative and may not cross a symlink at any component.
Observations are capped at 1 MiB, each evidence artifact at 32 MiB, and one
observation at 64 artifacts and 64 MiB total authenticated evidence. This
keeps the checked summarizer read-only and bounded; source, candidate, cache,
publication and network authority remain outside it.

## Incremental observation audit

One available task/lane/trial can be checked before the complete paired matrix
exists:

```sh
python3 scripts/agent-task-comparison.py audit \
  --manifest benchmarks/agent-task-comparison-v1/manifest.json \
  --observation evidence/task-lane-trial/observation.json \
  --task owned-signature-migration-v1 \
  --lane semaprax-graph-operational \
  --trial 1 \
  --output /absolute/path/to/audit.json
```

The command invokes the same complete observation and evidence-artifact
validator as `report`, including plan, prompt, available-lane, metric,
acceptance, outcome, path, byte and digest checks. It then requires the
validated tuple to equal the explicit selectors. The bounded 512 KiB
`semaprax.agent-task-comparison-audit.v1` result binds the exact plan and
repository head, exact observation-file digest and canonicalized observation
digest, eligibility basis, outcome, complete metric objects and ordered
acceptance rows.

An audit is one incremental eligibility record. It does not require or imply a
complete matrix, invoke an agent, compare lanes, aggregate productivity, rank a
result, infer statistical significance, or observe/estimate the external Zero
lane. Only `report` can describe the complete available-lane matrix, and its
existing limitations remain unchanged.

## Descriptive report

After collecting the complete two-lane matrix, produce a canonical report:

```sh
python3 scripts/agent-task-comparison.py report \
  --manifest benchmarks/agent-task-comparison-v1/manifest.json \
  --observation path/to/graph-task-1-trial-1.json \
  --observation path/to/source-task-1-trial-1.json \
  ... \
  --output /absolute/path/to/report.json
```

The command re-hashes every referenced artifact, rejects missing or duplicate
tuples, enforces exact task-prompt and plan bindings, and requires the same
model, tokenizer and state within each pair. It emits exact per-lane totals and
signed `left_minus_right` differences for each pair. It does not convert bytes
to tokens, compute a success rate over omitted trials, impute missing values,
rank lanes, claim causality, calculate statistical significance, or mention an
unobserved Zero result.

Three repetitions over three small tasks can support only a descriptive bounded
result. Representative repositories, warm-state trials, ownership-sensitive
changes, parallel agents, independent reviewers, cross-platform validation and
an executed Zero port remain separate requirements before a strong comparative
position is supportable. The graph-operational programme therefore remains
Partial.

## Authority-free normalized observation report

`agent_economics::normalize_task_comparison_observations` accepts one exact
canonical `semaprax.agent-task-comparison-observation-set.v1` byte string and
its SHA-256. The set binds the plan digest, repository head, task, corpus and
model. It requires exactly one `semaprax-graph-operational` and one
`semaprax-source-first` lane. The checked v1 manifest marks
`zero-graph-native` `external_unrun`, and this aggregate does not replay a plan
or availability document, so it rejects a caller-supplied Zero observation and
always emits the Zero comparison as `not_assessed_missing_observation`.
Duplicate, missing, unavailable or unknown lanes fail.

Each lane uses the distinct closed wrapper
`semaprax.agent-task-comparison-embedded-observation.v1`. The paired observations
must retain the existing contract's same harness, host, tokenizer, model
configuration and toolchain binding. Lane-specific compiler or tool identities
belong in authenticated evidence artifacts, not the paired `toolchain` field.
The wrapper also binds its source revision and, for the semantic lane, image and candidate revisions. It
also supplies wall time, protocol bytes and source bytes. Its `observation`
field contains the complete canonical bytes of the existing
`semaprax.agent-task-comparison-observation.v1` contract and
`observation_sha256` authenticates those exact bytes. The library checks that
document's closed keys, plan/task/lane/model/toolchain bindings, one-based
trial, state, outcome, complete twelve-metric inventory, artifact metadata,
evidence-reference closure and acceptance/outcome consistency. It does not read
or re-hash referenced artifact files; the external Python validator remains the
evidence-file authentication owner.

The Rust normalizer is a versioned bounded subset of the Python v1 observation
validator. It admits UTF-8 identifiers and other checked text through the
Python contract's 65,536-byte ceiling, but represents natural-number fields as
unsigned 64-bit integers rather than Python's arbitrary-precision integers.
It also applies the aggregate and report bounds below. Boundary regressions pin
the 65,536-byte text ceiling, reject the next byte, admit `u64::MAX`, and reject
the next natural number. The Rust route does not broaden or replace the Python
validator's plan, manifest, task, availability, or artifact-file replay.

The library input bound is 7,340,032 bytes. It leaves worst-case JSON string
escaping room for the two required existing 1 MiB documents, their wrappers,
and bounded future envelope headroom; that headroom does not admit a third v1
lane. Each embedded document retains the 1 MiB bound. The normalized report retains each embedded
document's digest and parsed metric/acceptance facts, not a second copy of its
raw string. It is bounded to 8 MiB and has a domain-separated revision.
It emits signed left-minus-right differences only when the derived existing
observation outcomes match. Different outcomes and absent Zero produce
`not_assessed` with no deltas. `superiority` is always `not_assessed`; values are
descriptive caller assertions, not productivity, causality, significance or
independently verified correctness.

Project Agent Transport v5 exposes `agent/task-comparison` as a semantic-read
query, inherited by generated TypeScript, Python and Rust clients and MCP. The
ordinary request limit gives it a narrower 28 KiB input bound, leaving room for
worst-case JSON string escaping and the request envelope; the library API
handles larger valid sets. Its closed envelope returns the complete report up
to 384 KiB, leaving the same response-envelope margin, with exact input and
report SHA-256 values. It is outside the
parallel-read subset. It executes no model, tool, validator, reviewer,
filesystem, network or runtime operation and grants no source or publication
authority.

Hostile library regressions are authored but unrun. This tranche records no
comparative observation, generated-client execution, MCP invocation, test
result or superiority claim; the framework remains external and unrun.
