# Database Access v1

Audience: language users, tool authors, and compiler contributors.

Status: first bounded slice of the `std.db` profile tracked by issue #190.
This tranche ships the dialect-agnostic, pure, effect-free core that every
database backend needs before it touches a socket: prepared-statement
descriptor validation, the transaction affine state machine, migration
ordering/checksum validation, and bounded-cursor limit checks. It ships as
`std/db`, executed on the interpreter, native C11, and Core Wasm lanes with no
provider and no host authority, exactly like `std.net` and `std.http`. It does
**not** ship a SQLite driver, a PostgreSQL driver, a wire protocol, a new host
operation, or a new effect. See [Non-claims](#non-claims-and-remaining-work).

## Objective

[Bounded Language Network I/O v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md) and
[HTTP Application Routing v1](HTTP-APPLICATION-ROUTING-V1.md) established the
pattern this tranche follows: give checked SEMAPRAX code a typed, bounded way
to reason about a domain (bytes on a socket, an HTTP route) without adding
ambient authority, and let the pure standard-library layer execute on every
backend while the host-authority layer stays behind an explicit, injected
provider. A database client is exactly where the capability invariant in
`AGENTS.md` gets violated first — an ambient connection string, a query built
by string concatenation, a transaction whose outcome is guessed after a
dropped socket — so this tranche specifies the semantics precisely before any
byte reaches a driver.

`std.db` models four things as closed, checked, deterministic computations:

1. **Statement descriptors.** A prepared statement's ordered parameter types
   and a result row's ordered column types are each a `Slice<u8>` of type
   tags. Binding is validated before any host call, and the same primitive
   validates a returned row's shape against its declared descriptor.
2. **Identifiers.** Table, column, and index names are validated against a
   closed safe-byte grammar so that string-built SQL text (used only for
   *identifiers*, never *values*, which are always bound parameters) cannot
   carry a quote, semicolon, backtick, or other structural byte.
3. **Transactions.** An affine resource with states `NONE`, `OPEN`,
   `COMMITTED`, `ROLLED_BACK`, `FAILED`. Every transition is a pure function
   from the current state (and, for one transition, an external signal) to
   the next state; an invalid transition — a nested `begin`, a `commit` from
   `NONE` — lands on the sticky `FAILED` state rather than silently
   succeeding or panicking, matching the repository's "failure selection is
   sticky" invariant.
4. **Migrations.** A migration ledger is an ordered sequence of one-byte
   stable IDs (`1..=255`) each paired with a one-byte content checksum tag.
   Clean apply, reapplication, checksum drift, missing predecessor,
   out-of-order attempts, and a concurrent runner's duplicate attempt are each
   one checked predicate over the ledger and the candidate migration.

Everything above is scalar arithmetic and byte comparison. None of it opens a
socket, reads a file, or reads an environment variable, so none of it needs an
effect, a `permit`, or a provider; the profile is `scalar` like `std.core` and
`std.num`.

## Type tags

| Tag | Meaning |
| ---: | --- |
| 0 | `i64` |
| 1 | `u8` |
| 2 | `bool` |
| 3 | `usize` |
| 4 | `Bytes` (a `Slice<u8>` column or parameter) |

Any other byte is an invalid tag. A parameter list or result-row descriptor is
a `Slice<u8>` of these tags in declared order; this is the "narrow row type
grammar compatible with current owned/public boundaries" the issue asks for.
It deliberately does not use a `record` or `variant` row type: [HTTP
Application Routing v1](HTTP-APPLICATION-ROUTING-V1.md#route-and-status-identity)
found that the bounded reference interpreter does not yet admit record-field
projection (`SPX-F102`), so a record-shaped row would type-check under
`semaprax check` but not execute end to end on that lane today. The same
closed-domain idiom `std.net.wait_is_*` and this profile's route table both
use is reused here so every function in this package executes on all three
backends now; a record/variant-shaped row is tracked in
[Non-claims](#non-claims-and-remaining-work), gated on the same interpreter
admission HTTP Application Routing v1 is waiting on, not a shape this profile
refuses to check.

## Descriptor validation

`std.db.descriptor.tag_is_valid(tag: u8) -> bool` is `tag <= 4u8`.

`std.db.descriptor.mismatch_class(expected: borrow Slice<u8>, actual: borrow Slice<u8>) -> usize`
classifies a bind or result attempt against its declared descriptor:

| Code | Meaning |
| ---: | --- |
| 0 | every position matches; binding or result mapping is checked |
| 1 | `actual` is shorter than `expected` — a missing bind or a short row |
| 2 | `actual` is longer than `expected` — an extra bind or a wide row |
| 3 | lengths agree but some tag differs — a type-mismatched bind or column |
| 4 | some tag in `expected` or `actual` is not a valid type tag |

`std.db.descriptor.matches(expected, actual) -> bool` is
`mismatch_class(expected, actual) == 0usize`. The same pair of functions
validates a prepared statement's bind list against its parameter descriptor
and a fetched row against its declared result descriptor — "exact ordered
parameters and result fields" is one checked shape, not two.

## Identifiers

`std.db.identifier.is_safe_byte(byte: u8) -> bool` admits ASCII letters,
digits, and `_` only. `std.db.identifier.is_valid(name: borrow Slice<u8>) ->
bool` requires a nonempty name of at most 63 bytes, whose first byte is a
letter or `_` (never a digit, to keep an identifier lexically distinct from a
numeric literal), and whose remaining bytes are all safe. A quote, backtick,
semicolon, hyphen, dot, space, or NUL byte is never a safe byte, so an
identifier assembled from user input cannot smuggle SQL structure even though
it participates in generated statement text rather than being bound as a
parameter value — the injection surface issue #190 names explicitly under
"failure and security cases": "SQL injection through identifiers ... even
when values are parameterized."

## Transactions

States are closed `usize` codes, the same idiom as
`std.net.wait_is_timeout`/`wait_is_readable`/`wait_is_closed`:

| Code | State |
| ---: | --- |
| 0 | `NONE` — no open transaction |
| 1 | `OPEN` |
| 2 | `COMMITTED` |
| 3 | `ROLLED_BACK` |
| 4 | `FAILED` |

`std.db.transaction.can_begin(state: usize) -> bool` is `state <= 4usize &&
state != 1usize`: this profile admits no nested transactions, so a `begin`
while already `OPEN` is refused rather than silently creating a savepoint,
but a connection is reusable across a sequence of transactions — `begin`
succeeds again from `COMMITTED`, `ROLLED_BACK`, or `FAILED`, exactly as a real
connection accepts a new transaction after the previous one settles.

Every transition function has `ensures result <= 4usize` and returns `4usize`
(`FAILED`) for any call that is not valid from the given state, so an invalid
transition is sticky failure rather than a silent no-op:

- `std.db.transaction.next_on_begin(state) -> usize`: `1usize` unless `state`
  is `OPEN` or not a valid state code, `FAILED` otherwise (covers the
  nested-attempt-refusal case while still admitting the next transaction on
  the same connection once the previous one has settled).
- `std.db.transaction.next_on_commit(state) -> usize`: `2usize` from `OPEN`,
  `FAILED` otherwise.
- `std.db.transaction.next_on_rollback(state) -> usize`: `3usize` from `OPEN`,
  `FAILED` otherwise.
- `std.db.transaction.next_on_connection_lost(state) -> usize`: `FAILED` from
  `OPEN` (the outcome of the in-flight operation is unknown, so the profile
  never reports `COMMITTED` on a guess), unchanged otherwise. A host calls
  this only while a transaction is open and the connection observably drops;
  it is the one transition driven by an external signal rather than a
  requested operation, modeling "transaction ambiguity after network loss".

`std.db.transaction.is_open(state) -> bool` is `state == 1usize`.
`std.db.transaction.is_settled(state) -> bool` is `state == 2usize || state ==
3usize || state == 4usize`. Once settled, every `next_on_*` transition from
that state (other than a fresh `next_on_begin` from a state a host has since
reset to `NONE`) returns `FAILED`: settlement here composes with the
"failure selection is sticky" invariant instead of special-casing it.

## Migrations

A migration ledger is modeled as one-byte stable IDs `1..=255` (the profile's
bounded model of a real content digest is a single tag byte; a production
adapter's real checksum is wider, this package validates the *decision
procedure*, not a specific hash width — see
[Non-claims](#non-claims-and-remaining-work)):

- `std.db.migration.is_out_of_order(last_applied: u8, candidate: u8) -> bool`
  is `candidate <= last_applied`: a candidate ID must be strictly greater
  than every already-applied ID.
- `std.db.migration.is_duplicate(applied: borrow Slice<u8>, candidate: u8) ->
  bool` scans the applied-ID ledger for an exact match — the concurrent-runner
  case, where two runners race to apply the same migration.
- `std.db.migration.missing_predecessor(applied_count: u8, candidate: u8) ->
  bool` is `candidate != applied_count + 1u8`: migrations apply in a gapless
  sequence starting at `1`, so any gap is a structural contradiction rather
  than a possibility the runner discovers later. (The language admits no
  numeric cast, so `applied_count` stays `u8` like every other migration
  quantity in this profile, bounding a ledger to 255 migrations.)
- `std.db.migration.checksum_drift(recorded: u8, candidate: u8) -> bool` is
  `recorded != candidate`: reapplying an already-applied ID with a different
  checksum is drift, reapplying it with the same checksum is the idempotent
  no-op the issue calls "reapply".

A clean apply is exactly the case where all four predicates are false for the
next candidate ID against the recorded ledger; the conformance module below
exercises every positive and negative case named in the issue's required-tests
list under "Migration clean apply, reapply, checksum drift, missing
predecessor, out-of-order, and concurrent runner" (partial-failure and true
concurrency both need a live host transaction and are out of scope for this
pure layer; see [Non-claims](#non-claims-and-remaining-work)).

## Bounded limits

`std.db.limits.within_bounds(rows: usize, max_rows: usize, bytes: usize,
max_bytes: usize, columns: usize, max_columns: usize, elapsed_ms: usize,
max_elapsed_ms: usize) -> bool` is true only when every measured quantity is
at most its bound. `std.db.limits.should_stop(consumed_rows: usize, max_rows:
usize) -> bool` is `consumed_rows >= max_rows`, the predicate a bounded cursor
consults once per row so "early consumer stop" is a checked decision instead
of an incidental loop exit.

## Non-claims and remaining work

This tranche adds no host operation, no new effect name, no new ABI, no
`import rust fn` declaration, and touches no file under `src/hir`, `src/wasm`,
`src/codegen`, `src/interpreter*`, or `src/cli`. Concretely, it does **not**:

- **Connect to anything.** There is no SQLite driver, no PostgreSQL driver, no
  connection handle, no host op, and no provider. `std.db` cannot read a
  connection string, an environment variable, or a filesystem path, because
  it has no capability to read anything — the acceptance criterion
  "credentials and network authority remain deployment/host-owned" holds by
  construction rather than by policy on top of an ambient channel.
- **Execute a query.** There is no SQL string, no query planner, and no
  execution engine. `std.db.descriptor` validates *shapes*; it never builds
  or sends a statement.
- **Model a typed row as a `record`.** See [Type tags](#type-tags); a
  record/variant row is future work gated on the same interpreter
  record-field-projection admission `HTTP Application Routing v1` is waiting
  on.
- **Provide a real content digest.** `checksum_drift` validates the decision
  procedure over an opaque one-byte comparison; a production migration
  runner's checksum is a wide cryptographic or CRC digest computed over
  migration content, not modeled here.
- **Provide a SQLite or PostgreSQL adapter.** Both need a real driver: either
  a from-scratch wire-protocol implementation (SQLite's on-disk file format
  and page format for a "deterministic/local adapter", or PostgreSQL's
  frontend/backend wire protocol for a "PostgreSQL integration adapter"), or
  a Cargo dependency such as `rusqlite`/`libsqlite3-sys` or
  `postgres`/`tokio-postgres`. Adding a dependency to this repository's own
  `Cargo.toml` is a decision outside a bounded worker's authority. [Project
  Dependencies v1](PROJECT-DEPENDENCIES-V1.md#rust-crate-inputs) already
  specifies the exact extension point a real driver would use without
  touching this repository's own `Cargo.toml`: a downstream Project's
  `[rust-dependencies]` table names the crate, `import rust fn` declares the
  typed, effect-gated boundary function (the same pattern
  `PROJECT-DEPENDENCIES-V1`'s own `same_file` example demonstrates), and the
  generated Native Rust SDK's `NativeRustSdkImports` trait is where a host
  implements it against the real crate. Effect names such as
  `database.connect`, `database.read`, `database.write`, and
  `database.migrate` compose with that pattern exactly like `host.filesystem`
  does in the referenced example: they are declared, not drawn from a fixed
  host-operation table, so wiring them needs no change to this repository's
  effect catalogue. Building that adapter is the follow-on tranche; this
  document records the exact seam so it is not reinvented.
- **Prove true concurrency or partial migration failure.** Two runners racing
  for the same migration ID, or a migration failing partway through DDL, need
  a live transactional host; `is_duplicate` and the transaction state machine
  above are the checked decision procedures a live runner must obey, not a
  simulation of concurrent execution.

A separate, Rust-only fixture (`src/database_fixture.rs`, local evidence only,
described in its own module documentation) exercises the same transaction and
migration decision procedures against an in-memory relational store to prove
the model composes into something a real engine could implement — it is
explicitly **not** SQLite, **not** PostgreSQL, and is not reachable from
checked SEMAPRAX source in this tranche.

## Local evidence

```sh
cargo test --locked -p semaprax --lib database_fixture::
cargo test --locked -p semaprax --test project -- standard_library::
cargo test --locked -p semaprax --test documentation
```

The first command covers the Rust-only in-memory fixture described above. The
second covers `std/db`'s canonical formatting, stable identities, examples,
and conformance module once it is registered in `std/packages.json`; that
registration is tracked separately (see the accompanying change's report) so
this document's local-evidence list can be checked incrementally.
