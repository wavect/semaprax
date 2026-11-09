# Validator Scope Checkpoints v1

Audience: compiler contributors and reviewers.

Status: private source implementation; OPT795 remains open. This document owns
storage and payment invariants, not execution evidence or a language extension.
[RFC 0001](RFC-0001.md) and [Resource ABI](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md)
retain their meaning. Independent hostile HIR validation acquires no authority
from resolver success, attached facts, a snapshot, or this directory.

Ordinary ownership checkpoints retain full ValidationBinding fields, ordered
lookup, original nonempty-prefix joins, freshness resets and nested publication
restoration under `hir/validation/scope_checkpoint/`. Their saved complete
iterative, recursive and contract validators remain the semantic oracles.

Generic-template Values retains ValueId ordering and exact replacement booleans.
Each node holds an existing identity Arc handle and a reference to the immutable
current HIR type; no fresh identity bytes, type clone, authentication index or
cross-invocation cache is created. Parameter, result, block, branch, contract,
match, loop and separate closure domains keep the original caller boundaries,
lookup order, collision order and complete expression/value identity checking.
Closure bodies start empty while captures are checked against their outer domain.
Contracts share a root but do not publish local bindings to sibling clauses.

The live invocation pays literal initial and local checkpoint control storage.
Every newly allocated AVL node pays its Rc reference-count header, actual node
layout and literal initializer stack before allocation. Shared paths construct
new path nodes transactionally; balancing moves their fresh unique path nodes
and existing sibling handles without another allocation. A refusal retains the
original complete root. A search path can mutate only when each node has exactly one
strong reference and no weak references. It prepays and allocates a missing leaf
before changing any node; insertion and rotations then move unique path nodes
and existing subtree handles without allocating or making a fallible payment.
Allocation-free replacements check sticky active refusal with a zero-byte
reservation before mutation; this adds no debit or allowance.
Published/shared nodes never mutate. Equal-key replacement retains the original
key identity. Drop releases carriers without refunding cumulative admission;
there is no separate quota, reset, forecast discount or allocation normalization.

The complete prior borrowed BTreeMap receiver and prior owned BTreeMap receiver
remain test-only independent policies, including complete snapshots and root
reads. The borrowed receiver isolates directory savings from earlier removal of
nominal type payload clones. Full ordered projections are observation helpers
constructed outside allocator windows, never retained production authority.

Qualification must compare complete environments, owned types, full symbolic
walks, identity sets, diagnostics and first-error order. Preserve existing
ordinary checkpoint, whole-subtree and original application owners. Additional
controls own monotonic/reverse and double-rotation insertions, snapshot forks,
replacements, nested contracts/closures, exact/one-short/sticky payment refusal,
unchanged publication on failure, actual allocation requests and final Drop.
Insertion-heavy setup and snapshot-heavy whole receiving lifetimes must be
observed separately against the prior borrowed directory; no gain is presumed.
Full application qualification and scoped Clippy remain mandatory in the
consolidated batch. Any remaining funding gap must be reported; private source
controls do not complete OPT795 or establish agent token savings.

Nine source-authored owners in
`hir::validation::generic_template::values::checkpoint_tests` cover the full
ordered policies/rotations/replacements, independent borrowed snapshot requests,
insertion and fork receiving lifetimes, exact/short/sticky Drop, unique/shared
failed publication, sibling domains/unwind, full retained symbolic applications
and contracts, nested depth/late errors, and zero-allocation replacement after
sticky refusal. Their module and both production storage children are joined
into all existing validator text audits. No tests, Clippy or application owners
have been executed for this source batch.

## Fixed refusal message backing

The source-authored [Rust facade v3 migration](EMBEDDING-API-V3.md) preserves
ordinary/generic charge equations, first refusals and complete old receivers.
Fixed budget/overflow messages and owned iterator-frontier storage refusals
borrow static diagnostic text, including at zero entry allowance. Exact and
one-short whole receiving/Drop owners, arithmetic/sticky controls and original
rich scope bindings are retained in the new `refusal_tests` children. Dynamic
error strings, output buffers and report Vec backing remain separate funding
obligations. No current execution or OPT781/OPT795 closure is claimed.

## Persistent function receiving scope

