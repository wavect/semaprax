# Typed Vec Constructors v1

Status: qualified for the focused v1 profile.
Audience: SEMAPRAX authors and compiler contributors.

`vec_of<T>(item, ...)` constructs an already admitted Vec element profile with
an explicit homogeneous element type. Empty construction has capacity zero.
The existing capacity, carrier, allocation-event, payload, source depth and
work limits remain. Square-bracket byte arrays retain their existing meaning;
nonbyte square-bracket literals retain T262.

This is source constructor syntax, not a variadic ordinary Call. Execution
first calls existing `vec_with_capacity<T>(count)`, then evaluates each original
item once in order and executes one existing `vec_push<T>` at a time. Each push
stages arguments left to right and transfers them together at its ordinary
commit. Capacity failure evaluates no item; item or push failure retains the
selected status, settles staged owners and the committed prefix, and evaluates
no suffix. Postconditions, non-result cleanup and publication retain their
ordinary boundaries. No bulk allocator or String clone rule is introduced.

The parser forecasts generated statements, expression and type carriers,
type-clone backing, generated names, whole-file identifier inventory and
provisional-name replacement backing before constructing the chain. Its
cumulative generated-storage reservation is bounded by the existing 64 MiB
builder ceiling even on standalone parse routes. Active budgets are additionally
charged. Original item depth plus generated ancestors is checked against the
existing source depth; bounded borrowed cursors avoid prefix rescans. P220
refuses malformed type arity, count, reserved-name collisions and oversized
expansion before generated storage is published.

The AST marker retains element, literal count and its normalized ordinary
Block (appended cache tag 29). HIR carries element/count/statements/tail directly
on one node (appended cache tag 37), with the same expression/result identity
and one owning Temporary. It is not an owning wrapper around another Block.
Both source verifiers authenticate exact expansion and whole-function hygiene;
T318 refuses forged markers. Names skip all token identifiers, including later
text, and canonical hiding requires independent no-read/no-collision checks.
HIR authenticates builtin identities, exact element arguments, capacity,
previous-owner chain, binding modes and tail. Original items cannot read or
borrow any generated partial-vector owner. The borrowed internal-owner inventory is reserved before construction and sorted
with fallible in-place comparisons; item node visits and identity-byte
comparisons have separate bounded counters. Normal HIR/loan validation separately
checks child identities, types, staged calls and freezes.

Selective Graph v81 wraps the existing v80→v79→v78→v77 composition and retains
constructor metadata alongside ordinary children. Managed manifests admit the exact v81 spelling; frozen evidence routes explicitly refuse it with G410. Ordinary per-file replay still authenticates every source and constructor marker. Retained replay matches
current source sites and type identity independently; a matching ordinary HIR
Block grants no constructor provenance. Marker-free programs retain their
existing profiles. Frozen native interop/public export profiles explicitly
refuse the new syntax; ordinary supported private engines reuse Block execution
and existing calls without a new runtime operation or ABI.

Owning definitions: `hir::vec_constructor::tests::` covers source/cache/canonical
roundtrip, graph, one result carrier, source and HIR forgeries, hygiene, empty
capacity and old diagnostics; `owned_leaf_vec::vec_constructor::` covers
interpreter/native O0/O2/private Wasm execution, failure/cleanup and reentry;
`public_api_descriptor::vec_constructor::` covers frozen closure refusal.
The original ShiftSim15 and Catalog23 corpus bodies remain unchanged; their
committed eight-server and two-item examples adopt the constructor. Exact old
example sources remain independent controls in
`tests/project/standard_library/application_json/owned/vec_constructor_adoption/`.
The owning `shiftsim_typed_records::` and `catalog_scoped_records::`
`typed_vec_constructor_retains_exact_explicit_chain_application_control` gates
execute both authored test modules twice on the Project interpreter and native
O0/O2. Their command profiles advertise native64; these application gates do
not claim a Wasm command profile. The bounded constructor Wasm differential
remains owned by `owned_leaf_vec::vec_constructor::`. Both application controls
and all 15 ShiftSim and 23 Catalog cases passed on the qualified source. The
eight-server and two-item examples record the authored changes. Agent tokens,
agent turns and runtime effects remain unmeasured.

Agent advice uses `help language author:vec-constructor` and exact diagnostic
lookups for P220/T318 through the compiler-checked quick-reference table. The
ordinary diagnostic-help producer regenerates its JSON companion after source
integration; help does not independently authenticate a constructor.

The bounded `owned_leaf_vec::vec_constructor::failure::` gate compares an
independently authored explicit capacity/push program with the constructor.
Native C11 O0/O2 sweeps every observed checked Vec allocation ordinal, including capacity
before any element and middle-element checked-capacity failures after staged String/Bytes acquisition; exact element-entry traces,
status tokens/domain/code, unpublished result sentinels, pointer-identity
settlement, empty vector authority and immediate same-context reentry must
agree. The same sources run on the strict private Core-Wasm host at success,
capacity refusal and push commits 1/2/4 with its exact finalizer checks.
All 19 focused checks passed, including the native fault sweep, strict
Core-Wasm controls, source and retained replay, frozen public projection and
bounded authoring help.
Legacy String/Bytes allocation sites are tracked for exact settlement but are
not refused: their current NULL route is an invariant failure, not a checked
semantic status. The fault sweep does not widen that source/runtime contract.
