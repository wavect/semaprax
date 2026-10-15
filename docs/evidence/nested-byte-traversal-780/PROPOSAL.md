# OPT780 nested immutable byte traversal: proposed implementation cut

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Source basis: a2311ae7be96f5529b63b9a8f38deef709a8e51c. Proposal only:
no production changes, compiler/runtime execution, savings or recovery claim.

## Owning contract map

- RFC-0001: checked cross-backend equivalence, explicit capability/ownership,
  persistent declaration identity, deterministic source/graph/diagnostics.
- DEVELOPMENT read map, ARCHITECTURE stage/trust boundaries, COMPLETION-MATRIX
  control-flow/ownership/useful-data rows and QUALITY-GATES full profile apply.
- BORROWED-BYTE-SLICE-FOR-TRAVERSAL-V1 owns current lowering, receiver/provenance
  proof, Graph v83, wire tag3 and construction accounting. Its explicit nested
  refusal is the concrete gap against issue780; existing code is substantial.
- OWNED-BOUNDED-VEC-FOR-TRAVERSAL-V1, SCOPED-RECORD-VEC-TRAVERSAL-V2 and
  SCOPED-RECORD-ROW-LOAN-V1 retain their exact closed mixed/row/Vec profiles.
- WHILE-LOOPS-V1, LEXICAL-STRUCTURED-EXITS-V1 and RFC-0003 own normal/abrupt
  completion, nearest target, per-iteration settlement and canonical cleanup.
- PORTABLE-INDEXED-BYTE-DATA-V1 and SHARED-LOAN-PLAN-V1 own authenticated
  byte_get/Option reads, exact roots, overlapping shared freezes and old caps.
- PROJECT-SEMANTIC-CACHE-V1 / PROJECT-FRONTEND-CACHE-V1 and WORKSPACE-SEMANTIC-
  GRAPH-V1 own decoded/current replay and independent structural path joins.

## Admission and public behavior

Add an additive byte-traversal v2 contract, keeping v1 as the independent old
profile. Admit only source `for inner in inner_view` nested within byte-only
source-for ancestry, where every view independently names an immutable available
Slice<u8> binding with current authenticated provenance. Nesting uses existing
expression depth, loan/point/edge limits and work budgets, with no new allowance,
quota reset or arbitrary new depth cutoff. Same view, subrange aliases, separate
roots and borrowed parameters can compose. All items are immutable fresh Copy
u8 bindings. Existing shadowing/rebinding diagnostics remain exact.

Every ancestor view/root remains frozen until its OWN lexical region ends.
An inner freeze ending must not release an outer freeze on the same root.
All levels snapshot their own length once on entry, visit ascending indices,
and preserve left-to-right evaluation, lazy calls and first selected failure.
Continue advances only the nearest loop once; Break leaves only that loop;
Return settles every exited region and reaches function postconditions. No
labelled exits, row-loan escape, captured view or owned-copy workaround is added.

Mixed nesting with Vec scalar, record-row or owning iterator loops remains
refused, in either direction. Computed/mutable sources, item shadow/reassign,
generic-instance byte traversal, unsupported effects/aggregate loop bodies,
forged/missing provenance and owner mutation/renewal/transfer remain refused.
An ordinary nested while retains its existing independent rules; it cannot
serve as a loophole for a mixed source-for ancestry.

## Source admission

Current while_rules rejects ALL nested For/ForBorrow/ForOwn before typed body
checking. Replace only the byte-only route with an explicit active traversal
profile carried by iterative enter/leave frames. Each nested source is checked
in its current ordinary scope, not guessed from its text/name. Preserve the
full old scanners in a compiled cfg(test) child and select them for full v1
negative controls. Keep non-byte entry behavior and diagnostic order intact.
Maintain outer freeze IDs/loans across child scopes and release exactly the
inner freeze at its exit; existing span+item IDs distinguish the statements.
Avoid rescanning every entire ancestor body for each nested loop. Ancestor
source/owner protections live in contextual state checked during the ordinary
iterative statement walk, with genuine backing paid before allocation.

## Independent HIR proof and all receiver consumers

The existing region() calls frozen_body on the whole authored subtree and
rejects ANY receiver there. Simply removing that rejection would admit forged
nested receivers and introduce O(nodes*depth) rescans. Separate exact local
region-shape reconstruction from complete contextual census.