The private function driver seeds a paid persistent scope directly, validates
requires clauses from an independently reset baseline, runs its body through the
existing iterative checkpoint publication guard, inserts the complete result
binding, and validates ensures clauses from their own full baseline. Every
baseline retains all mutable/type/ownership/availability/loan/move/partial fields
and changes only `contract_local=false`. Each clause owns a root checkpoint;
its mutations and local publication never update the function input or siblings.
Parameter and result insertion backing and full type clone payloads are paid
before those payloads are cloned. A sealed paid insertion owner holds the exclusive
scope borrow until its one consuming insertion. Dropping it after clone-payment
refusal leaves the prior root unchanged; there is no unfunded insertion entry. The body preserves Publication destruction
order and RestorePublication frames. The map adapter and its materialization
charges remain; no map is created on the persistent function route.

`scope_checkpoint/transaction.rs` stages the first shared (or weakly held) suffix
without changing the receiving root. A scalar depth records the preceding unique
prefix; no heap path directory is constructed. Each new node pays its own Rc
carrier plus the exact owning Node literal initializer before construction. Owned
insertion pays the binding carrier before the caller clones its already charged
payload; binding-copy insertion shares the existing Rc and ValueId Arc handles.
The sealed exclusive scope borrow holds through staging. Every payment and
allocation completes before the first prefix mutation. Publication and rotations
then use only unique Rc access and moves, with no fallible work or allocation.
Failed staging drops all incoming/staged ownership and preserves the complete
prior root and held snapshots. A zero-byte check retains sticky refusal even for
an allocation-free replacement. Weakly held paths rebuild conservatively.

The complete prior `3 * height + 8` receiving policy and original tree insertion
are retained as independent test controls in `scope_checkpoint/coarse.rs` and
`tree.rs`. Full rich-field owners cover all four rotation shapes under unique,
shared-root and mixed-path ownership, replacement, weak references, every paid
byte boundary and complete Drop. They compare actual receiving requests with the
coarse control: these strong-reference cases are expected to allocate the same
physical backing, while literal constructor payments remove the conservative
allowance. Weak rebuilding may make additional requests. These are authored
expectations pending execution, not allocation reductions or application-fit
evidence. Baseline, edit and publication charges are unchanged.

`scope_checkpoint/fresh.rs` owns persistent contract baseline construction.
It visits every current binding flag and both child subtrees on each invocation;
no assumed clear flags or cached membership authorize sharing. Clear bindings
and complete unchanged subtrees share their existing Rc handles. A true flag
receives one complete payload clone after its binding carrier and payload payment;
changed ancestors pay their own node carrier and literal initializer before each
Rc constructor. The source is never mutated, and only a completely staged root
is returned. Even an empty or entirely clear baseline checks sticky refusal with
a zero-byte reservation. Each contract clause then owns its independent checkpoint.
The complete generic full-copy baseline, its original tree builder and owning
controls remain test-only independent receivers with unchanged bodies. New owners
compare every rich field for clear/true/mixed flags and shared roots, every paid
byte refusal and exact admission, later flag mutation, clause isolation, complete
Drop and unwind. Panic infrastructure remains outside the separate allocator
payment owner. The original full function-chain and application controls remain
selected. Constructor removal and sharing are source behavior awaiting execution;
no observed allocation, work, application-fit or token gain is claimed.

The full prior map function is saved in `function_scope/old.rs`, retaining its
entire preflight, parameter order, body, result and contract order. Only the
entry name, owner-path qualification after relocation, and calls to complete
saved map expression/contract drivers differ. No original multi-line literal
content is dedented. New function-chain owners compare full diagnostics,
identity membership and auxiliary state over the entire saved iterative corpus,
contract owners and parameter/requires/body/result/ensures hostile failures.
Physical owners record actual requests, cumulative debits and complete Drop,
exact/one-short/sticky refusal and postvalidation unwind. Additional persistent
input owners retain rich nested type, loan, projection and partial payloads and
exercise checkpoint publication with live children during unwind. The original
rich refusal fixture is unchanged; its expected baseline now resets only the
freshness flag and compares every field while proving the hostile input unchanged.

All source-audit joins include the production child, complete saved body and new
controls. Original late-child, suppression, depth-512, full iterative corpus,
contract and checkpoint controls remain selected, as do whole-subtree and original
application owners at the unchanged 64 MiB allowance. This source-only batch
runs no compiler, tests, Clippy or application gates. Consolidated qualification
remains required; no allocation reduction, application fit or OPT795 closure is
claimed before execution. True-flag baseline payload copying remains a separate cost.

