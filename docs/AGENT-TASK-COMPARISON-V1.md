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
