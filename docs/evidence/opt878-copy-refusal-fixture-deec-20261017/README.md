# Actual878 mixed-refusal fixture correction

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only, no compiler/Cargo/test execution. Actualdeec owning-selected-28.log failed at SkeletonWork::new before exercising the suffix-copy frontier because the fixture had exhausted its physical reservation before the real constructor. Both old/current cases still use the original1<<24 cap, skeleton_remaining=0, original merge flag, and original new-row/control prepayment. The constructor now runs immediately after that prepayment and before deliberate exhaustion. Its complete unchanged original fees remain charged on this exact ledger; no fee is removed/reset/refunded or borrowed from another budget.

The same deliberate reserve(limit-charged) now consults work.budget AFTER constructor charges, and a new assertion verifies that charged exactly equals limit. Thus operation-start available physical bytes are still exactly zero, while the real constructor has honestly completed. The ample-byte case executes precisely the same setup as before. Both complete independent original/current operations, copy-before-work messages, workzero assertion and complete six-field comparison are unchanged. Fresh inputs and actual old/new producer/Drop scopes remain as before. No production source, prices, cap, new selector or harness changes.

Complete immediate-before/current source and the one exact inverse are retained. Parent must acquire the new testfile before unchanged historical source domains in the later batch; no source generation or ROOT mutation occurs here. The private checkout contains this one selected source file with other baseline paths skip-worktree.
