# Ordinary pair-local PLAN paths v1

Audience: compiler contributors and reviewers.

Status: focused comparator and external application checks executed.
The original 64 MiB application gate remains open. This contract extends
[Ordinary lexical PLAN cursor v1](ORDINARY-LEXICAL-PLAN-CURSOR-V1.md)
with pair-local path loans.

After both original left/right Cursor::prepare calls and resets succeed, the
first original observation-comparison work charge runs. One pair-scope request
then funds two complete path-index operations and two distinct local reference
receiving slots. Two &PlanPath loans live only inside that pair's inner loop.
They never enter ComparisonOwner, cross pair scheduling, or survive a paths.swap.
Immutable arena-backed segment slices retain their original independent lifetime.
Left cursor traversal always precedes right, including stepping right when left
is exhausted. The next original work charge follows an equal pair immediately
before the next paired reservation/traversal. Terminal comparison, malformed
prepare order, heap scheduling/swaps, full HIR comparison and Drop stay intact.

Every removed per-round path index is physically absent. PAIR_MATCH, segment
STEP/EMPTY/LOAD, fresh History-tail views, prepare/reset, final Zip/HIR, swap and
all compound payload fees remain. The new entry bank is two full PAYMENT_HEADERs:
one new pair-scope reserve lexical site and one additional observation-work
lexical site. Both actual/formal argument tuples and returned/received unit
Result slots are included; only the established nonrecursive ZST-success,
stable function/budget, one-terminal-Diagnostic primitive-frame rule applies.
It does not retain references or exempt any dynamic payload. The original
compare-entry positive payment acquires these frames before formation.

Typed total difference is current + 4*PATH_INDEX*N = immediate883 +
PAIR_SCOPE*P + 2*PAYMENT_HEADER, where PAIR_SCOPE is two full PATH_INDEX tariffs
plus two distinct &PlanPath slots. N counts actual successful observation-work
round admissions; P counts both-prepares-complete pairs. The equality applies
to complete matched successful comparisons. On the discovery host this reduces
to the proposed160*N-96*P-704 net opportunity, not a measured saving. Zero-pair
inputs honestly pay the new invocation frame bank and small inputs can regress.
There is no old padding, credit, refund, cap increase or alternate oracle route.
Deferring a new reserve site's own frame into its request would self-fund its
already formed arguments; this candidate does not take that shortcut.

The complete immediate883 comparator, owner, cursor macro and tariffs remain
independently compiled. Only child registration glue and explicitly inverted
cfg(test) fixed diagnostic counters differ. Every earlier compiled old control
and every prior owning test stays. Added controls cover real producer/comparison
work cuts and six-field diagnostics, exact own frontiers/sticky refusal/Drop,
malformed/deep/empty/tail/shared-HIR, typed missing/foreign/restored Oracle,
full allocation tuples, independently counted price equations, long equal paths
and honest zero-pair overhead. The N/P recorder is fixed preallocated test-only
storage; collection cannot charge, allocate, authorize or replace a result.
Checked generation and monotonic serial identities prevent stale guards from
mutating a reset invocation. Overflow, truncation, loss, unfinished guards and
unwind cannot claim a complete comparison.
Snapshots allocate only after observed owners/results/budgets have dropped.

The codec03 library-unit trace includes the preserved cfg(test)
UnoptedCrosscheck. Shipping qualification uses the separate external
native and Project owners linked against the production library.

The original 140-test comparator and retained lookup selection passed on
`a9cec46db`. The six expression-fact arena controls and the callable
boundary at 64 MiB also passed on its retained local test image. The
matched external comparison used the immediate predecessor `4624d6b6a`
and the pair-local-only successor `3f3f211de`. Both arms refused all
four external owners at the original cap. At the current 64 MiB diagnostic
limit both native codec tests passed, while Catalog23 and ShiftSim15
refused. Both applications passed at the shipping 128 MiB limit.
See the [external test receipts](evidence/opt903-loan-release-20261113/README.md#external-application-checks-for-opt-886)
for the refusal stages and the exact source and test bindings.
