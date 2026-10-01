# Agent task comparison v1

Audience: benchmark operators, agent integrators, and comparison reviewers.

This framework pairs identical coding tasks across two available Semaprax
workflows. It defines how to compare them, but contains no comparative model
observations or productivity result. The external Zero lane is reserved and
unrun; it has no implementation or parity claim.

Status: implemented framework and three-task corpus; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).

It closes the measurement-design gap identified by
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

### OpenCode host availability smoke

`scripts/opencode-provider-smoke.py` is a host-owned availability adapter for
one frozen prompt and one exact OpenCode model identifier. It is deliberately
outside the trial runner: it creates no candidate, ledger, observation or
acceptance result, and does not change the runner's `HUMAN_BLOCKED` live-pilot
gate. The initial non-editing smoke writes an agent-specific
`permission: {"*":"deny"}` policy only into a dedicated **empty** sandbox,
then uses OpenCode's documented `run --pure --agent semaprax-smoke --model
provider/model --format json --dir sandbox` shape. This is a tool policy, not
an operating-system sandbox claim.

The adapter uses OpenCode's host-managed authentication store; it accepts no
credential value, environment-variable name, endpoint or automatic fallback.
It caps the frozen prompt at 65,536 bytes, then captures raw newline-delimited
JSON events under a timeout and output cap. A completion or usage claim requires
the separate raw `opencode export` JSON and the SHA-checked frozen prompt. The
validator admits only the observed v1.18 chronological
`step_start,empty-reasoning,text,step_finish` profile: the empty export-only
reasoning marker has exact identity, shape and time placement while its metadata
remains opaque; every streamed event and corresponding assistant export part
must match exactly by session, message and part IDs; the assistant's parent
must bind one exported user text part whose observed quote-wrapped value equals
the frozen prompt; `providerID/modelID`, stopped completion, integer token counters and
finite nonnegative cost must agree. It records separate hashes for stream and
export. This is OpenCode/provider self-report rather than cryptographic
attestation. A future shape needs a reviewed extension instead of recursive
model-string matching or a permissive partial export match. A matching export
can report provider-supplied token usage; it never estimates billing from prompt
or response bytes. A successful availability smoke is not a pilot observation
or support claim.

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

Before executing the paired pilot, archive its counterbalanced order outside
candidate-visible task bytes:

```sh
python3 scripts/agent-task-comparison-runner.py schedule > schedule.json
```

The private `semaprax.agent-task-comparison-schedule.v1` document embeds the
existing authenticated matrix and its canonical SHA-256, then orders its exact
rows without changing their trial hashes. `rotating-tasks-alternating-pairs-v1`
rotates task position each repetition and alternates which lane runs first in
each adjacent pair. The eighteen-row pilot has nine pairs, so first positions
are necessarily split five/four; `first_lane_counts` records that imbalance.
Generate the final schedule after freezing the runner and repository revision.
It is an execution order only: it supplies no trial observations, reviewer
measurements, provider configuration, or execution authority.

The private `opencode-agent-task-pilot.py` transport uses a local MCP argv tool
for both lanes. Source-first exposes bounded source reads and writes with an
expected preimage digest plus ordinary compiler commands; graph-operational
exposes semantic compiler operations and bounded patch-artifact creation.
Native OpenCode shell/read/edit tools are denied. The host retains exact MCP
frames, provider exports, source bytes/diffs, independent acceptance results,
and monotonic validation duration, including failures. The owned task also
retains its generated review package. Local source/graph stub execution and
MCP discovery establish transport behavior, not real-model observations.

Captured records remain explicitly ineligible until all required context,
recovery, intervention and blinded-review measurements exist and pass the
existing ledger/observation audit. Provider export counters are reported
observations, not billing proof. Internal gateway argv/stdio counts are
separate diagnostics; only archived external MCP frames supply tool traffic
measurements. The transport uses OpenCode's documented
[local MCP configuration](https://opencode.ai/docs/mcp-servers/).

### Frozen two-model execution protocol and cohort accounting

An authorized future pilot supplies a canonical
`semaprax.opencode-agent-task-pilot-protocol.v1` JSON file to every
`opencode-agent-task-pilot.py run` invocation, together with one selected
protocol model ID. The protocol has exactly two identities, each with a
provider, model, immutable revision, OpenCode `provider/model` selector,
tokenizer and configuration identity. It also binds the canonical manifest,
runner revision, and non-secret budget and egress policy identifiers. It
contains no credential value, credential environment-variable name, endpoint,
or authorization claim.

Each new trial record carries the exact protocol SHA-256 and complete selected
model identity. `audit-cohort --protocol <path>` therefore requires the full
`2 models × 3 tasks × 2 available lanes × 3 repetitions = 36` tuple inventory
per host. It retains completed, failed and aborted rows; an inventory can only
be `eligible_for_scoring` when every retained record is eligible. The audit
performs no execution, provider call, score calculation, or second-host
inference.

`audit-cohort` without `--protocol` remains available only for the historical
September record layout. Its output labels that input `historical_evidence:
true` and `eligible_for_scoring: false`; it cannot turn the old 18-tuple
cohort into a current two-model result. A later two-host verifier must bind
two independently executed protocol cohorts and preserve the operator's host
custody evidence. A different hostname, copied record directory, or replayed
artifact is not evidence of independent physical or virtual host execution.

### Instrumented eligibility: the four previously missing measurements

Every one of the 18 real-model tuples in the
[13 September private pilot](AGENT-TASK-PILOT-2026-09-13.md) is ineligible for
exactly four missing measurements. `opencode_agent_task_pilot.eligibility`
(`scripts/opencode_agent_task_pilot/eligibility.py`) gives each a precise
definition, a deterministic serialization in the per-trial `record.json`, and a
fail-closed reader that reports the measurement `unavailable` with a named
reason rather than a fabricated value:

| Metric | Definition |
| --- | --- |
| Presentation bytes | The frozen task prompt's exact UTF-8 byte length (presented once, at session start) plus every MCP `tools/call` response byte returned to the model as tool context, summed with repeats across the trial. A fresh trial retains an exclusively-created canonical `presentation.json` that binds those totals and the frozen prompt SHA-256 to the exact archived `mcp-wire.jsonl` SHA-256; eligibility re-derives the record from both primary byte streams and rejects a missing, non-canonical, stale, substituted, or link-backed record. It never estimates the value from token counts or backfills an earlier trial from a later summary. |
| Blinded active review time | A reviewer starts with `opencode-agent-task-pilot.py start-review --evidence-dir <dir>`, which exclusively creates a label-free `review-packet.json` and a canonical `review-session.json` holding the host monotonic start timestamp plus exact packet/candidate digests. They finish with `finish-review --evidence-dir <dir> --reviewer-id <id> --verdict accept|reject --blinded`; the host records the stop timestamp and derives `active_ms` from that session, rather than accepting operator-supplied timestamps. The reviewer still attests that direct lane/model/runner labels were withheld: the local timer cannot prove human attention. `review.json` binds the diff, packet and session bytes, is written once, and stale/tampered sessions are unavailable, never zero. The retained `record-review` command is a compatibility import slot, but its manually supplied time is not eligible evidence. |
| Typed stale/recovery metrics | A closed enum classification of the manifest's synthetic drift trigger and the agent's response to it, derived only from primary `gateway.jsonl` records. The gateway's canonical successful `pilot-drift` receipt binds the triggering command plus distinct before/after source SHA-256 values; a source-read trigger additionally has to agree with the archived raw pre-drift `pilot-read` bytes. A successful `pilot-write-source src/core.spx` counts as `recovered_conditional_write` only when its expected digest is the receipt's post-drift digest and its response is the gateway's exact success bytes. `rejected_stale_write` needs the receipt's pre-drift digest plus the gateway's exact stale-precondition refusal; otherwise it is `unavailable`, never relabelled. Trigger kinds: `drift_on_source_read`, `drift_on_identifying_command`. Recovery outcomes: `recovered_conditional_write`, `rejected_stale_write`, `no_recovery_attempt`. A task with no declared drift scenario must show zero triggers; a task with one must show exactly one; anything else is internally inconsistent and reported `unavailable`. |
| Intervention ledger | An append-only, ordered JSONL record of every operator intervention (`interventions.jsonl`), each entry carrying a strictly increasing `sequence`, a non-decreasing `timestamp_ns`, a closed `kind` (`timeout_extension`, `manual_process_kill`, `manual_source_edit`, `manual_harness_restart`, `manual_config_override`, `other_operator_action`), and a `target`. The runner creates the (possibly empty) ledger file at trial start, before any run activity, so even a zero-intervention trial has an auditable basis; creation refuses links and existing targets. An operator appends mid- or post-trial with `opencode-agent-task-pilot.py intervene --evidence-dir <dir> --kind <kind> --target <target> [--note <text>]`; each append holds an exclusive POSIX ledger lock across its bounded read, validation, sequence allocation and write, so concurrent operators cannot claim the same sequence number. An absent ledger file is `unavailable`, never a fabricated zero. |

**Eligibility predicate.** `compute_eligibility` is the single fail-closed
gate: a trial is `eligible` only when all four measurements above are present
and internally consistent. It never raises — any defect in one measurement
(missing file, malformed JSON, an inconsistent count, an out-of-range
interval) makes that one measurement `unavailable`, which makes the whole
trial ineligible and names the exact missing measurement by string in
`record.json`'s `reason` field (and per-metric detail in `eligibility`). This
replaces the previous hardcoded `ineligibility_reason` string and the
hardcoded `eligible_observation: False` in
`scripts/opencode_agent_task_pilot/replay.py`, which never measured any of the
four at all. `replay.py`'s offline acceptance replay now recomputes this same
predicate from whatever `gateway.jsonl` / `mcp-wire.jsonl` / `review.json` /
`interventions.jsonl` bytes already exist in a trial's evidence directory.

**The 2026-09-13 cohort remains ineligible.** None of its 18 archived evidence
directories ever recorded a `review.json` or an `interventions.jsonl` (those
artifacts did not exist before this instrumentation), so recomputing
eligibility against that frozen cohort still reports it ineligible for at
least blinded active review time and the intervention ledger, regardless of
what its already-archived transport bytes can retroactively establish for
presentation bytes or typed stale/recovery metrics. This is by design: missing
measurements are never backfilled, inferred, or defaulted to zero for already-
captured evidence. Making the 2026-09-13 tuples — or any future tuple —
eligible requires one more, separately authorized cohort run, executed with
this instrumentation in place from the start (so the blinded reviewer and
intervention ledger exist as first-class run artifacts, not retrofits).

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

Hostile library regressions have HOSTED GREEN v0.4.0 evidence. The framework
records no eligible comparative model observations or productivity/superiority
result. The [13 September private pilot](AGENT-TASK-PILOT-2026-09-13.md) retains
18 real-model tuples across the two available lanes; all remain ineligible.
The reserved Zero lane remains unrun. Passing framework tests does not supply
missing experimental measurements.

### Offline OpenCode stream counters

When OpenCode session export is truncated, a separate write-once derivation may
use session-bound JSONL `step_finish` reports. It rejects duplicate step/message
identities and missing or malformed counters, and authenticates the raw stream
by digest. OpenCode 1.18.27 separates cache tokens from `input` and reasoning
tokens from `output`; the derivation adds them back and retains every component
([normalization source](https://github.com/anomalyco/opencode/blob/v1.18.27/packages/opencode/src/session/session.ts)).
The configured model identifies the CLI invocation; stream fields alone do not
independently authenticate a provider identity. This supplementary report does
not rewrite the original trial record or supply missing review/context metrics.

Offline acceptance replay (`scripts/opencode_agent_task_pilot/replay.py`) binds
the complete archived source inventory to its original run record and the frozen
compiler hash. It restores the candidate in a temporary directory and records
the existing task oracle in a separate, exclusive derivative. Original timeout
records remain unchanged. Authority replay, blinded review time, and missing
context or recovery measurements remain unavailable; this derivative does not
make a pilot observation eligible.

After that frozen cohort completed, export-counter extraction was corrected to
include cache and reasoning components, matching stream-v2 derivation. Original
trial records and prior counter derivatives remain unchanged.
