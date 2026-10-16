# Lexical structured exits v1

Audience: coding agents, compiler contributors and reviewers.

Status: implementation in progress. No source, HIR, cache, cleanup, interpreter,
native or Wasm qualification is asserted by this design. OPT #734 remains Partial.

This tranche admits unlabeled `break;`, `continue;` and `return value;` over the
already admitted carrier families. It adds no residual protocol, comparator,
ambient capability, label, closure escape or new generic/aggregate family.
`?` retains its existing independently specified restrictions.

## Source and typed flow

An exit is a terminal block tail with no normal value. It is not an integer
zero, searching Boolean, discarded value or source-only/backend-only jump.
`LexicalExitKind` is Break, Continue or Return. The authored AST and HIR node is
`LexicalExit { kind, value }`, with the HIR also retaining `target: ControlTarget`.
Break/continue have no value; return has exactly one evaluated value. Source
syntax cannot mint a target. Parsing rejects statements after an unconditional
exit instead of deleting their effects. A conditional with one exit alternative
joins its other normal type; two exit alternatives have no normal completion.
No bottom value may become a parameter, field, ABI slot, local or published result.
Presence scans retain ordinary expression-depth admission for functions without
an exit. The narrower lexical contour bound applies only when an actual exit
selects this profile; it does not narrow legacy scalar expression admission.
Typed replay independently scans all roots before skipping unused contour
scratch. A forged `Never` completion or typed lexical-loop marker still selects
the complete contour validator; an attachment cannot assert its own absence.
An owned record-match arm whose entire result flow exits abruptly retains its
arm region in the abrupt-only certificate. It creates neither a second normal
scope exit nor a normal match-result transfer; independent replay still rejects
a missing/repeated region or a forged normal endpoint.
The initial profile admits exits only on direct function/loop/control-block
statement-tail routes. An exit cannot cross initializer, call argument, aggregate,
return operand, yield, unsafe or closure evaluation. SPX-T312 owns that refusal;
SPX-T313 rejects an absent enclosing loop. Ordinary failures inside a returned
expression or staged call argument remain admitted with existing atomic cleanup.
After ordinary evaluation completes, a statement-shaped exit also settles retained
ordinary owned String literal/read clones in the contour it leaves. Each temporary
must resolve to the exact current owned String clone expression and inventory slot;
a temporary-shaped identity alone is insufficient. Live staged call arguments,
provisional results and other unfinished temporary kinds remain closed. Root locals
remain live through ensures; this does not change String read cloning or the allocator
failure contract. The reference interpreter retains the actual completed operands
in exact temporary/flag slots, including comparison operands and normal scoped
String results. Skipped lazy operands remain inactive; canonical condition,
region and exit finalizers release only their authenticated carriers. Frame and
slot backing are reserved before allocation when an active allocation budget is
held. Owning executable qualification of this repair remains pending.

`ControlTarget::Function(FunctionExecutionId)` names the current ordinary
function execution, including the exact generic instance. `ControlTarget::Loop
{ owner, site }` names the nearest lexical source loop, where site is the exact
condition ExpressionId of its authenticated resolved While. Closures establish a
fresh function boundary and clear the loop target stack. Outside-loop exits and
cross-function/closure targets fail closed. Source and HIR independently reconstruct
the context, complete target identity, reachability, type and ownership joins.

Resolve source loop targeting before For/ForBorrow lowering. Their generated
While condition and source structural mapping must be retained/authenticated.
While continue targets the next condition. Source For continue targets its
iteration latch: iteration cleanup first, then the exact generated advance once,
then condition/next-item evaluation. Dropping the latch would silently turn a
continue into an infinite loop or repeat an item. The exact retained owning iterator
condition independently authenticates Done as false and Yield as true. Only the
matching Yield replacement arm is runtime-feasible after entering that body.
Full structural HIR validation and inventory/census retain both original arms;
independent replay scans both and narrows only completion/path joins. Constructor
and native/Wasm emission derive the same scoped relation from the actual
condition, body and latch, so an all-abrupt Yield body has no ordinary latch.
The entry ownership union and exact latch restoration checks are unchanged.
The terminal-path census keeps a separate break class bound to the exact nearest
typed loop. Only that loop consumes it into continuation; normal body completion
and continue end at the latch, while return reaches the function postconditions.
Generated for-loop advances contribute their actual checked paths and work. Typed
skeleton paths retain an explicit continuation state independently of observation
tails: normal execution, an exact continue marker awaiting generated advancement,
an exact abrupt marker, or an exact target-qualified latch. Sequencing retains
the advancement phase until its latch; releasing a return or consuming the exact
loop break clears its abrupt continuation. Shared marker references retain site,
kind and complete owner/target identity without cloning identity payloads.

