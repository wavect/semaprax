# disposition (review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

One actual 9552-byte tools-disabled review completed in38.30s (receipt/raw verdict
retained). It reported no confirmed blocker, with eight conditional concerns;
this is source review, not qualification or approval.

1. Old Set::iter prepays one fixed Keys carrier; it has no per-step WorkCounter.
   New values prepays actual Keys+Result and both loops keep identical charge()
   sites and order. Byte boundaries intentionally change; work diagnostics do not.
2. Sparse live sets may cost more; CONTRACT records this tradeoff. No fit claim.
3. CfgReservation has no refund/Drop credit. New growth constructs/copies a fully
   prepaid replacement while the old Vec remains live, then drops old backing.
4. The unchanged logical work contract counts visits/edges, not AVL comparisons
   or machine copies. Normal input allocates one point-domain buffer; forged
   growth preserves the full u16 membership domain without a new allowance.
5. cfg_directory::vector exists and prepays Vec carrier plus exact backing.
   into_vec is test-only; exact len capacity covers all ascending yielded keys.
6. Corrected iterator to stop before advancing an exhausted word index, including
   repeated next() after None. No allocation or extra work charge is introduced.
7. Actual CfgReservation::reserve checks active Rc identity before its first
   sticky failure and debit. New fallible operations call it before mutation.
8. Four concrete owners added; actual Rust qualification remains pending.
