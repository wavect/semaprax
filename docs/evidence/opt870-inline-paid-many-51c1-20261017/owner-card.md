# OPT870 paid successor composition (private source checkpoint)

Audience: compiler contributors and reviewers.

Base: `51c1dfc49962e046566ad21ec31f20c6b3a9bdae`.
Status: source-only, not compiled or runtime-qualified. No application gain is established.

Covers:

- `src/cleanup_plan/replay/factored/canonical_shared.rs:672`: both actual
  unit-closed and general weighted successor producers retain first/repeated One,
  acquire new receiving controls, and dispatch paid flat promotion/add/finish.
- `src/cleanup_plan/replay/factored/canonical_shared/paid_many.rs:5`: composed
  One/tree/paid-flat ephemeral owner; tree remains the legacy route.
- `src/cleanup_plan/replay/factored/canonical_shared/paid_many.rs:12`: real
  parameter, initializer, returned/received enum/Result/Vec/tuple/Option controls;
  no old bill is credited. Growth uses the unchanged physical `reserve_vec` owner.
- `src/cleanup_plan/replay/factored/canonical_shared/paid_many.rs:47`: paid sorted
  promotion publishes only after work, capacity and both tuple insertions.
- `src/cleanup_plan/replay/factored/canonical_shared/paid_many.rs:102`: paid
  bounded search, checked repeated weight, charged shifts and fallible growth.
- `src/cleanup_plan/replay/factored/canonical_shared/before_paid_many.rs:1`:
  complete immediate old Canonicalizer, not a view of the new target receiver.
- `src/cleanup_plan/replay/factored/canonical_shared/before_paid_many/compare.rs:1`:
  complete old actual HIR/plan comparison caller, selecting the frozen receiver.
- `src/cleanup_plan/replay/factored/canonical_shared/paid_many/source.rs:30`:
  exact whole-preimage/body restoration, terminal equality, all115 unchanged
  helper and original-test sources, plus both production prepayment boundaries.
- `src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs:55`:
  complete original/immediate/composed work and overflow; actual paid
  allocations/Drop; both receiving families; tuple/iterator layout; all promotion
  byte cuts; full actual comparison/trace; foreign/sticky custody; six-field
  first-refusal order.

The original `successor_targets.rs`, all older complete receivers, the
independent incoming `incoming_flat_targets.rs`, and all existing tests remain
byte-exact. Each of six new Rust files is included once in each of the three raw
inventories, with required=maximum=1. The original raw sources restore byte-exact
by removal of those additions. Explicit child paths are source-checked; only an
actual later compile can establish registration.

No Cargo, rustc, Rust test, Clippy, Graft CLI, provider write, root mutation,
cap increase or full gate was run for this checkpoint. Rustfmt parsed the seven
changed/new Rust subjects without typechecking. The external source-only receipt
and exact ten pending lib selectors are under
`/Volumes/Semaprax/provenance/opt870-inline-paid-many-51c1-20261017/`.

Extra helper and search charges, geometric spare capacity and sorted shifts can
be unfavorable. Later qualification must report actual old/current charged,
allocated, peak and work tuples through final Drop, and retain the existing
CLI/ShiftSim/normalizer owners and unchanged ceilings. The independent incoming
helper tests are retained controls, not evidence that this producer has shipped.