The resource strategy is selected before the enumerating preflight can refuse.
Actual lexical HIR selects DeferredLexical resources, with no zero-work estimate
or proof authority. The independent HIR/structural/inventory/status/region/exit,
certificate and acyclicity checks still precede the unchanged exact ownership
replay. Only then does the strategy owner reconstruct both observation languages
once under the same cumulative work ledger and retained selected backing ledger.

DAG storage precharges node/refcount headers, tails, alternative/concatenation
members, merge indexes and vectors. Every child edge registers against one held
destruction frontier before node allocation, including old/new backing overlap.
The shared frontier releases the DAG iteratively without destructor allocation;
cross-reservation children refuse. Duplicate alternatives retain multiplicity.
Automaton and weighted canonicalization storage precharge vectors, map/refcount
headers, identity/projection payloads, frames, weighted key clones and replacement
backing. During the HIR walk, ordinary checked constructors and skeleton steps
share one temporarily lent work allowance. The held nonlexical skeleton pool is
restored unchanged after success or error; spent actual work is never refunded.

Copy discovery accepts scalar/static regions and exact authored call parameter
modes with Copy results, including compiler-owned Vec len/capacity/Copy get with
a borrowed receiver. That receiver may retain its Own Place HIR mode. Owned
parameters, unknown targets, wrong result/receiver types, nominal results, loops,
lexical exits and residual forms remain closed. Discovery binds an exact
expression identity before evaluation and closes before a caller sequences its
retained prefix. Only normal paths inside that scope may merge. Failure, residual,
abrupt, owned-source and pending-own-commit paths stay separate even if their
metadata matches. No function-wide or statement-run merge is admitted.

The exact census counts accepted routes per Success, Failure and LoopLatch
terminal. It distinguishes Known and Exceeded totals from ArithmeticOverflow
without capping or reclassifying any terminal class. Duplicate routes contribute
separately; cycles, unknown targets and nonterminal sinks refuse. A census alone
grants no equality or ownership authority. Ordinary Known cases choose Enumerated,
retain the legacy ceilings and reserve their old upper in addition to every held
program reservation. Exceeded cases choose LexicalFactored and compare the same
retained observation automata by checked weighted multiset equality. Multiplicity
overflow, work exhaustion or backing refusal cannot fall back or retry with fresh
credits. The selected physical profile always uses this metered factored route;
its existing primitive/direct-record provider and Vec/type exclusions are unchanged.

The plan converter emits exact certified return/break markers to their existing
continuation and exact continue/latch markers to authenticated advancement resumes
or the third latch terminal. Lexical exits create no epsilon edge. All authored
children remain structurally scanned, including infeasible Done. The enumerated
65,536-path ceiling, depth ceiling and 32,000,000 program-wide work budget remain
unchanged. Actual skeleton debits are observed separately from held reservations.

Independent small HIR-DAG Cartesian and plan-route multiset oracles include
duplicates, owning prefixes, borrowed Vec status traces, marker site/owner attacks
and omitted generated resumes. Forced ordinary strategies, exact 65,536/65,537
boundaries, above-cap equal-total hostile decisions, whole selected entry-to-Drop
work/backing exact/one-short limits and cumulative module ledgers are authored
qualification definitions. Their execution and original immutable ShiftSim all15
acceptance remain pending grouped qualification.
Unknown conditions, forged Done leaves, field modes and latch identities grant
no feasibility selection. Owning executable qualification remains pending. SourceForStep carries its exact
step ExpressionId; it is not identified by generated variable spelling.

## Authoring and diagnostics

A guard may end its selected arm with `if skip { continue; };` or
`if done { break; };`. Other paths continue normally. Do not add a dummy value
following an unconditional exit. Keep the loop's ordinary tail on paths that
complete normally. No labeled exit or valued break is admitted.

