# RFC 0002: Algebraic data, matching, and aggregate ownership

Status: partially implemented.

This RFC defines the next useful-core tranche: nominal records, algebraic variants, `Option`, `Result`, exhaustive matching, and ownership of aggregate places. It deliberately introduces a resolved semantic layer before new syntax reaches either backend.

The implemented record slice includes canonical declarations, construction,
projection, immutable update, persistent record/field identities, resolved HIR,
prefix-aware ownership, checked Native64/Wasm32 layouts, and independently
rebuilt and replayed cleanup plans. Its bounded public backend slice executes
nested `i64`/`bool` records through native C11/Clang at O0/O2 and browser Wasm
under Node; the empty product has frozen size and alignment one on both layout
profiles. The implemented copy-variant slice adds non-generic nominal variants
with unit and direct `i64`/`bool` payload fields, explicit qualified
construction, exhaustive copy-only `match`, scalar `i64`/`bool` arm results,
persistent case/payload identities, CleanupPlan v1 stable-case branching,
Graph v8, checked deterministic internal Native64/Wasm32 layouts, and native
C11 O0/O2 plus Node/Wasm execution. Generics, `Option`, `Result`, `?`,
resource- or record-bearing variant payloads, non-copy match ownership modes, a
stable public aggregate ABI, public resource-bearing execution, and general
aggregate execution remain outside that evidence.

## Canonical source

Public types and members carry persistent identities. This generic `Option`
example is the intended full-language syntax, not part of the bounded
non-generic executable slice:

```semaprax
@id("geometry.point")
record Point {
    @id("geometry.point.x")
    x: i64,

    @id("geometry.point.y")
    y: i64,
}

@id("core.option")
variant Option<T> {
    @id("core.option.none")
    None,

    @id("core.option.some")
    Some {
        @id("core.option.some.value")
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
- Cases are qualified: `Option::Some { value: expression }` and `Option::None {}`.
- `_` is a wildcard. The current bounded slice requires exact named payload
  fields, optionally written `field: binding`; bare nested bindings and `..`
  remain future work.
- The current slice has no guards, or-patterns, or nested patterns.
- Future non-copy scrutinees require `match own`, `match borrow`, or
  `match shared`; the implemented plain `match` is copy-only.
- Ambiguous generic construction requires a type annotation.

`Option<T>` and `Result<T, E>` are ordinary versioned prelude variants, not hidden backend primitives. Only postfix `?` receives dedicated typed lowering.

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

Executable Copy Variants v1 implements this only as a compiler-internal
Native64 profile: constructors evaluate authored payload expressions first,
zero the complete representation, write the selected payload, and publish the
tag last. Generated `_Static_assert`s bind size, alignment, tag, payload, and
field offsets. Aggregate parameters are internal `const struct *` values and
results are caller-owned; none of this freezes a public C ABI.

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

Graph v8 adds persistent `variant`, `variant_case`, and `case_field` nodes to the existing record/resource declarations. Revision-scoped expression nodes cover variant construction, match arms, variant/wildcard patterns, and payload bindings; cleanup edges select stable case IDs for one scrutinee expression. Future generic and propagation nodes/edges remain design rather than implemented evidence.

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
2. Add records through parser, formatter, resolver, verifier, Graph, transactions, C, and Wasm. **Frontend, Graph v7 record-update meaning, deterministic target layouts, target-neutral cleanup, and bounded public nested-scalar C11/Wasm execution are implemented; transaction breadth, resource-bearing public execution, a stable aggregate ABI, and general backend completion remain evidence-gated.**
3. Add bounded non-generic copy variants and exhaustive copy matching. **Implemented for unit/direct-`i64`/direct-`bool` payloads, scalar `i64`/`bool` arm results, CleanupPlan v1 variant-case replay, Graph v8, deterministic internal Native64/Wasm32 layouts, and native C11 O0/O2 plus Node/Wasm execution.**
4. Add generic variants, recursive-unsized rejection, and ownership-aware matching. **Not implemented.**
5. Add ordinary prelude `Option` and `Result`.
6. Add `?` with evaluation-once and unified epilogues.
7. Add member/case transactions, layout/interface hashes, and context traversal.

## Completion evidence

Required evidence includes canonical round trips, malformed grammar diagnostics, field/case construction errors, recursive layout rejection, deterministic missing witnesses, unreachable arms, aggregate partial moves, owned and borrowed matches, `Option<Resource>`, `?` evaluation-once, early-return postconditions, stable layout snapshots, exact graph fixtures, atomic member/case renames, native/Wasm equivalence for nested algebraic values, and cross-platform CI.

The completion-matrix rows remain Partial or Missing until their entire gates—not merely declaration parsing—are proven.
