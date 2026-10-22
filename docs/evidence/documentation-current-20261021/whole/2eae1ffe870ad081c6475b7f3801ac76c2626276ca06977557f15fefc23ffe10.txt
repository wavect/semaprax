# b5eca actual production compile correction

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Base b5eca55b96890e1a815f0d8d9e7b42c78d743b07. Source-only private correction;
no Cargo, Clippy, tests or ROOT changes. Actual prior core production Clippy
ended101 with12 diagnostics and zero runtime bodies; its complete log and SHA
are retained. This patch has not been compiled and claims no new PASS.

Three source corrections account for all12 reported diagnostics:

- paid_leaves' input names the actual crate::cleanup_plan::CleanupSlot rather
  than a nonexistent replay::CleanupSlot. With that type, matching the borrowed
  field yields the original borrowed flag/lifecycle bindings. No flag deref,
  lifecycle borrow, body, debit or ordering expression is otherwise changed.
  This addresses E0425 and its downstream E0614/E0308 inference diagnostics.
- Boundary admit's lexical local explicitly has the exact original closure
  parameter type &ExpressionId. The four loops supply &&ExpressionId; the old
  typed parameter coerced that to &ExpressionId. Restoring that same typed
  receiving local addresses eight E0277 comparisons, including the original
  body-site invocation which remains valid. No new reference or semantic
  comparison, Result, bank, work or authorization is introduced.
- The index macro reexport is cfg(test), retaining its real existing cross-
  module receiving_families owner while production continues to invoke the
  same local macro. No allow/unneeded suppression is added. This addresses
  the production unused-import diagnostic.

All three complete before-images and exact forward/inverse frames are stored.
Reversing them restores the whole original source, including every original
bank, test and control body. There are no new owners, selectors or binary roles.
Targeted pinned rustfmt --check and git diff --check pass on these source edits.

Integration needs one complete current->b5eca projection for these three source
inputs before inherited source guards; preserve all original hashes/tables and
apply the current successor front before any independent Main/input projection.
Canonical owns the combined source/authority seal. No large old proof replay,
new gate or qualification is implied by this finite correction.
