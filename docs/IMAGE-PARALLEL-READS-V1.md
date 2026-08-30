# Image Parallel Reads v1

Audience: embedding hosts, compiler contributors, and agent adapter authors.

Status: code and regressions authored, **unrun**. No compiler, test, scheduler,
latency, throughput, or memory gate has been executed in this change.

`VNextSession::handle_read_batch(frames, workers)` is an explicit host API for
concurrent immutable image and discovery reads. It takes a slice of raw NDJSON
frame bodies, without LF, and returns one optional response per input position.
The host supplies between one and four workers and one to sixteen frames.
Each frame retains the existing 64 KiB limit; each response retains the 1 MiB
limit. Input and retained response payload totals therefore cannot exceed
1 MiB and 16 MiB respectively. These are wire bounds, not complete heap, thread
stack, HIR, CPU, or RSS limits. Up to four independently bounded query
computations may be live simultaneously.

The implementation spawns scoped worker threads over an immutable shared image
and fixed discovery policy. It does not rebuild or clone a complete image per
request. Workers process disjoint input positions. All spawned workers join
before the host receives results; response order is restored to input order,
regardless of completion order. No result is streamed early. A worker spawn
failure or panic discards the complete batch result after joining the remaining
workers and reports `SPX-G295`; panic payloads are not protocol diagnostics.
There is no cancellation, work stealing, persistent thread pool, or background
refresh. Normal Rust panic hooks and allocator behavior retain their ordinary
host semantics.

`parallel_read_methods()` returns the exact supported subset from the selected
catalogue. It includes image symbol, function summary, facet, context and impact
queries; workspace open/status projections; and protocol capabilities, schemas,
instructions, client generation and query catalogue. This is an explicit enum
allowlist, not an assumption that every method marked `query` is safe to run
concurrently. Candidate operations, diagnostic attempts, tests, target/artifact
builds, refresh and preview, source-commit status/receipts, and publication are
excluded, even when the ordinary session has their grants. Excluded calls get
JSON-RPC `-32601`. No Git host, filesystem handle, registry, mutable cache, or
test interpreter is sent to a worker.

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
An all-rejected/notification batch does not authenticate source or spawn workers.
Valid requests carry the ordinary exact image expectation. Bounds are checked
before parsing any frame; a batch-level bound error is `SPX-G294` and returns
no rows. A frame submitted through this entry point still starts the session
for purposes of the existing startup-only Git approval guard. Rejected batch
configuration with no processed frames does not grant approval or alter source.

This is not JSON-RPC array batching or a new remotely available method.
`serve_vnext` and the `serve-workspace` CLI still process NDJSON sequentially.
An embedding host must explicitly collect a bounded group and invoke this API;
an agent cannot request worker count or widen the supported read subset. Method
capabilities and normal sequential response bytes remain unchanged. Transport
scheduling across independent incoming streams, parallel candidate queries,
cancellation, and measured throughput improvements remain outstanding.

`tests/image_parallel_reads_v1.rs` authors sequential-byte equality across
worker counts, request-order preservation, operation exclusion, invalid input,
silent notifications, startup approval preservation, and absorbing drift.
Module regressions force worker overlap and check that a worker panic still
joins other workers. These cases are unrun; they do not establish a measured
performance improvement or a complete concurrent transport implementation.
