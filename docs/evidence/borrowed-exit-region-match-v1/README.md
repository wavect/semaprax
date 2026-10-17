# Borrowed exit region comparison

Status: focused component and source checks passed; complete compiler and current-head CI pending.

Audience: compiler maintainers and reviewers.

The ActiveBuilder non-latch receivers compare the authored region prefix
without constructing owned source and target ancestry vectors. The matcher
validates each complete chain before returning equality, then aligns chain
lengths to identify the same first common ancestor as the original receiver.
Malformed-chain diagnostics take precedence over an authored prefix mismatch.
Loop latches and other payers retain their complete original owning path.

The full typed comparison frame, result carriers, references, checked counters
and options are reserved before initialization. Every traversal uses the
existing replay work account; no budget, depth ceiling or refund is added.
Both complete original region implementations remain unchanged. Five complete
source views, seven predecessor archives and hashes for both new and both retained
modules bind the change; older whole-source proofs compose their exact inverses.
Both native source unions and the HIR authority audit include the complete new module and its owning tests.

Component checks pass on Rust 1.88 and 1.98 under the unchanged 200 MiB aggregate
child-process limit. They execute the complete matcher, paid region module,
root region oracle, budget methods, allocator observer and diagnostics, with
explicit HIR/payload adapters. They cover all topologically numbered forests
through five nodes and reverse numberings, every source/target pair and all six
non-latch exit variants, prefix mutations, malformed-chain priority, cycle work
exhaustion, exact/short/zero funding, sticky refusal, held floors and last Drop.
The real produced-HIR regression is authored in the owning harness and awaits CI.
Both production dispatch blocks are also executed verbatim with fixture adapters.

For the 64-node chain component, the old/new debit is 1,899/1,050 bytes, observed
allocation is 1,032/520 bytes, and work is 346/463 units. For the 64-node fork,
the corresponding figures are 2,403/1,050 bytes, 1,536/520 bytes and 4,636/778
units. Both finish with zero live observed allocation. Chain work increases;
the existing 32,000,000-unit ceiling is unchanged. These component measurements
do not establish whole-service admission, process RAM or CPU performance.

Staged negative controls must reject omitted frame payment, an incorrect common
ancestor cut and an early mismatch return that skips malformed-target validation.
The complete installed service journey and all original workspace fixtures
remain current-head CI requirements, with their original limits and assertions.