The unchanged generic nested-block depth cases `[1,17,64]` and complete current,
borrowed-map and owned-map walkers now run in a joined 16 MiB oracle test thread,
following the existing expression depth-oracle convention. All original bodies,
fixtures, diagnostic and identity assertions remain. This is test-oracle stack
provisioning only; production admission depth, stack behavior and support claims
are unchanged and still require independent qualification.

## Ordered moved-place output (#936)

Conditional and branch joins may replace only the temporary owned moved-key
set with two independently ordered borrowed map cursors. Every missing branch
state is Available; equal keys consume both inputs, each other step consumes
one, and each non-Available result clones its retained output path once. This
preserves the original lexicographic final map and Availability::join.

The new root/count/arithmetic/iterator/output/clone/insert carriers are prepaid
before formation. The distinct join-output tariff retains final map nodes and
the original conservative path payload upper. Only the physically absent
temporary key-set nodes are omitted; no aggregate old debit is subtracted.
The availability-copy moved() tariff remains byte-for-byte unchanged, as do
definitely-partial candidate/prefix builders, loan unions, scope COW bindings
and commit/drop ordering. Inputs remain immutable through full result staging.

The complete immediate paid join functions and original generic producer stay
independently executable under a panic-safe test-only caller selection. Exact
source/inverse, state/empty/interleaved/prefix, full scope/full HIR caller and
exact/short/zero/floor/sticky/Drop controls are authored only. Strict many-field
allocation AND invoice gates must pass; a paid one-field cold loss remains
visible. No measured gain, capacity rebate or application recovery is claimed
by this source-only implementation.

## Terminal source-move roots (#939)

An exact `Place` root runs the complete original terminal operation directly
inside the existing funded root-kind match. It moves the caller scope into one
local `Owned` root before lookup, retains full place/type/resource/availability
and overlapping-loan checks in their original order, and restores that root
only on success. Errors drop the local root and leave the caller empty, exactly
as the previous directory receiver did. Nonterminal roots and the legacy map
receiver retain the entire old walker and all original growth payments.

This removes the two actual initial source-move directories; it introduces no
scratch cache, loan owner, callback, helper boundary or additional reservation
primitive. Existing selector and semantic-operation fees stay unchanged. The
whole immediate producer, older inert-root producer and old owned-map receiver
remain independently executable; exact current source plus all new children
are bound before the original source gate acquires its named old stage.
Authored controls require strict single/repeated and complete many-owned-argument
HIR allocation/invoice gains, unchanged tiny inert invoices, semantic/late-error
and failed-publication equality, exact/short/zero/floor/sticky boundaries and
complete Drop. No compiler, runtime, application fit, peak or RSS claim is made
by this source implementation.

## Retained partial-candidate output (source-only successor)

The bounded alternative to the four-way prefix-stream proposal retains the
ENTIRE original owned candidate builder and generic oracle. It changes only
the final consuming filter/collect into ascending BTreeSet::retain with the
same two left-to-right Availability::Moved predicates and short circuit. The
first candidate directory, every duplicate/rejected prefix clone and all
original candidate work remain. This is removal of the second collected tree
and its FromIterator staging, not a zero-directory prefix stream.

The existing first positive join CONTROL gateway additionally prepays the
retain receiver actual/formal pair, explicit two-reference closure environment
actual/formal pair, complete returned/receiving set and unit-return pair. The
unchanged callback loans and predicates retain their original ABI. No new
per-prefix payment, scalar selector or gateway exists. The original second
gateway and all moved-map, loan, final survivor payload and COW terms remain.
The third gateway directly derives one candidate node upper per original
visited prefix instead of two; it never subtracts a virtual debit or refunds
already reserved backing. Full immediate pre-retain paid conditional/branch
receivers remain executable independently of the older #936 saved controls.

Retain and collect can leave differently packed trees. Complete owning tests
therefore include successive joins, retained snapshots and later COW mutation
through Drop, as well as realistic depth-one-to-three scopes and a real
many-field HIR caller. Empty/all-reject and small cold outcomes are reported
without a universal gain assertion. All original state/error/early-skip/no-op
and publication order is retained. Production predicates are pure; arbitrary
injected predicate-panic global key destruction order is not promised equal to
collect's separated output/unvisited-input owners. Full deallocation, immutable
inputs and no failed publication remain required. This proposal has only parse
review; tests, complete invoices/allocation tuples and application owners are
unrun. It establishes no application fit, #937 closure or measured efficiency.
