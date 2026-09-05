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
- uses only direct `Bytes`, `i64`, or `bool` arguments;
- has, after exact owner-and-index substitution, at least one direct `Bytes`
  field; and
- has, after substitution, only direct `Bytes` or admitted Copy-scalar fields.

The substituted instance is flat. Nested generic arguments or storage,
`String`, arrays, slices, classes, variants, resources, generic functions,
record updates, Project exports, FFI, packages, Components, and public
aggregate ABIs remain closed. `Option<Bytes>` and the separately admitted
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

## Proof surfaces and lowering

This slice reuses the already-versioned identities it composes:

- Graph v12 represents explicit concrete generic-record identity and exact
  ordered arguments; Graph v21 or the later selected additive graph represents
  explicit owned/borrowed matching without erasing that identity.
- Cleanup Inventory v1 and CleanupPlan v5 represent the flat projected byte
  leaves, construction state, transfers, child-region settlement, and
  finalizers. This tranche adds no cleanup vocabulary or schema spelling.
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
- stable rejection of non-Copy, nested generic, class, variant, and two-owned-
  side `Result` shapes;
- reference-interpreter success and post-owner-creation failure settlement;
- separately optimized native C11 execution at `-O0` and `-O2`, with repeated
  entry and zero live allocations after success and failure; and
- structurally valid Node/Core-Wasm execution under a one-live-owner capacity,
  including repeated entry.

Focused evidence is necessary but does not promote generic ownership broadly.
Hosted execution, hostile plan mutation, exact/+1 aggregate bounds, all Copy
scalar substitutions, immutable update, nested storage, generic-function
composition, Project/public consumers, cross-platform ABI compatibility, and
distribution remain separate completion work.

## Nonclaims

This contract does not define a stable C, Rust, WIT, Component, Project, or
package representation. It does not admit generic variants, nested generic
ownership, resources, inferred type arguments, constraints, specialization,
mutable or escaping loans, concurrency, or production support. It is one
bounded internal composition step toward general ownership and public ABIs.
