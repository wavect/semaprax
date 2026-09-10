# RFC 0002: Algebraic data, matching, and aggregate ownership

Audience: language users, tool authors, and compiler contributors.

Status: partially implemented at the full RFC scope. Admitted v0.4.0 profiles have
**HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).

This RFC defines the next useful-core tranche: nominal records, algebraic variants, `Option`, `Result`, exhaustive matching, and ownership of aggregate places. It deliberately introduces a resolved semantic layer before new syntax reaches either backend.

## Current implementation baseline

The released implementation includes scalar and owned records and variants,
checked matching, compiler-owned Option/Result and admitted owned propagation,
concrete generic ownership, explicit nonidentity forwarding, bounded argument
inference through v3, nested record reconstruction, multiple record owners,
private generic authored variants and compiler collections. Their exact scope
is owned by [Generic Owned Result](GENERIC-OWNED-RESULT-V1.md),
[Explicit Forwarding](GENERIC-EXPLICIT-FORWARDING-V1.md),
[Argument Inference v3](GENERIC-ARGUMENT-INFERENCE-V3.md),
[Record Composition v2](GENERIC-OWNED-RECORD-COMPOSITION-V2.md),
[Multi-Owner Records](GENERIC-MULTI-OWNER-RECORDS-V1.md),
[Authored Variants](GENERIC-AUTHORED-VARIANTS-V1.md), and
[Compiler Collections](GENERIC-COMPILER-COLLECTIONS-V1.md).

All admitted release regressions are HOSTED GREEN. Constraints, broader
inference, general residual conversions, nested/resource payloads beyond the
owning profiles, general lifetime rules and a public generic ABI remain open.
An additive generic/cleanup/graph version does not widen a predecessor's frozen
contract. Public package and component support remains separately scoped.

## Historical profile and evidence records

