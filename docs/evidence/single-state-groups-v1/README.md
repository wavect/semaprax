# Reuse a sole incoming ownership-state tree

[STD-08 job 116728653928](https://github.com/wavect/semaprax/actions/runs/38824367721/job/116728653928)
still refused five JSON consumers at the unchanged 64 MiB builder limit. One
refusal was a 3,992-byte control-group tree node in final HIR replay. A block
with one incoming state constructed a control-key map and a fresh state tree
to represent the identity partition.

The paid one-state route now retains the existing owned state tree. It still
executes the complete original control-key constructor, including scratch,
nested backing and work charges, then drops that key at the old iterator's
yield boundary. It authenticates the live incoming-state payer before reuse.
The new iterator, result, input, payload, map-transfer and borrow/control layouts
are prepaid in full; no prior header or owner is credited against them. The
unobserved legacy route and every other cardinality use the whole old grouped
producer. Existing node prices and all limits remain unchanged.

The entire downstream receiver stays in place: exact lexical joins, full
control-group validation, ownership joining, branch normalization, lexical
boundary checks, transition and exit replay all execute unchanged. The caller's
complete semantic work charge remains unchanged. No plan vector is sorted,
repaired or reinterpreted.

The complete preceding root and both producer spans are archived. Source
custody proves exactly three root edits, the byte-exact whole general producer,
and both complete key constructors; the borrowed constructor adapts only its
three reference arguments. The older boxed-state inverse remains unchanged and
is composed after the new certified inverse. Its whole receiver test differs
only at that restoration call, also bound by a complete source hash/inverse.
Both HIR and semantic-graph source inventories include all new modules.

Sequential local probes compiled the actual partition and complete old producer,
boxed-state transfer, B-tree upper bounds, bounded-output ledger, Diagnostic and
allocation observer. Synthetic owned states/keys, storage census and budget
adapters isolate this component. Complete ordered groups, ordinary/atomic key
constructor counts, one-state requested and actual backing savings, empty/multi
and legacy cases, zero/exact/short physical limits and complete Drop passed on
Rust 1.88 and 1.97. Sampled compiler peaks were 169,376 and 180,960 KiB under a
200 MiB stop guard. Six actual source-custody/digest checks, including the whole
older inverse and bundle controls, passed at 123,728 KiB.

Real-HIR tests compare whole receivers, success, hostile diagnostics, semantic
work and plan immutability, plus exact physical limits and final Drop. They are
authored for CI. The local adapters do not qualify full HIR, oracle integration,
the hosted JSON gate or current-head repository verification.
