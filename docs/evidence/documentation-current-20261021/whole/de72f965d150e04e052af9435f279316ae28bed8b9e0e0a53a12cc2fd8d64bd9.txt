# OPT843 actual miss control fixture repair

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The original initial closure query visits roots[3] and roots[0] by epsilon;
roots[0] reaches sink by a movement, which closure deliberately does not follow.
The actual frozen3d4b owner therefore failed its premature sink-cache assertion.
The complete original test source and actual failure log remain archived here.

Retain the initial exact one-producer/multiple-memo control. Explicitly query the
still-cold sink and require one real producer, its exact singleton closure, then
an allocation-free one-work cache retry with unchanged count. After both original
comparison roots, independently query the disconnected LoopLatch root cold and
require the same producer increment/exact value/warm-retry controls. The existing
eight cached duplicate-root queries, same slot address across both roots, full
allocation/debit/Drop observations and all other old-reference/fee checks remain.
No producer implementation or fee changed. The whole test still measures all
query construction and final Drop in one active observation.

The full immediate source inverse restores the prior private7c10b8eae stage.
Parent assembly must join the new test source before older immutable views.
The owning exact selector is already selected at index241; no selector is added.
No Cargo, build, tests or Clippy ran. Source validation does not establish PASS.
