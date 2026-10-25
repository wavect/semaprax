# Retained expression-fact lookup v1

Status: SOURCE ONLY; private candidate, executable qualification pending.
Audience: compiler contributors and source-accounting reviewers.

The original v1 boundary and equations below remain executable against the
complete immediate predecessor. The transition continuation successor is specified
at the end of this document; it has no runtime qualification yet.

## Boundary

This candidate retains the dense certified ordinary status-source scratch through
the same invocation's pure initial edge-condition loop. It releases mutable
custody before program-dependent block transitions and never crosses invocations.
The owning issue is #885. No runtime gain, application admission, public API,
hosted support or completion-matrix promotion is established by source analysis.
The existing 67,108,864-byte application cap is unchanged.

The receiver first executes its original directory construction, held-prefix
acquisition, ordinal initialization, first insertion, and fact receiving payment.
Only at the first actual old lookup frontier does it authenticate zero bytes and
work, pay new selection controls, and consider retaining scratch. A query-free
route or error before that frontier acquires no new owner or selector.

Eligible owners are nonempty immutable Arena facts under ActiveBuilder, with a
status count at least the target-layout `MIN_ROWS`. That constant conservatively
requires a positive source fee difference on full successful traversal; actual
query count can be smaller on an early error. Short, empty and LegacyUnobserved
routes keep their old lookup algorithm after the paid first-frontier selector.
SelectedReservation, Tree, fewer-than-three status rows, and block-transition
lookups keep their original dispatch. Selector overhead on ineligible held-prefix routes is
a real cold-path cost, not a saving.

## Custody

The session holds immutable arena and function loans and an exclusive mutable
ReplayBudget loan. The arena's fact-payload lifetime is independent of the
function/proof/status-directory lifetime. Only their common local session loan
is shared; no `Facts`-inner-outlives-function bound is added to the validator. Its query identity loan must last for that entire session:
production queries come from the held function's immutable status and edge tables. The
actual slice-bearing comparison frame persists; it is neither a paid-once flag
nor a recreated full local comparison. Every field is reset on each query.
Returned facts borrow the original arena and never mutable scratch. There is no
allocation, RefCell, borrow guard, memoized verdict, pointer-only budget seal,
refund, future-query guard bank, or fallible scratch Drop.

Each row preserves `HeldPrefix::insert -> fact receiving payment -> lookup`.
The existing insertion owner authenticates the exact directory, function,
ReplayBudget and current ClassifierLedger before its existing payments. Its
complete original fees remain in force. The first insertion precedes owner
acquisition; every later insertion occurs inside the same closed continuation.
No callback or ACTIVE-switching operation exists between that guard and lookup.
Thus the old zero-byte/work lookup authentication still addresses the same held
invocation. This is source-bound lexical custody, not permission to expose the
session to arbitrary callers or resume it under another invocation. Reentry,
foreign function/budget/arena replacement and source-lifetime escape are not
admitted by the borrowed session interface. The existing enclosing proof and
insertion guards continue rejecting foreign/stale/nested invocations.

Changing the continuation's callee set or allowing a callback requires a fresh
custody audit; an unpriced dynamic preprobe cannot replace this closed scope.

## Funding

All formulas use actual target `size_of` expressions in the owning Rust modules.
There are no numeric host-layout assumptions in admission.

- `S = sizeof(LookupControl) + 2*sizeof(old ReceivingComparison)` is the exact
  immediate predecessor's reusable scratch, exported by the unchanged prefix
  implementation solely for source accounting.
- `A = SELECT_BYTES + Session::acquisition_bytes()` includes selector scalars,
  first-authentication/payment/helper Result overlap, helper actual/formal
  transports, full initialized/receiving Session and nested field initializers,
  constructor arguments, and acquisition reservation/work Result controls.
- `R = Session::query_bytes()` includes new mutable session actual/formal
  borrowers, five internal field borrowers, continuation budget reborrows,
  duplicate status-row source receiver, and scalar predicates. It is paid on
  each actual query through the original full-lookup reservation frontier.

