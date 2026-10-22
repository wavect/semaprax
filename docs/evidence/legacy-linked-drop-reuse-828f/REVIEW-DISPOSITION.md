# Bounded reused-argument Drop review

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

One distinct8929-byte tool-disabled Sonnet process completed56.1131seconds,
peak325943296bytes, exit0. Full raw payload/verdict/receipt retained unchanged.
Provider listcost0.084822USD; actual billing unknown. No compiler or runtime
execution, no retry, and no claim the original owner now passes.

All four listed blockers were explicitly conditional on omitted context:

1. Actual declaration_index.rs1164 now adds only the child registration and
   replaces the no-workspace queue with drain_legacy_type_roots(legacy_drop::dispose).
   Complete funded scratch branch stays byte-identical. The new exact source
   control compares its complete prefix to the whole old Drop archive and
   requires the precise current fallback call; no normalization is applied.
   nominal.rs27..40 drains actual case fields; map.rs81..85 sends Owned through
   NominalPayload::drain_recursive_types. elements.rs84 replaces each field type
   with Unit before callback. Thus ordinary later Owned Drop sees leaf Unit.
2. IndexMap::insert(map.rs16) accepts impl Into<V>. CaseFields implements
   From<Vec<ResolvedFieldDeclaration>> at88..92 to actual CaseStorage::Owned.
   Both direct case and record fixtures use explicit Vec targets; no collect
   inference or representation ambiguity is introduced.
3. ResolvedType at nodes.rs145..203 has no Drop impl and already supports these
   same field moves in the existing nominal disposers. Parent enum own Drop is
   the Rust move restriction; ordinary Drop on a field does not forbid moves.
4. Its complete current variant inventory has only Nominal and Function recursive
   children. Both are handled; other variants contain only scalar/text identity
   data. The unchanged whole nodes.rs dependency is bound in the source packet.

Unary continuation adoption requires no fresh backing; empty-parameter Function
spines move their Box result directly and now avoid recursive default glue.
Pending children are popped until empty. Function parameters are appended or
adopted before that result is processed. Wider frontiers may still grow ordinary
legacy scratch; this is disclosed and not paid by an ambient budget. Renamed
branching control to 'settles every allocation' so it does not imply preservation
of every original backing or universal zero allocation. It deliberately verifies
whole construction-to-Drop balance and the unchanged actual first sticky receipt.

The complete old Drop receiver is compiled via only impl/signature adapters and
consumed index. Its trailing automatic current Drop finds drained case/record/
variant maps; it can acquire no continuation. This extra empty teardown is not
omitted from the old observation. The before file's exact single-newline adapter
was checked directly against all original bytes. Metrics.allocated is usize.
Existing512 mixed-case control remains byte-identical and is separately required
for actual execution, including both orders/detached ownership/zero-allocation
assertion. Its full text archive assertion is a source guard, never runtime proof.
