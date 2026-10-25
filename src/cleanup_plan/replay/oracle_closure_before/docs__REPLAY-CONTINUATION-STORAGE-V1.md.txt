# Replay continuation storage v1

Status: source checkpoint; current-head owning execution qualification pending.

Audience: compiler contributors and reviewers.

Internal ordinary whole-program replay retains one empty continuation backing
for the exact immutable current Program. Acquisition remains lazy at the first
expression-skeleton entry after the original preceding checks. The existing
payer reserves the actual 516-element Frame layout before allocation. Returning
a lease clears all owned path, parameter and observation payloads. Function,
contract and expression observations remain independently rebuilt; backing is
neither proof nor a cached verdict.

A guarded variant re-enters the expression skeleton while its outer frames
remain live. That nested receiver keeps its original separate local allocation
and reservation. It cannot return into or replace the outer slot. On an error,
frame payloads are dropped before empty backing returns. The invocation stops
on its first error; physical refusal remains sticky. During unwinding the local
leased Vec drops and the storage remains unavailable until invocation Drop.

Standalone and selected receivers retain their original local backing route.
No work allowance, source admission, depth limit, physical cap or forecast is
changed. Removed requests correspond only to absent repeated top-level backing;
there are no refunds. Full compiler funding, application fit and runtime gains
remain unproved until their owning gates execute.

Source-authored lib owners are under
`cleanup_plan::replay::continuation_storage::tests`. They compare the saved old
local acquisition, complete paths/work/error outcomes, real guarded reentry,
whole replay and exact/one-short same-payer backing through Drop. The allocation
windows exclude checked source, active ledger setup and fixed observer metadata.
Qualification is pending.
