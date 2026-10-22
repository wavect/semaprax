# Pure-identity closure Memo gateway

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only candidate, no compiler/tests executed. Original261 remains byte-exact in the unchanged original paid_many/tests prefix. A full immediate-old Memo is independently compiled verbatim in before_identity in that same existing test file. Its old current() call intentionally demonstrates the prior captured-ledger mutation under nested mismatch; it is not relabeled as a passing old contract.

ClassifierLedger::current() and current_receiving_controls() remain whole-byte exact. Only the new current_identity method is added: it evaluates the original active clone/match/borrowed pointer comparison and returns that identity, without record_refusal or overflow mutation. It removes the old mutation control flow and one named bool local; all remaining actual/formal/ACTIVE clone/TLS/match/Rc/bool receivers have exact same-site counterparts in the unchanged prepaid current_receiving_controls bank. No new payment, cap, exemption, authority or cached admission is introduced. The containing Memo/ClosureReceiving acquisition pays that bank as before.

Only Memo::authenticate and Memo::preflight use the pure gateway. Function and budget pointer checks remain before any allowed acquisition; mismatch poisons the Memo and returns/refuses its views. Matching identity still reaches the exact old zero authentication and real payer reservation. Sticky/foreign refusal cannot acquire or revive a destination. All other global current() clients retain their original sticky mutation semantics.

The new one-owner regression loops both complete old/new Memo bodies, both gateways, and zero/full nested invocations. It retains six-field H006 comparison, nested zero charges/no first report, exact outer and logical remaining, no new allocations on mismatch/retry, restored-owner rejection, unpublished poisoned views, and complete Drop. The old body must reproduce outer refusal; the successor must leave both ledgers untouched. Original261 additionally retains function/budget/sticky attacks. Opaque failure260 is not diagnosed or claimed fixed by this patch.

All three whole files and exact single-range finite inverse are retained. Parent must compose finite current source acquisition before unchanged historical source rows for classifier_string, empty_memo and paid_many/tests. No historical table/assertion/cap is retargeted here; no new physical Rust owner/harness.