The selector uses a borrowed payer-tag pattern. It creates no owned
`AllocationPayer`, and no equality call or equality actual/formal references.
The disjoint selector site ledger is:

| Constant | Actual new roles paid before formation |
| --- | --- |
| `PAYER_TAG` | One `&AllocationPayer` borrowed scrutinee, two match-arm `bool` values, and one result receiver. |
| `ROW_LENGTH` | Two `&Vec<StatusSource>` roles for `Vec::len` actual/formal receivers and two `usize` roles for returning/receiving its length. |
| `ROW_BOUND` | One `usize` bound operand, primitive comparison `bool`, and its receiver. |
| `SHORT_CIRCUIT` | The `&&` result and the `if` receiver, both `bool`. |
| `FACTS_ARM` | A borrowed `&Facts` scrutinee receiver, the `&Arena` arm binding, and the variant-selection branch `bool`. |
| `NONEMPTY` | Outer `Arena::len` and inner `Vec<Node>::len` actual/formal receiver pairs, their two returning/receiving `usize` pairs, zero operand, primitive comparison result and `if` receiver. |
| `PAYMENT_AND_RETURN` | Eight complete `Result<(), Diagnostic>` roles: four for extra first zero-byte/work authentication, two for selector reservation, and two for helper return/receiving. |
| `PAYMENT_TRANSPORT` | Six `(&mut ReplayBudget, &ResolvedFunction, usize, &str)` tuples: actual/formal transports for the two extra authentication calls and selector reservation. |
| `CONTINUATION` | Both complete actual/formal six-reference transports for `complete_rows`. |

`SELECT_BYTES` sums exactly those named groups; it contains no unformed
`Option<usize>` donor entries. The private `Node` receiver layout is computed in
the arena module, where that actual type is available. No previous held-prefix
selector payment is reused.

The remaining one-owner acquisition term pays two complete `Session` layouts,
two each of its nested `Control`, `Compared` and `Bytes` initializers, both
three-reference constructor transports, four complete acquisition/work
`Result<(), Diagnostic>` roles, four corresponding payment transport tuples,
and two scalar acquisition receivers. The first-frontier reservation includes
this full term before `new_prepaid` forms any owner. These are distinct from
the selector/helper transports and from `R`; they do not precharge a future row.

The first activation also pays the entire original `S`, independently of `A`.
Every query retains both complete old compound
`Result<Option<&Option<CallFact>>, Diagnostic>` terms. There is no per-node
comparison credit, old Result credit, canonical-ID credit or old-fee exemption.
One owner adds twelve initialization work units. Every original ordered query
work event and byte comparison remains, including minimum/maximum probes,
certified suffix bounds and tree traversal. All new refusal paths use the
existing sticky prepaid diagnostic mechanism.

For actual query count `N` and owner activations `M`, the conservative byte
opportunity is `(N-M)*S - sum(A_j) - sum(R_i)`. For this one-owner route the
successful threshold is `floor((A+S)/(S-R))+1`, if `S>R`; otherwise retention is
ineligible. This is a source tariff expression, not a measured allocation or
runtime result. Early semantic errors may pay acquisition without amortizing it.
No future-loop/query payment occurs at acquisition.

## Qualification and source custody

The complete immediate-old prefix algorithm and Arena dispatch are independently
compiled in `arena/prefix/retained_before.rs`. The complete immediate-old
certified status receiver and ordinary caller are independently compiled in
`case_certified/lookup_before.rs` and `lookup_caller_before.rs`. The latter uses
the unchanged live old Arena lookup; source assertions bind that lookup to the
complete separately compiled predecessor. The relocated ordinary body has
only two exact inverse adaptations: remove its `pub(super)` test visibility and
map `crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20` back to the original
`super::CLEANUP_PLAN_SCHEMA_V20`. The constant is fully qualified because the
copied function's parent namespace is different. Full historical source
overlays remain necessary for independently built whole-composition qualification.

