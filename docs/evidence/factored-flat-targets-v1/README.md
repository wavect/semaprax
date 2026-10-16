# Direct weighted successor targets

Status: private implementation; full HIR and current-head hosted qualification pending.

Audience: compiler contributors and reviewers.

At main `3d5bef332`, hosted STD-08 job 117234379520 passed all eleven
library-depth cases, including the three prior budget failures. Its subsequent
application JSON group still failed 62 cases. Several first refusals were the
weighted canonical receiver's successor target trees. These are cumulative
builder reservations, not process RAM measurements.

The paid receiver now accumulates each label's targets in a sorted vector and
moves its backing into the immutable shared successor. It avoids the temporary
target tree and the separate vector copy. Target order, zero weights, duplicate
weight addition, checked overflow, outer label order and the complete weighted
comparison remain. Vector replacements and the shared header are prepaid on the
existing payer, including overlap with old backing. Binary search and insertion
shifts debit the existing work account. There is no refund or additional quota.
Unobserved compatibility keeps the original tree and work events.

Owning helper tests compare complete weight vectors and overflow verdicts and
observe acquisition through final Drop with the System allocator. They exercise
exact, one-short and zero byte limits, sticky authentication, work refusal, and
pointer equality when the vector moves into its shared owner. Existing complete
canonical receiver and unit-weight controls remain required.

Focused static-standard-library component checks run the complete before and
current successor methods, the actual target helper and its three owning tests
with a fixture-only replacement. They use the real bounded-output ledger,
diagnostics, physical storage and allocation observer; explicit thin adapters
supply ReplayBudget, labels, NFA nodes and epsilon closures. They cover empty,
unit, general, zero, duplicate, maximum and overflowing weights and preserve
legacy work. They do not qualify the complete HIR, memo, application budgets or
low-stack constructor.

Whole-file archives and inverses retain the complete prior canonical module,
its owning test module and every old unit-weight inverse. The helper and all
new controls remain inside those two existing audited modules. Current source
unions therefore continue to read their complete implementation. A leading
whole-module inverse restores the exact input of the original unit-weight
proof before its unchanged byte-offset inverses run. The new source controller
authenticates all three complete inputs and rejects tampered bytes.
