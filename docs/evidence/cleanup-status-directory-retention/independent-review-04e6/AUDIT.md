# Bounded retained status-directory source review

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Subject04e6fa2db, implementation64e2babd7. Source-only; no Cargo/runtime or broader approval.

Concrete gaps sent to ROOT and constructor author before qualification:

1. StatusSources binds the address of ReplayBudget, but ActiveBuilder is an enum tag rather than a captured Rc<Budget>. reserve_bytes(0) authenticates current ACTIVE. Keeping the same ReplayBudget while entering a foreign nested owner is therefore not rejected by the new token. Current production driver is straight-line with no scope switch found; this is an exact invocation custody/test gap, not a demonstrated current-driver bypass. Requested actual captured owner plus foreign/restored controls.
2. The token adds function/budget pointers and an Option<Directory> carrier without explicit StatusSources initializer/Result acquisition. Directory::new reserves zero, Vec growth retains its existing allocation payments. Requested identify exact enclosing receipt or precharge actual new simultaneous carrier layouts, including the proposed owner capture.

Verified bounded comparisons:

- The complete block/edge body, first edge through final Ok, differs from the complete old body only in two &statuses to statuses borrow projections. Every actual membership probe and remaining directory operation retains its charge.
- Actual removed work is the second identical immutable status-directory construction/insertion; there are no quota, work-unit, ceiling or refund edits in this source change.
- Full function lifetime is immutably borrowed in the token. Current ordinary caller preserves status, regions, entry, block/edge, exits order. No plan status row gains discovery authority.
- Retained directory backing remains owned through later passes and drops naturally on success/error. The new paired microphase test observes status+block receivers to balance; it does not itself include the intervening production regions/entry phases or prove the full owner17 succeeds.
- Existing unobserved work counter does not universally become sticky after checked_sub exhaustion; new foreign/work test checks only eventual Err and remaining0. Receiver propagation and ordinary active refusal checks remain the relevant boundary. No work-policy change is proposed here.

A separate bounded tool-disabled Claude process reviews the fixed pre-fix source packet. Its raw receipt/result must be interpreted independently; no retry after timeout/denial.
