# Narrow preflight branch successor

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Preserves fac23 and its entire packet byte-exact. The original unit preflight now returns binding presence directly from its existing if-let; callers branch on that scalar without a named receiving local. This replaces the original retained controls_paid/binding predicate and removes the additional Option::is_none receiver reported in review. A mismatched held binding returns present but sets failed; callers inspect failed before zero-auth. A failed/unbound receiver returns absent and callers inspect failed before original cold admission. Ledger-before-pointer and first-cold refusal ordering remain unchanged.

No new field, fee, positive/zero query, result, allowance or work call is introduced. The old warm bool branch/presence test is replaced, not credited against owned storage. Only canonical_shared.rs, empty_memo.rs and the strict source front change. Three whole current→Gitfac inverses precede the immutable fac→2d4b table; all five old/new test owners and original fee formulas remain exact. No other protocol redesign.

Run this child `verify-source.py --repo <checkout>` on physical successor input. The parent's old verify-source.py remains the exact historical fac-domain tool; the new verifier explicitly authenticates the fac named view then composes its unchanged nine rows against Git2d4b. No old verifier is silently run against new raw input. No Rust execution or gain claim.
