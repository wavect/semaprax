# Deferred original-capacity owned-function directory v2

Status: private source checkpoint; no compilation or executable qualification.
This supersedes the unexecuted exact-growth policy in V1. The e8d2a7d8 private
checkpoint, V1 document, evidence tables and complete old receiver stay intact.

GeneratedRoots starts with its existing empty owned Function vector. Borrowed
name-equal authenticated imports never acquire its backing. Before the first
actual owning fallback or synthetic-main payload is formed, the lexical macro
acquires once the original `imports + synthetic_main` upper-bound capacity.
It reads that exact bound from the already acquired FunctionEntry directory,
whose original checked count, exact-capacity check and non-growing policy are
unchanged. It performs no new classification/count pass, payload construction,
validation or Result-bearing helper call. Every subsequent owned payload uses
that same original full capacity. Ordinary owning fallback remains complete.

The first acquisition uses try_reserve_exact on the empty vector and checks
actual capacity against the full bound before initializing a Function. It
prepays the actual full backing; no removed fee substitutes for an allocation.
There is no repeated growth, cumulative full-backing billing or refund. With m
owned payloads and bound N, the new lexical control charge is m CHECK_CONTROLS
plus one ACQUIRE_CONTROLS plus N * size_of(Function). CHECK_CONTROLS names the
capacity receiver/result, comparison operand and branch. ACQUIRE_CONTROLS
names the entry receiver/count/binding, layout arguments/Result/value/receiver,
layout-size result, reserve receiver/argument/Result, exact-capacity comparison
and ordinary failure receiving carriers. Their concrete size_of expressions
live with the macro. No runtime/token/cost improvement is claimed: these added
controls can outweigh the removed original directory setup charges.

The all-borrowed case with authored main has zero owned capacity. All-owned,
mixed and synthetic-main cases retain original full capacity, even unused
slots. Allocation count and total backing for those owned cases should equal
the complete original eager receiver, with no added reallocation. The authored
observer prints absolute reservations, allocations, reallocations, allocated
and freed bytes, highwater, first refusal and Drop settlement for all four
cases; these values remain unmeasured until the focused batch runs.

The physical frontier intentionally changes: the original owned directory was
acquired before entry backing and import authentication. Entry backing now
comes first; owned backing moves after the unchanged fallback signature and
teardown payments but before its payload clone, or before synthetic-main
fields. The new finite controls can select an earlier budget refusal than that
payload formation. Sticky refusal, alias validation order, grouped receiving,
source/output identities and complete owning fallback remain unchanged.
Constrained-budget first refusal sites are not promised identical to V1 or the
original eager receiver. Full post-refusal Drop remains mandatory.

Four existing private lib bodies cover complete original/current HIR parity;
zero versus original capacity; absolute physical and reservation matrices;
zero/half/one-short/exact limits; actual partial-owned Drop; reserved-name and
sticky diagnostics; and complete source custody. The V2 table authenticates
these new source bytes before restoring exact e8 inputs for its unchanged V1
custody table and original 847 owners. The original eager producer is still
compiled independently with only module-parent paths lifted. No original
assertion, evidence table or archived payload is weakened. Renamed borrowed
views, native/Wasm/profile/application acceptance and complete core cache/image
qualification remain outside this private representation checkpoint.

## Receiving correction in the Maina3 integration

The immutable private1ae predecessor remains source-bound independently.
CHECK now includes both caller reborrow and callee-self references for capacity.
ACQUIRE includes both such slots for entry capacity, Layout size, mutable
try_reserve_exact and the post-reserve capacity check. These five concrete
additional reference slots have no claimed prepaid correspondence in the old
directory helper. The existing Receiver::reserve primitive keeps its own
unchanged Result/control bank; no unrelated fee is inflated. The once-only
original-capacity allocation algorithm and four original owners are unchanged.
