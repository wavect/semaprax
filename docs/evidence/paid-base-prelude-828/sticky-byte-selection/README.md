# Concrete first-byte selection correction

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Actual IdentityReservation source exposes an edge missed by the initial counter: wrong scope returns Detached, then a later zero request in restored scope returns Budget(first underlying receipt). Relying on callback requery therefore preserves the ledger receipt but changes the selected typed E. ROOT and #827 confirmed the actual invocation E is Copy IdentityReservation::Refused and approved retaining the exact first byte/work error inline.

The fixed-schema adapter alone now requires E:Copy and owns Option<FixedFailure<E>>. It copies only the inline first payload on retries, with no Clone bound/implementation, dynamic allocation, callback requery, lossy conversion or fabricated byte error. The actual enlarged Self/Result layouts are paid by the unchanged Layout calculation. Ordinary generic identity/materialization callbacks remain unconstrained. #827's invocation holder separately keeps its typed first failure and cannot reacquire a failed invocation.

A real nested/restored IdentityReservation control proves the original callback taxonomy actually changes while the fixed counter keeps Detached across byte/work zero/larger retries. The unrelated inner account remains unused/unfailed; the original outer first requested/remaining/floor/stage receipt stays intact. The isolated retry observer sees zero allocation. Existing complete constructor/refusal/final Drop and paid public work-report controls remain unchanged; their derived actual setup layout automatically covers the enlarged state.

Fourteen future exact selectors, no Cargo/build/runtime or new review. Parent prelude_bundle.rs and new invocation.rs are untouched here, avoiding #827 overlap. Prior counter/control/contract source and exact inverses are retained. No cap/P change or success/application claim follows.
