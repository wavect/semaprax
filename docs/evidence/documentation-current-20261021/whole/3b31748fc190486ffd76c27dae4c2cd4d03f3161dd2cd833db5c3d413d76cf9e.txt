# Status cursor caller prepayment (#882)

Source-only integration of main01527/e177. No compilation, runtime, fit, saving,
capture or pin qualification is claimed.

The incoming allocate helper reserved its caller/formal/Result bank after entry.
The incoming insertion helper likewise reserved its ABI inside the new callee;
the optional index match had already formed its borrowed binding before billing.
Shipping callers now reserve the complete unchanged incoming helper banks before
entering `allocate_prepaid` and `insert_reusing_cursor`. The original allocate
body and original insertion body remain compiled under cfg(test), the latter
with only its independent method name changed. All complete source endpoints and
whole inverses are retained in whole-source-inverses.json.

New actual primitive-payment/setup/dispatch frames are funded through existing
enclosing acquisitions, rather than through those frames' own new calls:

| Enclosing acquisition | Explicit typed slots |
| --- | --- |
| Held-prefix initial reservation | Existing two HeldPrefix + Result; new setup frame described below |
| Authenticated owner successful receipt reservation | Existing authorization + inner/outer insertion Results; new dispatch frame described below |
| Primitive payment frame | Two complete (&mut ReplayBudget, &ResolvedFunction, usize, &str) actual/formal tuples and two Result<(), Diagnostic> return/receiving slots |
| Setup frame | Primitive payment frame; two &mut Option<StatusIndex> as_mut receivers; two returned/received Option<&mut StatusIndex>; matched &mut StatusIndex; additional caller Result<Box<IndexSearch>, Diagnostic>; Some publication Option<Box<IndexSearch>>; payer-selection bool |
| Dispatch frame | Primitive payment frame; two &Option<StatusIndex> as_ref receivers; two returned/received Option<&StatusIndex>; matched &StatusIndex; two &Option<Box<IndexSearch>> is_some receivers; returned/received bools |

The setup/dispatch frames are acquired for their enclosing lexical scopes,
including small/fallback routes. This is new overhead. The former late individual
matched-reference charge moves into the enclosing dispatch frame. The actual
helper bank expressions and all original work calls and amounts stay intact;
allocation/reuse remains after authentication and the same original work cuts.
No paid-once flag, refund, cap increase or original baseline fee increase is used.

The incoming tests remain byte-exact. Their original comparison allocates the new
cursor and then removes it from one side, so its allocation baseline includes new
setup. The new `before_cursor` Directory and status-prefix modules preserve the
entire real pre01527 producer, including its independent old layouts and tariffs.
Only test-module wiring differs. New controls compare every final key/tree link,
report actual old/current paid/work/allocated/peak/Drop tuples, exercise zero,
short, exact and floor reservations, follow six actual refused receiving-bank
frontiers, preserve sticky failures and compare full foreign-function diagnostics.
No universal saving assertion substitutes for the pending actual receipts.
