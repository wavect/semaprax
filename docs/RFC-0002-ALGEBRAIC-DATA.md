# RFC 0002: Algebraic data, matching, and aggregate ownership

Audience: language users, tool authors, and compiler contributors.

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
program-wide Graph v13 above v12/v11/v10 when no generic function declaration
selects v14; a top-level wildcard alone does not.
Native C11 O0/O2 and Node/Wasm evidence covers recursive and whole-record
bindings, failure order, poison, and repeated entry; the Ubuntu gate is hosted
green in [run 31373317800, job
93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130).
The additive [Owned Byte Record Algebra v1](OWNED-BYTE-RECORD-ALGEBRA-V1.md)
slice admits only flat monomorphic records with one or more direct `Bytes`
fields plus direct Copy scalars. `match own` transfers every projected owned
leaf into an exact binding; `match borrow` creates arm-scoped aliases without
changing source liveness. CleanupPlan v5 and Graph v21 authenticate the mode,
stable field identities, projected transfers, child-region settlement, and
finalizers. Local interpreter, native C11 `-O0`/`-O2`, and Node/Core-Wasm
evidence is green. Nested/generic/class/variant/resource-bearing shapes,
aggregate arm results, and every public aggregate ABI remain closed; hosted
promotion is not claimed.
The separate bounded generic-function slice admits one or two owner/index-
stable parameters with direct `i64`/`bool` or own-parameter by-value signature
slots and explicit direct-scalar call arguments. Unused templates are checked
over every `2^N` substitution without materialization; explicitly referenced
instances receive exact domain-separated HIR/native/Wasm identities and
program-wide Graph v14. CleanupPlan v2 stays byte/schema/meaning unchanged and
template-ID-only, with exact instance authentication in HIR and Graph. Local
C11 O0/O2 and 4,096-entry Node/Wasm evidence plus security review are green;
the hosted matrix is green in [run 31385406865, Ubuntu job
93445428338](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428338).
Generic-function inference/constraints or aggregate/resource/non-Copy
signatures, nested/resource/non-Copy record arguments or fields,
refutable/literal/guard/or/rest patterns, nested variant patterns,
ownership-aware or non-copy propagation/matching beyond the exact flat Owned
Byte Record v1 slice, residual conversion, `?` in
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

A separate default-off Private Record-Pattern Projection Component v8 freezes
four monomorphic preserve/invert exports over exact same-layout-distinct
`Phantom<i64>` and `Phantom<bool>` instances in WIT package
`semaprax:private@0.6.0`, interface `record-pattern-projections`, world
`semaprax-private-v8`. It authenticates exact source/core/layout/Graph-v13/
plan/profile/component identity and record-pattern projection behavior. Local
validation, hostility, Node/core, source-lock, strict, and security gates are
green; the pinned Rust 1.97.1/Wasmtime 47 hosted runtime is green in [run
31385406865, job
93445428268](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428268).
V1-v7
bytes remain unchanged, and v8 establishes neither generic-function component
support nor general source selection, record mapping, imports/capabilities,
public ABI, browser/multi-engine conformance, or package negotiation.

