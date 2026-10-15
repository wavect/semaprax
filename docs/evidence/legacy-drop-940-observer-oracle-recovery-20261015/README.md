# Frozen 940 fixture and source-custody correction

Basis: 940fcf220e47d1975f757e76f0b3401691db9358. This is a source-only
correction for selected runtime owners 27, 33, 72 and 87. No Cargo, compiler,
runtime, full gate or physical gain measurement was run for this packet.
Production code and the complete independently compiled old disposers remain
unchanged. The parent owns subsequent qualification and combined-source joins.

Owner 27 retains eight case and eight record roots, depth 512, and both nominal
and function-spine cases. Pure nominal nodes retain 16,384 allocations for their
argument vectors and identity Arcs alone; metadata and transient identity Strings
exceed the fixed observer table. The existing explicit-capacity observer receives
three pointer slots per node plus eight per root. It preallocates instrumentation
before observation and still observes the entire construction through Drop,
requires no new Drop backing, zero live bytes, valid tracking and exact settlement.
This changes no compiler budget, allocator request, root count or depth.

Owner 33 previously cloned the current fixture. Nominal payload Clone now shares
its owner; draining the nonunique old owner deliberately exposes no roots. The
old control now constructs its own actual checked fixture and uses the existing
legacy mutable dereference to detach its values into the original owned storage.
The old queue function is byte-for-byte unchanged. Its original three allocations,
two reallocations and literal (4+8+16)*sizeof(ResolvedType) assertions remain. The
zero-argument nominal control also uses an independent old fixture. Additional
uninterrupted construction/materialization-through-Drop observations check the
same exact Drop deltas and settle every construction allocation for both old and
current controls. No observer is removed or reset between construction and Drop.

Owner 72 now parses and canonicalizes each unchanged linked fixture before both
old and current workspace calls. Both execute the actual builder/linker and keep
all equality/C-output and complete-old-body checks. This satisfies the ordinary
workspace canonical-input contract instead of bypassing it.

Owner 87 explicitly checks every original twelve adapter name and the thirteenth
integrated whole-source preinverse, including that adapter's count and both full
source hashes. It still restores the complete original defaults source and
compares its bytes/hash, and keeps the independent original dispatch restoration.
The integrated adapter was already present at frozen 940; no archive was changed.

`source-only.py` verifies four immediate full-source inverses, all ten active
legacy Drop inverses, every prior inverse/archive preserved exactly, the complete
unchanged old leaf queue helper, and the exact full static-error source restoration.
The active legacy tests inverse receives only an immediate full-source prefix;
`legacy-inverses.before.json` retains the full frozen metadata. Required ten
selectors are in `pending-selectors.json`; none is claimed executed by this packet.

Actual failing log inputs are in the parent's qualification directory:
`/Volumes/Semaprax/lane-prompts/large-940f-qualification-20261015/owning-selected-{26,32,71,86}.log`.
The pointer-table and sharing explanations are source-bound causal diagnoses;
rerunning these actual selectors remains necessary to confirm the corrections.
This does not close #825 or #828 or establish net performance gains.
