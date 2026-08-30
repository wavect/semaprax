# Prepared Project Interpreter and Source Trace v1

Status: authored, unrun, and unpromoted. The completion matrix owns product
status.

Audience: language-tool authors, agent builders, and compiler contributors.

This additive library lane prepares the exact entry and test closures of one
immutable `ProjectRevision` once, then evaluates them sequentially on one
long-lived fixed 64 MiB stack. It does not parse, resolve, link, scan closure
admission, or allocate another evaluator thread for each request. Existing
`ProjectRevision::execute_*`, `semaprax.project-execution.v1`, Interpreter v1,
and Project Agent Transport v1-v5 APIs and bytes are unchanged.

Preparation independently validates both retained HIR programs, replays the
explicit-entry/signature/profile closure gate, and creates an owned stable-ID
to function-index map for the exact transitive closures. The combined cache is
limited to 262,144 expression origins and 16 MiB of identity/source-index
facts. Each expression span must fit its authenticated Project source and one
complete origin fact must fit the minimum output envelope. A prepared value is
not cloneable and owns exactly one sequential worker.

## Library surface

`ProjectSnapshot::prepare_interpreter` clones only its authority-neutral
`Arc<ProjectRevision>`. The same operation is available on
`Arc<ProjectRevision>` and through `prepare_project_interpreter`. Preparation
takes `PreparedProjectInterpreterOptions`, whose byte/event ceilings bound all
later requests.

`PreparedProjectInterpreter::execute_entry`, `execute_test`, and `execute`
accept `PreparedProjectExecutionOptions` and one monotonic
`ProjectExecutionCancellation`. The cancellation handle is an in-process
atomic flag. It has no reset, deadline, clock, forced preemption, or transport
meaning. Cancellation is observed before a charged evaluator node and is
reported as `cancelled { before_step }`; a pre-cancelled request therefore uses
zero fuel and reports boundary one. Without cancellation, evaluation order,
fuel, normalized failures, call-depth behavior, and returned values use the
same evaluator as the legacy Project route.

| Bound | Value |
| --- | ---: |
| maximum evaluator steps | 100,000,000 |
| call depth | 256 |
| trace bytes | 65,536–16,777,216 |
| trace events | 1–65,536 |
| prepared origin nodes | 262,144 |
| prepared index content bytes | 16,777,216 |
| retained workers per process | 8 |
| execution/queued request per worker | one/zero; concurrent calls fail `SPX-F109` |

Defaults are 1,000,000 steps, 1 MiB, and 4,096 events. Once the event ceiling
or byte budget is reached, evaluation continues and the deterministic prefix
records exact `recorded_events`, `dropped_events`, and `truncated` facts. Trace
saturation never changes the language outcome. Prefix selection performs one
bounded linear scan: source strings and event JSON are retained only for events
that fit, and the renderer never repeatedly reconstructs oversized prefixes.
The index-byte ceiling accounts canonical identity and source-fact content;
the separate node and worker ceilings bound implementation overhead without
claiming allocator-specific heap-byte equality.

## Source Trace v1

`semaprax.project-source-trace.v1` uses the exact outer wrapper
`schema,digest,bytes,payload`; the digest domain is
`semaprax.project-source-trace.payload.v1\0`. Normatively, the digest is
`SHA-256(domain || little_endian_u64(payload_byte_length) || exact_payload_bytes)`.
Its wire form is the lowercase text `sha256:` followed by exactly 64 lowercase
hexadecimal digits. The payload binds Project schema, Project/Workspace
revisions, Project graph digest, closed entry/test role, module and stable entry
identity, limits/fuel/outcome, ordered events, truncation, and fixed nonclaims.

Each event records event index, charged fuel step, call depth, phase
(`requires`, `body`, or `ensures`), persistent function ID, revision-scoped
expression ID, logical Project path, exact source revision/digest, and byte/
line/column span. It includes no source excerpt. Outcomes are returned `i64`,
compiler-owned language failure, fuel exhaustion, call-depth exhaustion, or
cooperative cancellation.

`verify_project_source_trace` checks the closed JSON shape, canonical values,
bounds, status vocabulary, byte count, digest, and exact reconstruction.
`verify_project_source_trace_against_revision` additionally binds every event
to an expression in the exact transitive retained closure, its structural
requires/body/ensures phase, and its authenticated source fact. V1 does not
independently re-execute the dynamic path; its public digest detects byte drift
but is not producer authentication, while the revision verifier proves bounded
closure/source-origin consistency.

Diagnostics are closed: `SPX-F107` preparation/source-index admission,
`SPX-F108` request bounds, `SPX-F109` worker lifecycle/panic, and `SPX-F110`
trace rendering or replay.

## Authority and nonclaims

The worker is a reference interpreter, not target execution, a JIT, debugger,
profiler, or sandbox. SEMAPRAX code gains no filesystem, process, environment,
network, clock, backend, publication, or mutation authority. Atomic
cancellation timing is not schedule-deterministic; only its observed semantic
step boundary is canonical. Trace bytes are not provenance, approval,
compatibility, target, or production evidence. A live `ProjectSnapshot`
caller must continue to place work inside its existing held-input pre/post
authentication boundary.

Focused authored evidence is in
`src/project/prepared_interpreter/tests.rs`. Promotion additionally requires
legacy Project/Interpreter and Agent Transport byte preservation, runtime
parity, hostile trace replay, capacity boundaries, worker fail-stop, held-input
drift suppression, and exact-head hosted execution under the ordinary quality
policy.
