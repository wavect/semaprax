# One explicit-record census per immutable phase

The original callable-boundary test failed with the helper-census H006 diagnostic
in [unit-heavy job 116636394120](https://github.com/wavect/semaprax/actions/runs/38796808320/job/116636394120).
The ordinary consuming collector scanned the ordered declaration prefix once
for each function. With 1,024 scalar declarations and no explicit record, those
scans alone require at least 1,048,576 units, exceeding the unchanged 1,000,000
unit allowance before the other collectors finish.

`RecordPresence` runs that exact prefix and short-circuit predicate once on the
factory's existing work account. It prepays its complete metadata carrier before
inspection. Each use authenticates the exact borrowed `DeclarationIndex`, checks
the live payer with a zero-byte request, and spends a work unit. Its shared borrow
prevents index mutation. It proves only record presence: the consuming body,
iterator grammar, bindings, projections, ownership and emission precedence keep
their full checks. No allowance is raised or replenished.

The complete pre-change consuming module, ordinary directory module, and module
index receiver are retained here. `inverse.json` restores all three current
files byte for byte. The independently compiled historical `consuming/old.rs`
still contains the entire original collector function, exactly matching the
saved pre-change function; its unchanged fixed-array traversal remains an
independent control. The owning source test binds those complete sources and
hashes. The authority/format audit includes the new predicate and tests.

Local probes execute the actual predicate and Admission, the original full
declaration-prefix loop, and the actual bounded-output ledger and allocation
observer. Minimal declaration carriers cover absent/present prefixes, every
relevant enum combination, early/middle/late hits, foreign indexes,
exact/short/zero-work/sticky refusals, and the repeated-census work failure versus
one shared census under the same limit. Rust 1.88 and 1.97 passed with sampled
aggregate compiler peaks of 177,344 and 176,208 KiB under a 200 MiB child-tree
stop guard. No full compiler was built locally.

Real-HIR tests additionally cover exact metadata/work boundaries, the 1,024
function fixture, and complete old/current directory-entry and work comparisons
on the existing owning-record iterator shape. These full-HIR tests require CI;
the local component probes do not qualify them. The existing callable boundary,
one-over G171, full consuming collector, and traversal depth tests remain intact.
This production change also requires fresh hosted graph-budget captures.

After the boxed path-state change, hosted STD-08 job
[116690924082](https://github.com/wavect/semaprax/actions/runs/38812864014/job/116690924082)
passed both previously failing testing consumers and advanced to five JSON
budget failures. Three skeleton construction wrappers now forward reservation
caller locations so subsequent failures identify the actual allocating caller.
Their algorithm, prices, and reservations are unchanged.
