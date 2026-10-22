# #836 native C11 and private Core-Wasm backend packet

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Frozen basis: `e3eb476838442b58c02ccc815aae4bbdfebfd50e`.
Dependency: frontend interface `2677c34b3` (private cherry-pick `a4c91d7aa`).
The complete frontend/validation/cleanup/interpreter continuation
`4855f3363e9f6db449433167e789c840203503d1` is also required before executable
qualification. This backend packet does not duplicate those edits. It modifies
only native/Wasm implementation and corresponding modules of the existing
`cleanup_backends` and `owned_data` integration harnesses.

The agreed source form is
`vec_update_field<Row>(rows,index,value,"field")`; runtime children are exactly
`[source,index,value]`. Both backends independently replay the authenticated
field descriptor against current declaration facts, require the exact Copy RHS
type and stage operands left to right before checking bounds. Native uses the
existing mutable-Copy snapshot and Wasm the existing expression materialization;
a direct RHS-block mutation of the index is covered without a new staging
shortcut. Ordinary checked RHS failure remains selected before an invalid index.

Native adds one reachability-selected scalar helper. It authenticates the
existing complete descriptor/current generation, field ordinal and exact scalar
kind; validates canonical narrow scalar encodings; checks bounds and next
logical generation before touching the slot; writes exactly one eight-byte scalar
slot; then uses the existing one-owner transfer. It does not pack/rebuild a
row or access an owned leaf. No capacity, representation, public ABI, profile
or limit is raised. Legacy tag10 constructors bind their existing descriptor
only when the additive update is reachable. Existing runtime C and original
worker/allocator-probe bodies remain byte exact and archived in full.

Core-Wasm appends one private six-i64-to-i64 import, after the old optional
imports, keeping all old import indices exact:
`spx_vec_update_field_v1(handle,nominal_identity,shape,index,ordinal,bits)`.
The source owner remains in its authenticated call epoch on failure. A successful
non-null result is followed by exactly the existing call commit and source
clear. The strict private fixture authenticates constructor-bound nominal
metadata, generation and Copy ordinal before a direct word assignment, renews
the same authority slot and verifies unrelated word/owner identities remain
unchanged. It forbids payload/vector allocating/settling helpers during the
store. Its BigInt/typed-array fixture bookkeeping is not a claim of zero JS heap
allocation or a public Web host implementation.

Five exact new definitions are registered in `pending-selectors.json`, with five
unchanged direct controls. The paired source corpus covers all eight scalar
kinds, i64/i32 minima, u8/usize maxima, Unicode char, negative f32 zero, f64 NaN,
bool, owned String/Bytes identities, both selected rows, consuming loop renewal,
legacy two-Bytes layouts and a mutation of the index in the RHS. Initial selected
values differ from requested values, so omitting the store cannot pass. Empty,
last valid, one-past and usize-max indices, RHS-before-index failure, index
failure, single selected status, untouched result sentinels and complete
construction-through-Drop settlement are authored. Native generated-call probes
reject malloc/calloc/realloc/free at the actual store, compare all unrelated raw
row bytes, and check unchanged backing/length/capacity/authority with one renewed
generation. A complete independent clone/reconstruct/whole-replace source oracle uses
only the existing operations and remains independently executable. Wasm adds stale/restored generation,
wrong nominal/shape and non-Copy ordinal hostile checks at its real private host.

All Rust, generated C, Node/Wasm and allocator assertions are **unexecuted**.
No native O0/O2 pass, Wasm pass, allocation gain, application acceptance or
agent token/cost benefit is asserted. Live-loan and forged source/HIR/cache
admission belong to the frontend continuation, and real ShiftSim/inventory
adoption remains owned there. Parent qualification must include both packets.

`source-inverses.json` holds thirteen complete immediate-before source inverses
and six current child bindings. The shared backend source guard joins every
owner and child, reconstructs complete originals and checks current child
hashes; no old source audit is shortened or exempted. `unchanged-originals.json`
binds five complete original runtime/worker/probe archives. `source-only.py`
checks those bytes, declaration replay/staging/commit order, additive import
registration, absence of row/leaf allocator/drop calls in the native store,
exact unignored selector registration and unchanged Rust module budgets.
The Wasm root is 8683 lines within its existing 8698-line ceiling.

No global 67-packet/Source36 composition was run or relabeled. The integrator
must compose this packet's full source prefixes/child hashes once alongside
other adopted packets, preserving every older archive and dispatcher hook.
ROOT, #833 and #835 ready branches were untouched. No Cargo, compiler, tests,
Clippy, Claude, paid calls, review execution, push or remote writes occurred.
