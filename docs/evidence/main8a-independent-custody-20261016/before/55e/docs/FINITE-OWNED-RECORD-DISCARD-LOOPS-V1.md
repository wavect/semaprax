# Finite owned-record discard loops v1

Status: source checkpoint; current-head owning execution qualification pending.

Audience: compiler contributors and reviewers.

This source tranche is pending executable qualification. It preserves the
ordinary allocation limits and does not authorize arbitrary allocating while
bodies.

A parameter-free, effect-free monomorphic function returning i64 may use a
direct zero-initialized mutable i64 counter followed immediately by a while
whose condition is that binding less than a positive literal at most 32.
The body contains one fresh immutable monomorphic record and one direct
expression-statement owned record match, followed by scalar zero. The match
has one guard-free record arm. Its first and only statement increments the
exact counter by one; its tail may complete normally or use checked Continue,
Break or Return. Additional assignments, nested loops, closures and counter
shadowing are closed. Nested branch Blocks contain no additional statements. Source and HIR independently derive this exact shape
from current declarations and expression pointers.

The bound is conservative even when an exit terminates earlier. Existing
FiniteOwnedLoop capacity flow multiplies each body's actual allocation demand
by this bound. Legacy32's 32 successful Bytes-producing events and 2 MiB
payload remain unchanged; overflow, excessive demand and allocating conditions
still refuse. Induction is not ownership permission: independent type,
availability, loan, canonical cleanup and backend checks remain mandatory.
Every iteration-local descendant must settle before a latch, exit or failure.

Source capacity selects this flow only for the actual pointer-held While body;
it does not identify candidates by equal text or replace the ordinary ForBorrow
or ForOwn route. HIR capacity independently selects it from the actual condition,
body, bindings and declaration identities. Both folds retain complete callee and
producer demand and checked multiplication. The legacy `SPX-T267` message names
`bytes_copy` for all owned-byte producers, including `bytes_zeroed`: recognition
must apply to the proved flow rather than exempting a builtin name. A literal
counter condition has no allocating child. An allocating condition, excess total
events/payload, unknown extent or recursive producer remains refused.

## Explicit no-result statement

An authenticated parser Match expression-statement marker containing a lexical exit
lowers to HIR CopyDiscard { value, span }. It evaluates its value once, left to
right, and declares no binding or result storage. Normal completion must be one
of the existing eight Value Copy scalars. Never completion retains the existing
independent lexical target and certificate checks and has no fallthrough result.
Expression temporaries, call staging and arm-local owned bindings retain their
full canonical cleanup and sticky failure transitions.

A named Let initializer with identical match text remains a value boundary and
cannot cross Break, Continue or Return. StatementIf's existing scalar-zero
guard completion restriction is unchanged. Nonlexical expression statements
retain their existing representation.

## Projections and compatibility

CopyDiscard owns checked-HIR codec statement tag 5 and compiler cache
compatibility semaprax.project-checked-module-hir.v10. Decode grants no authority.
Current source, imported identity and complete independent HIR validation
remain required before reuse.

Programs containing CopyDiscard select semaprax.graph.v84, layered over the
existing checked graph. Its statement has kind copy_discard and a value child.
Expression paths remain .sN.value. Programs without this statement retain their
previous graph versions, including row-loan v82 and borrowed byte traversal
v83. Frozen evidence profiles refuse this newer feature.

For nested storage with flat local byte loans, v84 has a separate current finite
record composition frontier. Every CopyDiscard must occur within an actual
pointer-held finite-record body independently recognized from current HIR.
Selection follows the existing nested cleanup and native-import flag gates,
then complete HIR, loan, byte capacity and canonical cleanup rebuild/replay.
A statement tag, retained hint, detached function or parts-only view cannot
activate this frontier. Frozen nested schemas retain their projected-loan
restrictions. Other admitted CopyDiscard shapes continue through their existing
checked inner graph profile and the v84 statement layer. Canonical rendering
preserves the existing prefix contract and ordered loan/cleanup carriers.

`graph::copy_discard::tests::` authors complete source/HIR 1/8/32 controls,
canonical and cache replay, native-flag and forged loan/cleanup/counter refusals,
frozen/detached controls, and the existing nonfinite nested projected-loan
route. These source controls await consolidated executable qualification.


The original owned-subtree Break/Continue three-backend fixture is retained.
Owning controls independently derive exact source/HIR sites, capacity and wire,
reject detached sites and malformed ownership/counters/statuses, and preserve
ordinary named-initializer refusal. Authored controls are not passing receipts.

## Source repair regression owners

The source and HIR profile controls in `finite_owned_record_loop::tests::`
retain the complete original 1/8/32 record bodies and hostile ownership,
counter, lexical status and detached-node corpus. Added complete-body controls
count two independent producers per iteration at 1/8/16, refuse 17 iterations
under the unchanged 32-event bound, independently exercise exact 2 MiB payload
and one-iteration-over refusal, and count a real named-slice `bytes_copy`
with embedded NUL and 255-byte content. The ordinary/unproved copy-loop
`SPX-T267` controls remain separate.

The source-only HIR extent repair derives a named slice's length from the actual
pointer-held current function: a unique immutable binding chain must end at
`array_as_slice` of an immutable literal or repeat array, whose actual length
matches its resolved array type. The original embedded-NUL/255 three-byte array
therefore contributes three bytes per copy, nine across its three proved
iterations. Supplied provenance and retained totals grant no sizing authority;
complete independent HIR validation still rejects forged provenance, aliases
and array types. Unknown, mutable, assigned, duplicate or detached shapes keep
the original `MAX_OWNED_BYTE_VALUE_BYTES` fallback. No event or payload quota
increases and no ordinary allocating-loop refusal is bypassed. The borrowed
cursor walk uses fixed-depth stack storage and at most 32 alias steps, each
bounded to 65,536 visited nodes; repeated scans are an explicit bounded cost
tradeoff. The new exact/fallback/hostile owners remain unexecuted until the
consolidated qualification run.

`owned_data::nested_collection_record::subtree::lifetime::finite_record_named_slice_copy_preserves_live_exits_on_every_engine`
authors the corresponding interpreter, native O0/O2 and strict Core-Wasm
repeated-lifetime receiver, in addition to the original zeroed-buffer
Break/Continue corpus. These authored controls await the consolidated run;
source edits are not executable or hosted evidence.

The subtree cache owner retains its ScalarV1 `SPX-G174` refusal and uses the
existing Project v16 `useful-data.v2` private-entry/test role for both complete
canonical sources. It does not alter scalar public exports or remove private
owned helpers to make cache admission succeed.