A separate default-off Private Generic-Function Instance Component v9 freezes
WIT package `semaprax:private@0.7.0`, interface
`generic-function-instances`, world `semaprax-private-v9`, the three phantom
Copy templates `preserve<T>`, `invert<T>`, and `ordered<T,U>`, and exactly six
ordered Graph-v14 `FunctionInstanceId` exports with identical scalar
`(bool,s64)->result<bool,status>` signatures. It introduces no authored record
or layout roots. Exact source/Graph/core/plan/profile/raw/DAG KATs are
`218085fb5ea1bcc090c04ac0acb3395912d0dad09027b9118d8817978b2fde0c`,
`62907c4b95495bb573b2b37de9f0b08c7a82218934154521e8c0c8396158cc6e`,
`9f178207a0406f740198ee8c71d5d008efdf4d995ff04e11e80ea73b79155d44`,
`edd11c98bbc902d9dbc9c942375477fcf1e6c3f1befbe3c4a9f260107104485e`,
`365897ddb2770cc25a11690dddbfef5d232244ec5d328c79a24a1410e684615e`,
`3cf6c7d7d02e838fb374478a2b5b25077c7c612ad36e30deaffd15311a25a688`,
and `2623ff9a7eda5526616a15befd4951de86874a59911dcba2a7d3bcc2d178a474`.
Local core 5/5, component 4/4, CI-lock 4/4, full, hostile, and security gates
are green; pinned Rust 1.97.1/Wasmtime 47 execution is hosted green in [run
31392541096, job
93467490492](https://github.com/wavect/semaprax/actions/runs/31392541096/job/93467490492).
V1-v8
bytes remain unchanged. V9 is exact private instance-selection evidence, not
general source selection/export, inference/constraints, aggregate/resource/
non-Copy mapping, imports/capabilities, public ABI, browser/multi-engine
conformance, or package negotiation.

A separate default-off Private Source-Option Propagation Component v10 freezes
WIT package `semaprax:private@0.8.0`, interface `option-propagation`, world
`semaprax-private-v10`, and the exact compiler-owned `Option<i64>` through
postfix-`?` to `Option<bool>` export
`evaluate(input: option<s64>, divisor: s64) -> result<option<bool>, status>`.
It introduces no authored types, resources, templates, instances, imports, or
capabilities. Exact source/Graph-v11/prelude/two-layout/CleanupPlan-v3/core/
profile/raw/DAG KATs are
`98b8fc892c183499153142d5bbdb4162e31bda95ef145d34dbb1ff57c9b8fc72`,
`96083f90fab18c919a96cee48109e606e089159e109869a42bdf48831743d45d`,
`d37bad7e3911669bbf2c66b25c8b31d5c2e36eb181cc54fdc86c3a49a8fb9c5e`,
`79194fc88011ac060877e60293d0a4272429dd9e2d720674d0d54e804562deda`,
`dec126293ece7ec0e48d3d85ccdb494f7c7cfe4c3d4a9b1a61b50f6f862ff038`,
`d07fa51fc6f192a43318140264fa0e5964933ed90bc065cc8c74708e258ff92f`,
`16d1d34024e3fad920d8d00a61d7cb3bd010335ca382f23615b3b3da4143aaec`,
`f53a0c21638b5a360faa19ad4fdef68f6d861a5baffe39422847128686e82bef`,
`f5770bdfdbc862ea39640b2c706c1d9ea171164c220d18366e25b3219443ad0d`,
and `90ab80260c84abfe85d1edc666ab3750b81388e6e4cffd7ca21c301b9d0ee589`.
Typed and raw evidence covers `Some`/`None`, contracts, checked arithmetic,
sticky failure, status-first/tag-last publication, full poison, invalid
input/output tags and booleans, unknown status, repeated/fresh instances, and
out-of-band fuel exhaustion. Local core 5/5, component 4/4, CI-lock 4/4, full,
hostile, and security gates are green; pinned Rust 1.97.1/Wasmtime 47 execution
is hosted green in [run 31396483313, job
93481068502](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068502).
V1-v9 bytes remain unchanged. V10 is exact private Option-propagation evidence,
not general source selection/export, general `Result`/`Option`/`?` or algebraic
Component mapping, nested/resource/non-Copy carriers, imports/capabilities,
callbacks/async, callable/FFI or public ABI, browser/multi-engine conformance,
package negotiation, or `SPX-B104`/`SPX-W111` widening.

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
- Generic functions use the same explicit projection:
  `fn id<T>(value: T) -> T { value }` and `id<i64>(value)`. The bounded function slice accepts one or
  two parameters and only direct `i64`/`bool` substitutions.

`Option<T>` and `Result<T, E>` are ordinary compiler-owned variants from the
authenticated versioned prelude, not hidden backend primitives. Their reserved
names and stable IDs cannot be redeclared by source. Only direct `i64`/`bool`
arguments are admitted today. Postfix `?` is implemented only for these
direct-scalar Copy instances: `Result<T, E>` into `Result<U, E>` with exact
`E`, and `Option<T>` into `Option<U>`. Result-only programs retain CleanupPlan
v2 and Graph v10. Option propagation uses CleanupPlan v3 only for affected
functions and Graph v11 for the entire containing program/context unless a
generic record declaration selects v12 or an explicit record pattern selects
v13, or any generic function declaration selects v14. Residual
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

The additive [Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md) gives the existing
bounded synchronous immutable-borrow slice dense resolved-function-local loan
identities, exact owner-place and parent-reborrow provenance, multiple shared
loans, path-sensitive last-use edges, deterministic bounds, and
independent replay. Graph v23 keeps its unprojected schema and fields; additive
Graph v24 carries the direct field's stable projection and authenticated type
while preserving legacy schema selection and cleanup bytes. This proof foundation does
admit the authored-but-unrun
[Projected Owned-Byte Field Shared Borrow v1](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md)
only for `bytes_as_slice` of one stable-ID `Bytes` field on a named `own` flat
record. Constructors, temporaries, deeper projections, variants, generics,
resources, escaping borrows, general
lifetime inference, mutable borrowing, or a public borrowed ABI; those remain
evidence-gated extensions of this RFC.

The additive [Acyclic Nested Owned-Byte Records
v1](NESTED-OWNED-BYTE-RECORDS-V1.md) authors the next closed internal profile:
bounded monomorphic record trees containing Copy scalars and transitive owned
`Bytes`, whole-value movement, CleanupPlan v7, Graph v26/v27, and synchronous
shared loans through complete stable field-ID paths. It does not admit recursive
owned patterns, variants, generics, mutation, resources, Project exports, or a
public aggregate/borrowed ABI. Its executable promotion gate remains separate.

The separately additive [Acyclic Nested Owned-Record Exact Destructuring
v1](NESTED-OWNED-RECORD-DESTRUCTURING-V1.md) admits exact recursive
`match own` and `match borrow` only for that bounded record-tree profile.
CleanupPlan v8 and Graph v28/v29 preserve complete stable field-ID paths and
keep variants, generics, mutation, resources, non-Copy arm results and public
ABIs closed.

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

Graph v14 retains the Graph-v9 owner/index-stable type-parameter nodes, exact concrete nominal
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

Schema selection is program-wide: any authenticated generic function
declaration selects v14, including an unused template; otherwise an explicit
record pattern selects v13, a generic record declaration selects v12, Option
propagation selects v11, and legacy/Result-only programs use v10. Agent Context
v1 and v2 report the same program-level source schema regardless of root. V14 adds
exact function-template, concrete-instance, and call-instance nodes without
fabricating an unused instance. V13 adds exact recursive record-pattern nodes carrying concrete record
instances, stable record/field IDs, canonical binding IDs, and authored field
order. A top-level wildcard record arm is binding-free and does not by itself
select v13. CleanupPlan remains v2, or v3 only when Option propagation is also
present; the pattern adds no branch edge or cleanup action.

The same-schema v14 serializer correction adds the missing array delimiters
around function-template `type_parameters`; earlier two-parameter templates
produced invalid JSON in module, bounded-context, and Agent Context views. The
corrected module/Agent Context/bounded-context SHA-256 KATs are
`7a61fa6229f2db7aca6a035fd961720e8a401c138cc66c9cd71c64d45bed5efd`,
`2841401e7ba85fa8e47b3c35a15ae401b4a271d2500d70bbf3627f1453869eb6`,
and `d7bda2be1fc366195ffb00a9e20b2b03204b4dd6f46e8019842dd84f70b54ab8`.
Independent JSON parsing and these exact bytes are hosted green in [run
31390043736, Ubuntu job
93459346296](https://github.com/wavect/semaprax/actions/runs/31390043736/job/93459346296).

Context traversal follows signature types, constructors, projections, cases, patterns, contracts, and calls.

Semantic renames preserve meaning:

- type renames update type uses, constructors, and pattern qualifiers;
- field renames update labels, projections, updates, and patterns;
- case renames update constructors and patterns;
- shorthand `{ value }` becomes `{ renamed_field: value }` if necessary to preserve the local binding;
- stale, colliding, or unverifiable changes leave every source byte unchanged.

Bounded `semaprax.semantic-patch.v2` makes persistent field/payload-member and
variant-case renames executable, expands shorthand without changing binding or
place identity, and admits exact addressed direct-scalar generic-call argument
replacement under one pre-state transaction and a selective post-HIR semantic
delta gate. Schema-less v1 remains exact. The focused suite is 9/9, and the
exact `f95d243` matrix is hosted green in [run 31401200449 attempt
2](https://github.com/wavect/semaprax/actions/runs/31401200449/attempts/2),
including [Ubuntu job
93505622044](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622044).
Graph remains v10-v14 and CleanupPlan remains v2/v3. Patch-file provenance,
shape edits, layout/interface hashes, general type/generic edits, and
multi-file repair remain open.

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
- `SPX-T224` invalid bounded generic-function declaration or signature.
- `SPX-T225` invalid generic-function invocation or reserved execution identity.
- `SPX-T226` generic-function expression, effect, call-chain, or recursion
  outside the bounded slice.
- `SPX-M101` non-exhaustive match with missing witness.
- `SPX-M102` unreachable arm.
- `SPX-M103` incompatible pattern.
- `SPX-M104` duplicate or missing pattern field.
- `SPX-O108` move from borrowed/shared aggregate.
- `SPX-O109` use of partially moved place.
- `SPX-O110` place conditionally moved by another arm.
- `SPX-O111` non-copy match without an explicit ownership mode.
- `SPX-O117` invalid explicit ownership match mode, scrutinee, or owned-field
  binding shape.
- `SPX-G106` duplicate, conflicting, no-op, or overlapping Patch v2 edit.
- `SPX-G107` Patch v2 wrong owner/kind/persistence domain or compiler-owned
  identity.
- `SPX-G108` Patch v2 stale generic-call tuple, source/HIR index mismatch, or
  excessive semantic delta.
- `SPX-G109` invalid Semantic Impact v1 bounds or an undersized mandatory
  report envelope.
- `SPX-G110` Semantic Impact v1 source-consumer, schema, call-owner, selector,
  or explicit-persistent reverse-closure invariant failure.

Existing diagnostic codes remain reserved; implementation must resolve any collision before landing.

## Staged implementation

1. Add resolved nominal types, HIR, type facts, place paths, and deterministic layout keys without changing source behavior. **Implemented.**
2. Add records through parser, formatter, resolver, verifier, Graph, transactions, C, and Wasm. **Frontend, Graph v7 record-update meaning, Graph v12 bounded generic-record identity, Graph v13 exact recursive Copy-record pattern meaning, deterministic target layouts, target-neutral cleanup, bounded public nested-scalar execution, explicitly instantiated direct-scalar generic Copy records, and irrefutable recursive Copy-record destructuring are implemented through C11 O0/O2 and Node/Wasm; the generic-record gate is hosted green in [run 31365363898, Ubuntu job 93383304995](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304995), and the record-pattern gate is hosted green in [run 31373317800, Ubuntu job 93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130). Transaction breadth, refutable/ownership-aware/non-Copy patterns, nested/resource/non-Copy generic records, resource-bearing public execution, a stable aggregate ABI, and general backend completion remain evidence-gated.**
3. Add bounded non-generic copy variants and exhaustive copy matching. **Implemented for unit/direct-`i64`/direct-`bool` payloads, scalar `i64`/`bool` arm results, CleanupPlan v2 variant-case replay, deterministic internal Native64/Wasm32 layouts, and native C11 O0/O2 plus Node/Wasm execution.**
4. Add generic variants, recursive-unsized rejection, and ownership-aware matching. **Partially implemented for nominal variant templates with explicit direct `i64`/`bool` arguments, exact substitution/instance identity, Graph v10, internal layout digest v2, cleanup-free copy matching, and native/Wasm execution. Nested/resource arguments and non-copy ownership modes remain open.**
5. Add ordinary prelude `Option` and `Result`. **Implemented for compiler-owned `semaprax.prelude.v1` variants under the same direct-`i64`/`bool`, copy-only, internal-ABI limits; component/FFI mappings remain open.**
6. Add `?` with evaluation-once and unified epilogues. **Implemented for ordinary compiler-owned direct-scalar Copy `Result<T, E>` to `Result<U, E>` and `Option<T>` to `Option<U>`. Result uses exact CleanupPlan v2 staging and Graph v10; Option uses authenticated payload-free-None CleanupPlan v3 staging and program-bound Graph v11 unless a generic record declaration selects v12, an explicit record pattern selects v13, or a generic function declaration selects v14. Native C11 O0/O2 plus Node/Wasm evidence covers both carriers; Result is hosted green in [run 31353051690](https://github.com/wavect/semaprax/actions/runs/31353051690), and Option is hosted green in [run 31360176398, job 93367728277](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728277). Private Source-Result Component v4 maps the exact `Result<i64, bool>` to `Result<bool, bool>` fixture and is hosted green in [run 31356536123, job 93357169796](https://github.com/wavect/semaprax/actions/runs/31356536123/job/93357169796). Private Source-Option Propagation Component v10 maps exactly `Option<i64>` through postfix `?` to `Option<bool>` and is hosted green in [run 31396483313, job 93481068502](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068502). Residual conversion, nested/resource/non-copy arguments, generic-function `?`, contracts, public ABI, general component mapping, and callable/FFI signatures remain open.**
7. Add bounded explicitly instantiated direct-scalar Copy generic functions.
   **Implemented across canonical source, source verification, resolved HIR,
   program-wide Graph v14, strict native C11 O0/O2, and Node/Wasm. Hosted
   matrix evidence is green in [run 31385406865, Ubuntu job
   93445428338](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428338);
   the separate exact private Component v9 profile has local source/Graph/core/
   plan/profile/raw/DAG evidence and hosted Wasmtime execution in [run
   31392541096, job
   93467490492](https://github.com/wavect/semaprax/actions/runs/31392541096/job/93467490492).
   Inference, constraints, richer signatures, generic composition,
   callable/resource admission, general/public Component mapping, and stable
   ABI remain open.**
8. Add member/case transactions, layout/interface hashes, and context traversal.
   **Partially implemented: bounded persistent member/case transactions and
   exact direct-scalar generic-call argument replacement are hosted green in
   [run 31401200449 attempt
   2](https://github.com/wavect/semaprax/actions/runs/31401200449/attempts/2),
   including [Ubuntu job
   93505622044](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622044),
   while additive Agent Context v2 provides bounded directional call traversal.
   Bounded read-only Semantic Impact v1 now reports exact source consumers and
   generic-call reverse callers for one patch, while layout/interface hashes,
   authenticated patch provenance, multi-file repair, and general
   repository-wide/non-call traversal and impact remain open.**

## Completion evidence

Required evidence includes canonical round trips, malformed grammar diagnostics, field/case construction errors, recursive layout rejection, deterministic missing witnesses, unreachable arms, aggregate partial moves, owned and borrowed matches, `Option<Resource>`, `?` evaluation-once, early-return postconditions, stable layout snapshots, exact graph fixtures, atomic member/case renames, native/Wasm equivalence for nested algebraic values, and cross-platform CI.

The completion-matrix rows remain Partial or Missing until their entire gates—not merely declaration parsing—are proven.
