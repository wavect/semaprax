# Acyclic nested owned-byte records v1

Status: internal implementation tranche; executable promotion evidence is not
yet claimed.

Audience: language, HIR, cleanup, interpreter, native, Wasm, and evidence
maintainers.

## Purpose and boundary

This contract extends the existing flat Owned Byte Record v1 semantics to one
bounded tree of monomorphic records. It closes the compiler-internal movement,
cleanup, and synchronous shared-loan path before any public aggregate ABI is
widened.

An admitted root record is acyclic by value, contains at least one transitive
owned `Bytes` leaf, and contains only:

- direct `Bytes` fields;
- direct admitted Copy scalar fields; or
- another admitted monomorphic record.

The maximum record depth is **64**, the maximum transitive owned-leaf count is
**256**, and the maximum fields examined while classifying one root is
**4,096**. Every addition and work charge is checked. Exact-bound inputs are
accepted; the first additional depth, owned leaf, or field is rejected before
HIR or target admission. Classification must use an explicit bounded worklist,
not an unbounded host-language recursion stack.

Generic records, variants, `String`, arrays, slices as stored fields, authored
resources, classes, recursive types, mutation/update, ownership-aware recursive
patterns, imports, FFI, Project exports, Components, and public aggregate ABIs
remain closed. The existing recursive-type diagnostic remains authoritative.
Shapes outside this closed profile retain `SPX-T268` rather than being
partially lowered.

## Stable places and type facts

Every transitive leaf is addressed by one root `ValueId` and the complete
declaration-ordered vector of stable field declaration IDs. Display names,
source offsets, target offsets, and generated symbol fragments never identify
ownership. Source verification and resolved-HIR validation independently
derive the same bounded shape and exact paths.

The root and every nested owning record are non-Copy and sized. Copy siblings
carry no cleanup slot. Every `Bytes` leaf receives one independent liveness
flag and lifecycle under its complete path. A record with all of its `Bytes`
leaves initialized is not necessarily a completed value: a later Copy
initializer may still fail.

## Construction, movement, and cleanup

Constructor expressions evaluate fields left to right in authored declaration
order. A failure cleans only the successfully completed owned-leaf prefix, in
the exact reverse of actual completion. It never infers whole-value completion
from leaf count.

A completed whole-record move:

1. authenticates the complete source shape and epoch;
2. transfers every live descendant in recursive declaration order;
3. clears every descendant source flag as one canonical transition; and
4. creates one destination epoch with the same exact field-ID paths.

No interpreter or backend may implement this with an owning clone, shallow
aggregate assignment, `memcpy`, or Wasm `memory.copy`. Finalization consumes
the exact reverse recursive declaration order. Prefix and descendant moves may
not duplicate, omit, sort, or repair flags.

Owned call arguments stage left to right. Pre-commit failure leaves every
caller owner live. One existing `CallCommit` transfers all staged owners
together; callee failure never reacquires them. An owned provisional result is
published only after postconditions and non-result cleanup. The first selected
failure remains sticky over every recursive cleanup action.

## Nested synchronous shared loans

`bytes_as_slice` may borrow a transitive `Bytes` leaf from an exact named owned
root through a nonempty stable field-ID path. The path must resolve entirely
through records admitted by this contract and terminate at exactly `Bytes`.
Constructor temporaries, borrowed roots, variant cases, substitutions, and
non-`Bytes` leaves remain rejected.

The Shared Loan Plan retains the complete owner place. A live loan blocks a
move, assignment, owned match, or transfer of the leaf, any owning ancestor, or
any descendant. A disjoint sibling remains independent. Direct aliases and
range reborrows preserve the same complete place and cannot outlive their
parent. Loans remain synchronous, immutable, non-escaping, and target-neutral;
they create no runtime owner or cleanup action.

## Versioned proof surfaces

Nested owned-record functions select `semaprax.cleanup-plan.v7`. The v7 plan
retains the existing inventory and transition vocabulary but admits complete
multi-field record paths and authenticates the bounded recursive shape,
construction history, whole transfer, call commit, cleanup order, and result
publication. The canonical builder and independent replay must derive the
shape separately from already validated HIR and compare every byte-significant
field exactly.

Programs requiring nested ownership select additive `semaprax.graph.v26`.
Programs also carrying a nested projected Shared Loan Plan select additive
`semaprax.graph.v27`. V27 is not interpreted as v26 or v24. Programs outside
this tranche retain their previous cleanup and Graph schema and exact bytes.
Any combination not explicitly representable by the selected schema fails
closed rather than masking an older ownership, variant, native-import, or loan
contract.

Serialized plans and graphs are proof data. They grant no execution, source,
filesystem, process, publication, or ABI authority.

## Runtime lowering

The reference interpreter, native C11 backend, and Core Wasm backend consume
only independently validated HIR and CleanupPlan v7. Each uses the complete
stable-ID path for owned storage, movement, and borrowed projection. Target
layout remains an implementation detail and cannot replace semantic paths.

Native C11 must produce identical outcomes and cleanup traces at `-O0` and
`-O2`. Core Wasm must validate structurally and execute under the existing
bounded owner-token model. An invalid path, missing carrier, flag disagreement,
or impossible shallow move is an invariant failure before payload access,
cleanup, or result publication; it is never normalized to an ordinary source
failure.

## Evidence gate

Promotion requires all of the following at one exact revision:

- canonical source round trips, stable field identities, and exact type facts;
- exact and plus-one depth, owned-leaf, and field-work boundaries;
- cycle and every excluded-shape rejection with stable diagnostics;
- hostile HIR mutation of root, type, field path, ownership, loan parent, and
  last-use edges;
- hostile CleanupPlan mutation of every leaf, epoch, construction prefix,
  transition, call commit, finalizer order, status source, and publication;
- failure injection after every initializer, including a trailing Copy field
  after all owned leaves, and before/after every call commit;
- branch, loop, whole binding, own parameter/result, callee failure,
  provisional result, nested view/reborrow, ancestor conflict, sibling
  independence, last-use release, and repeated-entry cases;
- identical normalized interpreter, native `-O0`/`-O2`, and Node/Core-Wasm
  behavior and cleanup traces;
- native ASan/UBSan evidence and tight Wasm owner-capacity exact/plus-one
  evidence; and
- byte-frozen legacy CleanupPlan v2-v6, Graph v1-v25, Project v1-v10, public
  package, ABI, and generated-consumer fixtures.

Source-layout assertions, compilation, a single backend, or a serialized plan
alone do not satisfy this gate. Until the complete executable corpus passes,
the affected completion-matrix rows remain Partial.

## Nonclaims

This contract does not provide recursive variants, generic owned aggregates,
mutable or escaping borrows, closure or task capture, public borrowed calls,
aggregate mutation, stable layout, a Project v11 profile, C/Rust/WIT/Component
ABI, allocator transfer, foreign ownership, concurrency, or production support
by itself. It is one bounded internal semantic and three-engine execution
tranche toward general ownership and lifetime safety.
