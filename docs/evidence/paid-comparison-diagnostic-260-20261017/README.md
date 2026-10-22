# Owner260 diagnostic-only followup

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

No compiler/Cargo/test execution. The actual36 failure is opaque because two test-local branches erase Diagnostic to unit: physical setup Err and full comparison map_err. Neither production comparison loses Diagnostic. This patch records the exact still-live original error by borrowed Debug formatting (all six fields), plus arm/limit/stage and existing remaining/spent/sticky scalar state, into a fixed stack-only fmt sink. It emits that text only after measure() returns and all original in-window Drop/allocation assertions finish. No diagnostic clone/string allocation, payer call, replay or allowance occurs in the capture. A reporting-only16KiB capacity never becomes a compiler budget; any overflow is explicitly labeled incomplete and cannot replace the original failure.

Original Result-to-unit shape, unwrap and all acceptance/allocation/Drop/work assertions remain. Both complete production comparison bodies are unchanged, including independently compiled before_paid_many comparator. Every original test/assertion count is unchanged. Full previous testfile and exact five edits+helper inverse recover the completeab869 source. No production fee, baseline, cap, retry, Selected/Oracle dispatch or compilation source table is changed. No pass/fit/failure-operation claim is inferred; next actual owner260 execution must expose its original Diagnostic.

This is a separate successor commit after parkedab869, for a later consolidated batch only. Parent must authenticate the new complete testfile before the unchanged historical source views; no active162 selection or ROOT edits. Existing owner260 selector is sufficient; no additional harness/test is authored.
