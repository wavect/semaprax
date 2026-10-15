# Selected Tree fact payment layout repair

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Source-only, based on d32d0c575; no Cargo, build, tests or observer changes.

Frozen bf29 log119 panics at line97 in the Err arm `assert!(!success)`. Only the full row has success=true. The full case refuses; the receipt does not show full-minus-one success. Its SHA was verified against the frozen inventory.

Tree paid -> with_policy -> with_capacity pays sizeof(FactCapacity) plus 514 complete expression frames, then precisely two original BTree node reservations for Block/Int. The independently authored PayerLayout omitted the test-only OracleLease variant introduced by07146bb97. The actual enum retains an epoch-bearing alternate payload and discriminator even on the Selected route. The repair adds that complete literal field layout, with a separate size guard against the public replay-visible lease. No measured byte delta is claimed: repr(Rust) remains compiler-owned.

All four original limits (base, base+cursor-1, full-1, full), both success/refusal assertions, two-entry assertion, exact usage assertion, first receipt pointers, sticky retry and full allocation/freed/live Drop assertions are retained. Removing only the named additive layout blocks gives the entire original test source byte for byte. Production Tree routes and payment formulas are unchanged from bf29; the arena route is unrelated. Complete old test is archived for independent source comparison. No production defect or app improvement is claimed.

Pending owning selectors are in pending-selectors.json. No new modules or audit child includes. Claude Sonnet5.5 firstParty tool-free source review succeeded in38.60s; reported list cost$0.0888895, actual billing unknown. Current-head qualification is pending.