Authored controls cover complete borrowed hit/miss results, noncall/absent
identities, distinct allocations, NUL/UTF-8/prefix endpoints, small/large AVL
shapes, all old query work cuts and six diagnostic fields, exact/one-short
first and later payments, sticky retry, source lifetime custody, nested/foreign
existing-owner refusal, successful/refused/unwind Drop, and whole old/current
ordinary composition at the unchanged cap. A separate authored lifetime control
drops independently sourced fact payloads and their function owner before
reading the returned status directory; selector source controls bind the typed
ledger and exact namespace inverse. These controls have not run in this
source-only lane. The existing HIR, semantic-graph and native source-inventory
functions now include all seven complete new modules beside their owning roots.
The two live modules keep production registrations; five independent controls
retain their exact `cfg(test)` edges. A source control binds every edge and one
literal per module per raw inventory. The native file's two finite inline joins
keep its existing 1500-line budget without removing an original include.

The existing prefix, Main8a, Maina3 and region-entry controls acquire named exact
pre-885 inputs before their unchanged historical authorities. The shared
`before_885_source` test helper reverses only the unique complete arena module
registration, directory reexport and new raw-source join expressions. Complete
restored arena/directory/HIR/graph/native source bytes must equal fixed SHA-256
priors from `8c95f77750aaaba9eaa59da865eb65658900646c`. It rejects drift,
truncation and already-historical input; an unlisted input is passed to its
original authority without granting acceptance. The prefix CI transport keeps
the original caller/path and feeds that acquired prior into the existing
CI/semantic-reflow helper. Original snapshots, fixed digests, edit cardinalities,
old producer bodies and final full-source equality assertions remain intact.

Final 241/98/174/F6 metadata, ordered raw-domain tables, complete production/test
class authority and physical pin qualification remain owned by the combined
integration batch. These source joins do not authorize stale metadata or a new
price/budget claim.

Before promotion, run that combined focused batch with one build job: exact old
and current full callers, mandatory original-ledger crosschecks, codec/catalog/
starter owners, source inventories and scoped production Clippy. Instrument
actual eligible `N`, activations `M`, every `A`/`R` payment, byte/work refusals,
reserved floors, allocator allocated/reserved/reallocated/peak/live metrics and
Drop. Require a positive measured whole-composition difference and disclose
cold, tiny, early-error, dense/miss and fallback costs. Edge-spanning retention
and the remaining broad #885 acceptance controls stay open.

## Same-ABI initial-edge continuation (source only)

The ordinary validator's existing signature is directly reexported from
`receiving/retained/ordinary_cross_phase.rs`. Its lexical status receiver is the
complete current certified receiver. The eligible branch inlines the complete
`complete_rows` body, preserving every acquisition, receiving, insertion,
producer, proof, ordinal, work and refusal payment. The Session is formed only
at the existing first lookup frontier. The full immediate ordinary caller is
independently compiled in `ordinary_cross_phase_before.rs`; all original status,
region, entry and edge APIs also remain independently callable.

After the last status query, the original held prefix freezes, the original
status control payment executes, and the affine attempt completes. The published
StatusSources retains the original function/budget pointers and captured ledger.
The original region and entry validators run before the original edge receiver
binding/current-ledger checks. The initial edge IDs/endpoints precede each
condition exactly as before. Always and status conditions retain their complete
old paths. Only BooleanResult, VariantCase and ArmSelected use retained search
scratch. Their original zero-byte/work authentication and query payment occur
before the new mutable Session/query actual arguments form. Both complete old
compound Result layouts remain in that payment.

The actual edge-site ledger is `Session::EDGE_QUERY_BYTES`:

| Role | Complete typed payment |
| --- | --- |
| Mutable Session and query actual/formal pair | `2 * size_of::<(&mut Session, &ExpressionId)>()` |
| Internal arena/function/budget/control/comparison field loans | `size_of::<(&Arena, &ResolvedFunction, &mut ReplayBudget, &mut Control, &mut Compared)>()` |
| Budget reborrow and receiver | `2 * size_of::<&mut ReplayBudget>()` |