A guard with a valued alternative, `if skip { continue; } else { 0 }`, may also
end with `;` when another block item follows. The parser admits that `;` only
for a bare chain whose parsed tree actually contains an exit, with no postfix or
operator continuation, and only when `}` or end of input does not follow.
Canonical text omits the `;`, so the next item must also be unable to continue
the value `if`: one beginning with a binary operator, `(`, `.`, `?`, `::` or
`with` (for example `-x;` or `(x);`) keeps the generic discard and `SPX-T312`. It
reuses the existing statement `if`: the same parser-minted StatementIf marker,
normalized value tree, hygienic discard name, nearest targets, latches, cleanup
plans, graph and canonical text as the unterminated spelling; the `;` only widens
the statement span. No AST, HIR, cache, graph or backend variant is added. The
source contour admits a statement route only through that marker on its own
normalized shape (an immutable, unannotated discard of a value `if`); a marker
on a mutable, annotated or non-`if` binding is refused with `SPX-T312`. The
source contour also independently requires every reachable normal completion of
the guard to be the scalar discard zero `0`: an owned, aggregate, borrowed,
non-`i64` or nonzero normal arm would be stored, so its exits cross a value
boundary (`SPX-T312`) in either spelling before HIR replay. Names and
punctuation grant nothing. Independent HIR replay still authenticates the
guard itself: an unread immutable `i64` Value binding whose reachable
completions are exactly the scalar discard zero, with exact nearest targets
(`SPX-H006` otherwise). The `;` gives no lexical authority to any generic
discarded initializer, condition, `let`, argument, aggregate, return operand,
closure, unsafe or yields boundary (`SPX-T312`). Outside a loop the guard keeps
`SPX-T313`, an all-exit chain keeps `SPX-P209` and a final `;` keeps `SPX-P106`.
Owning differential, hostile-marker/target/completion, nesting-bound,
cache/graph and interpreter/native O0/O2/Core Wasm regressions are authored in
`language::statement_if::exit_guard_semicolon`, the parser cache unit test,
`cleanup_backends::lexical_exits` and `owned_data::lexical_exits`. Their
execution is pending qualification, and no token, turn or runtime gain is
claimed.

`SPX-T312` rejects a lexical exit crossing an initializer, argument, aggregate,
return operand, closure, unsafe or yields boundary. `SPX-T313` requires an
actual enclosing loop for break/continue. `SPX-T103` and `SPX-O104` preserve the
function's declared result and owning transfer. Independent HIR validation uses
`SPX-H006` for forged function/nearest-loop targets, latch identities or stored
bottom types. A returned call may fail through the ordinary staged-argument
failure route; that does not authorize another lexical exit in its argument.

A loop return may hand off one direct, whole named owned binding into the
function result domain: that owner need not be restored to the loop entry.
This exemption grants no call, effect, allocation or invocation authority.
Every other return operand retains the loop's existing executable admission,
and every operand retains ordinary expression/type/ownership and lexical-target
validation. Source and independent HIR must both reject a closed loop operation
hidden inside `return`, even though the returned expression executes at most once.

## Finite cleanup proof

The selective proof is `semaprax.lexical-control-flow.v1`, joined with selective
cleanup v18 and loan-plan v3. Functions
without lexical exits keep their prior graph/cache/cleanup schemas and bytes.
Older proof profiles refuse new exit nodes rather than guessing an interpretation.

Existing cleanup while proof linearizes one iteration. Keep that finite model,
with an explicit typed loop certificate binding target, condition/body/step IDs,
loop region, after-loop block, and canonical entry liveness/history. A LoopLatch
continuation is a checked handoff to that certificate, not an arbitrary CFG
back-edge. Replay verifies exact entry state restored, every exited region's
canonical finalizers, all source/HIR identities, no staged call epoch or renewal
reservation left outstanding, and precise control kind. It does not enumerate
unbounded iterations or reconstruct cleanup from generated C/Wasm blocks.

Break settles iteration locals and traversed inner regions, retains outer owners,
and joins the same after-loop state as the zero-iteration condition-false edge.
Continue settles the same local regions before the latch/condition. An outer owner
cannot disappear on an exit path unless the independently admitted loop invariant
and after-state permit it. ForBorrow aliases and TraversalFreeze obligations end
at the exact contour. ForOwn continues preserve the remaining iterator and settle
an unmoved current item; break/return settle the remaining owned iterator in its
canonical region order. Existing ownership commit and renewal rules remain exact.

