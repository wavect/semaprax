# Source-only followup disposition

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The raw conditional verdict and all seven uncertainties remain in review.md; this is no runtime approval. One 8,404-byte tool-disabled dispatch completed in 72.38 seconds, peak process-group RSS 324,435,968 bytes. No repeat was dispatched.

B1 is resolved by the actual existing physical_budget.rs reserve_vec producer: it precharges the full target, performs try_reserve_exact on the same Vec and checks exact capacity before returning. resize_with then cannot allocate on the paying path. A sparse-high-first and subsequent boundary-growth control now asserts actual capacity equals the exact producer target.

U1 is qualified by the stated source-carrier model: named array initializer, heap array and named Box pointer; moved values do not establish additional simultaneously owned source objects. No machine-stack/ABI-size claim is made. U2 caused a concrete correction: the named initializer is now Option<Binding>, priced as that exact type, with no separate nested Binding fee. U3 caused a concrete correction: the authorization Result is charged on each authenticated paying call, independently of first receiver activation. ReplayBudget's payer has no later assignment, so the paying predicate cannot transition while a memo is held.

U4 is an explicit private-input precondition: real keys are existing indices of the already physically constructed automaton Vec, not arbitrary usize input. Existing cycle behavior is unchanged. U5 is resolved from ClassifierLedger: capture/current use the existing ACTIVE Rc; positive builder bootstrap does not create ACTIVE. LegacyUnobserved explicitly skips all positive fees/bootstrap and has a separate full constructor/Drop control. U6 remains the private call-lifetime invariant: one local canonicalizer and the same borrowed function/budget across both roots in factored::equivalent; no escaping views or new callers. Pointer equality grants no general durable identity. Foreign function/budget, stale ledger, foreign/restored scope and cached-id controls are authored.

For U7, closure authenticates before its first read, all subsequent failing charges/insertions propagate immediately with ?, and no intervening operation can change its bound immutable function or owner. The final getter therefore occurs only on the same successful invocation. Failed Memo getters no longer expose retained views. Cached id now authenticates before its state lookup. Complete before_index and canonical_before receivers remain independent, and all old owning controls remain required. Actual compilation/runtime, per-owner physical bounds and application results are still pending.

Final source API check also corrected the draft fixture ancestor path from five to three levels, and explicitly imports the retained original tests hook in before_index (the full old producer body still calls its original note_actual_entry hook unchanged). These are lexical registration adapters, not algorithm changes.
