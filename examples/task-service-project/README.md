# Task service project

A small multi-user task-tracking service composed entirely from bundled
standard-library decision layers: register, log in, create/update a domain
record, enqueue and complete a background job, query its status, log out,
and reject an unauthorized or invalid request. Every step is deterministic
fixture mode -- no socket, no file, and no real clock are touched.

```sh
semaprax check examples/task-service-project
semaprax test  examples/task-service-project
semaprax run   examples/task-service-project
```

## What it demonstrates

- **Two `[dependencies]` edges on bundled standard-library packages**:
  `std.auth = "=0.1.0"` (session lifecycle, password-hash policy bounds) and
  `std.jobs = "=0.1.0"` (claim/lease/retry/idempotency state machines). Both
  packages existed under `std/` before this change but were not yet wired
  into the compiler's closed bundled-dependency registry
  (`src/project/standard_dependencies.rs`), so no ordinary project could
  declare them; this change adds `std.auth`, `std.db`, `std.http`, and
  `std.jobs` to that registry (see the accompanying Rust regression in the
  same file). Only `std.auth`/`std.jobs` are used here -- see the ceiling
  below for why.
- **Row-level authorization**, not just session validity:
  `task_service.core.task_owner_authorized` requires both a usable session
  *and* that the session's own account matches the row's owner. A retired
  (logged-out) session and a different account are both refused.
- **Idempotent job enqueue**: a retried request carrying the same
  idempotency key is a harmless duplicate (`std.jobs.idempotency`'s outcome
  `1`), never a second job.
- **The full acceptance scenario and both rejection paths** are exercised
  twice: once end to end in `task_service.core.run_scenario` (driven by
  `task_service.app.main`), and once as four independent, narrower
  assertions in `task_service.tests` (success path, unauthorized access,
  invalid input, duplicate enqueue).

## Non-claims

- No password is hashed and no signature is verified: `std.auth` is a pure
  policy/state-machine layer with no hashing or signing host capability (see
  `docs/AUTHENTICATION-SESSIONS-V1.md`'s own non-claims). A real deployment
  performs both outside this decision.
- No socket, database, or job queue is opened. `run_scenario` is a fixture
  walk over caller-supplied ticks and byte literals, not a running server.
- The domain record and job are modeled as plain scalar facts (an id, an
  owner account id, a status), not an authored `record`, because
  `Public Useful Data Export v1` -- the `[exports].web` gate every
  `semaprax.manifest.v1` project must satisfy -- admits no authored aggregate
  anywhere in a project that also declares a web export (`SPX-W121`).
- The identifier and HTTP-method grammars are small local reimplementations
  of `std.db.identifier.is_valid` and `std.http.method_is_valid`, not those
  packages themselves -- see the ceiling below.

## Limits this example is shaped by

**`SPX-G171`** (the workspace semantic graph's 18,874,368-byte
`builder_bytes` pre-bound) is charged against the whole link closure -- own
source plus every dependency actually reached -- exactly as
`examples/agent-response-project/README.md` already documents for a single
dependency. Composing all four candidate packages (`std.auth` 17,208 B +
`std.jobs` 8,983 B + `std.http` 5,746 B + `std.db` 4,739 B = 36,676 B of
dependency source, transitively pulling in `std.bytes` too) with roughly ten
distinct functions used across them exceeded the bound outright, even with
this project's own source under 12 KB and even after consolidating from six
of this project's own modules down to three. Dropping to two dependencies
(`std.auth` + `std.jobs`, 26,191 B combined) with nine distinct functions
admits cleanly. The breakpoint tracks the **combined reached closure**, not
raw dependency byte count alone: a first, minimal probe using all four
packages but only one function from each (four total) also admitted, so the
number of *distinct functions pulled from each package* -- not merely which
packages are named in `[dependencies]` -- drives the charge. This is
consistent with `agent-response-project`'s own measurement that "adding a
second dependency ... costs far more than it saves," and is further evidence
for issue #241.

Separately, the built-in persistent semantic cache (`semaprax
semantic-cache-persist`/`-load`, `docs/PERSISTENT-SEMANTIC-CACHE-V1.md`) could
**not** be exercised on this project at all: even the two-dependency
(`std.auth` + `std.jobs`) configuration exceeds `SPX-G256`'s separate,
smaller 16,777,216-byte (`MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND` in
`src/project/incremental.rs`) checked-module-cache construction pre-bound,
even though the same project fits under `SPX-G171`'s larger 18 MB workspace
graph bound for plain `check`/`test`/`run`. This is new evidence for issue
#241: the persistent-cache path has a tighter ceiling than plain checking,
so a project that compiles today may still be unable to use the semantic
cache. See this repository's fast-restart measurement (in the issue #194
worker report) for the exact reproduction and the calculator-project numbers
measured in its place.
