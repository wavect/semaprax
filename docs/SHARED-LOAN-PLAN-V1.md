# Shared Loan Plan v1

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, device/simulator, or separately provisioned evidence
below retains its narrower scope; public promotion, registry publication and
broader product completion remain separately gated.

Audience: language users, tool authors, compiler contributors, and reviewers.

Shared Loan Plan v1 records proof for synchronous immutable borrows admitted
by verified HIR. Each loan names its owner place, provenance, and path-sensitive
lifetime. Source verification and HIR replay use those facts to reject moves,
transfers, or assignments that overlap a live loan. The compiler-owned plan
creates no runtime reference, cleanup action, capability, or authority.

This is the required foundation for nested owned-aggregate borrowing. The
additive [Projected Owned-Byte Field Shared Borrow v1](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md)
admits only one direct field profile; general nesting remains closed.

## Closed admission

Version 1 attaches proof only to non-escaping shared immutable loans whose
ultimate verified root is move- or mutation-capable `own` storage within one
resolved function. A program whose candidates are all borrow-rooted
requires no ownership-conflict attachment and preserves its legacy Graph
schema. The borrowed value is used synchronously and cannot be returned,
stored in an aggregate, captured, exported, sent to another task, or cross a
function boundary except through an already admitted synchronous borrow
parameter whose result contains no borrow. The additive
[Synchronous Borrowed Text Closures v1](CLOSURES-BORROWED-V1.md) permits one
direct `borrow str` parameter descriptor to be captured by a closure that is
created and invoked only in that same synchronous frame. Its independent
source/HIR scope proof forbids aliases and escape; it does not permit capturing
an own-root local view or shorten the caller's owner loan.

Every attached loan has a dense resolved-function-local identity
`0..loan_count` and an authenticated expression `site` recorded separately
from its control-flow start. Identities are assigned in canonical verified-HIR
discovery order, are not stable across source revisions, and are never
addresses, runtime handles, capabilities, or cleanup flags. A plan contains at
most **256 loans**.

## Owner places and provenance

Each loan names the exact verified owner `Place`: its resolved root value and
the canonical projection vector derived from HIR. Own-root loans are normally
unprojected. The one additive field profile admits exactly one stable field-ID
projection for `bytes_as_slice` on a named `own` flat monomorphic Owned Byte
Record v1 place. A root loan records no parent. A reborrow records the
exact live parent loan and preserves the ultimate owner place and provenance
chain. Parent identities must precede their children, every chain must be
acyclic, and a child cannot start before or end after its parent.

Version 1 accepts multiple simultaneous immutable loans. Equal-root sibling
loans and reborrows remain separate loans with separate uses and endpoints;
different owner roots remain independent. The overlap replay compares complete
places, including prefix/descendant relationships, so sibling fields are
independent while a field and its parent overlap. While any overlapping loan is
live, the verifier rejects assignment, unique transfer, `match own`, or move of
the owner place or an overlapping prefix/descendant. Shared
loans never make an unavailable owner available and never alter Cleanup
Inventory or CleanupPlan liveness.

Mutable loans, ownership through a borrowed/shared boundary, and every
projection outside the exact direct owned-byte field profile remain closed.

## Last use and path edges

A loan's `site` identifies the authenticated borrow expression, while its
`start` identifies the canonical Before program point where that loan becomes
live. These coincide for direct views and borrowed calls; per-arm `match
borrow` loans start at the arm or guard entry below their shared match site.
The lifetime ends after the last use on each reachable control-flow path, not
merely at the closing source brace. Branches can therefore produce several
termination edges for one loan. The plan identifies every edge on which a loan
stops being live and records
the destination program point as an end summary. A path with no later use ends
the loan at the earliest authenticated edge that preserves all observable
left-to-right and lazy-control-flow behavior.

A synchronous borrowed call that completes a contract or body root has no
outgoing edge after that root. If the ordinary lifetime construction otherwise
has no termination edge, completion is recorded on the incoming edge to that
exact root's After point. Only non-executable terminal After points of the
function's requires/body/ensures roots are excluded; evaluated call arguments
and uses remain live, overlap rejection still runs, and child lifetimes must
remain contained by their parents. Independent replay derives the same edges.
Already accepted plans use their original path without additional work charges
or changed bytes. This handles terminal contract calls without a new runtime
borrow carrier or cleanup action.

`Try` and `TryOption` operand completion has two authenticated successors:
normal unwrapping and immediate residual return from the enclosing contract,
body, or postcondition root. A loan used later remains live only on the normal
successor and terminates on the residual-return edge.

