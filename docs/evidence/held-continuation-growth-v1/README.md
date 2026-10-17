# Grow invocation-held continuation backing as reached

Status: focused component and source checks passed; real HIR and current-head hosted qualification pending.

Audience: compiler maintainers and reviewers.

The earlier production callable-limit fixture refused a 70,176-byte fixed
continuation buffer during HIR final validation with only 2,869 bytes remaining
in its unchanged 64 MiB builder account. The buffer reserved all 516 frames even
for shallow traversal. This change addresses that request; it does not prove
that every later operation fits or that the complete fixture now passes.

Authenticated invocation-held storage now starts each top-level/nested lease
with two frame slots. The runner's two existing push chokepoints grow a full
buffer before publishing the next frame. Each replacement pays its complete
backing while the old backing remains live, plus new controls and explicit
relocation work. Frames move in their original order. Empty buffers remain
leased to the same program/invocation and are reused at the same nesting depth.
The original 516-frame bound supplies traversal authority independently of
capacity. No account, limit, quota or refund changes.

The original fixed constructor and all unobserved, standalone and independent
oracle acquisitions remain. Storage fields, Frame variants, owned payloads,
source evaluation order and every original skeleton work event remain. Only
the authenticated ordinary program-storage constructor selects the small
buffer. Foreign/restored invocation refusals retain the existing sticky owner.

Four new complete physical/authentication/depth controls pass on Rust 1.88 and
1.98. They check all ten capacities from 2 through 516, every live frame's
length/order/kind, pointer stability on refusal, exact/one-short/zero allowances,
reserved floors, final Drop, work exhaustion, nested reuse, unchanged legacy
capacity and exact original depth diagnostics. Removing backing prepayment or
replacing relocation with an empty sequence fails those controls on both
compilers. A fifth real-HIR roots/guard comparison is authored in the existing
owning harness and awaits hosted execution; every original test body remains.

The component uses the complete actual Storage/Frame modules, actual budget
paying/work/refusal methods, classifier ledger, diagnostics and System observer.
HIR and non-executed payload/operator types are explicit adapters; the scalar
continuation structs and parameter enum retain their actual definitions.
It does not execute the full compiler or the real callable-limit fixture.
The measured direct constructor lifetime allocates 70,694 bytes on the fixed
route and 790 on the held route; cumulative charges are 70,694 and 1,303 bytes.
Both finish with zero live bytes. The 69,904-byte allocation reduction is an
observed component result, not process RSS or a whole-program budget forecast.

Focused Clippy passes. The complete current SourceReflows suite and complete
original Oracle-a controller pass in separate small binaries. Existing moderate
and compact source controllers pass too. Source-proof execution peaked at
97,584 KiB; its compile peaked at 193,728 KiB under the unchanged 200 MiB guard.

Ten complete predecessor archives and authenticated inverses preserve every
byte of the original state machine, fixed constructor, test bodies and dependent
source packets. The root differs only at the two push guards. All fourteen
original receiver proofs, formatting hashes, prior status-prefix controls and
other historical assertions remain. Compact unique substitutions replace
repeated nested snapshots; full current/predecessor hashes and full archived
byte equality still authenticate every input. No semantic edit is registered
as a formatting repair.
