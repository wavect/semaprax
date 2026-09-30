# Task service project

A small multi-user task-tracking service composed entirely from bundled
standard-library decision layers: register, log in, create/update a domain
record, enqueue and complete a background job, query its status, log out,
and reject an unauthorized or invalid request. Every step is deterministic
fixture mode -- no socket, no file, and no real clock are touched.

`service-config.schema.json` is the closed host-configuration contract.
`service.config.json` is its credential-free fixture instance: database, HTTP,
and telemetry adapters are all explicitly `fixture`, every endpoint is absent,
and every secret is represented only by a nullable host-owned reference. A
host deployment may select the durable `snapshot` store, native HTTP/TLS, and
`semaprax-json-events` only by supplying a separately validated configuration
and resolving its secret references outside Semaprax source. SQLite,
PostgreSQL, and OTLP labels are refused. Neither generated file carries
credentials or grants database, network, telemetry, or secret-store authority.

`service-host-adapter-request.json` is the canonical bounded handoff derived
from that fixture configuration. It declares an empty capability list, so it
cannot ask a host to open a database, serve TLS, resolve a secret, or emit
telemetry. A valid host-mode configuration instead renders three named
requirements as a request only; providing and executing an adapter remains an
explicit host responsibility.

```sh
semaprax check examples/task-service-project
semaprax test  examples/task-service-project
semaprax run   examples/task-service-project
```

## What it demonstrates

- **Ten `[dependencies]` edges on bundled standard-library packages**:
  `std.auth = "=0.1.0"` (session lifecycle and password-policy bounds),
  `std.db = "=0.1.0"` (identifier, migration, and transaction decisions),
  `std.http = "=0.1.0"` (request-line admission),
  `std.jobs = "=0.1.0"` (claim/lease/retry/idempotency state machines), and
  `std.log = "=0.1.0"` (bounded level admission),
  `std.log.redact = "=0.1.0"` (bounded field-count and protected-field
  admission),
  `std.metrics = "=0.1.0"` (guarded metric-series admission),
  `std.tracing = "=0.1.0"` (pure W3C trace-context shape and caller-classified
  secret policy), and `std.export.policy = "=0.1.0"` (bounded exporter-batch
  and sink admission), and `std.webhook = "=0.1.0"` (signature-envelope,
  replay-window, retry-bound, idempotency-composed, and caller-classified
  secret policy). The first four packages became project-selectable when
  `std.auth`, `std.db`, `std.http`, and `std.jobs` joined the compiler's
  closed bundled-dependency registry; `std.tracing` was already selectable.
  Metrics, export policy, webhook, structured logging, and redaction are now
  likewise compiler-bundled. None of these dependencies contributes telemetry
  emission or delivery authority.
- **Row-level authorization**, not just session validity:
  `task_service.core.task_owner_authorized` requires both a usable session
  *and* that the session's own account matches the row's owner. A retired
  (logged-out) session and a different account are both refused.
- **Idempotent job enqueue**: a retried request carrying the same
  idempotency key is a harmless duplicate (`std.jobs.idempotency`'s outcome
  `1`), never a second job.
- **Webhook admission without delivery**: the webhook policy combines an exact
  descriptor replay outcome from `std.jobs` with `std.webhook`'s signature
  envelope, replay-window, attempt, and caller-classified-secret rules. A
  conflicting reuse, stale timestamp, ninth attempt, or classified secret is
  refused before signing, queueing, scheduling, or transport.
- **The full acceptance scenario and rejection paths** are exercised twice:
  once end to end in `task_service.core.run_scenario` (driven by
  `task_service.app.main`), and once as focused assertions in
  `task_service.tests` (success, unauthorized access, malformed HTTP target,
  migration replay/skip, duplicate enqueue, guarded metrics, and bounded
  exporter admission).
- **Three execution lanes, not only the interpreter**:
  `tests/useful_data/task_service_project.rs::entry_and_conformance_return_zero_on_interpreter_native_and_wasm`
  runs entry and conformance on the interpreter and native C11 at `-O0`/`-O2`;
  Core Wasm under Node runs the conformance closure only -- the same shape
  `agent_response_project.rs`/`vector_stats_project.rs` already assert for
  their own sibling reference projects.

## Non-claims

