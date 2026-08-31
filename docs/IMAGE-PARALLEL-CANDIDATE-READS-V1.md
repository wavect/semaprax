# Parallel retained semantic reads v1

Status: implementation and regression evidence authored, unrun.

Audience: embedding hosts, agent builders and compiler contributors.

The existing `VNextSession::handle_read_batch(frames, workers)` host API also
accepts explicitly selected immutable candidate, draft and diagnostic reads.
Parallel agents can inspect alternatives, pending work and rejected attempts
without serializing every query or sending a mutable session into workers.
Canonical source, ordinary method grants and separate publication authority
remain unchanged.

## Selection and worker inputs

The host still chooses one to four workers and one to sixteen frames through
the [original batch API](IMAGE-PARALLEL-READS-V1.md). There is no new RPC method,
request-controlled concurrency setting, JSON-RPC array batch or concurrent
NDJSON server. `parallel_read_methods()` lists the intersection of the fixed
read allowlist and the methods already selected by the host's startup policy.
The `query` flag alone does not establish worker eligibility.

Inside the held-source authentication boundary, the host resolves only the
immutable subjects selected by each request. Workers receive those selected
candidate, draft or attempt references, the shared immutable image, parameters
and fixed descriptive policy facts. A comparison receives its two selected
candidates. Symbol diagnostics receives the bounded immutable attempt-reference
inventory in registry order. Its ordinary provenance checks precede matching
and capacity checks, preserving diagnostic precedence and considered counts.
Workers receive no registry, registry mutation, source handle, mutable frontend
cache, test execution handler, Git host, approval or archive-store root.
Policy-bearing discovery payloads are prepared on the serial coordinator inside
authentication. Workers receive those descriptive values rather than the host
policy. Static test-plan and validation-catalogue reads receive only the boolean
fact that their ordinary test profile is selected, never interpreter limits or
an execution handler.

Read payload functions are shared with the ordinary sequential handlers.
They return descriptive values and diagnostics rather than registry mutations.
Resolving an unknown candidate, draft or attempt remains semantic request work:
even a batch containing only such failures passes through source authentication.
Codec, parameter, unavailable-method and stale-image rejection retain the
original early-rejection behavior.

## Admitted reads

The original image/discovery reads remain available. The additive selection
includes these method families, only under their ordinary host grants:

| Family | Methods |
| --- | --- |
| Candidate inspection | `candidate/query`, `candidate/compare`, `candidate/impact`, `candidate/validate`, `candidate/recovery-export` |
| Typed discovery | `change/catalog`, `expression/catalog`, `candidate/contract-expression-catalog`, `candidate/interface-catalog`, `protocol/constructor-schemas`, `validation/catalog` |
| Semantic review | `candidate/semantic-delta`, `candidate/semantic-delta-catalog`, `candidate/interface-delta`, `candidate/contract-delta`, `candidate/ownership-delta`, `candidate/cleanup-dependencies` |
| Pending work | `hole/query`, `hole/recovery-export`, `hole/archive-export` |
| Diagnostic review | `attempt/summary`, `attempt/query`, `attempt/repair-catalog`, `candidate/symbol-diagnostics` |
| Conformance and admission | `protocol/conformance`, `image/target-admission` |
| Static test selection | `candidate/test-plan`, only when the ordinary selected catalogue includes it |

Validation, deltas, archive preparation and repair discovery can independently
replay source, admit ephemeral candidates and regenerate compiler projections.
These remain their existing source-only checks. No temporary repair candidate
is retained, no pending hole is completed and no generated target is executed.
Static test planning executes no interpreter; selecting that query does not
execute the separately granted `candidate/test` method.

Candidate, draft and attempt digests retain their own exact subjects. The
current image expectation still binds the session's held source, but does not
relabel a historical candidate or recovered draft as current. Shared references
do not clone complete HIR per request, persist a result, or turn an archive into
authority. A merged draft's private last-valid candidate is not installed in the
candidate registry merely because a worker queries or exports the draft.

## Exclusions and failure

All open, apply, attempt creation, repair application, discard, fill, complete,
restore, rebase and merge operations remain excluded. So do refresh and refresh
preview, `candidate/test`, target/carrier build methods, artifact deltas, source
commit, commit status and receipt queries, and all host storage/approval APIs.
Their ordinary grants cannot make them eligible for this worker path. Excluded
RPC methods return `-32601` without running their operation.

The exclusive mutable session borrow prevents concurrent registry mutation or
refresh through the same session while a batch runs. Workers process disjoint
request positions. All workers join and bounded response preparation completes
before the final held-source check; results then return in input order. Source
drift discards every row and retains the snapshot's ordinary absorbing-invalid
state. A successful row cannot escape a failed final authentication check.
This is point-in-time source authentication, not a filesystem lock against
arbitrary outside writers.

The original `SPX-G294` batch/frame/worker bounds and `SPX-G295` worker failure
behavior remain. Selected report, selector, continuation and replay diagnostics
remain owned by the sequential query implementation. Ordinary bounded protocol
errors are per-row; worker failure or source authentication failure returns no
partial batch. Notifications remain silent and do no semantic work. Every
processed nonempty frame still closes the startup-only host approval window.

## Bounds and evidence

Frames remain at most 64 KiB and responses at most 1 MiB. At most sixteen
responses are retained, so their wire bytes total at most 16 MiB. The existing
candidate/draft/attempt registry limits and each selected report's own bounds
remain unchanged. Symbol diagnostic snapshots preserve the existing sixteen
considered attempts and four matching repair-discovery limits rather than
copying an unbounded history.

Small output chunks do not bound query work: complete validation, recovery,
archive and delta reports can be generated before slicing. Up to four such
computations can run together. This contract makes no total heap, RSS, stack,
latency, CPU, model-token or throughput guarantee. It introduces no cancellation,
persistent worker pool, automatic scheduling, durable cursor or session recovery.

`tests/image_parallel_candidate_reads_v1.rs` authors sequential-byte parity,
historical and pending selection, immutable parent retention, closed method
grants, malformed/stale selection and source-drift cases. The original batch
evidence continues to own input ordering, worker bounds, join and panic behavior.
Tests, compiler checks and long local quality gates were not run. Executed
concurrency/isolation evidence and representative multi-agent measurements
remain required; no completion-matrix row is promoted.
