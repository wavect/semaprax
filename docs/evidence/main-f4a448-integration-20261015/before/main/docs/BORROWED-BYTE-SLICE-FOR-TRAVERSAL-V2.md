# Borrowed Byte Slice For Traversal v2

Audience: compiler contributors, tool authors, and reviewers.

Status: private source implementation for #780, executable qualification pending. No build, runtime, application-fit or backend-support claim is made by this authored contract. V1 remains the complete independent prior receiver.

## Narrow additive profile

A named immutable `Slice<u8>` byte traversal may contain another byte traversal whose view binding already exists at the OUTERMOST byte traversal entry. Both may name the same view or different whole/subrange aliases of one root. Each item remains a fresh immutable Copy `u8`, with unchanged left-to-right evaluation, one length snapshot per loop entry, ascending unit counters and ordinary checked `byte_get` result reception. No byte buffer or owned view is copied.

Every enclosing source-for must itself be an admitted immutable byte traversal. Computed and mutable iterable expressions, a view first declared inside the outer body, mixed source Vec/row/owned iterator nesting, generic-instance traversal, item shadowing/reassignment and closure admission remain closed under their existing diagnostics and contracts. An ordinary `while` preserves byte ancestry; it does not create byte admission outside a byte region. Function and closure-body checks start with their own scope. This first profile does not close all of #780.

These source admission limits do not impose a blanket Vec/Iter-operation or manual-While ban on a byte body. Existing independent collection, ownership, row, owned-iterator and lexical-exit validators keep their full restrictions. HIR proves actual lowered semantics and does not infer surface syntax from a marker or cached Boolean.

The resolver independently locates the exact authored while body under its
enclosing byte-only traversal ancestry before allowing a while bridge. An
ordinary while outside that ancestry cannot create it, including when the
caller passes parsed AST directly without running source verification first.
Closure bodies start a separate ancestry, and mixed traversals stay closed.

## Independent contextual replay

A private non-Clone, non-serialized census borrows one immutable function and its exact current replay context. It indexes exact expression references and complete IDs; a same-ID clone, foreign function/context or stale decoded graph is not a current member. Current HIR must be freshly replayed after decode.

Context identity compares the held object's data address, independently of
trait-object vtable duplication across compiler codegen units. Function and
expression membership still require their exact held references.

Local shape replay authenticates every region's snapshot, view, receiver, counter, item, discarded authored body and optional BorrowBytes latch. This is structural evidence only. Before an early loan-plan receiver borrows a region, a complete function census independently proves outer-entry definition availability, exact receiver association and every enclosing freeze over the ENTIRE inner subtree. It never skips an inner body or exempts its lowered statements from an outer freeze. Hidden receiver payloads cannot escape into any authored body. Duplicate expression/binding identities fail closed.

Late HIR validation runs after ordinary type/scope/ownership checks and fresh byte-provenance derivation. It adds each authenticated ultimate root to the same complete ancestor freeze proof. The loan planner independently resolves aliases and creates a distinct `TraversalFreeze` loan per region, ending after that exact region. Ending an inner same-root loan must preserve the outer loan, including on a back edge and after inner Break. Each lexical Break/Continue still targets its exact nearest current loop; Continue executes that loop's own latch once. Return, failure selection, postconditions and cleanup remain unchanged.

Contextless `region()` remains the complete strict v1 receiver and cannot lend nested authority. Actual ordinary/paid/retained loan-plan builders and lexical contour validation retain one paid slot. It lazily acquires one function census only when the existing complete v1 region check selects its nested-receiver refusal, then lends exact matching region references. This adds no separate selector cursor pass. Static v1 diagnostic text preserves all six fields and avoids allocating/discarding an owned temporary message on this newly admitted path. Native/cleanup standalone wrapper lookups remain guarded by the owning-iterator latch; they grant no byte authority. No new source origin descriptor, graph tag, wire layout, ABI, intrinsic or host authority is introduced. Current Graph v83 and checked-HIR v11 retain complete replay obligations.

## Storage and refusal

The slot, result and retained controls are separately priced, including ordinary functions that hold an empty slot; there is no exemption or unchanged-byte-debit claim. Private node/definition/receiver/ancestor/freeze lookup rows have exact admitted counts. Their Vec backing, control/receiving carriers and reusable iterative cursor are reserved before heap acquisition on the actual captured active ledger. Private sorting touches lookup rows only, never canonical HIR or cleanup vectors. Node and binding search are logarithmic; ancestor interval prefix maxima preserve all outer same-root freezes after inner exit. There is no new language depth quota or new budget.

The captured ledger rejects foreign/nested owners, preserves its first refusal after restoration and requires a zero authentication before reuse. Partial/current owners hold only borrowed references and fixed Copy rows; disposal allocates no memory. Allocation refusal never falls back to the old receiver or formats an owned diagnostic. Existing logical loan-work units, limits, caps and forecasts receive no discount or new allowance.

With no ACTIVE owner the ordinary public semantic API still performs complete replay, but its allocations are explicitly unqualified; it is not described as prepaid. No fake payer, cached admission flag or allowance is substituted. The new physical controls qualify the explicit same-owner census route only, not every public producer or whole application.

## Required executable owners

The new source and HIR unit children cover exact old/current nonnested replay, complete v1 nested refusal, source projection/graph/cache round-trip, same-root loans, every ancestor binding, foreign/restored owners, exact/short/zero/every-acquisition refusal and full same-observer Drop. Nearest-target drift and unrelated ordinary Vec accumulation remain independent checks. The useful-data child compares nested traversal with a separately authored manual loop and Rust Cartesian-product oracle on interpreter, native C11 O0/O2 and Core-Wasm, including empty/singleton/same-root views and sticky body failure. Existing v1 hostile HIR, lexical exits, stale-root, subrange, mutation, cache and public API owners must also run unchanged.

All these are authored gates until an exact-source execution receipt records their results. #780 remains open for residual source profiles and full qualification.
