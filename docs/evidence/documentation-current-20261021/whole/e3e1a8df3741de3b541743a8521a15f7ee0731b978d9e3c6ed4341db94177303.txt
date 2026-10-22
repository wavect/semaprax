# Direct singleton and selected-adapter authentication closure

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source candidate from `36c6deb98720e243b0daaf81e813fbe3ec3b0a2d`.
No Cargo, compiler, Rust tests, Clippy or application qualification ran here.
The original focused452 failures and all existing assertions remain unchanged.

The actual singleton foreign-first owner returned success before producing any
authentication failure. `reserve_bytes(0)` authenticates the allocation payer
and sticky refusal; `charge(0)` authenticates the logical account. Neither bound
a function. Selected setup only used the supplied function to size refusal text.
The real ordinary production caller already authenticates its enclosing Memo,
but the direct selected-budget compatibility control bypasses that owner.

Selected setup now stores an exact function pointer and existing ClassifierLedger
scope inside the selected PhysicalReservation. Full typed Option/anchor/capture
receivers are paid before the anchor or capture is formed. The ordinary budget
carrier and constructors are unchanged; an independently compiled whole old
budget declaration asserts equal layout. General reserve/charge and multi-function
Session spending do not consult this anchor. Only the direct selected receiver
authenticates it before payer queries, work or output. Wrong owner, same-public-ID
foreign pointer, missing anchor, foreign scope and sticky failure reject without
work or charged-byte mutation. Scope mismatch poisons the held scope, not the
foreign current payer. No capability or admission comes from this metadata.

The selected ledger's actual layout grows; its initial existing typed reservation
uses that real new layout. The exact old ledger type and every old method remain
independently compiled, with a new owner comparing sticky arithmetic behavior
and reporting old/current sizes. Anchor initializer/capture prices and the normal
direct caller's query context are additional. There is no refund, cap change,
old-fee discount or ordinary net-gain claim. Historical selected cost analyses
must distinguish the new common setup from the true old ledger layout.

The other actual failure occurred in the test-only selected-successors adapter:
it acquired an invocation slot before asking the existing Memo to reject a
foreign function, budget or scope. Both current and immediate-old compatibility
entries now do the same identity/sticky preflight already used by the adjacent
id/closure adapters. Cold activation and all old payment order after preflight
remain. The unchanged original bodies are separately compiled under explicit
before names; a registered negative forces their original payment-before-scope
failure. This is a direct test-adapter fix. The production held-receiving path
and its existing owner authentication are unchanged.

Whole before inputs and full current-to-before inverses accompany the patch.
The entire old singleton producer remains compiled; its foreign-first acceptance
is retained as a registered negative. No original producer/assertion body is
weakened or removed. Fresh execution of the original349/358 owners and all added
negative/layout/same-ID/scope controls is required. Source-custody registration
and finite generation refresh remain pending the final integrated source freeze.

The old direct receiving price is retained verbatim under its before name.
The complete previous selected producer uses that before price and before
singleton helper through finite namespace substitutions; it does not share the
new current query price. The new current price adds physical/anchor Option
borrows and callback actual/formal roles, function-pointer conversion and
comparison roles, the existing complete ClassifierLedger current-query inventory,
and its possible empty refusal constructors. It is charged conservatively at
the current direct call boundary, including ordinary calls where selected-only
arms do not run. This can increase current ordinary reservation cost. No
historical baseline price was increased to obtain an optimization advantage.