The canonical `semaprax.loan-plan.v1` carrier records declaration-ordered
loans with their site, cause, origin, parent, start, end program points, and
exact terminating CFG-edge identities. Its `endpoints` vector contains the
canonical Before/After program points and records incoming/outgoing may-live
unions plus starts and kills; its `edges` vector carries the exact
path-specific live vector. Loan vectors use dense loan-identity order. Graph
projection preserves them exactly and must not sort, deduplicate, infer, or
repair them; execution backends consume no runtime loan carrier.

The plan contains at most **4,096 program points** and **4,096 CFG edges**.
Each canonical build, including the rebuild used for independent replay, has a
deterministic **1,000,000-work-unit** ceiling over examined HIR nodes,
control-flow edges, loan relations, and overlap checks. Exceeding any bound
fails closed before Graph projection or backend admission; the implementation
may not truncate, merge, sort, or repair the plan.

An `SPX-H006` checked-work refusal identifies the resolved function's persistent
identity and, when available, its body location. Split a large function into
smaller helpers and keep borrowed views local to their last use. Splitting files
alone does not change this per-function bound. Missing source provenance is not
replaced with a fabricated location; existing diagnostic blame remains intact.
This recovery guidance does not raise the limit or change admitted loan plans.

Checked work counts actual deterministic traversals. Within each canonical
build, forward reachability is derived for all loans together: each point
holds one loan-ID bit per reachable start in one through four 64-bit words.
Seeding pushes reverse dense loan-ID order onto a fixed point stack; changed
successors move to its front in reverse canonical successor order. Masks
propagate until no bit changes. The queue has at most one pending entry per
point and two fixed link slots and a membership byte per point. Work charges
each initialized word, seed, dequeued point, changed-successor queue update,
and actual edge-word merge. Reverse liveness retains the exact per-loan seed traversal and stops at
that loan's own start; reachable-seed examinations are charged as well. Parent
recomputations retain reverse child order and overwrite the parent with that
child's exact seed-derived live set, rather than unioning sibling results.
Edge liveness examines only canonical outgoing edges whose source is live for
that loan. Retention does not cross a function build or independent replay.
The ordered plan and Graph bytes must match independently traversed per-loan
reachability and the full-edge membership scan; the work ceiling stays fixed.

The maximum forward payload is 4,096 points times four words times eight bytes
(**131,072 bytes**), plus 8,192 `u16` queue links (**16,384 bytes**) and 4,096
membership bytes (**4,096 bytes**). The four boxed-slice carriers and queue
indices have fixed `size_of`-derived overhead. Each expression's paired points
contribute at most 74 bytes to the workspace resolver's structural footprint
proof; its fixed per-function bundle includes the carriers. The unchanged
structural expansion factor and 64 MiB workspace budget cover these footprints
through compile-time assertions. This bounds the additional allocations; it
does not claim a measured peak-memory reduction or change the existing live
sets, CFG, retained proof capacity, or their accounting.

Alias provenance reconstruction first traverses borrowed immutable prefixes,
checks cycles, and counts the exact projection payload. It then materializes
one final projection vector in canonical prefix order. Both alias passes debit
each visited link before activity; the first pass also debits each inspected
projection. Zero-projection chains need no materialization pass. These are
planner visits, not a byte/comparator-work measurement, under the unchanged 1,000,000-work ceiling;
this does not claim identical consumed work or admission at the old boundary.
The materialized vector, carrier, and newly cloned projection-ID text are
reserved before allocation on the existing active physical ledger. Cached reconstruction retains the complete occurrence census and every map,
recipient, live-set, and old-proof allowance. For ordinary aliases only, one
materialization removes the prior per-link accumulated-vector multiplier.
Any borrowed Match with a direct nonempty Variant arm retains the original
multiplier, conservatively covering native receiver aliases that replicate a
shared scrutinee prefix. Cached reconstruction also pays the actual reservation
on the same ledger; this conservative double debit is not a RequestViews
cache-fit or measured budget-recovery claim.

## Independent replay

Validated HIR does not trust an attached Shared Loan Plan. Independent replay
rebuilds the canonical plan from typed HIR and its verified control flow, then
requires exact agreement. It rejects, at minimum:

- sparse, duplicate, reordered, or out-of-range loan identities;
- a forged owner root, projection, provenance chain, or parent relation;
- a missing use, premature or extended endpoint, or an endpoint on the wrong
  path;
- a child outside its parent's live range;
- a move, assignment, or unique transfer overlapping a live loan;
- a missing or extra loan, endpoint, or edge.

The plan is descriptive proof data. Neither a serialized plan nor its digest
authorizes source changes, execution, memory access, or publication.

## Graph v84 finite record composition

