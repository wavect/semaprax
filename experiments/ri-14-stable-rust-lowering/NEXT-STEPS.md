# RI-14 closure gaps

This fixture is intentionally not an RI-14 completion claim.  A go/no-go ADR
requires all of the following executable evidence at one source revision:

1. A feature-gated, nondefault backend that consumes verified HIR and the
   compiler-produced canonical cleanup plan.  Handwritten source in this
   directory must be replaced by deterministic generated Rust and exact source
   digests.
2. The same bounded corpus must execute through interpreter, native C11, and
   Rust-source lowering, comparing value, selected status, effects, and cleanup
   traces.  The lexical-`Drop` control must fail that differential gate.
3. The admitted type/effect/ownership island must be explicit.  Borrowing,
   contracts, resources, async, unadmitted aggregates, and foreign owners need
   a deterministic rejection or an explicit existing boundary.
4. A real generic Rust library call plus callback must run from generated Rust,
   and its stable diagnostics must bind the generated source digest, target,
   toolchain identity, and selected HIR revision.
5. The experiment must measure wrapper count, type-check fidelity, diagnostics,
   build time, allocations, optimized-call overhead, C11 coupling, and
   maintenance cost against the RI-01 bridge.  Default targets and the stable
   C11 backend must remain unaffected while it is disabled.

Until then, this directory is a narrow stable-Rust source witness only.
