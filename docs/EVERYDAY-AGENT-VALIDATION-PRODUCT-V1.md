# Everyday Agent Validation Product v1

Status: local evidence only, **unsupported and unpublished**. This document
freezes the bounded validation product built for issue
[ABI-09A.17 (#166)](https://github.com/wavect/semaprax/issues/166); it is not
a support or publication decision, and it does not narrow or pre-empt PG-9
(#165/#175).

Audience: reviewers of #166, maintainers deciding whether to raise
`MAX_BUILDER_BYTES` (`SPX-G171`) or `MAX_BYTES_COPY_SITES` (`SPX-T267`) for
bounded workloads, and any later agent extending
`examples/everyday-agent-project`.

## Why this document exists

#166 asks for a "versioned end-to-end validation product/profile" pinning
request/fixture schema, Agent definition, checkpoint/recovery rules, output
schema, evidence schema, bounds, supported target(s), and nonclaims. Those
facts already existed, spread across
`examples/everyday-agent-project/README.md` and
`tests/agent_runtime_v1/everyday_agent_product.rs`. This document is the one
place that pins them together with exact identifiers, matching this
repository's convention for a versioned product profile (compare
[Project Profile Admission v1](PROJECT-PROFILE-ADMISSION-V1.md),
[Public Generic Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md)).
It restates no capability the linked sources do not already have; it adds no
code, `.spx` source, diagnostic, or CLI surface.

## Profile identity

| Layer | Identifier |
| --- | --- |
| Product profile | `semaprax.everyday-agent-public-generic-validation.v1` |
| Project | `examples/everyday-agent-project` (`semaprax.toml`, `profile = "filesystem-io.v3"`) |
| Data/JSON/filesystem module | `everyday.manifest` (`src/manifest_json.spx`) |
| Source-declared Agent | `everyday.agent` (`src/agent.spx:84`) |
| Agent module | `everyday.agent.lifecycle` |
| Validation harness | `tests/agent_runtime_v1/everyday_agent_product.rs` |
| Depends on (blocked) | [ABI-09A.13 (#162)](https://github.com/wavect/semaprax/issues/162), [ABI-09A.14 (#163)](https://github.com/wavect/semaprax/issues/163) |
| Governed by | [PG-9 decision](https://github.com/wavect/semaprax/issues/165) — not affected by this document |

This project combines two already-shipped foundations, but they are
**not wired together** as an Agent-effect-calls-fs pipeline.
[Nonclaims](#nonclaims) explains the boundary and provides reproductions.

## What it proves

Verified by `semaprax check`/`semaprax test` on the project (including the
Agent module in the authenticated source set), `semaprax check` on the Agent
module standalone, and
`cargo test --locked -p semaprax --test agent_runtime_v1 everyday_agent`
(13 of 126 tests in that binary; see [Evidence](#evidence)):

- canonical `.spx` source and an exact ProgramRoot (`semaprax check` reports
  a stable content digest for both the project and the Agent module alone);
- bounded, typed `fs.read` / `fs.write-new` (no-clobber) filesystem
  interaction against a real temporary directory, through the same
  `with_authenticated_project` / `execute_filesystem_command` Project route
  `std.fs`'s own tests use;
- bounded JSON validation, member lookup, and classification over a fixed
  flat manifest;
- a canonical, bounded, deterministic report;
- a source-declared Agent (`@id("everyday.agent")`), admitted by the
  authenticated Project source set and compiled/selected by stable ID
  (`compile_source_agent_lifecycle`);
- explicit authorization before the external boundary
  (`everyday.agent.fn.authorize`), proven to stop **before** the boundary
  (`read.calls == 0` on refusal, not merely a refused status);
- durable per-operation checkpoints and crash/restart recovery without a
  duplicate external read, across all five `CrashPoint`s
  (`BeforeIntent`/`AfterIntent`/`AfterEffect`/`AfterSettlement`/`BeforeDelivery`),
  with genuine "uncertain, no blind retry" semantics at the two boundaries
  where the read may or may not have happened;
- checkpoint tamper detection (truncation, journal reorder, one injected
  byte), `resume()` refusing a revoked `policy_epoch`, and `resume()`
  reporting a display-only source rename as `source_drift` (ProgramRoot
  drift);
- replayable evidence (`DurableRun::evidence()`) binding identities, digests,
  and counts, asserted **not** to carry the fixture bytes, the authorization
  seal, or the task payload.

## Request/fixture schema

The fixed, flat, three-record manifest (`fixtures/input.json`):

```json
{"schema":"everyday-agent-manifest.v1","record_count":3,
 "rec0_id":"m1","rec0_tag":"ok","rec0_payload":"alpha",
 "rec1_id":"m2","rec1_tag":"warn","rec1_payload":"beta",
 "rec2_id":"m3","rec2_tag":"ok","rec2_payload":"gamma"}
```

`record_count` is pinned at exactly 3 (see [Bounds](#bounds) for why); each
record has an identifier, a Copy-scalar `tag` (`"ok"` accepts, anything
else including `"warn"` is rejected), and an owned text `payload` bounded at
16 bytes. This fixed fixture schema is not the issue's configurable
`input_manifest`/`output_report`/`max_input_bytes`/digest-bearing work request.
Building that general request layer on `std.fs` plus `std.data.json.doc` hits
`SPX-G171`, as described below.

## Output report schema

The canonical, 7-byte report: `{"a":<accepted_digit>}`. Because
`record_count` is pinned at 3, a reader recovers `rejected_count` as
`3 - accepted_count` without a second field. For the fixture above:
`{"a":2}` (two `"ok"` tags accepted, one `"warn"` tag rejected).

## Checkpoint/recovery rules

Unmodified Agent Lifecycle v1 durable machine
(`bind_durable_agent`/`DurableAgent`, [Agent operation checkpoint
v2](AGENT-OPERATION-CHECKPOINT-V2.md)): one settled observation per intent,
one external read per completed run, uncertain dispatch
(`AfterIntent`/`AfterEffect`) left `DurableStatus::Unknown` until explicit
host reconciliation, revoked `policy_epoch` refused before any further
external effect, and display-only source rename reported as `source_drift`
— the *last* of eight checks in `CheckpointBinding::drift`
(`src/agent_lifecycle/durable/checkpoint.rs:185-207`), so the assertion
proves the display-only case is not silently absorbed by an earlier,
broader drift check.

## Evidence schema

`DurableRun::evidence()` is one canonical JSON line binding: terminal
`status`/`reason`, `boundary_crossings`, an `evidence_digest`
(`sha256:`-prefixed), and the checkpoint chain's own digest
(`AgentCheckpoint::decode(..).digest()` equals the store's recorded digest).
It is asserted to omit the authorization seal (`"AZ"`), the manifest-review
identity strings, and the task payload — evidence binds facts, not payload.

It does **not** bind descriptor/provider/carrier digests, because there is
no real descriptor/provider/carrier call in this product (see
[Nonclaims](#nonclaims)).

## Bounds

Two are real, reproducible **language/tooling limits this product ran
into**, re-verified against current `main`, not arbitrary round numbers:

| Bound | Value | Site | Reproduction |
| --- | --- | --- | --- |
| Workspace Semantic Graph `builder_bytes` (`SPX-G171`) | 67,108,864 bytes | `MAX_BUILDER_BYTES`, `src/workspace_graph.rs:59` | The former 18,874,368-byte ceiling rejected this product's `std.data.json.doc` + `std.fs` closure at a 23,741,424-byte forecast. The raised ceiling admits that bounded closure; oversized projections still fail closed at the same diagnostic. |
| `bytes_copy`/`bytes_set`/`writer_write_u8` interprocedural site count (`SPX-T267`) | 16 sites | `MAX_BYTES_COPY_SITES`, `src/byte_data_capacity.rs:13` | Extend `report_writer` past the 7-byte report: `bytes_copy path reaches 17 sites; limit is 16`. |

The interprocedural site count remains why `record_count = 3` and the 7-byte,
one-field report are this product's checked bounds with a real `fs.write`.
The graph ceiling was raised for the measured, bounded multi-package workload;
future increases still require a measured capacity case rather than an
application-local workaround —
this is the same class of ceiling already blocking #124 and forcing the
reference application (#194) to reimplement `std.db`/`std.http` locally, so
this product is a third, independently-reproduced case to decide that
question against rather than a new one.

## Supported validation target

Reference interpreter only (`semaprax check`/`semaprax test`, and the
`cargo test` harness above, which runs on the host toolchain — no native
C11 or Core Wasm Agent-stage execution is claimed by this product). Native
and Wasm public-generic *provider* execution is explicitly out of scope
(below).

## Nonclaims

This product does **not** deliver, and does not claim to deliver:

- **A real public-generic descriptor/provider/carrier call.** The Agent's
  `execute` effect is backed by the test harness's own `std::fs::read` of
  the real fixture, standing in for "the host executes the explicitly
  selected generated consumer." The generated-consumer assets under
  `src/public_generic_consumer/` are explicitly test-only template fixtures
  bound to a fixture provider literally named `unsupported-unpublished`.
  #162 and #163 (both P0 and open at the time this profile was written) own
  turning that into a real, supported call; this product does not pre-empt
  or imply their outcome.
- Native or Wasm **provider** execution of that call, for the same reason.
- Evidence binding descriptor/provider/carrier/result-carrier digests, since
  there is no real descriptor/provider/carrier here to bind.
- The Agent's operations (`initialize`/`observe`/`authorize`/`reduce`)
  themselves performing `fs.read`/`fs.write`. Only `execute` is an
  `effect fn`, realized by the host — this is the existing Agent-runtime
  authority boundary, not a limit this product introduces.
- The iterative, multi-turn `AGENT-ITERATIVE-LIFECYCLE-V2` profile,
  source-live repair, or `AGENT-STATE-MIGRATION-V3`. The Agent source is an
  authenticated Project source and therefore available to the repair route's
  checked-source lookup, but it remains a Lifecycle v1 Agent: `reduce`
  returns `Report`. V2 requires an authored `Step` variant and a `reduce`
  operation returning that `Step`. Converting this product needs a real
  V1-to-V2 lifecycle and checkpoint-evidence migration.
- Hostile cross-paired-provider rejection, since there is no real provider
  pairing here to cross.
- The Agent's own `.spx` operations reading real JSON — the manifest is
  passed to the Agent only as an opaque `Bytes` byte count for its own
  bookkeeping. The genuine JSON validation/classification lives entirely in
  `everyday.manifest`, run separately.
- General distributed exactly-once execution.

## Evidence

- `semaprax check examples/everyday-agent-project` — verified, exit 0.
- `semaprax test examples/everyday-agent-project` — project tests passed,
  exit 0.
- `semaprax check examples/everyday-agent-project/src/agent.spx` — verified
  standalone, exit 0; the same source is also admitted by the Project check.
- `cargo test --locked -p semaprax --test agent_runtime_v1 everyday_agent`
  — 13 passed, 0 failed (all cases named above).
- `cargo test --locked -p semaprax --test agent_runtime_v1` (full binary,
  regression check) — 126 passed, 0 failed.

All local; no hosted CI run is claimed for this product.