Return first evaluates its expression left to right. Partial construction or
argument failure uses the ordinary selected failure and live-prefix cleanup.
Successful owned return moves into callee ProvisionalResult. It then leaves inner
loop/block regions, retains root locals through ensures, runs every non-result
cleanup action, and reaches the single successful CommitResult epilogue. Each
return has its own checked execution domain; repeated source postconditions reuse
their authenticated source storage/region identities with exact per-domain scope
selection, rather than cloning an unrelated contract or exposing another return's
live owners. The ordinary postcondition temporary cleanup remains mandatory. Failure
in the return expression or ensures never publishes; cleanup cannot replace the
selected status or reacquire arguments committed to a callee. Borrowed publication
uses the existing independent lifetime law, with no new borrowing privilege.

### Backend execution guards

Selective v18 also records proof-only function-entry and successful loop-condition
boundaries. Each binds the exact expression, canonical block and region, exited
region chain, and complete ordered finalizer vector from the authenticated live
builder state. Independent replay reconstructs every boundary, including repeated
resume domains. Native call-depth refusal and private Agent-stage semantic-fuel
exhaustion retain their existing status domains, stickiness and charge points;
they settle only the selected boundary before the common ABI failure epilogue.
Parameter carriers and flags are initialized before entry settlement. A boundary
is neither a new ordinary StatusSource nor owner-admission or ABI authority. It
does not make fatal String allocator failure recoverable or establish an OOM
recovery or production measurement claim.

## Consumers and trust boundaries

Interpreter flow carries exact Break/Continue/Return targets separately from
failure. Native emits only validated plan routes and canonical guarded finalizers.
Each ordinary terminal exit runs its own exact vector at its source-selected
site, so distinct exits may legitimately finalize common owners in different
canonical orders. Native never sorts or merges those vectors: when no single
order honors every terminal exit, it requires every such exit to be uniquely
selected by status source or root commit, and the unreachable common epilogue
fails closed. Ordinary source reaches such opposing orders: sealed conditional
variant owners finalize in reverse push order until a branch join rebuilds them
by storage identity, so exits before and after the join legitimately disagree.

A function without lexical control has a narrow exact nonlexical route mode in
`codegen/native_bytes/terminal_exits/nonlexical`. Native admits it only when
every ordinary terminal exit is authenticated by its own plan link: each
failure by exactly one declared status source, exactly one failure exit for that
source, and the exiting block's sole `SelectFailure`; success by the root
region's normal scope end, which must leave exactly the root region and be the
plan's only ordinary commit. Each exit then runs its canonical vector verbatim at
its site. Vectors are never sorted, merged or reinterpreted, and no certificate
is inferred. Owned parameters live at entry refuse, because a nonlexical plan
has no function-entry boundary certificate. Any plain `while` in the contracts
or body refuses, because the plan has no iteration certificate. The plan
linearizes one iteration without a back-edge, so the HIR is scanned for the
loop, and metered iteration emission refuses again as a backstop. The common
epilogue performs no finalizer. It traps if any terminal slot is live, so it
completes only the empty uncertified entry refusal. Any other unauthenticated,
ambiguous or forged link still refuses with SPX-B104 naming the function, both
exits, the conflicting slots and the nonlexical reason. This mode grants no
general nonlexical cleanup authority. It adds no source, graph, cleanup-plan or
canonical change, and claims no runtime or token advantage. Its witness is the
local metered O0/O2 parity of one loop-free checked source against the
interpreter; that is source-authored evidence, not a qualification.
Core Wasm branches use validated lexical-depth bindings and staged result locals,
then the same postcondition/cleanup epilogue. Neither emitter manufactures target
or cleanup authority from source spans, C labels or Wasm stack layout. Cache and
graph replay reject forged/stale targets, missing or extra exits, wrong latch/step,
wrong region contours, reordered finalizers and mismatched result publication.
Source, retained graph and builder census charge every target/certificate/exit,
child, identity clone and path state at existing capacity limits.

The existing acyclic guard-return normalizer remains the compatibility path for
its admitted source shapes and exact canonical value projections. A source body
requiring real lexical exits uses this explicit profile; it must not partially
normalize away one branch while hiding a loop exit. Canonical formatting retains
explicit exit spelling for this profile, deterministically replaying the same
source/HIR control identities. The profile selection itself is independently
reconstructed from source, not a cached assertion.

