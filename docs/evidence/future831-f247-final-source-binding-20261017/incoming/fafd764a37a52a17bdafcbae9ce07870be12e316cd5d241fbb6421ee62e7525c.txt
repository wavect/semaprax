# String-slot byte helpers and declaration-bound diagnostics

Status: focused shape, frontier and source checks passed; complete compiler and current-head CI pending.

Audience: compiler maintainers and reviewers.

The nonallocating byte-helper closure proof now admits the existing primitive
String-slot read. Source requires zero explicit type arguments, exactly two
operands and a named root. HIR reuses the complete canonical slot inspector:
the fixed String specialization, ordinary whole Vec root, Value usize index,
no generic instance and exact Borrow Str or fused Borrow Slice<u8> result all
remain mandatory. Other Vec operations keep their original closed predicate.
The canonical inspector compares the existing Vec type by reference rather
than constructing a temporary type owner. No operation, source intrinsic,
capacity increase, loan exception or backend permission is introduced.

The rejected-storage declaration frontier also preserves independent generic
T223/T224 declaration receipts. It retains the complete prior diagnostic prefix,
checks function declarations, keeps only those additional generic declaration
refusals and returns through the unchanged finalizer. Executable body and
pattern checks remain stopped. This preserves the original global generic
owned-leaf bound regression while retaining the unsupported-storage diagnostics
and native-import priority controls.

Nine complete predecessor sources and the new owning test module are bound by
full hashes and exact inverses. Older custody packets compose the new inverses;
all original source and diagnostic assertions remain. The three new byte-helper
owning cases cover named/fused slot reads, canonical/graph projections and two
renewal sites, allocating or malformed source refusals, and eighteen typed
forgeries. A declaration regression combines the 257-leaf limit with an
unreachable invalid executable call. These real compiler cases await CI.

Focused probes compile on Rust 1.88 and 1.98 under the unchanged 200 MiB aggregate
child-process guard. They execute the complete changed AST/HIR conditional
blocks, canonical slot arguments/inspector and exact Vec comparison, with the
complete before-inspector as an oracle. Explicit AST/HIR carrier adapters cover
16 source shapes and 34 fixed HIR shapes, including both IDs, malformed arity,
type arguments, roots, ownership, index, instance and result carriers. The real
System observer sees zero allocation for the new inspection; the old temporary
Vec allocates in these fixture types. Fixture byte sizes are not a production
compiler bill or a process RAM measurement. Negative controls reject the old
source exclusion, an untyped HIR bypass and the allocating comparison.

A separate declaration probe executes the complete changed frontier slice and
unchanged diagnostic finalizer with the real Diagnostic/Severity types and
explicit checker adapters. It checks preserved prefix errors and warnings,
T223/T224 receipts, stopped executable work, discarded signature cascades and
native B107 priority on both compilers. Reverting the frontier or admitting all
signature cascades fails its negative controls. This is not execution of the
actual 257-leaf source regression.

Full source/typed traversal, the real owning regressions, the installed service
journey, all original application limits and complete current-head CI remain
required. These focused checks do not establish whole-service admission or
cross-backend execution results.