The finite owned-record discard profile independently authenticates its actual
current HIR sites before composing flat local byte loans with nested storage in
Graph v84. Complete loan-plan validation, HIR and capacity authentication, and
canonical cleanup rebuild/replay remain required. This separate frontier does
not widen the frozen nested graph profiles, native-import flag admission, or
parts-only projections. Other CopyDiscard programs retain their existing inner
profile restrictions. The owning finite record specification records authored
regression controls and their pending executable qualification.

## Graph v23/v24/v32/v33 and compatibility

A program carrying a nonempty authenticated unprojected Shared Loan Plan v1
selects `semaprax.graph.v23`. The exact projected field profile selects
additive `semaprax.graph.v24`, which adds the stable projection and resolved
field type to byte-slice provenance. Both project dense loan identity, exact
owner place, parent provenance, and canonical path edges. Consumers must reject
either schema until explicitly implemented; v24 must never be read as v23.

A module that also requires owned-variant Cleanup Inventory v2 or CleanupPlan
v6 selects additive `semaprax.graph.v32` for unprojected loans and
`semaprax.graph.v33` for projected loans. These combined schemas serialize the
complete Graph v22 conditional cleanup facts together with the corresponding
v23/v24 loan and provenance facts. They never relabel the composition as one
of its incomplete base schemas. Evidence flows remain fail-closed until they
explicitly admit v32/v33.

Programs that require no Shared Loan Plan preserve their legacy Graph version,
bytes, Cleanup Inventory, and CleanupPlan schema and meaning. Shared Loan Plan
v1 does not widen CleanupPlan v6, Graph v22, the interpreter value model,
native C11 layout, Core-Wasm layout, or any public ABI.

Semantic Workspace v1 admits v32/v33 as exact source-schema facts and rejects
unknown or composite-looking spellings. Change views preserve the same schema
selection without masking either ownership contract.

## Executable evidence

The local evidence gate owns:

- canonical dense identities, unchanged unprojected Graph v23 schema selection
  and serialized fields, and
  deterministic projected-provenance Graph v24 fixtures;
- exact unprojected owner roots, the implemented direct stable-ID
  owned-byte field profile, direct loans, parent reborrows, and multiple
  equal-place shared loans;
- straight-line and branch-specific last-use endpoints, including a move that
  becomes legal only after every overlapping path has ended, plus the distinct
  normal and residual-return successors of `TryOption`;
- rejection of premature moves and transfers plus hostile mutations across
  loan, endpoint, edge, ordering, and omission surfaces;
- the exact 256-loan boundary and 257-loan rejection, plus proof that a large
  loan-free function does not acquire the plan's CFG limits;
- deterministic resolved-HIR fixtures at exactly 4,096 program points and
  exactly 4,096 CFG edges, independent canonical rebuild at both boundaries,
  hostile boundary-carrier reorder rejection, and isolated first-overflow
  diagnostics (the first representable point overflow is 4,098 because every
  expression contributes paired Before/After points);
- one real 256-loan canonical fixture whose disconnected typed Boolean roots
  are composed from independently measured production-planner charge deltas
  to consume exactly 1,000,000 work units, plus exact replay, a limit-minus-one
  rejection on the final unit, and a production-limit overflow that stops at
  unit 1,000,001; and
- preservation of Graph v22 selection with no loan carrier when no plan is
  needed, while the existing cleanup-plan gates remain the cleanup authority;
  and
- deterministic Graph v32/v33 composition of owned-variant conditional cleanup
  with unprojected or stable-field-projected loans, plus Semantic Workspace and
  checked-HIR cache replay without graph-builder accounting drift.

The dedicated exact-boundary fixtures are part of the implemented v0.4.0
regression corpus, with the accepted hosted-green baseline. Source presence
alone is not execution evidence; changed implementations require fresh checks
at their own claimed revision.

Interpreter, native, and Wasm regression gates must continue to prove that
admitted programs retain identical observable behavior, but they consume no
runtime loan object and establish no borrowed ABI. Evidence described here is
local repository evidence only; hosted promotion is explicitly unclaimed.

## Non-claims

Shared Loan Plan v1 does not claim mutable borrowing, general lifetime
inference, escaping borrows, borrow-valued results, closures, async or
concurrent loans, cross-file or cross-task lifetimes, regions, arenas,
retain/release, ARC, raw pointers, foreign ownership, or a public/native/Wasm/
Component ABI. Beyond the exact direct field profile, it does not admit nested
owned aggregate borrowing, generalize
`match borrow`, execute cleanup, prove process memory safety outside verified
HIR, or complete either ownership row in the completion matrix.

## Private validation storage

