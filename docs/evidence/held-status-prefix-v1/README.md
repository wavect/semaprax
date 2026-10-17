# Hold the status-directory prefix during paid construction

Status: focused component and source-custody checks passed; full compiler and current-head CI qualification pending.

Audience: compiler maintainers and reviewers.

The production core-retry fixture exhausted its unchanged 32,000,000-unit work
account while inserting status identities. Recomputing the common prefix from
both AVL extremes repeats work on long identities at every insertion.

Paid construction of three or more status sources now holds an exclusive mutable
borrow of the directory for the complete insertion loop. Its prefix is the common
byte prefix of exactly the inserted keys. Comparing each new key against the
current minimum, clamped to that prefix, produces the new complete-set prefix.
The ordinary full suffix/length/lane comparator, insertion links, balancing and
all status fact/producer checks remain. The original function, directory, budget,
classifier-ledger and source-order authentication runs before each insertion.
The bound cannot outlive the exclusive borrow or authorize immutable lookups.

Every new control, comparison and step is funded through the existing accounts;
there are no refunds, new allowances or raised limits. Empty, one/two-source,
unobserved and tree-mode paths retain their complete prior validation body.
The new loop preserves that entire body with only the held-builder substitutions.

Focused Rust 1.88 and 1.98 probes execute the complete actual directory and prefix
modules, all three original owning controls and four added controls. They use the
actual paying/work/refusal budget methods, classifier ledger, diagnostics,
physical storage and System observer, with explicit fixture/HIR and non-status
key adapters. They check every AVL link, full byte/lane ordering, common-prefix
shrink, physical frontiers through final Drop, sticky refusals and foreign,
restored and wrong-subject authentication. A synthetic 16,000-identity case
exhausts the original work ceiling on the old insertion route and succeeds on
the held route. This does not qualify the real production HIR fixture.

The split probe builds peaked at 187,728 KiB under the unchanged 200 MiB cap;
focused Clippy passed. Disabling only prefix publication is rejected by the
owning shape control, and mutating the directory with a live prefix is rejected
by the Rust borrow checker on both toolchains. These are component checks,
not whole-compiler or hosted qualification.

Three complete semantic views and six full predecessor archives preserve all
original formatting inputs, source-reader assertions and historical checks.
The unchanged fourteen formatting proofs, prior allocation followup, ordinary
setup bridges and helper bridges remain. All fourteen current SourceReflows
and Oracle-a tests pass, including full copied-loop equality and tamper checks.
