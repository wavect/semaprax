# Explicit unfunded Legacy nominal sharing (#827)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Basis: 07b76aa3c0d2099f804fb8c09be3664b63c0fbf3. Source-only; no Cargo,
compiler, Clippy, Rust tests or application execution in this worktree.

The complete original Legacy index constructor still runs canonical_nominals=false,
including original identity acquisition, duplicate/error ordering, inheritance and
type facts/layout checking. Only its successful result enters the new unfunded
postpass. Initial case-field copies are still constructed and discarded. Later
index/HIR/cloned-program payloads share actual immutable owners; no ID substitution
or authority is inferred. A case-field coordinate replaces only an existing fully
equal field fact; missing/different facts remain unchanged. Equality traverses
Nominal/Function trees iteratively.

Public NominalPayload remains a struct with private Storage and the same Vec
Deref/DerefMut/iteration signatures. A private immutable OwnerMode distinguishes
Funded from LegacyUnfunded. Legacy implicit mutation detaches to the old Owned
fallback: unique owners move the original Vec, aliased owners clone complete
values using an unfunded iterative type copier. Mutable iteration follows that
same gate. Case-field Legacy coordinates detach just their full field sequence.
Funded Shared and funded case coordinates still reject implicit mutation; the
existing explicit try_make_mut body and refusal ordering remain byte-identical.
The sealed element trait gains only a hidden default compatibility clone method;
only the three already-sealed compiler element types can implement it.

from_legacy_vec_unfunded computes its disposal frontier before publication and
uses ordinary Vec/Arc allocation. It discovers no active ledger, asks no callback
for payment, and creates no controlled budget failure. An observed zero active
allowance must remain untouched. These allocations are explicitly unfunded even
inside an existing builder. No quota, work allowance, forecast/P credit, protocol
admission or application-fit claim changes.

OwnerMode never changes after construction. Arc Clone preserves it. The funded
constructor creates a separate new owner from caller-owned Vec storage; explicit
mutation first creates separate Owned backing. Thus no current operation creates
Funded and Legacy handles to one backing. is_shared remains Funded-only;
has_shared_owner is physical storage information used only by disposal and the
Legacy postpass. The private paid sharing helper rejects a Legacy owner before
it can accidentally invoke implicit COW. Paid Arc/initializer/receiver fees use
the actual Owner size including its new mode; no former header is discounted.

Consumer audit: nominal_payload.rs storage matches implement read/clone/drop,
explicit materialization and the guarded Legacy detachment. map.rs paid canonical
still requires is_shared; legacy_unfunded separately requires is_legacy_shared.
DeclarationIndex drain skips any physical owner and lets its last Arc run the
existing preacquired iterative disposer. linked_owned_data.rs still selects paid
coordinates only through is_shared, so Legacy never satisfies that gate. Its
legacy field-copy fallback stays explicitly unfunded. codec.rs remains unchanged:
wire is complete values and decoding produces Owned, never a shared authority.
The source-schema test host adapter still calls explicit try_make_mut. Equality,
Debug and wire ignore allocation mode. Protected original constructor/receiver
files are unchanged and independently compiled.

Seven new exact owners are in pending-selectors.json, alongside six required
existing paid/old controls. They cover complete public HIR/graph/cache/IDs and
function-work parity, direct and mutable-iteration isolation across HIR/index/
sibling, complete old diagnostics, paid negative controls and refusal preservation,
zero-active unfunded allocations, actual 512-depth shared/unique/detached Drop,
and both Drop orders. Allocation observations execute a genuine Owned baseline
through the unchanged complete saved index; the current counterpart differs only
at index construction. Source verification is outside both cost windows. Actual
allocation/debit/peak tuples are printed after the observer, with no universal
net-saving assertion or fitted expectation. Ordinary infallible OOM behavior is
not a budget-refusal guarantee.

Own18 full current-to-before inverses, original receiver bytes, the unchanged
semantic-constructor body/Owned Clone/paid mutation/Owned Drop, active nominal v14
79 whole originals/current bindings, and existing source36 are source-only checks.
The v14 chain adds only six leading exact compositions; all old hooks, source
snapshots and archive hashes remain intact. Other shared packets will need final
combined-head composition after integration; no original is re-pinned.
