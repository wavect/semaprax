# Loan-plan entry stack phases

Status: private implementation; hosted 128 KiB constructor and full quality qualification pending.

Audience: compiler contributors and reviewers.

The independent diagnostic run 38962850637 built main 65601d26a and captured the
original indexed-match case stopping on SIGSEGV in the lexical presence scan,
inside `loan_plan::build_plan` during validator construction. It did not reach
`constructor complete`. This repeats the earlier b4523cc7c failure after the
borrowed cursor became smaller; the cursor reduction alone is insufficient.
A successful diagnostic job records the capture, not a passing test.

The public loan-plan entry keeps the original presence scan first. Presence,
optional owner/lexical validation, and subsequent candidate/CFG construction now
have explicit non-inlined call boundaries. The entry no longer combines later
planner scratch with the earlier scan in one optimizable function body. No heap
spill, allocation, debit, work/depth bound, fixture, stack size or diagnostic is
introduced by this refactoring. The original candidate/CFG body and guidance
mapping remain verbatim. Lexical owner validation still precedes all candidate
work, and its errors still bypass the later guidance mapping.

The complete original producer stays compiled in the existing loan harness.
The owning differential gate compares full plans and diagnostics, allocation
observations through Drop, complete refusal receipts and debits on scalar,
owned-root and lexical fixtures at unlimited, zero, exact and short limits. A
missing lexical owner retains its stable diagnostic. This compares both routes;
it does not claim to repair unrelated planner funding defects.

Focused static-std controls execute both complete producer bodies, the new
helpers and the actual diagnostic/guidance implementation across all 24
selection/failure combinations. Explicit HIR/plan and callee adapters exclude
real planning, heap/cost parity and 128 KiB admission from those local claims.
Whole-source inverses bind the complete original production and test sources;
the earlier cursor/presence custody keeps every original archive and hash.

The original 512-level indexed-match case, unchanged 128 KiB thread, full HIR,
backend behavior and every current-head CI/full quality gate remain required.