## Owning regressions and adoption

Define success and stable hostile controls in existing language, owned_data,
cleanup_backends/native, workspace/cache and HIR harness children: nearest nested
loop targets; continue latch exactly once; locals constructed then exited; outer
owners retained; partial aggregate return failure; own arguments before/after
atomic commit; ensures failure; forbidden borrowed publication; unreachable effects;
forged target/region/certificate/schema; missing/extra/reordered finalizers; and
outstanding renewal reservations. Every claimed backend must agree at native O0/O2,
interpreter and Core Wasm with original immutable expected outputs/status/traces.

The authored `examples/shiftsim-typed-record-successor/src/schedule.spx` now uses
nearest-loop break for the first waiting patient, first free server and exhausted
dispatch pair. It preserves ascending scans, priority/arrival/identifier ordering,
event advancement and the peak-queue observation before dispatch. Invalid requests
return the unchanged invalid ScheduleResult before report construction; successful
requests return the original report shape. The command source uses returning error
guards before encoding and retains exactly one stdout write only after successful
validation and encoding. Its acyclic guards may use the existing compatibility
normalizer; the scheduler's actual loop breaks select the lexical profile.

This is unqualified application authorship: the original ShiftSim SPEC, all15
acceptance cases, hostile wire inputs, >65536-byte whitespace and independent exact
output oracle remain unchanged. The owning typed ShiftSim fixture reads these real
source files; canonical formatting, complete Project admission and same-source
backend/application checks must execute after the batch joins. Bounded help and
later matched application measurement remain separate obligations. No measured
advantage or issue closure follows this adoption.

Owning contracts: RFC-0003-CLEANUP-AND-RESOURCE-ABI.md (return staging and publication),
WHILE-LOOPS-V1.md, GENERIC-OWNED-RESULT-V1.md, ACYCLIC-GUARD-RETURNS-V1.md,
SCOPED-RECORD-VEC-TRAVERSAL-V2.md and the exact OPT #734 acceptance.

The graph projects the `Never` completion marker on exit expressions but omits it
from the value type-facts registry: completion has no storage or ABI facts.
An ordinary single-arm record match may also complete only through a terminal
arm. Its expression and arm retain `Never` with Value mode; the independent
lexical contour validator proves the actual exit rather than trusting that type
marker. Forged ordinary values marked `Never` and ownership-bearing completion
remain rejected. The owning two-return cleanup and ShiftSim gates qualify this
repair in the consolidated batch; authored changes alone prove no backend result.

## Fresh contract-local owners (OPT-743)

This authored extension permits a clause-local `Bytes` created by literal-sized
`bytes_zeroed` (at most 131072 bytes), a String literal, and available aliases of
those exact owners. A consuming ordinary pure call may receive a named fresh
local after the usual type, loan and staged-argument checks. Constructor results
used directly as owned call arguments remain closed. General owned call results,
owned aggregates, captures, projections, effectful calls and ownership transfer
from function parameters, body locals or the provisional `result` remain closed.
The same ordinary owner cannot be transferred twice. Source and HIR reconstruct
freshness independently for each clause; no graph, cache or supplied Boolean
establishes it. Freshness cannot cross a requires/ensures boundary.

```spx
@id("contracts.check")
fn check(value: own Bytes) -> bool { byte_len(bytes_as_slice(value)) == 2usize }
@id("contracts.main")
fn main() -> i64
    requires { let value = bytes_zeroed(2usize); let moved = value; check(moved) }
    ensures { let text = "ok"; string_len(text) == 2 }
{ 29 }
```

Each contract-local owner retains ordinary canonical cleanup and call commit.
Requires temporaries settle before the body. Every selected return checks its
ensures with its own exact failure/result domain, settles ensures temporaries and
non-result owners, and then publishes. Contract failure remains sticky and must
leave caller result storage uninitialized. Existing root owners and provisional
results remain protected until their ordinary settlement/publication point.

The allocation census is cumulative across requires, body and ensures: 32 byte
allocation events and 2097152 statically charged bytes, without refund when a
local is released. Clause freshness does not grant runtime allocation recovery;
ordinary native String/Bytes physical OOM retains its existing contract. OPT-724
remains a separate obligation. Regression definitions cover source/HIR origin
forgery, aliases, duplicate transfer, clause isolation, exact/+1 resource bounds,
canonical/graph/cache replay, native O0/O2 cleanup and Core-Wasm/interpreter
refusal/reentry. These definitions are authored, not evidence of a passing gate.