- No password is hashed and no signature is verified: `std.auth` is a pure
  policy/state-machine layer with no hashing or signing host capability (see
  `docs/AUTHENTICATION-SESSIONS-V1.md`'s own non-claims). A real deployment
  performs both outside this decision.
- No socket, database, or job queue is opened. `run_scenario` is a fixture
  walk over caller-supplied ticks and byte literals, not a running server.
- No log, metric, span, or webhook is emitted, exported, collected, signed, or
  delivered. `std.tracing` only
  checks a caller-supplied trace ID, parent ID, flags, and whether the caller
  has classified the event as secret; an adapter must perform classification
  and any eventual emission outside this pure decision. `std.metrics` and
  `std.export.policy` only admit a safe series and bounded host-owned batch;
  `std.webhook` only admits a caller-owned delivery intent. They do not retain
  a series, mutate a queue, schedule a retry, or deliver a byte.
- The domain record and job are modeled as plain scalar facts (an id, an
  owner account id, a status), not an authored `record`, because
  `Public Useful Data Export v1` -- the `[exports].web` gate every
  `semaprax.manifest.v1` project must satisfy -- admits no authored aggregate
  anywhere in a project that also declares a web export (`SPX-W121`).
- `std.db` and `std.http` are pure decision layers only. The reference checks
  identifier grammar, migration order, transaction state, and request-line
  safety, but it does not open a database, bind a listener, or send a request.

## Limits this example is shaped by

**`SPX-G171`** is charged against the reached link closure -- own source plus
the selected dependency declarations -- exactly as
`examples/agent-response-project/README.md` documents. Reachability pruning
keeps unused bundled declarations out of the builder charge, letting this
reference compose `std.auth`, `std.db`, `std.http`, `std.jobs`, `std.metrics`,
`std.tracing`, `std.export.policy`, and `std.webhook` as real dependencies. The application still makes only pure
decision calls: it does not turn a successful check into database, network,
or telemetry authority.

The built-in persistent semantic cache (`semaprax semantic-cache-persist`/
`-load`, `docs/PERSISTENT-SEMANTIC-CACHE-V1.md`) is exercised on this project
by `tests/semantic_cache_store_cli_v1.rs`. Its checked-module construction
pre-bound is tied to the same 64 MiB `MAX_BUILDER_BYTES` ceiling as ordinary
Workspace Semantic Graph admission, removing the smaller independent 16 MiB
limit that rejected this project. `SPX-G256` still fails closed at one byte
over the shared finite-resource ceiling; the cache does not bypass graph
admission, equate the phases' estimates, or claim an allocator/RSS bound.

**The stable-ID semantic-change workflow is exercised end to end.**
`tests/useful_data/task_service_project.rs::stable_id_rename_inspect_preview_apply_and_retest_preserve_the_service`
discovers the legal rename operation for the public
`task_service.core.identifier_is_valid` stable identity, renders its bounded
context and impact, validates a `rename_display_name` preview, applies the
result to an immutable `ProjectCandidate`, and retests both entry and the
complete named-test closure. The candidate keeps the same stable ID and web
export while changing only the human-facing declaration name and all checked
call sites. The checked-in source remains unchanged: candidate application is
authority-free and does not publish or write a source file.

This works even though the selected bundled dependency closure carries
comments (`std.auth` alone has 253 `//` lines and `std.jobs` 34). Issue #274
made the v1 precondition differential: only sources an operation actually
rewrites must be comment-free canonical, while untouched dependency bytes are
preserved exactly. `src/core.spx` is intentionally authored comment-free so
that it is eligible for this demonstration; the prose explaining its
database, HTTP, authentication, job, trace, metric, and export-policy facts
lives here rather than in source comments that a canonical rename would have
to drop.

The public coding-agent transport is exercised separately against this exact
project: it opens the authenticated workspace, derives the same public rename,
renders its preview, impact and review, applies the validated candidate inside
a disposable project copy, and runs the applied revision's full test closure.
This confirms the public transport's derived/prepared/applied state flow for a
real bundled dependency closure; it does not write the checked-in fixture or
publish a candidate.

**The observability policy closure now fits, but only as pure policy
dependencies.** Its reached closure includes `std.encoding`, `std.log`,
`std.log.redact`, and `std.num.overflow`; the checked-in
`structured_log_policy_is_admitted` call admits only a bounded level and at
most 32 fields with no classified protected field. The
`trace_context_is_admitted` call validates W3C-shaped IDs and refuses a
caller-classified secret, while the metric and export calls refuse
secret-classified labels, full queues, injected target IDs, stale webhook
timestamps, conflicting idempotency descriptors, over-budget attempts, and
classified secret payloads.
`tests/useful_data/task_service_project.rs` pins that real closure and runs
the application’s tests on the interpreter, native C11, and Core Wasm; the
entry runs on the interpreter and native C11 only. This is not structured-log
emission, an adapter, span export, or a claim that any observability profile is
supported. `std.log` and `std.log.redact` remain bounded pure admission
dependencies; no supported fallback writer or delivery exists here.

**This project's manifest qualifies for a deterministic local OCI artifact.**
`semaprax build --target oci examples/task-service-project --output <new-dir>`
replays this Project v3 Useful Data package's exact carrier, then writes an
OCI Image Layout whose sole layer is its verified `app.wasm`. The layout is
bound to this manifest's project/workspace/graph revisions and selected entry
module. It is an OCI artifact, not a runnable container image: it has no base
image, root filesystem, entrypoint, database adapter, network configuration,
registry push, signature, or publication claim. The fixture-mode service
remains a source-level policy product; packaging it never grants host
authority to run a database, bind a socket, read a secret, or start a job.

This support is exact to `useful-data.v1` on the Project v3 route used by this
manifest. It does not make the generic npm target, Useful Data v2, or another
Project profile an OCI input.
