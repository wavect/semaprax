# Owned Nested Outcomes v1

Status: focused executable qualification complete for the bounded compiler and decoder scope. Matched agent trials remain pending.
Audience: coding agents, compiler contributors and reviewers.

This is the compiler prerequisite tracked by #729 for finite application JSON
construction in #724. The focused gates below qualify the owning outcome and
the two bounded decoder examples. Fresh matched trials are still required to
measure turns, net input, authored tokens, fixed context, estimated
accepted-task cost, and wall time.

## Authenticated shape

An ordinary authored monomorphic, invariant-free variant has exactly two cases,
in either declaration order. Every variant, case, field, and transitively used
record declaration has an explicit stable identity. One case has exactly one
field whose type is an acyclic owning record. The other case has exactly three
fields with types `i64`, `usize`, `i64`, in that order. Names carry no authority.
The scalar error case contains no default record and creates no payload owners.

The record closure admits the existing eight Copy scalars, `string`, `Bytes`,
further monomorphic invariant-free records, and the exact existing bounded Vec
carriers admitted by Owned Collection Records v1. No Vec element shape or
capacity changes. String/Bytes runtime caps remain unchanged. Maps, resources,
classes, generic records, nested variants, optional fields, and recursive trees
are outside this tranche. At least one owned leaf must be reachable from the
success record.

Bounds cover the complete variant: depth at most 64, expanded visited fields at
most 4096, and at most 256 owned leaves. The variant occupies the first depth;
the success record starts at depth two. The four case fields count toward the
field budget. A Vec carrier counts as one owner; its existing independent
capacity and content bounds still apply. Repeated record occurrences count
separately. No generated-name, cache, or graph marker grants admission.

## Ownership and checked behavior

Ordinary source constructors, `own` arguments/results, borrowed inspection, and
owning/borrowed matches carry this shape. Argument and field evaluation stay
left to right. A success payload transfers only after its construction succeeds;
partial record owners settle through the canonical cleanup plan. Owned calls
stage all arguments and commit them together. Later argument failure leaves
staged owners with the caller. Callee/postcondition failure cannot publish an
owned result and cannot replace the first selected status during cleanup.

Finite collection-outcome matches may yield another independently admitted
collection outcome in own mode, or one admitted Copy scalar/flat Copy record.
The scrutinee and result each rederive their ordinary carrier classifiers.
Arm type and ownership must agree; borrowed matches cannot publish an owner.
This does not admit arbitrary owning variant results or use generated codec
names as authority. Both source paths, retained HIR, independent cleanup replay
and backends authenticate the same closed join.

The cleanup inventory and independent cleanup plan retain the complete
`case / payload field / record field ... / owned leaf` identity path. Borrowed
payload bindings alias the active case's storage and cannot move or clone its
owners. Their existing record projection loans retain the payload binding's
lifetime. Moves before those loans end remain compile-time errors.

Native and Wasm layouts use the ordinary target-specific AggregateLayout for the
success record, not a Vec-width placeholder. Native moves the inert Copy shell
and commits each physical owner through the existing canonical leaf slots.
Wasm authenticates the tag before selecting the payload, recursively moves the
active record, and clears the moved storage. Interpreter values use the same
independently authenticated type closure. Inactive payload bytes grant no owner.

## Explicit command profile and replay

The additive private command profile is
`language-command-io.nested-outcome.v1` (Project schema v32). It retains all v31
private helpers and adds the authenticated owning outcome and its record helper
signatures. Nested Copy helper records use the same independent explicit closure
walk and pass by value; cached Copy facts alone do not admit them. Selected entry/command roots remain `fn() -> i64`; process effects,
module permits, manifest grants, provider quotas, and public ABI stay unchanged.
Older command and public export profiles reject the new carrier, including unused
functions before source closure cropping and body-local constructions behind
scalar signatures. The existing pure `owned-data-api.v1` internal-library route
with an empty authenticated export selection admits ordinary nested outcomes; it
produces no public descriptor or package. Source, declaration, cleanup and retained
HIR validation still apply. A nonempty export selection retains the frozen public
refusal, even when all nested-outcome helpers are unused. Direct native command
adapters perform the same negative and independent positive validation.

Graph v74 projects `semaprax.owned-nested-outcomes.v1` with `authority:false`.
Source revision, case/field ordering, explicit identity provenance, exact types,
cleanup, and loans are independently replayed after cache decode. Older graph
and evidence routes cannot mask this behavior. The cache gains no new authority
or opaque executable type: it stores ordinary existing record/variant HIR.

## Owning gates

- `--lib -- nested_outcome` covers canonical source and Graph v74 round-trips,
  source drift, cache replay, forged provenance and case types, complete
  cleanup paths, target-sized record payloads, source and HIR structural
  limits, Project v32 command execution, and older profile refusals.
- `--test owned_data owned_nested_outcome::` exercises identical source on the
  interpreter, native C11 at O0/O2, and strict Core Wasm. It checks success and
  error, borrow and move, empty and full Vec capacity, live and released
  projected loans, staged failures, and repeated owner settlement.
- `--test project standard_library::application_json::nested_request::`
  exercises independent order/configuration and scheduler decoders, including
  detached owners, typed malformed and capacity errors, and pure decoder
  admission through the frozen export and older command boundaries.

Full arbitrary nested JSON, optional/nullable schemas, user-defined variants,
and new collection element layouts remain separate obligations of #724.
