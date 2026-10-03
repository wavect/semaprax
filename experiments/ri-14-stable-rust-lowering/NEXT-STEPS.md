# RI-14 closure gaps

This fixture is intentionally not an RI-14 completion claim.  A go/no-go ADR
requires all of the following executable evidence at one source revision:

1. The feature-gated `stable_rust_lowering` seam validates real HIR for a
   parameter-free i64 literal and one owned-`Bytes` identity. The latter emits
   a physical `Option::take` for its two canonical transfers through a temporary.
   It still needs a deterministic generated Rust corpus beyond this plan, with a
   checked artifact digest bound to each executable fixture.
2. The same bounded corpus must execute through interpreter, native C11, and
   Rust-source lowering, comparing value, selected status, effects, and cleanup
   traces. The owned-`Bytes` identity selector compares the shared value across
   generated Rust `-C opt-level=0`/`3` and C11 `-O0`/`2`, proves the generated
   Rust transfer trace and lexical-`Drop` control, and refuses a forged version
   of that fixture's cleanup plan before source emission. Interpreter and C11
   still expose no cleanup trace to compare. The lexical-`Drop` control must
   fail a common-trace differential gate.
3. The admitted type/effect/ownership island must be explicit.  Borrowing,
   contracts, resources, async, unadmitted aggregates, and foreign owners need
   a deterministic rejection or an explicit existing boundary.
4. A real generic Rust library call plus callback now invokes the generated,
   checked-HIR owned-identity artifact in the focused selector. This proves
   source provenance at that boundary, but not generic Semaprax callback
   lowering. Stable diagnostics still need to bind the generated source digest,
   target, toolchain identity, and selected HIR revision.
5. The experiment must measure wrapper count, type-check fidelity, diagnostics,
   build time, allocations, optimized-call overhead, C11 coupling, and
   maintenance cost against the RI-01 bridge.  Default targets and the stable
   C11 backend must remain unaffected while it is disabled.

Until then, this directory is a narrow stable-Rust source witness only.