## Retained primitive replay backing (OPT-745, OPT-749)

The selected retained-replay constructors are authored; executable qualification
is pending. They preserve ordinary cleanup order, source/HIR authentication and
work limits. The first selected context is module-local and primitive; imports,
nominal carriers, generic declarations and delegated runtime providers remain
closed. This restriction is a replay boundary, not broader application support.

Structural exit validation compares borrowed parent links and exact leave order
without constructing an expected-region vector or ancestor tree. CFG traversal
reads authenticated successors directly and reserves its colors and stack before
construction. Selected vector and FIFO frontier growth request `max(required, 2 * capacity)`
through checked arithmetic, charging the complete replacement backing while the
old backing remains charged. Spare capacity cannot refund or reset a refusal.
Ordinary replay retains its existing allocation route. After selected-budget
admission, one actual-capacity refusal-text buffer is prepaid from that same
physical ledger. Budget exhaustion moves that buffer into the first SPX-H006
without allocating after refusal; repeated refused receivers remain allocation-free
and keep the error identity. Internal operation labels are bounded independently
of authored identities. This does not grant storage credits or refund dropped
proof/diagnostic backing. Initial budget-admission errors retain their existing
route before a selected budget exists.

Owning regressions observe requested allocator Layout bytes and overlapping
backing independently of reservation arithmetic. They retain exact/+1 source
limits and one-short/sticky backing refusal; these observations do not establish
RSS bounds or measured agent-token, billed-cost or application speed gains.

### Inactive reachable context inventory prerequisite

`cleanup_plan::replay::context_closure` constructs a private borrowed inventory
for an exact held monomorphic function, its ordinary callees, and nongeneric
record/variant signature dependencies. Contracts and bodies are scanned; current
namespace membership, declaration kind/owner and field/case parity are checked.
Callee and type cycles refuse. Reference/state tables and fixed cursors are
reserved before construction against the same cumulative physical/work budgets.
Unrelated declarations confer no authority and need not enter this inventory.

This inventory is inactive and is not a current-source validation lease,
`ReplayContext`, storage classification or selected replay admission. Ordinary
whole-program validation remains mandatory. Reachable generic/import/dynamic
callables, compiler intrinsics, resource/class types and expression metadata not
fully reconstructed by this first inventory refuse. Nominal dependency discovery
therefore does not admit nested/Vec storage or qualify an application. Owning
executable qualification and the exact authenticated activation boundary remain
pending for OPT #745.

### Primitive source function matching scratch

Primitive source fact capture retains one invocation-local borrowed function-ID
index across signature/body comparison and final fact construction. Its fixed
reference backing is reserved before allocation; in-place ordering and binary
lookup debit the same cumulative matching-work allowance. Every existing source
origin, path/module, signature, parameter mode and typed-body digest comparison
still runs. The index supplies pointers, never namespace or replay authority.
Owning wide-fixture differential, exact/one-short work and overlapping-backing
regressions are authored; executable qualification remains pending.

## Iterator composition qualification

The constructor declares lexical status producers in complete typed authored
postorder before CFG lowering. Continue copies reuse exact producer identities;
an unreachable ordinary latch does not remove or reorder its authored status.
The declaration walk and CFG membership checks share one bounded work counter,
and status backing is reserved before construction.

Graph composition authenticates a lexical overlay from retained typed HIR, its
exact execution owner and independent cleanup replay. A schema tag alone, or a
frozen projection without its retained program, supplies no overlay authority.
Wasm repeats the authored owning-iterator Next only inside distinct certified
resume contexts: its exact argument carrier is saved and restored around the
complete latch. Duplicate carriers within one context remain refused.

The status exact/one-short, hostile metadata and closed normal-latch regression
definitions require owning executable qualification; this change does not claim
that those gates have executed.

The captured primitive module owns one selected replay session: its borrowed
callee inventory and the 32-million-unit work ledger survive all selected
function validations. Physical storage debits remain cumulative too. Every
function independently reconstructs its selected profile, ownership and renewal
lease; a failed member makes the session refuse further replay or successful
completion. Dropping the session publishes no diagnostic and refunds no debit.