Build one invocation-local borrowed ByteTraversalCensus per exact immutable
function/root. It holds authenticated region/receiver associations and current
provenance, using ordered borrowed identities with exact expression-reference
checks. No source spelling, cached bool, pointer by itself, hash or ID alone
is authority. Reject duplicate identities independently where necessary.

The census walks enter/leave frames once, retaining only active byte regions.
A counted ordered index of frozen ValueIds covers every active length, counter,
item, discard, receiver payload, view and provenance root. Nested same-root
entries increment counts; popping the inner frame decrements only its entries.
Assignments/rebindings/shadowing against ANY ancestor are rejected. Hidden
payload reads are authorized only inside the exact matching receiver; a raw
receiver elsewhere remains detached. Current nested closure handling exposes
only captures; separately validated bodies cannot borrow outer authorization.
Preserve current local-shape/provenance errors ahead of final detached-receiver
census error for all old inputs.

Important actual callers: lexical_exit/wrapper::source_for_wrapper_with calls
region(); cleanup_plan/build/lexical_break and native_emit/expression/
lexical_break and lexical_lookup consume that wrapper. They must NOT silently
switch to shape-only proof. Thread the exact borrowed completed census through
actual per-function owning contexts where reused. Existing public/standalone
wrapper APIs continue complete checked replay when no matching census is held;
no permissive flag/cache fallback. A census is derived from current immutable
HIR, never transported/persisted, and expires before its borrowed program can
change. It supplies semantic proof only, never allocation/commit authority.

Loan-plan traversal reconstruction must independently recognize every complete
nested byte region, hold separate TraversalFreeze loans and retain canonical
CFG/loan order. The caller's existing shared work units/caps remain real; no
estimated credit is granted for a census. Any new retained scratch, temporary,
node, Vec/result/frame carrier, replacement and final Drop frontier must use
the existing receiving payer before construction. Legacy no-owner paths remain
explicitly unqualified rather than silently acquiring a fictitious payer.

## Lowering, backends, wire and source projection

Reuse existing byte_len/byte_get/Option receiver lowering recursively with the
same structural paths; every inner path extends its actual authored-body path.
Do not mint an alternate backend loop or unchecked byte. Source formatting
keeps nested for source; independently reconstructed workspace call/type edges
must match exact nested identities. All interpreter, native C11 O0/O2 and Core-
Wasm receiver/latch consumers must run the same checked nested corpus, including
failed byte access selecting their existing invariant failure, never a default.

No new AST/HIR node or latch tag is proposed: BorrowBytes tag3 already expresses
all levels. Graph/checked-cache selection follows ACTUAL current version owners,
not stale v1 prose. If no new wire fact is needed, preserve existing bytes for
all non-nested programs; any discovered new serialized fact requires its owning
version bump, not an exemption. Cached nested HIR is decoded then independently
replayed, and warm Project caches must match cold current-source outcomes.

Agent help and compiler-checked example change together with additive v2 spec,
source/HIR/graph/backend controls, and source inventories. Keep every complete
old source/archive and all old mixed/hostile assertions; explicitly reroute
only the old positive-now-admitted nested fixture through the compiled v1
oracle while asserting its ordinary v2 success separately.

## Focused owning controls required in the next combined batch

1. Independent manual index-loop differential on interpreter/native O0/O2/
   Core-Wasm: empty inner/outer, singleton, all bytes, two/three levels, same
   root/subranges/separate roots/parameter views; exact source/HIR/graph/wire.
2. Nearest inner/outer continue/break, return, lazy skipped failure, body error,
   failed byte access, complete selected status and canonical cleanup order.
3. Full old v1 projection on unchanged inputs; retained old nested refusal;
   current mixed Vec/row/own refusal, shadow/mutable/computed/escaping/generic
   negatives. Stable full diagnostic fields/order, not code-only summaries.
4. Hostile HIR for every ancestor identity/root/counter/view/payload mutation,
   same-typed swapped receiver/provenance, detached/duplicated receiver, stale
   cache and foreign function/closure census. Public standalone wrapper checks
   must refuse the same hostile inputs without a borrowed census.
5. Actual same-payer every-acquisition refusal, exact/one-short/zero/sticky,
   no allocation after selected E, complete partial/final Drop; same-root inner
   exit retains outer loan and all original cap/work refusal controls unchanged.
6. Cold/warm/mutated Project cache full revision and current provider parity,
   module/source-custody joins, scoped Clippy/full quality profile at root only.

This is a bounded implementation proposal, not evidence of completion. Issue780
also retains any other unmet acceptance/qualification until actual execution.