Core HIR validation still independently rebuilds each function and instance's
loan plan before using its canonical identities or liveness. Its private
canonical-origin directory moves each rebuilt `Place` once and associates each
live endpoint with that origin's ordinal. Ordinals carry no source or runtime
authority, and function-local loan IDs retain their distinct function context.
All projection order, unknown-endpoint, duplicate-liveness, and start/site checks
retain their existing order. Attached plans remain independently validated.

This storage optimization does not reduce a forecast, change a limit, or give
pure validation a physical account. Complete caller prepayment for validator
storage remains a separate owning contract; the relative allocation control is
not a whole-validator funding or application-fit receipt.

### Caller-funded canonical lookup directories

The ordinary inspection caller constructs its function/permit directory, then
helper ModuleIndex, then runs the original ordered constructor preflight. A
private seed binds the exact immutable program and authenticated lookup before
any independently rebuilt plan is admitted to canonical lookup storage. The
caller prepays actual Vec probe/global backing with the active builder account;
an absent account is explicitly the legacy unobserved policy. Spare-capacity
and zero requests do not clear an active sticky refusal. Full replacement
capacities, including overlap with old backing, are charged before growth;
allocator excess is charged after observation and fails closed.

Private sorted indexes carry source ordinals solely to preserve the original
endpoint/loan error encounter order. Local duplicate LoanIds retain last-wins
lookup; unknown endpoints precede duplicate liveness, endpoints precede start
checks, and each complete current plan is processed before rebuilding the next.
Canonical vectors are never sorted or repaired. Origins move once in original
loan order. Closure validation reruns constructor preflight, borrows only this
exact-program inventory, and keeps fresh scopes/identity sets. Equal cloned or
reordered programs cannot receive the inventory. Ordinary compatibility
constructors retain the original owned indexes for independent oracle controls.

This prices only the explicit lookup/probe vectors. Independent standalone loan-plan
builder scratch, semantic diagnostics, other validator storage and temporary
closure function construction remain open payers. The standalone/legacy
`loan_plan::validate_program` remains an independent rebuild; the joined
invocation route below removes only its repeated terminal rebuild. No forecast discount, quota change,
whole-validator funding, application fit or runtime advantage follows from this
source tranche. Storage observers exclude input plans and fixed ledger setup
explicitly; complete compiler/application inequalities remain unchanged.

### Private canonical CFG scratch carrier

The exact-current-member funded planner uses one affine caller reservation for
roots, pending and child vectors, identity/root/point directories, edge storage,
flat adjacency, starts/kills rows and endpoint live sets. Vector growth retains
the existing geometric policy and prepays the full requested replacement
payload while old backing remains paid. Observed excess capacity is charged and
refused; dropping prior backing never refunds that invocation's cumulative debit.
The explicit local vector, directory, adjacency, CFG and result carriers are paid
before construction. No work limit, source identity or canonical plan order changes.

Private ordered directories use unique AVL boxes. An absent key prepays the
actual node heap layout and node initializer before construction or publication.
Replacement retains the old key and checks the same sticky payer even when it
allocates nothing. Rotations move unique nodes without allocation or fallible
payment; lookup and insertion preserve logarithmic height for arbitrary key
orders. Edge and live-set extraction visits each node once in original key order into
one exact paid vector. It never sorts or repairs a canonical plan. The original
public BTree builder and complete saved independent builder remain available.

Source controls compare ascending, descending and interleaved insertion-heavy
whole directory receivers with the original BTree policy, and compare complete
plans, CFG point/edge/adjacency identities and original checked-work equations.
Actual owner construction, receiving, replacement, extraction and Drop are
observed together; exact and one-short allowances retain sticky refusal and
failed-node publication leaves the complete prior directory unchanged. Per-node
allocation overhead and CPU tradeoffs require execution evidence; this source
change claims no efficiency improvement or issue closure.

The source owners, pending qualification, use the selector prefix
`loan_plan::cfg_directory::tests::`:

- `whole_ordered_directory_ascending_descending_interleaved_receivers_keep_log_height`
- `exact_short_and_sticky_directory_payment_preserves_prior_publication`
- `cfg_carriers_preserve_complete_canonical_graph_and_original_work_equations`
- `complete_saved_planner_constructor_receiving_and_drop_have_exact_short_sticky_payment`
- `duplicate_keys_retain_original_key_and_detached_payer_cannot_publish`

This is the private funded CFG route only. Standalone unobserved rebuilding,
legacy semantic diagnostic rendering, unrelated validator storage and temporary
closure function construction retain their separate funding seams. Existing
whole-subtree/application inequalities and original hostile owners remain intact.

### Private planner discovery directories