The unknown-expression check uses a bare `if let None` pattern, with a targeted
Clippy attribute documenting why no Option method is called. No Option-method
receiver pair or bool-returning predicate is formed or charged. Both full old
Result terms remain unchanged.

There is no new Session acquisition, selector, scan, callback, Option<Session>,
returned Session, or continuation argument/result. The lexical `facts`/`proof`
references correspond to the exact old certified receiver formals. The tail's
storage/leaves/status references correspond to the exact old edge receiver
formals. No removed call or unused old charge is credited: all original charges,
including the complete selector/helper frame, remain unchanged. Moving the
existing `session.budget` field into the old budget binding consumes exclusive
custody before the first transition callback without constructing a drop-call
frame or another owner. The Session cannot be queried after that partial move.

The completed LexicalAttempt remains live longer. Its unchanged Drop reads
Finished and makes no mutation; no external callback receives the private proof.
Its already initialized, fully paid original carrier is retained, not copied or
recreated. This extends the physical overlap of the existing guard and Session
with region/entry state. The owning test reports actual requested heap backing,
and the layout observation reports the retained guard width. No stack-peak or
CPU saving is claimed. Treat a change that copies/returns that guard, adds a
wrapper, or creates a fresh carrier as a new A obligation.

Let Ns be actual status queries, Q actual expression-bearing initial-edge
queries, S the complete old scratch, A the unchanged existing status acquisition
and selector, Rs the existing status-query receiver, and Re EDGE_QUERY_BYTES.
The full old-prefix equation is:

`(Ns + Q - 1) * S - A - Ns * Rs - Q * Re`.

Against the complete immediate status-retained caller the incremental equation
is exactly `Q * (S - Re)`: M_new=0, no new acquisition/transport carrier is
formed, and the original Session acquisition remains paid in both arms. The
owning caller test requires S > Re and derives Q from actual resolved conditions.
Q=0 executes no new edge payment and must preserve the complete old bill; early
region/entry/edge-ID errors likewise retain complete original tuples. A target
layout or later source change that invalidates that equality fails the owning
control; it does not authorize a tariff adjustment. All measurements and controls
are authored but unrun. General source custody/recursive joins are deferred to
the next combined batch, and #885 remains open pending actual qualification.

## Transition continuation successor (source revision 2)

The selected dense ActiveBuilder receiver retains that same Session and its
already paid terminal/output slots through the final expression lookup in the
ordinary block-transition loop. No new owner, activation selector, ledger clone,
status lookup saving, or initial-edge saving is credited here. The immediate v1
receiver is separately compiled in `ordinary_block_before.rs`; its body is kept
verbatim with one test-registration exclusion whose exact inverse authenticates
the complete original source. Original C5 equations continue to select that old
receiver. New controls select the shipping continuation directly.

Each transition query preserves the old zero-byte/work authentication and exact
ordered AVL comparison work. It resets every `Control`, `Compared`, and output
field through `fact_edge_into_prepaid`. Its actual/formal argument packet and
internal field reborrows are paid on every query, plus its copied optional output
receiver. The old compound query Results physically disappear at these sites;
all unrelated returning Results and all old ordinary fees remain paid.

The immutable arena and function remain borrowed. A callback never receives the
Session, scratch fields or output/error slots: original helpers receive only a
temporary reborrow of the original budget. A borrowed `StorageParent` keeps a concrete parent Budget loan, so its later
checks do not rematch or dereference the held Rc. Acquisition occurs once on
the known payer before the transition loop; it grants no allowance or ACTIVE
admission. The containing ledger remains alive, and the witness holds no TLS
RefCell guard. Its field, Option return/receive, pattern, acquisition method and
Rc dereference controls are included in the cold bank.

Before a renewal binding callback, variant declaration callback, or complete
CallCommit parameter/drop-type helper sequence, the original payer reserves
each actual call's TLS, borrowed-current, pointer identity, sticky predicate,
method/closure and reserve receiving controls. After the successful sequence,
that witness checks a freshly read ACTIVE and sticky state. Restored outer
invocations remain legal; foreign or sticky invocations refuse before another
scratch query.

