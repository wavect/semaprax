# Concrete Generic Owned-Byte Records v1

Audience: language, HIR, cleanup, interpreter, native, Wasm, and evidence
maintainers.

Status: locally exercised internal implementation tranche; hosted promotion is
not claimed.

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

Nonconcrete arguments, `String`, arrays, slices, classes, variants, resources,
unbounded or cyclic nesting, Project exports, FFI, packages, Components, and
public aggregate ABIs remain closed. `Option<Bytes>` and the separately admitted
one-owned-side `Result` profiles keep their compiler-owned rules;
`Result<Bytes, Bytes>` remains rejected.

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
  capacity, including one-too-small rejection and repeated entry.

Focused evidence is necessary but does not promote generic ownership broadly.
Hosted execution, the broader nested destructuring/update/loan corpus,
generic-function composition beyond the flat relay, Project/public consumers,
cross-platform ABI compatibility, and distribution remain separate completion
work. The focused
local source/HIR/layout and interpreter/native/Wasm gates exercise the complete
Copy-scalar set; no hosted execution is claimed until the required Linux CI
step records a real run.

## Project integration prerequisite

An owned-data Project may retain an explicitly imported flat generic record
template when its fields are only its own type parameters, direct `Bytes`, or
the admitted Copy scalars. The completely linked Project must still validate
every concrete use through the ordinary HIR, ownership, cleanup, native, and
Wasm gates. A frozen Project v8 scalar public export may call through that
internal closure, but its descriptor exposes only the already-admitted scalar
signature. Project v9 and v11 descriptors continue to reject a selected
generic result and no existing descriptor, carrier, or package schema widens.

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

This contract does not define a stable C, Rust, WIT, Component, Project, or
package representation. It does not admit generic variants, nonconcrete or
cyclic generic storage, resources, inferred type arguments, constraints,
specialization, mutable or escaping loans, concurrency, or production support.
It is one bounded internal composition step toward general ownership and public
ABIs.