Candidate aliases, bound identities, ownership and root-ownership memo maps,
planner aliases/bindings/ownership/native match-parent maps, nonconsuming and
terminal-root inventories, and parent/alias cycle sets now use the same exact
caller-paid AVL nodes. The alias projection cycle set borrows stored ValueId
handles. Source traversal, cache-first lookup, binding-before-parent precedence,
last-wins replacement, checked-work increments, complete projection construction
and full canonical result order remain unchanged. The private borrowed receiver
seam does not change public directory types or admit a new language/provider shape.

Complete pre-change funded receiver, candidate, planner, native inventories,
pattern walker and traversal bodies remain separate test-only BTree oracles;
the public standalone planner and older saved planner remain independent too.
Actual whole owner construction, transfer/receiving and Drop are measured against
both policies without normalizing requests or asserting relative savings.
Literal alias/binding/seen heap layouts and cumulative debits have exact/one-short,
sticky refusal, cycle/missing-root/binding-priority, nested UTF-8/NUL projection,
complete error and checked-work controls. Full renewal and nested collection/
variant application corpora remain. The source owners, pending execution, are:

- `loan_plan::funded_flat::directories_tests::complete_old_btree_receiver_constructor_drop_exact_short_and_original_applications`
- `loan_plan::funded_flat::directories_tests::complete_old_btree_plan_and_checked_work_first_errors_keep_exact_priority`
- `loan_plan::cfg_directory::planner_receivers_tests::alias_binding_parent_receiver_literal_actual_heap_debit_exact_short_sticky_drop`
- `loan_plan::cfg_directory::planner_receivers_tests::alias_parent_cycle_missing_and_binding_priority_match_complete_old_receiver`
- `loan_plan::cfg_directory::planner_receivers_tests::borrowed_alias_seen_receiver_literal_heap_exact_short_drop_and_full_old_verdict`
- `loan_plan::cfg_directory::planner_receivers_tests::borrowed_alias_projection_cycle_and_work_diagnostics_equal_complete_old_constructor`

### Private seed, liveness and predicate sets

The private planner uses `CfgDraft<cfg_directory::Set<u16>>`; the public and
saved BTree draft type remains the default. Every seed singleton/clone, live-node
receiver, parent propagation union, traversal allowed-identity set and nominal
predicate active-declaration set uses exact paid AVL nodes. Private materialization
and native parent helpers borrow the corresponding draft type without inspecting
seed storage or changing canonical result order. Complete old paid call-argument,
predicate and materialization helpers are saved independently too.

Borrowed iteration prepays precisely two optional node-reference fields before
constructing its cursor. It holds no heap traversal scratch: minimum/successor
lookup preserves original ascending seed/node order in O(n log n), compared with
the original BTree iterator's O(n) traversal. The existing CFG point bound (4096),
not the 256 loan-count bound, bounds live inventories. Consuming ordered extraction
retains its existing linear one-visit drain. Removal checks the same sticky payer
with reserve(0) before mutation and moves/rebalances only existing owned boxes;
neither removal nor Drop refunds cumulative debit. All original seed, LIFO,
predecessor reverse, parent iteration, edge traversal and checked-work equations
remain unchanged. The shared overlap helper only generalizes its borrowed draft/
node-membership interface; its diagnostics and first-conflict order are untouched.

Source controls retain whole old/new constructors and original application corpora,
full rows/edges/error/span/help/work results, literal actual cursor/node layouts,
heap/debit/Drop equations, exact/one-short frontiers, every denied cursor byte,
sticky present/missing removal refusal and insertion/removal adversaries through
4096 entries. Additional owners, pending execution, are:

- `loan_plan::cfg_directory::live_receivers_tests::paid_seed_clone_cursor_and_remove_literal_heap_debit_exact_short_full_drop`
- `loan_plan::cfg_directory::live_receivers_tests::paid_cursor_refusal_and_sticky_removal_preserve_complete_original_set`
- `loan_plan::cfg_directory::live_receivers_tests::complete_old_seed_clone_and_all_avl_removal_shapes_keep_order_and_actual_receivers`
- `loan_plan::paid_live::tests::paid_seed_live_and_edge_receivers_equal_old_full_rows_and_checked_work_frontiers`
- `loan_plan::paid_live::tests::paid_full_plan_late_overlap_and_first_work_error_equal_saved_btree_receiver`

Opaque BTree node estimates remain only in the explicit public/legacy or saved
receiver policy, outside the converted private directory route. Semantic diagnostic
construction and standalone rebuilding retain separate funding seams. This source
tranche makes no whole OPT792 closure, request savings, application-fit, measured
CPU improvement or executable qualification claim. Physical payer frontiers can
change while ordinary full-plan/error/work results remain exact.

## Invocation-local deferred full comparison

