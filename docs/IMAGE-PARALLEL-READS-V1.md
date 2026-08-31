# Image Parallel Reads v1

Audience: embedding hosts, compiler contributors, and agent adapter authors.

Status: code and regressions authored, **unrun**. No compiler, test, scheduler,
latency, throughput, or memory gate has been executed in this change.

`VNextSession::handle_read_batch(frames, workers)` is an explicit host API for
concurrent immutable image and discovery reads. The additive
[retained-read contract](IMAGE-PARALLEL-CANDIDATE-READS-V1.md) extends its explicit
allowlist to selected candidate, draft and diagnostic reads without changing
the scheduling and authentication rules below. It takes a slice of raw NDJSON
frame bodies, without LF, and returns one optional response per input position.
The host supplies between one and four workers and one to sixteen frames.
Each frame retains the existing 64 KiB limit; each response retains the 1 MiB
limit. Input and retained response payload totals therefore cannot exceed
1 MiB and 16 MiB respectively. These are wire bounds, not complete heap, thread
stack, HIR, CPU, or RSS limits. Up to four independently bounded query
computations may be live simultaneously.

The implementation spawns scoped worker threads over an immutable shared image
and selected immutable read inputs. Policy-bearing discovery payloads are
prepared on the serial coordinator inside authentication; no host policy enters
the retained-read workers. The coordinator shares its pinned image rather than
cloning complete HIR to dispatch each worker. Selected query owners can still
replay source or derive projections under their own bounds. Workers process
disjoint input positions. All spawned workers join
before the host receives results; response order is restored to input order,
regardless of completion order. No result is streamed early. A worker spawn
failure or panic discards the complete batch result after joining the remaining
workers and reports `SPX-G295`; panic payloads are not protocol diagnostics.
There is no cancellation, work stealing, persistent thread pool, or background
refresh. Normal Rust panic hooks and allocator behavior retain their ordinary
host semantics.

`parallel_read_methods()` returns the exact supported subset from the selected
catalogue. It includes image symbol, function summary, facet, context and impact
queries, including the v5 declaration dependency query; workspace open/status
projections; and protocol capabilities, schemas,
instructions, client generation and query catalogue. This is an explicit enum
allowlist, not an assumption that every method marked `query` is safe to run
concurrently. Candidate mutations, diagnostic-attempt creation, test execution,
target/artifact builds, refresh and preview, source-commit status/receipts, and
publication are excluded, even when the ordinary session has their grants.
The retained-read extension owns its additional pure query selection. Excluded calls get
JSON-RPC `-32601`. No Git host, filesystem handle, candidate registry, externally
mutable cache, or test interpreter is sent to a worker. The shared image may
initialize its bounded source-derived dependency index once; that memoization
does not change image identity or authority.

The same allowlist includes compact dependency summaries and reference-bound
detail pages. Their deterministic selectors remain image-local values, not
shared mutable session cursors or permission to widen the worker's authority.
It also includes `image/analysis-coverage`, a pure retained-input inventory of
known facts and uninspected analysis boundaries. Its workers receive no access
to deployment configuration, generators, external services or runtime state.
When candidate preparation is selected, `candidate/analysis-coverage` performs
the same pure inventory over the exact detached immutable candidate. The worker
derives and discards its candidate image and receives no registry, external
input, execution or publication authority.

When the host attached a verified package graph before any request, its
package summary/consumer methods also join the allowlist. Workers borrow that
independent immutable subject; they do not receive package inputs, acquisition
handles, or permission to rebind the current Project to it. The ordinary held
Project source authentication remains required for the enclosing session.

Every accepted semantic batch rechecks its one held source snapshot before
starting workers and after all workers join and render bounded results. An
observed drift discards all rows, including otherwise successful rows, and
leaves the snapshot absorbing-invalid as on the ordinary request path. The
host must not infer freshness from a partial result because there is none.
This remains point-in-time held-input authentication, not filesystem locking
against arbitrary external writers. Explicit refresh is still required for
recovery and cannot overlap a batch through the session's exclusive mutable
borrow.

Malformed requests and invalid parameters receive the existing codec errors.
Notifications and empty frames produce `None` and perform no semantic work.
An all-codec/parameter/unavailable-method rejection or notification batch does
not authenticate source or spawn workers. Unknown retained subject handles are
semantic query failures and remain inside authentication.
Valid requests carry the ordinary exact image expectation. Bounds are checked
before parsing any frame; a batch-level bound error is `SPX-G294` and returns
no rows. A frame submitted through this entry point still starts the session
for purposes of the existing startup-only Git approval guard. Rejected batch
configuration with no processed frames does not grant approval or alter source.

This API itself is not JSON-RPC array batching or a remotely available method.
The separate opt-in [Parallel Read Protocol](IMAGE-READ-BATCH-PROTOCOL-V1.md)
lets a startup-configured host expose bounded groups through NDJSON and MCP.
`serve_vnext` still processes outer NDJSON frames sequentially. Without that
selection, an embedding host must explicitly collect a group and invoke this
API. An agent cannot request worker count or widen the supported read subset.
Default method capabilities and ordinary response bytes remain unchanged. Transport
scheduling across independent incoming streams, cancellation, and measured
throughput improvements remain outstanding. Selected parallel candidate reads
are authored in the separate retained-read extension. Candidate impact summary
and page reads join that immutable detached subset: each worker recomputes the
same candidate-bound artifact, mutates no registry, and returns bytes in request
order. This scheduling does not make a truncated impact artifact complete.

`tests/image_parallel_reads_v1.rs` authors sequential-byte equality across
worker counts, request-order preservation, operation exclusion, invalid input,
silent notifications, startup approval preservation, and absorbing drift.
Module regressions force worker overlap and check that a worker panic still
joins other workers. These cases are unrun; they do not establish a measured
performance improvement or a complete concurrent transport implementation.
