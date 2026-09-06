# Concrete Generic Owned-Byte Records v1

Audience: language, HIR, cleanup, interpreter, native, Wasm, and evidence
maintainers.

Status: locally exercised internal implementation tranche. The pre-nested-relay
generic-owned corpus is hosted green in [CI run 34031917437, Ubuntu job
101482963175](https://github.com/wavect/semaprax/actions/runs/34031917437/job/101482963175).
The additive nested-relay and identity-forwarding selectors are hosted green
in [CI run 34048713967, Ubuntu job
101528399406](https://github.com/wavect/semaprax/actions/runs/34048713967/job/101528399406).

## Purpose and boundary

This contract composes the existing explicit generic-record identity and
substitution rules with the flat Owned Byte Record v1 ownership model. It
admits concrete instances such as `Box<Bytes>` and `Pair<Bytes, bool>` without
making a type parameter itself an executable owned carrier.

An admitted instance:

- names an authored `record`, never a class, variant, or resource;
- supplies exactly one concrete argument per declared type parameter;
- uses only direct `Bytes`, a Copy scalar (`i64`, `i32`, `u8`, `usize`,
  `char`, `f32`, `f64`, or `bool`), or another fully concrete admitted record
  as an argument;
- has, after recursive exact owner-and-index substitution, at least one
  transitive `Bytes` field; and
- has, after substitution, only `Bytes`, admitted Copy-scalar, or admitted
  concrete-record fields.

The initial substituted instance is flat. The additive nested-storage profile
also admits a fully concrete acyclic record tree such as
`Box<Pair<Bytes, bool>>` or `Pair<Box<Bytes>, i64>`. Every reachable nominal
node must be an authored record, every parameter is substituted by exact
owner-and-index before examining descendants, and the final leaves remain only
direct `Bytes` or the admitted Copy scalars. One global worklist enforces the
existing nested-record bounds of 64 record levels, 256 owned leaves, and 4,096
visited fields; recursive classifier calls may not reset any bound.

The additive owning generic-function relay is narrower than general nested
generic composition. It admits exactly one `own` parameter whose type is
identical to the return type and whose shape is any bounded acyclic authored
record-template tree under the same 64-level, 256-owned-leaf, and 4,096-field
work limits. Every explicit type argument must be one of the eight Copy
scalars. The focused corpus exercises `Box<Pair<Bytes, T>>` and
`Pair<Box<Bytes>, T>` as representative opposite nestings. The call transfers
the one aggregate owner; it does not copy, split, borrow, project, or expose
that owner. Exact template identity, owner/index-stable parameters, concrete
instance identity, recursive field paths, and parameter/result ownership are
re-derived independently by source verification and HIR validation.

The additive forwarding profile permits one already-admitted generic template
to call another such template directly. The callee's explicit type arguments
must be exactly the caller-owned type-parameter vector in declaration order:
this tranche is identity forwarding, not substitution, permutation, omission,
duplication, or inference. Both caller and callee must independently satisfy
either the direct-scalar generic profile or the one-owner identical-result
relay above. Forwarding may continue through a deterministic transitive
instance closure of at most 256 entries, but the template-call graph must
remain acyclic. Each
materialized caller argument vector therefore derives one exact corresponding
callee instance vector and identity; no backend chooses or repairs instances.
The call adds no construction, projection, matching, update, variant, resource,
effect, or public-boundary authority.

The additive flat expression-composition profile remains within one owning
parameter and an identical generic-record result. After exact substitution it
admits a direct Copy-field projection, a top-level immutable update, a
`match borrow` whose result is one bound Copy field and whose loan ends before
the owner is reused, and a `match own` whose arm reconstructs the same owner.
The profile is exercised for all eight explicit Copy-scalar substitutions and
does not admit a second owner or a different aggregate result. Construction is
available only as the reconstruction expression inside that authenticated
owning relay; standalone constructors and consuming projections remain closed.
Other expression bodies and source shapes remain closed; this tranche's
hostility evidence mutates authenticated HIR and backend facts rather than
claiming a complete negative-source boundary.

Nonconcrete arguments, `String`, arrays, slices, classes, variants, resources,
unbounded or cyclic nesting, direct Project exports of generic records, FFI,
Components, and public aggregate ABIs remain closed. One exact cross-file
Project may execute an internal concrete generic record behind its unchanged
scalar-only Project-v8 descriptor and package boundary. `Option<Bytes>` and the
separately admitted one-owned-side `Result` profiles keep their compiler-owned rules;
The record profile does not authorize prelude carriers; the exact
`Result<Bytes, Bytes>` instance is admitted separately by
[Owned Byte Variant Algebra v1](OWNED-BYTE-VARIANT-ALGEBRA-V1.md).

Closed generic-argument shapes retain the existing `SPX-T223` or `SPX-T268`
diagnostics. Source verification and hostile-HIR validation independently
derive admission; a backend may not widen it from layout alone.

## Exact substituted ownership

Type-parameter identity remains the declaration owner plus parameter index.
Every use substitutes declaration fields against the complete concrete
argument vector before deriving type facts, constructor types, match bindings,
layout, cleanup shape, or runtime storage. Display names and native offsets are
not semantic identities.

For `Pair<Bytes, bool>`, a field declared as `left: T` is exactly `Bytes` and
receives one compiler-owned `core.bytes.drop` leaf. A field declared as
`right: U` is exactly `bool` and receives no cleanup leaf. Cleanup inventory
order remains authored field order. No downstream phase may sort, repair, or
reconstruct the generic substitution from target representation.

Construction evaluates initializers left to right. Failure settles only the
completed owned prefix in reverse completion order. Whole moves and owned
calls transfer all projected byte leaves at the existing atomic commit
boundary. `match own` transfers the substituted byte fields to exact owned arm
bindings; `match borrow` creates arm-scoped aliases and leaves the source owner
live. Failure selection and result publication retain the ordinary cleanup
contract.

Immutable update admits only an exact owned base of the same concrete record
instance. It evaluates the base and replacement initializers left to right,
substitutes every replaced or retained field against the complete argument
vector, and transfers the completed result only after all replacements
succeed. Failure before a constructor completes settles exactly its completed
owned prefix. Failure during update settles completed replacements and the
staged base exactly once; displaced and retained byte leaves follow the
authenticated child-region plan. The operation adds no mutation or authority.

## Proof surfaces and lowering

This slice reuses the already-versioned identities it composes:

- Graph v12 represents explicit concrete generic-record identity and exact
  ordered arguments; Graph v21 or the later selected additive graph represents
  explicit owned/borrowed matching without erasing that identity.
- Cleanup Inventory v1 represents flat projected byte leaves. Whole-owner
  flat relay plans retain v2; explicit flat record matching selects v5.
  Bounded nested concrete storage selects the existing CleanupPlan v7 and Graph
  v26 recursive field-path contracts. This tranche adds no cleanup vocabulary
  or schema spelling.
- Graph v34 adds concrete generic-function ownership to the retained earlier
  graph projection. Its `base_schema` identifies that projection; the explicit
  legacy renderer retains the earlier bytes for frozen consumers. Programs
  without concrete function instances retain their previous graph schema.
  This additive graph does not reinterpret CleanupPlan v2, v5, or v7.
  V34 also composes checked nested owner cleanup with authenticated ordinary
  local byte loans elsewhere in the same program. The earlier nested
  Graph-v26-v31 contracts retain their projected-loan-only rejection; when
  there is no admitted legacy projection, `base_schema` is v34 itself.
- Native64 and Wasm32 aggregate layouts substitute fields before computing
  offsets, sizes, alignments, digests, symbols, or carrier operations.

The interpreter stores record members under persistent field declaration IDs.
Native C11 and Core Wasm move each owned byte carrier independently and poison
the source carrier. A shallow owning aggregate copy, `memcpy`, `memory.copy`,
or inferred clone is invalid. Layout and proof plans carry no host authority
and establish no ABI promise.

## Concrete generic-function graph closure

For each concrete instance, Graph v34 retains both its existing execution
identity and a revision-bound semantic identity derived from the template's
persistent declaration ID, complete ordered concrete argument vector, and
checked source revision. Arguments remain indexed by their template owner and
parameter index. Display names, discovery order, target offsets, and backend
symbols do not select an instance. The instance array has deterministic identity
presentation order; cleanup inventory and plan vectors retain their own
contracted order unchanged.

The additive `generic_instance_ownership` facts expose declared and substituted
parameter/result types, ownership modes, concrete record identity, cleanup
roots and ordered owned-leaf field paths. Each fact also resolves its checked
body and contracts through its execution identity, and includes effects, loan
relationships, generic call edges and their
owner/index forwarding maps, exact concrete callee vectors and identities,
cleanup inventory and plan digests, and the complete selected cleanup plan.
The enclosing semantic-program association records the checked source revision;
it is not an independently minted runtime root or publication authority.
ProgramRoot binds the exact Graph-v34 bytes in the semantic-program node's
additive v2 contract; its replay rejects a graph paired with a
different retained Project or ProgramRoot.

Cleanup schema selection runs from validated HIR before target lowering. Direct
scalar instances and whole-owner flat relays select CleanupPlan v2; explicit
flat record matching selects v5; bounded nested owned-byte relays select v7.
Selection is operation-sensitive: a flat layout alone does not imply v5. The canonical profile classifier
is invoked independently by plan construction/replay and graph derivation.
Graph derivation rejects disagreement with the retained plan. Interpreter,
native, and Wasm continue through ordinary HIR and cleanup validation; a
backend cannot repair or substitute the selected profile.

Submitted graph evidence is replayed against retained checked source and
compared as exact canonical bytes. A self-consistent remint does not grant
compiler authority. Changed instance identities, ordered arguments, ownership,
leaf paths, cleanup schema or plan, forwarding closure, or source association
must reject. Existing source/HIR rejection continues to bound cyclic and
first-over-limit instance closures before graph or target output.

The independent Linux CI job `GEN-05B generic instance semantic closure`
groups graph/schema replay, the full relay matrix,
flat expression composition and hostility, and the exact ScalarV1 dependency
fixture. A configured selector is not hosted evidence; promotion requires a
passing run on the exact commit. Existing public Project, package, C, C++, Rust,
WIT and Component signatures remain unchanged.

## Local evidence gate

The local gate requires:

- source verification plus resolved-HIR assertions for exact concrete type
  arguments, substituted match bindings, and byte/no-drop cleanup leaves;
- exact nested concrete instances `Box<Pair<Bytes, bool>>` and
  `Pair<Box<Bytes>, i64>`, with recursively substituted HIR/cleanup shapes,
  one global exact/+1 depth bound, and record-only rejection of nonconcrete,
  class, variant, resource, `String`, and cyclic arguments;
- every direct Copy-scalar substitution beside `Bytes`, with exact Native64
  and Wasm32 layouts and distinct instance/layout identities;
- recursively substituted Native64 and Wasm32 nested layouts, with distinct
  outer identities and rejection of forged child digests and carrier kinds;
- stable rejection of nonconcrete or cyclic nesting, class, variant, resource,
  non-Copy leaf, and two-owned-side `Result` shapes;
- independent cleanup-plan replay plus rejection of type-argument, liveness,
  and authored-field-order substitutions;
- reference-interpreter immutable-update success, partial-construction
  failure, and partial-update failure settlement;
- separately optimized native C11 execution at `-O0` and `-O2`, with repeated
  entry and zero live allocations after success and failure; and
- structurally valid Node/Core-Wasm execution under the exact required owner
  capacity, including one-too-small rejection and repeated entry; and
- one cross-file Project-v8 scalar closure retaining `Pair<Bytes, bool>` only
  internally, with repeated Project interpreter entry and test execution,
  generated native C11 execution at `-O0`/`-O2`, and repeated calls from an
  external Node consumer through the generated npm/Core-Wasm scalar API; and
- representative `Box<Pair<Bytes, T>>` and `Pair<Box<Bytes>, T>` one-owner
  relays with source/HIR identity and hostile-shape checks for all eight
  explicit Copy-scalar substitutions, plus one three-template forwarding
  chain for every scalar in each flat and nested shape (72 concrete instances).
  The runtime matrix executes all 24 shape/scalar combinations successfully
  and rejects each combination's contract failure, with repeated interpreter,
  native C11 `-O0`/`-O2` and Core-Wasm entry, zero live native allocations,
  and no extra Wasm aggregate copy. The representative `bool`/`i64` corpus
  additionally covers ensures and staged-call failure settlement; and
- one flat one-owner-identical-result expression-composition relay over all
  eight Copy scalars, covering Copy-field projection, top-level immutable
  update, borrow matching that returns a bound Copy field, and own matching
  that reconstructs the same owner; update and reconstruction failure
  settlement; hostile HIR/backend mutation replay; and repeated interpreter,
  native C11 `-O0`/`-O2`, and Core-Wasm execution, with no aggregate
  `memory.copy` added
  relative to the direct-relay baseline; and
- one three-template `Box<Pair<Bytes, T>>` forwarding chain over all eight Copy
  scalars, with exact instance IDs, nested parameter/result types, CleanupPlan-v7
  leaf paths, hostile instance/signature/inventory/liveness/call-argument replay,
  source permutation/duplication/omission and direct/indirect-cycle rejection,
  and an exact 256-instance closure plus first-over-bound rejection; and
- one exact ScalarV1 Project dependency whose authenticated Subject-v3 source
  exercises a three-template identity-forwarding chain over
  flat `Pair<Bytes, bool>` with CleanupPlan v5 while exactly one no-argument
  `i64` function crosses the package and public boundaries, with Project check, repeated entry/test,
  native C11 `-O0`/`-O2`, Core-Wasm and unchanged scalar Web-package evidence,
  plus public-selection and dependency-tamper rejection.

Focused evidence is necessary but does not promote generic ownership broadly.
Hosted execution, the broader nested destructuring/update/loan corpus,
generic-function composition beyond the exact bounded one-owner relay and its
flat expression-composition profile, direct
generic-record Project/public consumers, cross-platform ABI compatibility, and
distribution remain separate completion work. The focused
local source/HIR/layout and interpreter/native/Wasm gates exercise the complete
Copy-scalar set. The earlier corpus has the first hosted run identified above;
the additive nested relay and generic-forwarding selectors have the later
hosted Ubuntu result identified in the status. That focused evidence does not
promote broader generic ownership, public ABI, or cross-platform support.

## Project integration prerequisite

An owned-data Project may retain an explicitly imported flat generic record
template when its fields are only its own type parameters, direct `Bytes`, or
the admitted Copy scalars. The completely linked Project must still validate
every concrete use through the ordinary HIR, ownership, cleanup, native, and
Wasm gates. A frozen Project v8 scalar public export may call through that
internal closure, but its descriptor exposes only the already-admitted scalar
signature. Project v9 and v11 descriptors continue to reject a selected
generic result and no existing descriptor, carrier, or package schema widens.
The focused Project product gate executes this exact internal closure through
repeated retained-Project interpreter entry and tests, generated native C11 at
`-O0`/`-O2`, and a generated npm/Core-Wasm package called by an external Node
consumer. Every observable parameter and result remains scalar or the existing
borrowed byte-slice input; neither the descriptor nor the consumer sees the
generic record identity, fields, layout, or owner.

Separately, the ScalarV1 scalar-export classifier now admits the exact reachable
internal flat generic-owned-record composition regardless of source provenance.
Every callable and public export still has a frozen value-scalar signature; the
generic record declaration, template, concrete instance, fields, layout and
owner remain internal to the body. The focused cross-package fixture reaches
that general profile through an exact local Subject-v3 dependency and exposes
only one no-argument `i64` function. Exact Report-v2 and Subject-v3 replay,
coordinate/target resolution and held-source authentication happen before
ordinary linking. This adds no `use type` edge, generic package signature,
public descriptor field, or new package/report/Wasm schema. Focused evidence
for this additive path is local and unhosted.

A sound public generic-owned revision still requires all of the following:

- a new versioned Project descriptor and carrier rather than reinterpretation
  of v8, v9, or v11 bytes;
- an exact public type grammar binding template identity, ordered concrete
  arguments, substituted fields, target-neutral ownership, limits, and replay;
- candidate ABI-delta rows that select the public generic signature and retain
  those ordered arguments through mutation, recovery, and independent replay;
- generated external-consumer mappings with bounded allocation, failure
  settlement, hostile-input rejection, and byte-exact metadata replay; and
- hosted native and Wasm consumer evidence plus explicit cross-platform ABI
  policy before any stable C, Rust, WIT, Component, or package claim.

## Nonclaims

This contract does not define a stable generic C, Rust, WIT, Component,
Project, or package representation. In particular, it does not add Project
v14 or widen the frozen Project-v8/v9/v11 descriptors. It does not admit
generic variants, nonconcrete or cyclic generic storage, resources, inferred
type arguments, constraints, non-identity type-argument forwarding,
specialization, mutable or escaping loans, concurrency, or production support.
It does not add nested-nonflat templates with multiple owning parameters or a
non-identical owning result, out-of-bound or cyclic template trees, or a public
generic ABI. Generic variants, nested expression-result composition,
standalone constructors, consuming projections, and cleanup or public
descriptor schema widening remain closed or unclaimed. Graph v34 is an additive
internal projection; it grants no broader language or public ABI admission. The legacy flat
generic-function admission outside this exact additive profile is unchanged. The
ScalarV1 internal-body path carries only value-scalar calls and does not expose
or serialize the generic record; its dependency fixture specifically uses
`fn() -> i64`. This is one bounded internal composition step
toward general ownership and public ABIs.

Frozen patch, repair and review graph-delta contracts retain their legacy
projection because their normalization rules predate revision-bound instance
identities. Target and package evidence digests and workspace source-schema
metadata likewise retain the frozen projection while checking complete HIR.
Ordinary graph and query output uses v34; linked generic closures are bound by
[SemanticProgram v2](CANONICAL-SEMANTIC-WORKSPACE-REVISION-V1.md#additive-semantic-program-node-v2-generic-instance-closure).
Legacy rendering does not admit additional source shapes or grant execution
authority. A future semantic-delta contract for the new instance facts requires
its own versioned evidence.