The ordinary complete HIR validator and the private byte-partition validator
compare each entire independently rebuilt plan with the exact current
function attachment before consuming the rebuilt storage. A private borrowed
receipt retains the first mismatch in functions-then-instances order; it does
not publish a diagnostic until the original final shared-loan validation
boundary, after all preceding HIR, identity and cleanup checks. A receipt for
an equal copied program is refused. Attached plans, storage projections,
serialized markers and cache hashes cannot construct this receipt.

The public standalone loan validator retains its independent rebuild. The
joined route removes its repeated final rebuild, without refunding the first
build or discounting forecasts. It changes actual allocation and debit counts,
so low-budget refusal thresholds are not claimed identical. Independent
builder scratch and detailed diagnostic funding retain their existing scope.

## Private flat CFG storage tranche (OPT-792)

Source implementation; focused executable qualification is pending. The private
CFG keeps its original sorted edge vector and work traversal order. Successors
and successor edge indices borrow contiguous runs of that vector. Predecessors
use one flat backing with exact per-point offsets, preserving the old per-edge
push order. The 4,096-point/edge and 256-loan limits remain unchanged.
Evaluation-child discovery borrows fields and operands in the original order
instead of allocating a temporary child vector. Closure bodies remain excluded
from capture evaluation; structured control still owns its separate CFG rules.

The saved c8a planner has independent nested adjacency, original child vectors,
queue, reachability, liveness, complete plan materialization and work accounting.
It explicitly excludes newer CopyDiscard statements before shared shape helpers.
Owning controls compare complete plans and diagnostics, work-limit ladders,
child pointer order, and actual complete planning through Drop. Detached padded
functions are structural planner fixtures, not complete checked-source evidence.

This tranche reduces actual scratch construction; it does not fund every CFG
allocation. Roots, directories, queue/liveness and diagnostics still require the
explicit caller-payer work in OPT-792. Original whole-subtree allocation/debit
bounds and application corpora remain intact. No public schema, cap, physical
attribution, application-fit or agent-token claim follows from these sources.


### Ordinary factored memo construction ordering

Ordinary factored replay assigns private DAG construction ordinals in one
`SkeletonWork` invocation. Memo ordering uses ordinal and continuation, while
every hit and replacement checks the exact retained `Rc` pointer. An ordinal is
storage metadata, not a source/session certificate or selected-helper lease.
Detached nodes without an ordinal refuse. Same-content nodes remain distinct.

The ordinary union, sequence, concatenation and prepend constructors reserve
actual `Rc` node and nested vector backing on the existing replay payer before
allocation. Their legacy destructor remains unchanged. The memo reserves its
actual enlarged pointer/end value carrier. Existing work events remain; each
new ordinary construction ordinal and exact memo pointer check adds one unit.
This does not claim unchanged low-budget thresholds, a forecast reduction or
whole replay funding. Owned legacy diagnostic rendering remains its existing
boundary. `Observations::Ord` still compares prefix pointers; the factored
receiver does not use the enumerating path sort, whose flat prefixes are absent.
The complete selected owner preserves exact old/current/removed allocation
and byte equations in both measurement orders; narrow memo controls also use
real opposite Rc address ranks with the saved pointer-key result oracle.


One memo receives one `hir_skeleton_paths_with_index` result. That producer
constructs exactly one `SkeletonWork`, covering requires clauses, the body,
argument/guard evaluation, and ensures clauses. A modeled call resolves its
current target parameter metadata and records call observations; it does not
inline a callee skeleton or import a callee DAG. Callee functions receive their
own separate validation/work/memo invocation. The independent multihelper
control retains complete ordinary HIR validation, saved flat enumeration
against the cleanup CFG, factored comparison, and an ordinal-to-exact-pointer
census of each returned family. The ordinary ordinal counter is thus scoped to
the complete family reaching that memo, not reused across combined work owners.
### Private caller-held flat-storage funding (OPT-792, source-only)

A private affine CFG reservation captures the existing active builder ledger
and a dedicated prepaid refusal String. It creates no quota. Every request,
including zero bytes, checks the same invocation identity and sticky first
refusal. Physical failure is distinguished by an owner-held Copy tag; a legacy
semantic empty H006 is not a funding receipt. Paid refusal transfers the same
text backing without formatting or acquiring a replacement ledger.

The private receiver prepays actual flat adjacency and batched forward
reachability arrays before construction and retains the unchanged semantic
work order and complete saved planner oracle. Exact-capacity discrepancies
account excess and fail closed. Its observer encloses acquisition, all those
constructors, refusal publication and Drop, excluding only prior source/CFG
construction and the already established Budget context.

