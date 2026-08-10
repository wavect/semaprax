# RFC 0002: Algebraic data, matching, and aggregate ownership

Status: partially implemented.

This RFC defines the next useful-core tranche: nominal records, algebraic variants, `Option`, `Result`, exhaustive matching, and ownership of aggregate places. It deliberately introduces a resolved semantic layer before new syntax reaches either backend.

The implemented record slice includes canonical declarations, construction,
projection, immutable update, persistent record/field identities, resolved HIR,
prefix-aware ownership, checked Native64/Wasm32 layouts, and independently
rebuilt and replayed cleanup plans. Its bounded public backend slice executes
nested `i64`/`bool` records through native C11/Clang at O0/O2 and browser Wasm
under Node; the empty product has frozen size and alignment one on both layout
profiles. The implemented copy-variant slice adds nominal variant templates
with explicit direct `i64`/`bool` arguments, monomorphic unit/direct-scalar
cases, explicit qualified construction, exhaustive copy-only `match`, scalar
`i64`/`bool` arm results, persistent template case/payload identities,
CleanupPlan v2 exact-scrutinee/stable-case branching and Copy-result staging, Graph v10/revision v2,
checked deterministic internal Native64/Wasm32 layout digest v2, and native C11
O0/O2 plus Node/Wasm execution. Compiler-owned ordinary `Option<T>` and
`Result<T, E>` use the same generic-variant machinery. Generic/prelude evidence
is hosted green in [run
31347109201](https://github.com/wavect/semaprax/actions/runs/31347109201).
The bounded postfix-`?` slice accepts only direct-`i64`/direct-`bool` Copy
carriers: `Result<T, E>` into `Result<U, E>` with exact `E`, and `Option<T>`
into `Option<U>`. It authenticates every compiler-owned carrier member,
evaluates the operand once, stages `Err` or payload-free `None` as a normal
outer result, skips later body expressions, and joins shared
postconditions/publication. Result evidence is hosted green in [run
31353051690](https://github.com/wavect/semaprax/actions/runs/31353051690).
Option evidence is hosted green through Native C11 O0/O2 and Node/Wasm for
both source/outer layout directions, physical-status separation, poison,
invalid tags, and re-entry in [run 31360176398, job 93367728277](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728277).
The bounded generic-record slice admits declarations such as `Box<T>` and
`Duo<T,U>` whose fields are direct scalars or parameters owned by that record,
with explicit direct `i64`/`bool` instantiation only. Template field identities
remain stable while concrete field types are substituted exactly. Full
record-ID-plus-ordered-argument identity keys HIR facts, deterministic
Native64/Wasm32 layouts/digests/caches, native symbols, and Graph v12. Cleanup
remains v2 and introduces no action because every admitted instance is Copy and
resource-free. Native C11 O0/O2 and Node/Wasm execute construction, projection,
immutable update, parameters/results, ordered multi-parameter substitution,
failure order, poison, and repeated entry. Graph v12 is program-wide and takes
precedence over v11 Option and v10 legacy output; older outputs remain
byte-compatible when no generic record is declared.
The bounded record-pattern slice adds irrefutable recursive destructuring for
resource-free Copy records. A record match has exactly one arm whose top-level
pattern is either `_` or the exact record constructor; constructor fields are
listed exactly once and may bind, ignore with `_`, bind an entire Copy-record
field by value, or recurse into another exact record pattern. The scrutinee is
evaluated once, bindings retain exact concrete instance and stable field
identity, the arm result remains scalar `i64`/`bool`, and CleanupPlan v2/v3
stays straight-line without new slots, transitions, status sources, or
variant-case edges. An authenticated explicit record pattern selects
program-wide Graph v13 above v12/v11/v10; a top-level wildcard alone does not.
Native C11 O0/O2 and Node/Wasm evidence covers recursive and whole-record
bindings, failure order, poison, and repeated entry; the Ubuntu gate is hosted
green in [run 31373317800, job
93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130).
Generic functions/inference, nested/resource/non-Copy record arguments or
fields, refutable/literal/guard/or/rest patterns, nested variant patterns,
ownership-aware or non-copy propagation/matching, residual conversion, `?` in
contracts, resource- or
record-bearing variant payloads, a stable public aggregate ABI, public
resource-bearing execution, and general aggregate execution remain outside
that evidence. A separate default-off Source-Result Component v4 maps
one exact effect-free closure using this bounded `Result`/`?` slice to WIT
`result<result<bool, bool>, status>`. That private fixture does not widen the
language slice, public aggregate ABI, callable/FFI signatures, or general
component mapping. Its Wasmtime execution is hosted green in [run 31356536123,
job 93357169796](https://github.com/wavect/semaprax/actions/runs/31356536123/job/93357169796).
A separate default-off Private Generic Record Component v7 freezes four exact
exports over `Duo<i64,bool>`, `Duo<bool,i64>`, `Phantom<i64>`, and
`Phantom<bool>` in WIT package `semaprax:private@0.5.0`, interface
`generic-records`, world `semaprax-private-v7`. It authenticates exact ordered
source instance identities, concrete layouts, Graph v12, component mappings,
and the distinction between the same-layout Phantom instances. Local
source-lock, hostile, Node/core, component, strict-quality, and independent
security gates are green. The isolated Rust 1.97.1/Wasmtime 47 typed runtime is
hosted green in [run 31373317800, job
93406924922](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406924922).
V7 does not establish general generic-record selection or mapping,
nested/resource/non-Copy records, imports/capabilities, public aggregate ABI,
browser/multi-engine conformance, or package/version negotiation; v1-v6 bytes
and known answers remain unchanged.

## Canonical source

Public types and members carry persistent identities. Authored generic variant
templates use the bounded syntax below:

```semaprax
@id("geometry.point")
record Point {
    @id("geometry.point.x")
    x: i64,

    @id("geometry.point.y")
    y: i64,
}

@id("geometry.choice")
variant Choice<T> {
    @id("geometry.choice.none")
    None,

    @id("geometry.choice.value")
    Value {
        @id("geometry.choice.value.value")
        value: T,
    },
}
```

Canonical expressions and patterns are explicit:

```semaprax
fn example(point: Point, value: Option<i64>) -> i64 {
    let moved = Point { x: point.x, y: 20 };
    let updated = moved with { y: 22 };
    match value {
        Option::Some { value } => updated.x + value,
        Option::None {} => updated.y,
    }
}
```

- Record construction is `Point { x: expression, y: expression }`.
- Immutable update is `point with { y: expression }`.
- Projection is `point.x`.
- Generic constructors carry explicit concrete arguments:
  `Choice<i64>::Value { value: expression }` and `Option<i64>::None {}`.
  Patterns use the checked scrutinee instance, for example
  `Option::Some { value }` and `Option::None {}`.
- `_` is a wildcard. The current bounded slice requires exact named payload
  fields, optionally written `field: binding`; bare nested bindings and `..`
  remain future work for variants.
- Record scrutinees additionally admit one irrefutable arm with an exact record
  constructor whose fields each contain a binding, `_`, or another exact
  record pattern. A field binding may bind the complete Copy-record field by
  value. Record guards, literals, or-patterns, rest patterns, and nested variant
  patterns remain future work.
- Future non-copy scrutinees require `match own`, `match borrow`, or
  `match shared`; the implemented plain `match` is copy-only.
- The bounded slice requires explicit generic constructor arguments rather than
  inference.

`Option<T>` and `Result<T, E>` are ordinary compiler-owned variants from the
authenticated versioned prelude, not hidden backend primitives. Their reserved
names and stable IDs cannot be redeclared by source. Only direct `i64`/`bool`
arguments are admitted today. Postfix `?` is implemented only for these
direct-scalar Copy instances: `Result<T, E>` into `Result<U, E>` with exact
`E`, and `Option<T>` into `Option<U>`. Result-only programs retain CleanupPlan
v2 and Graph v10. Option propagation uses CleanupPlan v3 only for affected
functions and Graph v11 for the entire containing program/context unless a
generic record declaration selects v12 or an explicit record pattern selects
v13. Residual
conversion, contract use, and non-copy, resource, or nested arguments remain
closed.

## Resolved semantics

The parsed AST is not a sufficient backend contract. Introduce resolved nominal types and HIR:

```text
TypeDeclaration
  stable_id
  name
  type_parameters
  kind = resource | record(fields) | variant(cases)

ResolvedType
  i64 | bool | parameter(index)
  nominal(declaration_stable_id, arguments)

HIR
  construct_record | construct_variant | project_field | update_record
  match(mode, scrutinee, arms) | try(operand, residual_type)
  place(root, projections)
```

Resolution attaches stable declaration/member IDs to every nominal reference. Generic instantiation identities derive from declaration IDs and resolved arguments, never display names.

Compute recursive type facts once for all consumers:

```text
copy | contains_resource | sized | needs_drop | layout_key
```

Reject by-value recursive cycles. Safe source layout remains abstract until an explicit versioned ABI annotation such as future `@layout(c)` is present.

## Ownership

Aggregate ownership tracks places rather than only variables:

```text
request
request.payload
result::<Ok>.value
```

Required rules:

- Constructing an aggregate consumes each non-copy field expression left-to-right.
- Moving a field invalidates that place and any parent operation requiring the complete aggregate.
- `match own value` consumes the scrutinee and yields owned non-copy payload bindings.
- `match borrow value` creates arm-scoped borrows and leaves the scrutinee available afterward.
- Borrowed/shared payloads cannot escape as owned.
- Every match arm joins through `Available`, `Moved`, and `MaybeMoved` place states.
- `with` evaluates and consumes a non-copy base first, evaluates replacements left-to-right, and transfers untouched fields.
- `?` evaluates once and routes success, residual return, postconditions, and cleanup through a unified epilogue.

The ownership model must support `Option<Resource>` and records containing
resources rather than create a second scalar-only ownership system. The current
public scalar-record lowering and private resource-record proof harness both
consume the same validated cleanup plan; the private harness does not open a
public resource ABI or admission gate.

## Exhaustiveness

Use a constructor-pattern matrix:

- variants have their declaration-ordered finite cases;
- `bool` has `true` and `false`;
- a record has one constructor with field children;
- `i64` is infinite and requires a wildcard/binding fallback after literal arms;
- nested patterns recursively specialize the matrix.

Diagnostics include a deterministic missing witness. Unreachable arms are rejected. The scrutinee is evaluated once, arms are selected top-to-bottom, and every reachable arm has the same result type and ownership mode.

## Backend representation

### Native bootstrap

C type names derive from stable IDs plus a deterministic collision suffix. Records use declaration-order fields. Variants use an explicit `uint32_t` tag and a union of case payload structs. Tags follow declaration order. Reordering a public field or case changes the interface/layout hash and is an ABI-breaking semantic change.

Do not niche-optimize initially. Explicit representation is easier to audit and keeps native/Wasm semantics aligned.

Executable Copy Variants implements this as compiler-internal Native64 and
Wasm32 profiles. Layout digest v2 authenticates the complete concrete nominal
instance plus template and substituted payload-field types; full-hash native
symbols keep distinct instantiations separate. Constructors evaluate authored
payload expressions first, zero the complete representation, write the selected
payload, and publish the tag last. Generated `_Static_assert`s bind size,
alignment, tag, payload, and field offsets. Aggregate parameters are internal
`const struct *` values and results are caller-owned; none of this freezes a
public C ABI.

### WebAssembly bootstrap

Keep scalars in the current direct ABI. Aggregates use caller-allocated stack-frame storage in linear memory:

- aggregate parameters are pointers;
- aggregate results use a caller-provided result pointer;
- a mutable shadow-stack pointer and compile-time frame layouts manage temporary storage;
- records use deterministic offsets;
- variants use an aligned tag plus maximum payload area;
- resources stored in aggregates remain integer handles;
- every exit restores the frame through the unified epilogue.

The current Wasm32 copy-variant profile uses a four-byte tag, target-specific
four-byte bool payload cells, an aligned maximum payload, and a one-byte empty
payload policy. Invalid tags select a private negative invariant sentinel,
restore the shadow stack, and trap at the public wrapper rather than becoming
a semantic `Result` or status. Real Node evidence covers repeated re-entry, but
does not establish browser/multi-engine or Component Model conformance.

The browser export `semaprax_main -> i64` remains stable during this tranche.

## Agent graph and transactions

Graph v13 retains the Graph-v9 owner/index-stable type-parameter nodes, exact concrete nominal
argument trees, compiler-owned identity provenance, and an authenticated
`semaprax.prelude.v1` contract to the persistent `variant`, `variant_case`, and
`case_field` declarations introduced in v8. Revision-scoped expression nodes
cover variant construction, match arms, variant/wildcard patterns, and payload
bindings; cleanup edges select stable template case IDs for one exact scrutinee
expression. It additionally serializes `try_result` with exact source/residual
instances, compiler-owned member IDs, one evaluation, normal-result Err exit,
and shared-postcondition epilogue meaning; CleanupPlan v2 serializes the exact
body or Try-residual Copy-result producer. Graph revision v2 length-delimits and hashes canonical source plus
the prelude schema/contract. Future propagation nodes/edges remain design rather
than implemented evidence.

Schema selection is program-wide: an authenticated explicit record pattern
selects v13; otherwise any generic record declaration selects v12; otherwise
Option propagation selects v11; otherwise legacy and Result-only programs use
v10. Agent Context v1 reports the same program-level source schema regardless
of root. V13 adds exact recursive record-pattern nodes carrying concrete record
instances, stable record/field IDs, canonical binding IDs, and authored field
order. A top-level wildcard record arm is binding-free and does not by itself
select v13. CleanupPlan remains v2, or v3 only when Option propagation is also
present; the pattern adds no branch edge or cleanup action.

Context traversal follows signature types, constructors, projections, cases, patterns, contracts, and calls.

Semantic renames preserve meaning:

- type renames update type uses, constructors, and pattern qualifiers;
- field renames update labels, projections, updates, and patterns;
- case renames update constructors and patterns;
- shorthand `{ value }` becomes `{ renamed_field: value }` if necessary to preserve the local binding;
- stale, colliding, or unverifiable changes leave every source byte unchanged.

Shape edits later carry match obligations and typed repairs; they are not textual insertion operations.

## Diagnostics

- `SPX-T212` unknown or duplicate field/case payload.
- `SPX-T213` missing required field.
- `SPX-T214` invalid projection.
- `SPX-T215` constructor/type mismatch.
- `SPX-T216` match-arm result mismatch.
- `SPX-T217` illegal by-value recursion.
- `SPX-T218` invalid `?` context.
- `SPX-T219` propagation residual mismatch.
- `SPX-M101` non-exhaustive match with missing witness.
- `SPX-M102` unreachable arm.
- `SPX-M103` incompatible pattern.
- `SPX-M104` duplicate or missing pattern field.
- `SPX-O108` move from borrowed/shared aggregate.
- `SPX-O109` use of partially moved place.
- `SPX-O110` place conditionally moved by another arm.
- `SPX-O111` non-copy match without an explicit ownership mode.

Existing diagnostic codes remain reserved; implementation must resolve any collision before landing.

## Staged implementation

1. Add resolved nominal types, HIR, type facts, place paths, and deterministic layout keys without changing source behavior. **Implemented.**
2. Add records through parser, formatter, resolver, verifier, Graph, transactions, C, and Wasm. **Frontend, Graph v7 record-update meaning, Graph v12 bounded generic-record identity, Graph v13 exact recursive Copy-record pattern meaning, deterministic target layouts, target-neutral cleanup, bounded public nested-scalar execution, explicitly instantiated direct-scalar generic Copy records, and irrefutable recursive Copy-record destructuring are implemented through C11 O0/O2 and Node/Wasm; the generic-record gate is hosted green in [run 31365363898, Ubuntu job 93383304995](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304995), and the record-pattern gate is hosted green in [run 31373317800, Ubuntu job 93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130). Transaction breadth, refutable/ownership-aware/non-Copy patterns, nested/resource/non-Copy generic records, resource-bearing public execution, a stable aggregate ABI, and general backend completion remain evidence-gated.**
3. Add bounded non-generic copy variants and exhaustive copy matching. **Implemented for unit/direct-`i64`/direct-`bool` payloads, scalar `i64`/`bool` arm results, CleanupPlan v2 variant-case replay, deterministic internal Native64/Wasm32 layouts, and native C11 O0/O2 plus Node/Wasm execution.**
4. Add generic variants, recursive-unsized rejection, and ownership-aware matching. **Partially implemented for nominal variant templates with explicit direct `i64`/`bool` arguments, exact substitution/instance identity, Graph v10, internal layout digest v2, cleanup-free copy matching, and native/Wasm execution. Nested/resource arguments and non-copy ownership modes remain open.**
5. Add ordinary prelude `Option` and `Result`. **Implemented for compiler-owned `semaprax.prelude.v1` variants under the same direct-`i64`/`bool`, copy-only, internal-ABI limits; component/FFI mappings remain open.**
6. Add `?` with evaluation-once and unified epilogues. **Implemented for ordinary compiler-owned direct-scalar Copy `Result<T, E>` to `Result<U, E>` and `Option<T>` to `Option<U>`. Result uses exact CleanupPlan v2 staging and Graph v10; Option uses authenticated payload-free-None CleanupPlan v3 staging and program-bound Graph v11 unless a generic record declaration selects program-wide v12 or an explicit record pattern selects v13. Native C11 O0/O2 plus Node/Wasm evidence covers both carriers; Result is hosted green in [run 31353051690](https://github.com/wavect/semaprax/actions/runs/31353051690), and Option is hosted green in [run 31360176398, job 93367728277](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728277). One private Source-Result Component v4 maps the exact `Result<i64, bool>` to `Result<bool, bool>` fixture to nested WIT result/status and is hosted green in [run 31356536123, job 93357169796](https://github.com/wavect/semaprax/actions/runs/31356536123/job/93357169796). Residual conversion, nested/resource/non-copy arguments, contracts, public ABI, general component mapping, and callable/FFI signatures remain open.**
7. Add member/case transactions, layout/interface hashes, and context traversal.

## Completion evidence

Required evidence includes canonical round trips, malformed grammar diagnostics, field/case construction errors, recursive layout rejection, deterministic missing witnesses, unreachable arms, aggregate partial moves, owned and borrowed matches, `Option<Resource>`, `?` evaluation-once, early-return postconditions, stable layout snapshots, exact graph fixtures, atomic member/case renames, native/Wasm equivalence for nested algebraic values, and cross-platform CI.

The completion-matrix rows remain Partial or Missing until their entire gates—not merely declaration parsing—are proven.