### Retained monomorphic type preparation (#745)

The retained local lexical context now authenticates its current function
signatures and flat-record layouts through the same `SelectedReplayPreparation`
that funds subsequent independent replay. Record and field identities, origin,
owner, defining path/module, spans, types and declaration order remain checked
against the immutable source-captured facts. A refusal poisons that preparation;
rebuilding an index does not restore credits. The earlier physical-only local
constructor remains only as an independent test oracle.

The private source signature inventory can represent one exact compiler-owned
`Vec` argument over a scalar, String, Bytes or source-authored local flat record.
Its cloned type-argument backing is reserved before construction. The minimal
index reconstructs `core.vec` as the canonical compiler-owned Record with one
`T` parameter at index zero and no fields. This metadata grants neither element
admission nor generic function-instance authority: receiving inventory, status,
call, ownership and lexical replay still apply independently. Nested Vecs,
record fields containing Vecs, imported nominal types without defining-source
layout facts, templates and generic function instances remain closed here.

`ReplayContext::type_count` and `type_at` expose borrowed current members without
a whole-program clone, analogous to the existing function accessors. Context
construction also verifies that its defining module is the actual held member
of the current module universe. These source-only changes have not been
executed or qualified; #745 remains partial, including the imported nominal
declaration/type closure and complete String/Vec receiving qualification.

Defining modules demanded by an explicit monomorphic type import retain a separate
source-bound flat-record sidecar before owned HIR filtering, including providers
without static literals. A borrowed context authenticates the exact retained type
pointer, defining path/module, record and field identities, source revision, and
layout before constructing only the demanded declaration in its minimal index.
Capture work, held evidence backing, lookup scratch and index construction consume
the same preparation that is moved into replay. Unsupported layouts and generic
instances remain closed; this source checkpoint is not an executable completion
claim for imported record or Vec composition.

The defining-source helper producer also captures monomorphic scalar-returning
helpers whose argument-free flat-record parameters are exclusively borrowed.
Only an exact current parameter root and scalar declared-field projection extend
the existing pure helper walk; owned roots, nominal results, local record
construction, generic instances and allocating Vec helper bodies remain closed.
The minimal imported function declaration checks that same borrowed signature
against independently reconstructed record metadata. This producer shape does
not substitute for the complete cleanup/loan/status replay required downstream.
The next monomorphic receiving tranche independently reconstructs exact
compiler-owned Vec signatures and one-field Vec lifecycle inventory from the
current type directory. It checks actual operation, element, arity, type and
ownership; generic instances, nested Vec elements, owning iterator latches and
SortBy/StrAt remain closed. Borrowed flat-record source-for reads retain their
actual field identities, index modes and current Vec roots. String condition
reads and append moves are rebuilt with prepaid bounded borrowed cursors;
append history is reserved and published in the ordinary canonical order.
Renewal and byte-helper reconstruction debit the same invocation work balance.
The authored independent ordinary-plan, malformed-fact and exact/one-short
controls are unexecuted. These receiving facts grant neither imported layouts
without a defining-source sidecar nor a completed #745 qualification claim.

The selective direct expression-statement CopyDiscard extension is owned by
[Finite owned-record discard loops v1](FINITE-OWNED-RECORD-DISCARD-LOOPS-V1.md).
It does not relax named Let initializers or StatementIf guard completion.

### Ordinary paid conditional-finalizer validation

Allocation payment and primitive selected admission are independent. Ordinary
replay under the active builder keeps the exact variant storage, nominal type,
case path, lifecycle and unique-leaf checks of the original finalizer validator.
Its guard set requests backing from the same payer before growth. The selected
primitive route continues to refuse conditional ownership.

Execution-boundary validation authenticates all referenced regions before
comparing finalizers in canonical runtime order. Ordinary live and conditional
contours are compared as borrowed iterators, without temporary vectors or
cloned variant guards. This does not reorder, repair or omit any finalizer.

The `ordinary_paid_conditional_finalizers_keep_legacy_authority_and_selected_refusal`
owner compares actual resolved Option/string finalizer facts against the original
validator, including malformed case/type/storage/lifecycle and duplicate guards,
selected refusal, exact/one-short backing, and successful allocation through
Drop. These controls are authored; qualification and the unchanged complete
ShiftSim acceptance remain pending. No application or agent-cost gain is claimed.