This is scoped planner storage funding. Candidate
discovery, native/pattern alias inventories and current CFG containers also
use the captured owner on the private path. Map/set payment uses a disclosed
conservative pinned B-tree node upper for each genuinely new key and one
possibly held initial root, separately from cloned projection payload.
Draft/live inventories, native arm-parent maps, traversal freeze scratch and
canonical result materialization also use the captured owner, including the
actual starts/kills vectors and compact-list shrink replacement while old
backing remains live. Projected-record discovery retains the old generic
substitution verdict through a paid iterative twin: frames, actual type
argument directories, callable result boxes and cloned argument backing are
paid before construction. Physical failures propagate; a discarded semantic
substitution error alone retains the original false predicate result.
Lexical preflight remains at its original position. Its temporary type
comparisons are borrowed, actual generic execution-ID clones are paid, and
its borrowed-record freeze inspection uses the same explicit caller owner.
Rejected execution-owner probes no longer construct discarded diagnostics;
selected missing/ambiguous diagnostics keep their original text and order.
Whole private planner observers now include projected record Vecs, String
Vecs and lexical For freeze validation through exact/short refusal and Drop.
The active canonical caller acquires one owner after the unchanged ModuleIndex
and ConstructorSeed phases. Every function/instance rebuild, complete Deferred
comparison and actual append uses it before storage is lent to the pure
validator. Physical failure publishes that owner's paid text; semantic errors
retain their legacy diagnostic unchanged. A failed owner is never replaced
within the invocation. Standalone unobserved planning remains the original
receiver, and raw-plan storage-only tests retain their independent payer.
Upstream seed/index work, later validation allocations and legacy semantic
diagnostic text remain outside this scope. Original final independent loan
validation, forecasts and every literal limit remain unchanged. These source
controls are unexecuted and establish no application fit or whole-validator
funding claim.

The initial resolver attachment also captures one existing CFG owner across
its authored-function and concrete-instance groups. Each full current-member
planner invocation and exact staging backing is paid before allocation. The
first successful plan precedes staging allocation, preserving the original
fallible collection's semantic-error order; the paid directory is deliberately
exact and does not claim the original fallible shunt used exact capacity.
Authored plans are installed only after their complete group succeeds and
before instance construction, whose group has the same publication boundary.
A physical failure consumes the captured owner's prepaid refusal text; a
semantic diagnostic is moved unchanged. No new allowance or refund is created.
Later independent canonical reconstruction and full comparison remain required.
The added saved-original attachment, authored/instance late-error, actual
planner-through-Drop and exact/short/sticky controls await execution. This
extends initial planner funding only: original whole resolve→validation→replay
and resolved-HIR Drop observers remain unchanged, and no complete pipeline
funding or application fit is established by this source change.

Linked-workspace cleanup metadata rebuilds now reuse that same exact-current-member
initial attachment boundary. Their original inventory construction, canonical
cleanup attachment and final independent HIR validation remain unchanged. The
linked provenance scan borrows complete authored functions followed by instances,
with the same derivation and first-error order; no backing debit is discounted.
Declaration-index Drop skips queuing scalar roots and empty nominal argument lists
and retains its iterative nested-type disposer. Independent original cloned-input
and queue-through-Drop controls are authored; their current-head execution remains
pending. These changes do not establish whole-pipeline funding.

### Private point-domain live scratch

The ordinary funded retained planner stores its temporary live CFG point sets
as exact paid u64 bit words. The first insertion uses the already computed CFG
point count; empty live sets allocate no backing. Membership and ascending
extraction preserve the prior set semantics and all existing logical work
increments. Larger forged u16 keys retain their old membership through paid
replacement growth, rather than receiving a new admission bound. Seeds, parent
union scratch and other directories retain their AVL policy; attached plan
vectors and canonical order do not change.

Full Nodes/Result receiving carriers, iterator carriers and simultaneous old/new
word backing are paid to the same authenticated affine byte owner before actual
construction. Drop grants no credit. Foreign/restored and selected refusal stay
sticky, including duplicate/spare-capacity operations. The complete old AVL
liveness producer and full old planner remain independent test receivers. Sparse
sets can require more bytes; no application fit or exclusive failure attribution
follows from source accounting. The source-only contract, exact whole-source
inverses, four new owning selectors and bounded review are retained under
`docs/evidence/dense-loan-live-scratch/`; actual qualification remains pending.

### Owned paid materializer inputs (OPT-932, source-only)

The private `paid_materialize` receiver consumes its existing draft owner in
order. It moves each origin projection directory and takes the corresponding
owned termination row at the original indexed per-draft frontier. A short row
inventory still reaches that indexed failure; it cannot silently truncate an
owned zip or bypass an earlier empty-termination diagnostic. Termination order,
adjacent `ends.dedup()`, parents, schema selection, and all semantic work remain
unchanged. Existing site, cause and program-point shared-handle clones remain.