The complete mismatch and record_refusal producer is prepaid ONCE before the
loop, including its Cell receivers, receipt, site and literal roles. Every false
check immediately returns the original full refusal through the already paid
Failure slot, so no second mismatch can consume this bank. It has no counter,
mutable credit, refund, callback-visible state or fallible Drop. The original
ClassifierLedger guard and its full per-call bank remain unchanged for other
callers. Callback
errors return their original complete diagnostic before this guard is evaluated.

CallCommit custody covers target/interface/provider resolution AND all later
owned-parameter/type-needs-drop callbacks. Variant and renewal semantic failures
keep their original left-to-right priority. Concrete place/directory/status
helpers retain their original calls and charges. Session and error/output slots
end immediately after the transition loop, before exit/path/skeleton callbacks.
No retained query occurs in those later continuations.

For successful selected traversal, let `q` be actual transition lookups and `c`
actual guarded callback sequences. The complete additional comparison is:

- old tail: `q * (OLD_SCRATCH + 2*sizeof(Result<Option<&Option<CallFact>>, Diagnostic>))`;
- new tail: `q * (into_edge_local_bytes + into_edge_query_bytes + sizeof(Option<&Option<CallFact>>))`
  plus `c * (StorageParent::CURRENT_CONTROLS + sizeof(usize) + 3*sizeof(&str) + sizeof(bool))`
  plus the one cold `StorageParent::ACQUISITION_CONTROLS + StorageParent::TERMINAL_CONTROLS + sizeof(usize) + 3*sizeof(&str) + sizeof(bool)` bank.

These are actual target-layout costs. The positive gate uses the complete
immediate invocation, including its unchanged acquisition, prior retained phases,
all callback guards and whole-caller work. No CPU, application-fit, physical RSS,
or runtime benefit follows from the source equation. New success, full-field
error/work cuts, zero/short/exact/floor, callback reentry/restoration, foreign and
sticky ledger, source immutability and whole-Drop controls remain unexecuted until
the consolidated owning qualification.

The invocation-backing candidate rebuilds the complete arena for every function
and instance. It retains only cleared Node allocation capacity and cursor slots
inside one private program-and-ReplayBudget-bound owner. Before returning the
backing, all keys and CallFact payloads are dropped, every cursor slot is reset,
and root/first become NIL. No expression identity, AVL edge, lookup result,
validation verdict or prior per-function work allowance survives that boundary.
The original ArenaCapacity owner, Arena initializer, per-node fact production,
AVL and payload charges remain reached. Only replacement Vec construction and
its actual relocation work can disappear when already-held empty backing fits.

The original program preflight selects its first function and completes its
unchanged skeleton-work reservation before the additional complete owner,
ledger, actual/formal call and return banks are paid. Empty programs construct
no backing and capture no ledger. Acquisition prepays the complete fixed call/receiving/authentication banks for
every immutable program function and instance, plus the full actual Lease-scratch
release tuple. Its checked count/product/Options are paid first. No new positive
debit precedes ledger authentication at a receiving call; authentication repeats
after the full_program callback. Each successful build then prepays its new
frame-reset slots before a lease can be formed. Original RELEASE remains whole. Lease Drop is
infallible and returns empty storage before the original typed-control phase.
A failed build or failed cleanup acquisition drops all local backing instead.
There is no refund of a completed constructor or fact tariff.

The complete immediate-old five-argument receiver and producer remain compiled
independently. The complete old program caller remains executable under cfg(test)
at its original interface. Whole-source inverses authenticate the changed owners;
the new source-bound controls cover alternating large/small functions, complete
fact equality, later duplicate identities, wrong-budget refusal, exact/short/zero
and sticky acquisition failures, and full Drop. These controls are authored but
unrun. First-function and tiny programs can regress; no net or application-fit
claim follows from this source candidate.
