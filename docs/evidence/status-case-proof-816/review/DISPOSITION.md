# Source-only review disposition

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

One actual tools-disabled Sonnet dispatch, 9,836 payload bytes, 88.20 seconds,
282,329,088-byte process-group peak, terminal exit 0. Full raw review and usage
retained; provider list cost $0.13062 is not a billing receipt. An earlier local
10,717-byte packet was rejected before prompt write/Popen; no process ran for it.
No Cargo, compiler, runtime, memory-gain or approval claim follows.

B1 confirmed: Attempt::complete lacked authentication. It now authenticates
function/budget/live ledger/sticky state before any payer use, requires Active,
pays its actual Result and two assignments, then consumes the proof.

B2 confirmed: a shared begin permitted multiple attempts/success. The exact paid
State field is now Ready/Active/Finished/Failed; only Ready can begin, and only
Active can acquire membership/complete. Reentry poisons the proof, incomplete
Drop poisons it, successful completion transitions Finished. An additive whole
observer control covers overlapping begin, poisoned completion, foreign payer
completion and second acquisition after success. No raw view revocation claim.

Conditional review uncertainties retained:

- Budget pointer authentication is safe at the actual ordinary call site: it is
  the same borrowed &mut budget parameter, never moved/dropped/replaced between
  mint and receiver. This private proof is not a generic address-ABA guarantee.
  Function/table borrow is enforced by Rust; ledger is checked each acquisition.
- The full new source contains no continue: exactly one charged scalar ordinal
  increment follows every successful row. Every early return drops Attempt.
- Facts are the actual arena built from the same immutable function immediately
  above the complete walk in replay.rs; the proof does not authenticate a new
  Facts authority. Its only reused conclusion is independent case uniqueness.
- ordinary_status.rs is archived whole and byte unchanged. Lines 75–84 compare
  exact operation/full ordered cases; complete() checks exact table length;
  389–414 contain all six fixed nonempty/unique lists. No plan-as-authority scan.
- physical_budget.rs reserve0 checks ACTIVE/selected/oracle sticky state; charge0
  checks prepare_builder/refusal_spent/current work owner. No unconditional bridge.
- Accounting is cumulative acquisition. New carrier fees are not peak estimates
  or credits against removed Directory allocations; runtime is still pending.
- is_none_or and explicit size_of imports already occur in the unchanged owning
  directory implementation. Diagnostic::io uses SPX-H006 and full six fields;
  String::new has no backing. Prefix-owner raw directory binding is unchanged.

No second review dispatch. The raw conditional verdict remains unedited.