The following records retain the original narrower profile boundaries and
execution subjects. Their mentions of local evidence or future tranches are
historical; use the current baseline above and staged implementation below for
current work. Numeric graph/cleanup versions and known-answer bytes continue to
refer to their original profiles, not to the complete v0.4.0 implementation.

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
This authored `record Box<T>` spelling remains inline nominal storage. The
separately selected compiler-owned `core.box` from
[Owned Bounded Box v1](OWNED-BOUNDED-BOX-V1.md) is a distinct non-Copy logical
allocation admitted only through its three explicit intrinsics over eight Copy
scalars. Prelude v4 selection prevents the two meanings from being mixed while
leaving authored-Box programs that use no compiler Box intrinsic unchanged.
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
The additive [Concrete Generic Owned-Byte Records
v1](CONCRETE-GENERIC-OWNED-BYTE-RECORDS-V1.md) composes explicit generic-record
identity with that flat ownership model. Exact instances such as `Box<Bytes>`
and `Pair<Bytes, u8>` substitute every direct Copy scalar (`i64`, `i32`, `u8`,
`usize`, `char`, `f32`, `f64`, and `bool`) before HIR facts, cleanup, layout,
matching, and execution. Local interpreter, native C11 `-O0`/`-O2`, and
Node/Core-Wasm evidence covers success, repeated entry, borrowing, owned
destructuring, post-construction failure settlement, and hostile-plan replay.
Explicitly instantiated generic functions additionally admit one exact owning
flat-record parameter/result template and all eight Copy-scalar substitutions;
local interpreter, native C11 and Core-Wasm evidence covers exact instance
dispatch, reentry, and postcondition-failure settlement. The additive narrow
flat expression-composition tranche preserves that one-owner,
identical-result boundary while admitting Copy-field projection, top-level
immutable update, `match borrow` returning a bound Copy field, and `match own`
reconstructing the same owner. All eight Copy substitutions are exercised;
update and reconstruction failures settle exactly, hostile HIR/backend mutation
replay fails closed, and repeated interpreter, native C11 `-O0`/`-O2`, and
Core-Wasm execution adds
no aggregate `memory.copy` relative to the direct-relay baseline. This does not
admit variants, nested expression-result composition, standalone constructors,
consuming projections, a public generic ABI, or Graph/schema widening. The
focused hostility evidence mutates authenticated HIR/backend facts; other
source shapes remain closed without a complete negative-source gate. The
additive bounded
nested relay admits one `own` parameter and an identical result over any
bounded acyclic authored-record template tree when every type argument is an
explicit member of those eight Copy scalars. The focused corpus exercises
`Box<Pair<Bytes, T>>` and `Pair<Box<Bytes>, T>` as representative shapes. It
retains Graph v14 and CleanupPlan v7 while locally exercising every scalar in
source/HIR and the representative `bool`/`i64` instances through success plus
failure settlement on the interpreter, native C11 `-O0`/`-O2`, and Core-Wasm.
An additive forwarding rule permits a generic template in either the
direct-scalar profile or that one-owner-identical-result relay profile to call
another already-admitted generic template directly. The callee type arguments
must be exactly the caller-owned parameter vector in declaration order. The
acyclic template-call graph derives one deterministic transitive instance
closure of at most 256 entries for each concrete caller vector; cycles, remapping,
permutation, omissions, duplicates, missing instances, and cross-profile calls
fail closed. Graph v14 and CleanupPlan v2/v5/v7 retain their existing identities
and meanings. This is not inference, constraints, richer construction,
projection, matching, variants, resources, effects, or public generic ABI.
Real cross-file
Project linking retains the internal concrete identity without widening frozen
public descriptors. A bounded additive storage profile admits fully concrete
acyclic record trees such as `Box<Pair<Bytes, bool>>` and
`Pair<Box<Bytes>, i64>` with one global nested-record budget and independently
derived recursive cleanup/layout facts. Nonconcrete or cyclic storage, generic
classes and variants, resources, and public aggregate ABIs remain closed. The
pre-nested-relay generic-owned corpus is hosted green in [run 34031917437,
Ubuntu job
101482963175](https://github.com/wavect/semaprax/actions/runs/34031917437/job/101482963175);
the additive nested-relay evidence remains local until its own pushed run.
The ScalarV1 scalar-export classifier additionally permits the exact reachable
internal flat generic-owned-record composition regardless of source provenance.
Every callable and selected Web export retains its frozen value-scalar
signature; no record, template, instance or owner crosses either boundary. The
focused package fixture reaches this general profile through an authenticated
Subject-v3 dependency exporting exactly `fn() -> i64`. Report-v2, Subject-v3,
manifest, Project, public descriptor and Wasm schemas remain frozen. This
evidence is local and unhosted; general generic package signatures and public
aggregate ABIs remain closed.
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
Generic-function inference/constraints, construction or matching inside a
template, other aggregate/resource/non-Copy signatures, nested/resource/
non-Copy record arguments or fields,
refutable/literal/guard/or/rest patterns, nested variant patterns,
ownership-aware or non-copy propagation/matching beyond the exact bounded
one-owner relay in Concrete Generic Owned-Byte Records v1, residual conversion,
`?` in
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
`967b0c4aff16ad55042a1c856079e0b387558909fa857fcf3aabc9fc53f54283`,
`f53a0c21638b5a360faa19ad4fdef68f6d861a5baffe39422847128686e82bef`,
`741ff89e1ee67d1426ae12e576bc7c4187e065913baf5dea5507901bfa16f8db`,
and `1cc90d12b135b79a64b14b04d5c1d9851767bcebfbc4311a2b0fe679c017897d`.
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
- Match expressions cannot generally produce nominal aggregates. The only
  admitted exception is the authenticated flat generic-owned relay above,
  where one `match own` arm reconstructs the identical owner. Other arms that
  yield a record, variant, `Option`, or `Result` are rejected at source
  verification with `SPX-T258`; construct the aggregate with `if`, or extract
  scalars in the arms.

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
while preserving legacy schema selection and cleanup bytes. Graph v32/v33
compose those unprojected/projected loan facts with the complete owned-variant
Graph v22 conditional cleanup contract so neither base schema masks the other.
This proof foundation does
admit the implemented
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

The additive [Acyclic Nested Owned-Record Immutable Update
v1](NESTED-OWNED-RECORD-UPDATE-V1.md) admits top-level `base with { ... }`
reconstruction only for the same bounded record tree. CleanupPlan v9 and Graph
v30/v31 retain exact stable field paths, left-to-right replacement completion,
replaced-old settlement and one atomic result commit. Dotted update, in-place
mutation, variants, generics, resources and public ABIs remain closed.

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
- `SPX-T226` generic-function expression, effect, unsupported or cyclic call
  chain, or recursion outside the bounded slice.
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

These are implementation stages, not promises tied to a future package version.
Each released bounded implementation has hosted-green regression evidence;
full RFC completion still requires the broader gates stated here.

1. **Resolved semantics:** nominal types, HIR, type facts, place paths and
   deterministic layout keys are implemented. Independent source/HIR replay
   remains mandatory before backend admission.
2. **Records:** construction, projection, immutable update, exact recursive
   Copy/owned destructuring, nested owned records and concrete generic records
   are implemented under their versioned profiles. The released generic
   [record composition v2](GENERIC-OWNED-RECORD-COMPOSITION-V2.md) also covers
   nested expression results, reconstruction into explicitly declared shapes,
   match/call/branch composition and alias update. [Multiple owning parameters](GENERIC-MULTI-OWNER-RECORDS-V1.md)
   retain exact nominal typing and transfer each owner once. The ScalarV1
   dependency fixture retains scalar-only cross-package signatures; it does
   not provide a public aggregate ABI. Broader resource/cyclic shapes, general
   ownership-sensitive patterns and public generic package signatures remain open.
3. **Copy variants:** unit/direct-scalar payloads, exhaustive matching,
   case identities, deterministic Native64/Wasm32 layout and admitted
   native/Core-Wasm execution are implemented. Older cleanup v2 and graph
   contracts retain their original meaning.
4. **Generic owned variants:** the admitted direct Bytes/Copy payloads,
   exact owned cases and [private generic authored-variant functions](GENERIC-AUTHORED-VARIANTS-V1.md)
   support checked construction, owning/borrowing matches, reconstruction,
   calls and branches within their explicit substitution limits. Broader
   nested/resource and multi-case composition remains separate; this does not
   open public variant signatures.
5. **Option and Result:** authenticated compiler-owned prelude identities and
   their bounded Copy/owned-byte carriers are implemented. The current
   [Generic Owned Result](GENERIC-OWNED-RESULT-V1.md) includes
   `Result<Bytes, E>` for eight Copy error substitutions or Bytes, and
   `Result<T, Bytes>` for eight Copy success substitutions. Broader payloads,
   public ABI and general component mappings remain separately gated.
6. **Propagation:** Copy Result/Option and admitted owned Result `?` execute
   once, preserve the selected case, and share postconditions and the sticky
   failure/cleanup epilogue. Mixed Copy/Bytes and generic-function propagation
   are implemented, not future first steps. Cleanup v2/v3/v6 remain selected
   by the owning profile. General residual conversion, unrelated live-owner
   composition beyond admission, resources and public callable/FFI signatures
   remain separate. The exact historical run/Component records above retain
   their original subject and cannot be reused as a new target observation.
7. **Generic functions:** explicit bounded instantiation, exact instance
   closure, internal ownership, nonidentity [forwarding](GENERIC-EXPLICIT-FORWARDING-V1.md),
   argument [inference v1–v3](GENERIC-ARGUMENT-INFERENCE-V3.md), private
   collection and callable/closure composition are implemented. Identity-only
   programs keep the applicable earlier graph bytes; nonidentity mappings
   select their additive graph contract. Source and HIR independently check
   every admitted substitution, not just reachable instances. Constraints,
   broader evidence expressions, owning closure captures, general composition
   and public generic ABI remain open.
8. **Semantic transactions and review:** bounded member/case/nominal rename,
   declaration and field changes, signature mappings, expression/contract
   changes, candidate replay/recovery, rebase/merge and context/impact queries
   are implemented under their owning specifications. The candidate
   [ABI Delta](PROJECT-CANDIDATE-ABI-DELTA-V1.md) is a structural comparison,
   not a compatibility guarantee. General semantic conflict resolution,
   external consumer migration and complete repository-wide behavioral
   equivalence remain future requirements.

Compiler-owned [Vec v1/v2](OWNED-BOUNDED-VEC-V2.md),
[Box v1/v2](OWNED-BOUNDED-BOX-V2.md), [owning iterators](OWNING-ITERATORS-V1.md),
[owned iterator payloads](OWNING-ITERATOR-PAYLOADS-V2.md),
[function values](FUNCTION-VALUES-V2.md), [scalar snapshot closures](CLOSURES-V2.md)
and [generic iterator operations](GENERIC-ITERATOR-OPERATIONS-V1.md) are
implemented additions. They preserve independent prelude, graph, cleanup,
package-alias and public-export restrictions instead of implicitly changing
this RFC's older profiles. Broader iterator interfaces, owned captures,
regions/arenas and general allocation still require their own implementation.

## Completion evidence

Required evidence includes canonical round trips, malformed grammar diagnostics, field/case construction errors, recursive layout rejection, deterministic missing witnesses, unreachable arms, aggregate partial moves, owned and borrowed matches, `Option<Resource>`, `?` evaluation-once, early-return postconditions, stable layout snapshots, exact graph fixtures, atomic member/case renames, native/Wasm equivalence for nested algebraic values, and cross-platform CI.

The completion-matrix rows remain Partial or Missing until their entire gates—not merely declaration parsing—are proven.
