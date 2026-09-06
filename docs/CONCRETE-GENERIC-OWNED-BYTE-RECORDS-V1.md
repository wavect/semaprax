# Concrete Generic Owned-Byte Records v1

Audience: language, HIR, cleanup, interpreter, native, Wasm, and evidence
maintainers.

Status: locally exercised internal implementation tranche. The pre-nested-relay
generic-owned corpus is hosted green in [CI run 34031917437, Ubuntu job
101482963175](https://github.com/wavect/semaprax/actions/runs/34031917437/job/101482963175).
The additive nested-relay and identity-forwarding selectors pass locally and
are wired into one named Linux CI step, but that step remains unhosted until a
pushed run records its result.

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
- Cleanup Inventory v1 and CleanupPlan v5 represent flat projected byte leaves;
  bounded nested concrete storage selects the existing CleanupPlan v7 and Graph
  v26 recursive field-path contracts. This tranche adds no cleanup vocabulary
  or schema spelling.
- The bounded nested owning relay retains program-wide Graph v14 generic
  template/instance identity and the existing CleanupPlan v7 recursive
  field-path meaning. It changes neither schema nor serialized vocabulary.
- Native64 and Wasm32 aggregate layouts substitute fields before computing
  offsets, sizes, alignments, digests, symbols, or carrier operations.

The interpreter stores record members under persistent field declaration IDs.
Native C11 and Core Wasm move each owned byte carrier independently and poison
the source carrier. A shallow owning aggregate copy, `memcpy`, `memory.copy`,
or inferred clone is invalid. Layout and proof plans carry no host authority
and establish no ABI promise.

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
  explicit Copy-scalar substitutions, plus `bool`/`i64` representative success
  and requires/ensures/staged-call failure settlement on the interpreter,
  native C11 `-O0`/`-O2`, and Core-Wasm; and
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
generic-function composition beyond the exact bounded one-owner relay, direct
generic-record Project/public consumers, cross-platform ABI compatibility, and
distribution remain separate completion work. The focused
local source/HIR/layout and interpreter/native/Wasm gates exercise the complete
Copy-scalar set. The earlier corpus has the hosted run identified above; that
result predates and does not promote the additive nested relay, whose required
named Linux CI step has not yet recorded its own real run. The
generic-forwarding addition is part of that same authored but unrun step and
likewise remains local and unhosted.

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
generic ABI. The legacy flat generic-function admission is unchanged. The
ScalarV1 internal-body path carries only value-scalar calls and does not expose
or serialize the generic record; its dependency fixture specifically uses
`fn() -> i64`. This is one bounded internal composition step
toward general ownership and public ABIs.