Starts and kills directories are still constructed by the complete original
loan/edge loops. Their rows then move into the unchanged paid compact-list
conversion at the original endpoint-field positions. An over-capacity nonempty
row still pays the complete replacement while the original buffer lives; empty
and exact-capacity rows retain the original conversion rules. The actual termination producer grows rows geometrically with an initial
capacity of four. Moving that spare capacity into the retained plan would
change the previous lifetime bound, so a separately prepaid lexical
Box-slice-to-Vec conversion compacts a nonexact row. Its complete replacement
backing is charged while the original remains live, and all Box/Vec, selector,
return and Drop transports are acquired first. Empty spare rows release their
old backing without a new allocation. Published termination capacity remains
exactly its length as in the immediate old caller.

Separate typed banks precede consuming iterator construction, each next/terminal
turn, indexed take and its empty replacement, and endpoint-row Option/receiving
transports and Drop. No completed tariff is subtracted and no surviving clone
is credited. `paid_materialize/immediate_before.rs.txt` retains the complete
8086 immediate source, with its full same-ABI function separately executable in
`immediate_before.rs`; older before/legacy controls remain. The owning
`owned_inputs_tests` controls compare full plans, wire/graph projections, actual
allocation through Drop, exact/short/zero/sticky/floor/foreign frontiers,
malformed first-error/index behavior and unwind. The complete funded caller
also executes the immediate-old materializer through a test-only director and
checks full mutable retained capacity, allocation and Drop, including exact
and short receiving budgets. Origin production either clones exact projection
backing or enforces the exact resolved projection count before publication.
These controls are authored
but unrun; cold/small tariff regressions and actual capacity remain visible.
No application admission, net tariff, token or runtime improvement is claimed.

### Independent canonical borrowed-row receiver (source candidate)

The captured canonical-validation route independently repeats the complete
current-function, candidate, CFG, alias, ownership, live, parent, overlap and
terminal-repair derivation. Its private `canonical_sink::Source` owns that
result for one receiving invocation. The attachment is compared against every
schema, loan, endpoint and edge field in original order; it supplies no size,
origin, candidate, pruning decision or lookup authority. Empty independent
results compare the complete empty schema too. The original complete planner,
materializer, canonical caller and append remain independently executable.

The one-shot canonical receiver borrows the raw ordered rows and emits the
existing final local/identity/live lookup tuples directly. The two original
ordered-union vector producers and their full fees remain. No intermediate
endpoint lookup vector or callback per endpoint is introduced. Lookup ambiguity
is selected by the earliest original source ordinal, with unknown-loan refusal
preceding duplicate liveness at that same occurrence. A mismatch remains in the
original late Deferred frontier, behind earlier semantic and cleanup errors.

New Source/Parts/closure/argument/Result, borrowed loan view, sticky-comparison,
cursor, drain and receiving/drop roles are separately paid before formation by
the captured CFG reservation. A separately paid checked pass sums independent selected edge-row lengths
before one probe reservation. Selected unions are subsets of these rows; duplicate
branch memberships can overreserve and all actual upper capacity is paid while
old backing remains live. Empty/cold and branching inputs can cost more; no completed fee is refunded. Independent
CFG scratch survives through receiving, so complete high-water and Drop must be
observed. Draft origins retain the exact projection capacity established by
clone_place/funded::resolve; arbitrary external drafts cannot enter this route.

The source candidate includes complete old/current caller, field mutation,
late-priority, work-cut, foreign-owner, exact/short/zero/floor/sticky and actual
allocation-through-Drop controls. Historical stage-only invoice controls still
execute their complete immediate-before receiver; corresponding new raw/shared
controls independently measure the new route's invoices. These controls are
unrun. No compiler, backend, application-fit or measured byte-gain claim follows.


The combined source candidate keeps three explicit test stages. The complete
8086 canonical caller delegates to the complete 8086 materializer through its
panic-safe original-stage director. The immediate owned-row stage runs that
same caller with the owned V2 materializer; streamed current uses the independent
borrowed-row receiver. Both original caller bodies and both materializer bodies
remain executable. Historical stage attribution controls select the original
delegate on both sides, while new raw/shared receiving controls exercise all
three stages at independently measured invoices. The 193-loan strict allocation
and invoice gate remains against 8086 and is also required against owned V2;
the one-view shape is additional. Cold loss and each stage's actual high-water
remain visible. Whole former source metadata is retained byte-for-byte, and
current source inverses still reach the same complete 8086 targets plus the
complete immediate owned-stage targets. These are source-only, unrun controls;
combining the two changes does not add their separately observed savings.
