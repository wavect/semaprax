# Ordinary PLAN comparison owner v1 (#880)

Audience: compiler contributors and reviewers.

Status: isolated source candidate. Compilation, focused runtime, physical
measurements, source registration and application qualification are pending.
No completion row or application-fit claim is promoted.

The existing ordinary PLAN select/produce/caller admission remains the only
shipping entry. The comparison receives the same function, ReplayBudget,
owned HistorySet and expected HIR slice. Its private owner lives only inside
that invocation. It stores cursor backing, integer path positions, heapsort
continuations and an Ordering result; it stores no function/ledger reference,
observation borrow, source identity, authority token or cross-invocation cache.
All surviving budget calls use the original function/budget bindings. No warm
owner can escape, be attached to a second payer, or authorize another call.
The existing original-selected test route remains independent.

One literal typed bank is acquired at the existing compare-entry reservation
before constructing ComparisonOwner. Build/Drain/Sift/PairPrepare/PairNext/
Siblings/Root continuations replace the two sift call sites and their nested
order calls. The new body has neither order/sift function calls nor their
Result-producing frames. The retained owner holds the traversal, cursor pair
and ordering state throughout sorting. This is not a paid-once flag around
unchanged helper calls. Scalar state transitions replace the original loop,
call/return and condition roles; no graph traversal is added by a transition.
The exact original work requests, sibling-before-root comparisons, left-before-
right cursor steps, terminal ordering and heap swaps remain ordered as before.
Only ephemeral proof paths are sorted, never canonical runtime cleanup vectors.

Cursor::prepare/next and their full PREPARE_CALL/NEXT_CALL payments remain
unchanged and are still paid for each surviving call. The flat/shared HIR tail
keeps its complete comparisons and original selector/cursor tariffs. A direct
error call replaces the local mismatch closure; its four actual lexical
receiver sites are explicitly prepaid. The typed bank separately lists owner,
index/swap, pair/pattern, iterator, mutable cursor initialization, scalar and
state-copy transports, payment ABI and diagnostic-return roles. Actual cursor
backing is still acquired after the owner bank and before cursor preparation.
No tariff is removed for a surviving prepare/next call or backing constructor.
No cap, refund, forecast, selected fee or independent crosscheck is changed.
Small inputs pay the honest larger owner-entry bank; net saving and the useful
size range require actual measurements, not static totals.

The immediate comparator's complete constants, macros, order, sift and compare
bodies are independently compiled in before_comparison.rs. Its parent Cursor
implementation is unchanged. The existing full frontier owner uses that named
immediate predecessor; all original phases, cuts and assertions remain active.
New current-owner cases independently compare output, each actual work cut,
all six Diagnostic fields, sort swap counts, malformed links/depth, mutated
terminal/multiplicity, exact/short/zero/floor, first receipt, empty sticky retry
and full destruction. They are authored, not executed. The old/new source
and caller namespace edits have complete preimages and finite inverses.

The test-only fixed aggregate census extends the existing diagnostic observer's
function/route final and maximum packets. It counts actual paid-macro
comparison/call-control attempts, attempted requested bytes, acquired ledger
deltas and refusals, with separate arithmetic-completeness flags. It never
uses the4096-event frontier buffer or influences admission, allocations,
compiler work or failure selection. It does not count physical allocations,
and it is not a census of all uninstrumented direct HIR/backend payments.
The actual old prefix's 7,563,141 PathComparison bytes are an opportunity
boundary, not an achievable saving or the successful application's deficit.

Before adoption, register all three new modules in the existing complete raw
inventories without changing old caps/counts/assertions; qualify original PLAN
owners plus the new owner/source/census cases and scoped Clippy in the parent's
consolidated batch. Then run the authentic codec/native/catalog owners at
unchanged limits, report actual paid/allocated/peak/live/work and refusal
positions, and earn moved pins through their existing admission. No agent
turn/token/cost or general application-fit claim follows from source review.

Review followup: the entry bank does not exempt recreated payloads. Paired
observation tuples, match/cmp receivers and path-index calls are paid at every
paired NEXT execution; flat/shared match payloads are paid at their actual
NEXT sites. Each swap pays its full actual receiver bank. Final Zip construction
is paid once, including Vec auto-deref; every Zip.next pays its Option/tuple,
path bindings and terminal receivers, including the final sentinel request.
Two explicit cursor backing calls remove the initializer iterator entirely.
This adds real physical frontiers and may reduce or reverse savings on long
observation prefixes. Original work, PREPARE_CALL and NEXT_CALL prices remain
unchanged; no gain is claimed.

Only the nonrecursive primitive caller frames are retained by lexical site:
successful Result<(), Diagnostic> carries no owned payload; the sole owned
failure exits immediately, and no frame escapes or overlaps a later use at
its site. The fixed function/budget places do not change. This narrow rule
does not cover recreated Option/tuple/cmp payloads. The current frontier owner
now compares the full first Diagnostic with the unchanged real reserve
primitive at the independently observed prefix, and verifies retry allocation
counters are unchanged as well as work and ledger balance. That oracle is
test-only state replication, never a production allowance or fee credit.

The reachable cursor primitive-bank payment is attributed to the existing
comparison-owner diagnostic category. All thirteen original categories,
indices, fixed backing capacities and checked counters remain; the complete
route/counter/refusal test additionally exercises the new owner-bank alias.
An explicit ignored STD-08 preparation-prefix witness prints settled ledger
data after the original result and resumes its original failure. This unit
observation is not an integration, backend or application qualification.
