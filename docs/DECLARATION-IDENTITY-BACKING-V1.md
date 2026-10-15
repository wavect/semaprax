# Declaration identity backing v1

Status: source checkpoint; current-head owning execution qualification pending.

Audience: compiler contributors and reviewers.

The private carrier has only valid text: CompilerStatic(&'static str) from a closed
compiler declaration enum, or Shared(Arc<str>). The actual 64-bit size/alignment and
Option footprint are asserted against the former String carrier. Equality, order,
hash, Borrow<str>, Display, Debug and codec wire consult exact text, never an address,
variant, fingerprint, budget or admission flag. Compiler constants grant no source,
prelude, layout or backend authority. Dynamic identities cannot select static text.

Legacy public new(Into<String>) remains infallible and explicitly unfunded. It does
not consult an ambient budget; borrowed input makes a temporary String before the
shared backing. Full dynamic construction migration remains open. No exhausted
unrelated budget can alter a pure expected-type query's declaration identity.

Private paid_borrowed reserves the shared backing on its supplied payer before
allocating and makes no temporary String. paid_owned additionally requires an already
owned/paid String; it reserves only the new backing while the old String remains
live, then releases that old owner. No refund or silent extra allowance occurs.
Refusal is Err outside the carrier, never empty text or a sentinel usable as a key.
The backing policy includes both counters, UTF-8 byte extent and alignment padding;
allocator observations pin that actual request separately from inline carriers.

Compiler Vec/Option/lifecycle/call constants use the closed enum. Existing physical
reservation routes and conservative text terms remain where retained by their
owners; static construction does not itself spend or reset a ledger. In particular,
flat-record lifecycle construction continues through ReplayBudget::reserve_bytes,
which must retain the current Selected/ActiveBuilder selection at integration.

DeclarationId codec preserves the former nested String tuple wire. Decode borrows
validated UTF-8 wire bytes, pays its decoder-local cap and then the caller's existing
active reservation before the backing allocation. Local capacity and owning refusal
remain separate; unchanged outer Diagnostic rendering is not claimed funded. The
private static representation is not serialized, so a decode returns ordinary
shared text without transferring compiler authority.

Graph set lookup/removal borrows str keys directly. Existing source association,
canonical replay, cache equality, failure priority, ceilings and forecasts are
unchanged. Shared clones allocate no identity payload; existing conservative clone
forecasts remain until a separate owner proves a reduction.

## Ordinary empty-renewal branch clone

The ordinary ActiveBuilder path-state branch receiver has a separate empty-map
constructor. Before publishing a borrowed payload it proves the current renewal map empty and pays producer,
outward-result and receiving borrowed-view controls and all cloned payload
backing. Compound types keep the original type census scratch. Atomic type tags
have no child containers and use a closed stackless clone census instead; no
cursor is constructed or charged for those tags. Its affine borrowed view
contains the other seven state fields and cannot name the renewal map. The
receiving constructor creates `BTreeMap::new()` and copies every other field;
it has no map to clone. Nonempty maps, selected reservations, independent oracle
accounts and unobserved replay retain the complete original clone and census.

This tightens a conservative reservation bound and avoids the atomic type
census's fixed stack cursor. On the pinned standard library
the original empty-map clone already makes no heap allocation, including after
history removal; no runtime-allocation, RSS or speed improvement is claimed.
The new constructor makes that absence explicit independently of map clone's
internal representation. Its new borrowed controls remain charged. It neither
subtracts from the old census nor refunds a debit, changes a ceiling, resets work
or omits a semantic replay. The original branch-history work request remains at
its original position before physical receiver selection.

`cleanup_plan::replay::path_join::storage::empty_renewals::tests` owns full
eight-field parity, all atomic type tags, both measurement orders, independent
requested-layout counts, unchanged work, complete result/budget Drop, every reservation frontier,
exact/short/zero and sticky retry. Selected and nonempty negative controls retain
their old receiver. These current-source owning tests and whole-application fit
remain unqualified until exact-head CI executes them. The small local standard
library smoke exercise covers the copied constructor body and field parity only.

## Ordinary atomic body control-key clone

The ordinary ActiveBuilder ownership join separately selects a present `Body`
staged result whose type is one of the same closed twenty atomic tags. Its
immutable affine payload borrows only the five actual control-key fields;
renewal, live-order and conditional-variant containers cannot be cloned through
that payload. Before publication it pays both borrowed views, the outward
Result, the returned key carrier, and all returned-place projection backing.
The receiving constructor preserves pending and selected failure, the complete
staged result, publication state and returned source in their original order.
It constructs no type-walk cursor because those exact types have no child
containers. This is a narrower physical constructor, not type admission.

Absent results, compound types, every residual-result variant, selected/oracle
payers and unobserved replay retain the complete original reserve-and-clone
route. The original join work request, grouping, tree insertion, state replay,
limits and sticky failures remain unchanged. No debit is subtracted or refunded.
This changes the conservative reservation and avoids a fixed stack cursor;
heap request, RSS, speed or application-fit improvements are not established.

`atomic_controls::tests` retains the complete independently compiled old tuple
receiver, checks all twenty tags and all five fields with both measurement
orders and populated histories, independently prices the complete new request
sequence, measures through result/budget Drop, and exercises exact/short/zero,
every request frontier, sticky retries and the original-route exclusions.
These actual-HIR owning tests require exact-head CI execution; a small local
constructor smoke is not a compiler integration or full-funding receipt.

Authored owning controls cover old owned-String trait/wire parity, all empty/NUL/
Unicode distinctions, duplicate independent backings, static/shared equivalence,
borrowed and already-owned overlap, zero/exact/one-short refusal, sticky same-ledger
retry, last-owner Drop, decoder-local/owning cap separation, exhausted-budget pure
queries and graph key lookup. Their allocation scopes are narrow identity scopes,
not whole resolver/replay/graph funding evidence. Literal constructor source controls
cover only named migrated domains. No full OPT785 completion or fit claim follows.

Clone-only replay precharges price no DeclarationId text backing: both valid
static and shared variants retain their existing storage. Type-argument and
function-parameter Vecs, boxed result types, projection/case/flag directories,
renewal carriers and String-backed FunctionInstanceId clones remain charged.
The path-state clone census retains its original depth/work bounds. Zero-byte
clone requests still consult the same payer and cannot reopen a refused ledger.
Mixed retained-owner capacity, fresh construction, decode, source forecasts and
literal compiler/runtime ceilings are unchanged. Narrow authored observers cover
static/shared clone requests, nested type and conditional-state clone backing,
and real selected setup through exact/one-short refusal, zero retry and Drop.
These source controls await execution; they establish no application fit or
whole-validator/engine funding claim.

## Initial index explicit reservation seam

Initial index construction has a private callback route that requests only
actual shared identity backing bytes from the caller. It constructs the exact
borrowed source spelling internally; the callback cannot supply an identity or
verification authority. All ten original construction positions, metadata
mutations, duplicate/type/layout errors, and semantic diagnostic ordering remain
in the shared implementation. Identity refusal returns the exact caller error
inline and drops the partial index; it constructs no Diagnostic or report Vec.
The ordinary infallible wrapper remains explicitly unfunded.

The two owning controls compare the complete saved index and literal seventeen
initial identity requests, preserve empty/NUL/Unicode/duplicate behavior, and
observe full constructor/partial-index Drop. Their selected physical ledger pays
only identity backing; other index map/name/type storage and original owned
semantic diagnostics are not asserted paid by that ledger. Resolver activation
remains pending an actual caller-owned reservation and outward fixed report
boundary across verification and both invariant passes. No callback presence
changes source admission or makes an unsafe mutated index a verified subject.

### Private explicit resolver caller (identity-only, not public activation)

The private `hir::identity_resolution` route preserves source verification and
both invariant-resolution passes. It carries `Identity(E)` unchanged and uses
one caller callback for all initial index identities; source and semantic
errors retain their legacy owned channels. SourceIndex construction remains
before index construction in each pass. Ordinary public activation is described
separately below; this private callback API remains unchanged.

`bounded_output::identity_reservation::IdentityReservation` captures the
existing ActiveBuilder budget Rc without allocating a new budget. Before any
identity request the caller reserves its actual inline carrier, one Diagnostic
slot and the fixed 48-byte outward message. Setup refusal is Copy-only and
creates neither report nor diagnostic. Exact report capacity is checked before
publication; allocator excess is debited then refused, never normalized.
Identity reservations use that exact owner and cannot spend after its scope or
inside a nested budget. The existing first sticky receipt survives; publication
moves the paid String and Vec unchanged. No refund or new allowance is issued.

The two-pass tests observe partial-index Drop and original source error
priority. The report tests separately observe actual acquisition, first
identity refusal, zero/positive retries, publication and Drop, plus exact and
one-short setup. They do not assert payment for source verification, index
maps/names/type payloads, invariant cloning, resolver scratch, legacy semantic
errors or the complete public diagnostic pipeline. Public activation and those
remaining funding obligations stay open.

### Ordinary workspace module resolution (initial index identities only)

The actual non-hit module resolution in `workspace_graph` now acquires one
`IdentityReservation` on the resolved-core ledger before either the selective
or fresh route and forwards that owner through `resolve_checked` (owned input,
function reuse) or `resolve_source` (borrowed input), including both invariant
passes. Cache hits charge the stored full resolver residual without acquiring
another owner or building an index. Setup refusal stays Copy through the
`workspace_graph::identity_resolution` receiver and is adapted to the legacy
`Vec<Diagnostic>` layer only outside it; that outer graph/report layer remains
unqualified. Public `hir::resolve`/`analyze` stay explicit unpaid legacy.

Only the initial `DeclarationIndex` identities are paid by this route. The
resolver's current `held_identity` queries share carriers already constructed
by its exact pass index; they create no fresh identity backing. Genuinely new
agent/generated identities, other initial-index callers (selected fused
candidate, byte partition, compiler prelude link), source/semantic diagnostics
and operation constructors remain outside this payment scope. No
whole-resolver funding claim is made.

### Ordinary analyzed caller activation (identity-only)

With an existing ActiveBuilder invocation and enough backing for the exact
identity/report setup, public `hir::analyze` now uses one captured affine owner
for both initial declaration-index passes. Original source verification still
runs first. Clause inventory, SourceIndex and compiler prelude discovery keep
their original order before acquisition and the first identity request. Each
identity is constructed internally from the exact borrowed current source text;
no callback can supply an ID or bypass ordinary validation. The second invariant
pass keeps the same owner and sticky first receipt.

A late identity refusal moves the already-paid capacity-one diagnostic Vec and
literal text directly into Analysis. It does not pop the report and allocate
another outward Vec. Successful construction also removes the former temporary
String copy before each shared backing allocation. Both original source warning
behavior and semantic error replacement remain unchanged.

The strict private verified constructor acquires its owner before constructing
inventory, so initial refusal is inline and allocates no inventory backing.
The public compatibility receiver keeps its original inventory-first order;
this private constructor control is not evidence of zero-allocation public
setup refusal. Public zero/short-setup calls retain
their original legacy diagnostic route; allocator/capacity setup refusal also
uses the legacy receiver on the already-refused ledger, lending the same held
invariant inventory and helper index rather than constructing them again. This
setup-only outward channel remains an explicit OPT781 gap, not a funded report.
Calls without an active owner preserve the old infallible index receiver. There
is no fresh ledger, additional allowance, refund, profile selection, source
forecast adjustment or physical identity sentinel.

Authored controls compare full current HIR/diagnostics with the saved ordinary
analysis across both invariant passes; a literal 29-request source census pins
removed temporary text and new report backing through complete Drop. Actual
public late refusals pin exact/one-short first identity requests and zero retries,
plus prepaid report pointers at the shared caller seam. Setup/source precedence
and the original nested/detached-owner controls remain. These are same-process,
source-only controls, not executed current-head or cold-static evidence.

This activation pays identity backing and its refusal report only. Source
verification, invariant cloning, SourceIndex proof storage, index maps/names/type
payloads, resolver continuations and legacy semantic diagnostics retain their
existing separate owners or open gaps. It establishes neither whole-resolver
funding nor an application fit claim.

### Borrowed canonical type-fact cache queries

Flag-only queries preserve the current cache's complete byte-key ordering and
arbitrary cached facts. A private heterogeneous ordered query streams the exact
existing key bytes without copying text. Zero-argument nominal keys borrow the
current identity directly. Compound keys prepare a borrowed fixed-stack plan
with at most 64 nodes/depth, 65,536 encoded bytes and 256 census visits; overflow
uses the unchanged owning `identity_key` receiver. These optimization bounds do
not restrict admitted types or substitute for current-HIR validation.

The plan holds 64 optional borrowed nodes; each comparison has 64 optional cursor
frames and a platform-bounded decimal buffer. There is no recursive Rust walk,
heap directory or whole-cache scan. Comparisons restart their byte cursor, so
CPU cost is proportional to compared key bytes times ordered-cache comparisons;
the old route constructed one owning key first. Both representations retain
exact Unicode/NUL bytes, decimal lengths and current cache-first precedence.

The saved owning builder remains the byte/verdict oracle with adequate backing.
Its compound `CappedString` can truncate after a physical refusal; that truncated
key is not a semantic authority to preserve in the allocation-free route.
Existing caller refusal stays sticky and must still prevent publication. This
query change neither pays the remaining compiler storage nor changes forecasts,
quotas, wire keys, fresh identity construction or validator admission.

The full owned `type_facts` API uses the same exact cached-key query. A hit still
clones and returns the complete `TypeFacts`, including its mandatory owned layout
String. A miss performs the original recomputation; optimization overflow uses
the original key builder. The full prechange API is retained under `cfg(test)`
as an independently callable owned-key oracle, and allocation owners that claim
old key requests call that saved API explicitly. Scoped test-only old-key policy
also compares complete ordinary generic/invariant diagnostics and resolved HIR.
No production policy switch or new public receiver is introduced.

## Late Agent identity producers

`hir/resolve_program/agent_identity` preserves the complete Agent carrier and
its original agent/type-role/operation/model-wait identity order. The explicit
resolver and ordinary analyzed resolver pass their existing identity owner
through this late construction point in both invariant passes. Each borrowed
source spelling reserves the complete shared backing before acquisition;
identity refusal remains separate from the unchanged semantic diagnostic.
The unfunded wrapper still accepts all legacy text. Name, runtime JSON, role
vectors and source associations keep their original owners; this pays identity
backing alone. The complete frozen resolver receiver and Agent builder remain
compiled independent controls, alongside exact/one-short/sticky/Drop controls.

ARC strong-count and scoped task/scope/failure queries borrow exact str keys;
known cancellation reuses the held scope key after the original authentication,
completion and unknown-scope checks. Their complete frozen model receivers,
trace and receiving-Drop controls remain compiled. Function-reference and
generic callback owner keys reuse the same resolution pass's held function ID.
Infallible public model constructors and other dynamic producer domains retain
legacy behavior. These source-authored controls await focused execution; they
do not close OPT785 or establish whole-pipeline funding or application fit.
