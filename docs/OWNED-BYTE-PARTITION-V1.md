# Owned Bytes consume-and-partition v1

Status: Private source receivers, selected graph/cache replay and the explicit
native copy-out bridge have passed focused local checks for OPT #746. Ordinary
source and additional public target activation remain pending.

Audience: language, ownership, runtime and backend contributors and reviewers.

## Summary

Consume one existing Bytes value and publish a bounded Vec of disjoint owned
ranges over its backing. Administrative sharing never makes a source value
Copy, creates overlapping mutable owners, or grants public pointer authority.
The private receivers implement the single-backing strategy. The unchanged
4,096-row constructor loop is covered by the separate construction-fusion
contract.

## Proposed source operation

```text
bytes_partition(buffer: own Bytes, count: usize, width: usize) -> Vec<Bytes>
```

The spelling and intrinsic identity require ordinary registry review; neither
is registered by this document. Evaluation and argument staging are left to
right. The operation consumes exactly one whole named Bytes owner at one
commit boundary. Its authority must independently identify an ordinary whole-
backing lease; a child range is not an eligible partition input, even when its
offset is zero or its extent happens to equal the backing extent. Existing loans
must have ended before staging its transfer.
There is no borrowed-input overload, projected-owner shortcut, overlapping
window operation, implicit clone, arbitrary pointer input, or padding/truncation.

Validate positive width, checked count times width, exact equality with the
input's authenticated logical extent, then the existing Vec<Bytes> capacity
and selected-domain metadata bounds, in that order. No guessed element count
or payload identity may replace these checks. Count zero is valid only for an
empty input and positive width; it returns an empty owned Vec. Every nonempty
child has extent width and offset index times width. Concatenating children
in index order reproduces every original byte, including NUL and non-UTF-8.

The Vec has length and logical capacity count. The existing element classifier
and carrier envelopes determine its maximum capacity; this proposal does not
choose a larger count because a fixture needs it. The implementation must
record the exact current Vec<Bytes> bound in its executable boundary tests.

## Transaction and failure selection

Before commit, authenticate the input lease and full backing extent, reserve
all required Vec, backing-administration and child-lease metadata, and prepare
the complete unpublished child inventory. The original owner remains the
caller-owned staged value until every semantic check and metadata reservation
succeeds. Failed preparation releases only unpublished temporary metadata;
ordinary canonical call-failure cleanup still owns the original Bytes.

At commit, invalidate the source lease generation and transfer its backing
ownership into the complete child inventory. Publish the Vec only after normal
result staging, postconditions and non-result cleanup. A failure cannot expose
a prefix Vec, initialize a caller result, leave hidden live leases, or replace
the first selected status. A zero-child success settles the empty source owner.

Proposed normalized domain: `semaprax.byte-partition.v1`. The following symbols
have assigned additive codes in the private operation receiver. Ordinary
registry and target activation remain pending:

| Symbol | Code | Selected condition |
| --- | --- | --- |
| `width_zero` | 1 | width equals zero |
| `extent_overflow` | 2 | count times width exceeds semantic usize |
| `extent_mismatch` | 3 | product differs from authenticated input length |
| `metadata_capacity` | 4 | count or complete administrative inventory exceeds its bounded capacity |

Operand-evaluation failures keep their existing domains and left-to-right
precedence. These four operation failures occur before transfer. The eventual
mapping remains required on interpreter, native and selected Core-Wasm;
no backend may invent a local code or reuse an unrelated arithmetic code.

Physical allocator exhaustion is distinct from semantic capacity refusal.
Under the current byte contract it remains invariant fail-stop. The native
receiver releases the unpublished Vec reservation and any allocated rows before
fail-stop if either metadata allocation fails. Its probe injects each failure
and checks that the original owner and payload counters remain intact.
Native C11 stores the exact source lease, geometry and unpublished allocations
in the reserved Vec authority record. The caller holds the issuer, slot and
monotonic reservation ticket. Commit and abort authenticate that ticket before
accessing the reservation. Reusing a Vec authority slot for the same source and
geometry still invalidates every retired ticket. The reservation reuses the
existing Vec authority record's storage and adds one 64-bit counter per registry.
Each backing records its one pending Vec reservation. A second preparation
refuses before reserving slots or allocating, even when both request zero
children. Abort and preparation allocation failure clear the claim, and commit
clears it as the whole lease is consumed. Registry settlement checks for
outstanding claims even when their child-slot charge is zero.

Native Vec operations that publish a successor check its generation before
moving a child, dropping rows, or replacing the row array. The paired Core-Wasm
host likewise acquires a successor handle before changing its Vec. The
exhaustion controls check that the current Vec and child leases remain live,
that native output slots stay untouched, and that the original owners can
still settle. Capacity and index failures retain their existing precedence.
The [hosted run at `81650ead3`](https://github.com/wavect/semaprax/actions/runs/43527506209)
passed all 43 selected tests and scoped production Clippy.

The paired Core-Wasm registry binds each preparation to the live invocation
and claims the exact buffer for the ticket's lifetime. A second array view of
the same buffer cannot prepare overlapping ranges. Abort releases the claim
and retires the ticket. Commit consumes the claim and leaves it bound to the
buffer identity. Pending tickets are bounded by the backing inventory and
count toward settlement even when they reserve zero child slots.
Preparations also hold child tokens in the shared module token namespace.
Other registries and ordinary allocations cannot spend those tokens while a
ticket is pending. Commit consumes the held capacity, and abort or invocation
abandonment releases it without rewinding the token counter.

A recoverable physical-metadata allocation domain would require a separately accepted checked
allocator contract, failure-before-publication implementation and physical
fault gates. Neither fuel exhaustion nor a logical counter proves physical OOM.
Forged/stale/foreign leases are boundary rejection or invariant failure, according
to the selected entry contract; they are never ordinary partition outcomes.

## Backing, range and generation ownership

Each live lease authenticates invocation/module domain, backing allocation
identity, offset, extent, generation and live state. Backing identity and lease
identity are different. One backing record owns the original allocation, exact
allocation extent and bounded live-lease accounting. Creating ranges retains
administrative backing ownership; it never copies payload bytes.

Live child ranges are pairwise disjoint. A renewed bytes_set owner retains the
same exact range and advances the appropriate lease generation. A borrow binds
the current lease generation and range; existing byte_get/byte_range rules cannot
read or write outside it. Child leases remain fully affine Bytes for the already
admitted normal operations,
including stores, views, transfers, clone and drop. V1 explicitly refuses
partitioning a child lease; recursive partitioning needs a separate contract and
metadata proof. No source parent or prior generation remains usable after commit.

Pop, iterator next and consuming traversal transfer the original child lease.
A popped child survives Vec drop, sibling drop and iterator abandonment. The
last live lease releases the backing exactly once; earlier child drops release
only their administrative lease. Clear and all early exit/failure cleanup follow
canonical element order. Administrative retain/release is not a source finalizer
or permission to reorder cleanup. Outstanding borrows forbid owner drop/move.

Cloning or nonconsuming owned copy-out deep-copies the selected range into fresh
ordinary backing and retains the existing allocation charges. Clone isolation
must be observable under later writes. No aliasing clone or descriptor copy may
stand in for that contract. Partial clone failure settles its prefix exactly
once while preserving the source lease and first selected failure.

## Quotas and metadata

The existing source-derived limits remain 32 allocation-producing Bytes events
and 2,097,152 cumulative payload bytes per executable path, with no drop refund.
The existing 131,072-byte individual Bytes extent, foreign input, Vec carrier,
frame and builder limits also remain unchanged. Partition issues leases; it does
not allocate new payload backing or erase the original constructor charge.
Deep copies and all other backing-producing operations retain their charges.
Runtime counters cannot replace independently checked source call-path bounds.

Lease metadata is a separate finite physical inventory, including slot spare
capacity, generation records, backing administration and Vec backing. A selected
invocation must precharge its complete inventory before allocation. A bounded
count alone does not prove total metadata size or repeated whole-backing partition
capacity; child repartitioning remains closed.
The private interpreter, native and Core-Wasm receivers use an 8,192-child
limit across committed backing tables and any outstanding preparations. The full table charge lasts through
the backing's last child, even when earlier children settle. After the last
child settles, a new partition may use that capacity. This release leaves the
32-event and 2 MiB cumulative backing counters intact. The executable contract
must authenticate that bound, reject slot-generation exhaustion without ABA
reuse, and test exact/one-short refusal.
No unbounded registry, global cache or per-child hidden heap allocation is allowed.

## Proposed native representation and public boundary

Only a new selected internal profile may use a three-word native carrier:

```text
{ uint64_t held_issuer; uint64_t exact_slot; uint64_t nonce; }
```

The issuer identifies the explicitly held invocation/module context; slot and
nonce authenticate the current generation before any backing access. Offset and
length come only from the immutable authenticated slot, rather than redundant
caller-supplied length or guessed pointer provenance. O(1) lookup must reject
unknown issuer, slot and stale nonce before obtaining backing plus offset. No
negative-offset header read, allocation-magic probe or foreign-address
dereference is permitted. Exact target size, alignment, row stride and view
carrier layout require their own assertions and physical predebits. The semantic
Vec owned-leaf cap does not substitute for actual target backing bytes: row
backing is funded at its original Vec allocation boundary, and invocation/slot/
view metadata at its permitted first-Bytes preparation boundary, by one account.

Selection must close over ALL reachable Bytes producers and consumers: literals
and constructors, copies/clones, partition, stores, views, record/variant/Vec
transfers, iterator operations, cleanup and imported calls. A carrier may never
reach ordinary free(ptr), a legacy helper or an unauthenticated foreign provider.
Unsupported crossings refuse before emission. Empty owners also need real live
lease identity and cannot borrow a sentinel as ownership authority.

The frozen public two-word pointer/length ABI is unchanged. A supported public
bridge must authenticate input, snapshot into selected backing, copy results out
through the existing host ownership contract and provider drop, and settle
private leases. It cannot export an interior pointer as a host-owned allocation.
The native runtime copy-out helper stages a fresh provider buffer and
publishes it after the private lease settles. Provider allocation failure
preserves both the private owner and the caller's output slot. The C11
O0/O2 probe checks failure, retry, provider drop and the backing's exact-once
release. Public adapter selection still requires its own contract and gates.

The explicit C11 emission API `codegen::emit_native_partition_copy_out`
selects a current source function by its authored `@id`. The function must
take no arguments and return owned `Bytes`. The source still has a normal
scalar `main` for compiler validation. The generated
`spx_partition_copy_out_v1` uses the two-word provider carrier. It returns 0
and writes the copied result after private settlement, 1 and writes the
selected language domain and code, or 2 when provider allocation refuses.
On allocation refusal the bridge settles the child and leaves all caller
outputs untouched. The matching `spx_partition_provider_drop_v1` frees the
provider allocation. This API is separate from the existing Project-v8
provider and the Core-Wasm host profile.
The generated native test returns an actual partition child through an owning
iterator. It checks that language failure and injected provider allocation
refusal preserve the caller result, and that a subsequent invocation can
publish a fresh copy which the provider drops exactly once.
The selected native receiver also binds `bytes_copy` and `bytes_set` to the
shared-owner runtime. The generated test copies a child containing NUL and
non-UTF-8 bytes after its iterator has settled, mutates the original child,
and verifies the copy retains its bytes. The adjacent out-of-bounds store
selects the byte-buffer status and settles both backings before returning.
The shared `byte-partition-private-clone.spx` fixture executes the same
early iterator return, copy, mutation, and byte reads through the private
interpreter, native C11 at O0/O2, and paired Core Wasm. It observes the mutated
child and the original bytes in the copy, including NUL and 0xFF. Each
invocation accounts for two backing allocations and eight payload bytes.
The interpreter and paired Core-Wasm failure controls check the byte-buffer
status, settlement and reentry after a write past the child extent.
The interpreter checks the callee's lexical return and the caller's success
and store-failure cleanup routes against the current source-owned HIR. The
caller follows exact loan edges for the copy, mutation, and byte observations.

## Independent frontend and target proof obligations

Source admission must verify exact intrinsic/type/ownership, argument order,
call-path quotas, count/extent bounds and normal no-live-loan transfer rules.
HIR independently reconstructs the exact declaration and instance, operand
types, transfer commit, result Vec<Bytes> layout and range-domain selection.
Loans retain exact lease/range/generation provenance. Cleanup authenticates
source staging, provisional Vec, each initialized child and all failure paths;
attached plans, schema numbers, names or cached booleans grant no authority.

Graph/cache extensions require selective versioned tags through their owning
registries, exact source replay, missing/duplicate/foreign/overlap/stale range
controls and unchanged legacy bytes when unused.
Builder accounting includes full retained carriers and metadata spare capacity.

The private partition cache projection uses a dedicated v1 header, the current
canonical source revision and the complete bounded HIR encoding. Replay takes
held source, reruns the partition compiler and compares the cache bytes with
the fresh HIR encoding. Backends receive the freshly resolved program. A
transported HIR, including one with a current revision and stale operands,
cannot supply partition authority. Ordinary graph and public target entry
points continue to refuse partition source.

The explicit `graph::to_partition_json` and `graph::verify_partition_json`
APIs select Graph v86 from held source. The graph retains the checked
operation and its ordered operands, cleanup and loan plans, and the partition
status table and allocation limits. Verification regenerates the complete
bounded graph and compares exact bytes. Existing graph and frozen evidence
entry points continue to refuse this profile.

Implementation scopes can be disjoint: frontend owns registry/source/HIR/loan/
cleanup and graph/cache replay; interpreter owns backing-versus-range identity;
native owns the selected carrier/domain and complete producer/consumer closure;
Core-Wasm owns a distinct authenticated handle domain, immutable table bounds and
read/write fences; adapter owners decide public/profile support independently.
All scopes must join coherently before source admission. Existing public tokens,
private/public envelopes, import permissions and frozen target refusals remain.

## Executable qualification requirements

Required same-source interpreter/native O0/O2/strict private Core-Wasm cases:
positive-width empty and nonempty partitions; exact/+1 extents and arithmetic;
byte sum/order/NUL/Unicode and non-UTF-8 preservation; mutation isolation;
pop/child surviving parent and siblings; deep-clone isolation and charges;
partial preparation/clone failure, postcondition failure, early exits, reentry,
sticky status, untouched failed outputs and exact-once final backing release.

Hostiles include stale generations, wrong domain/module, foreign handles,
overlapping forged ranges, child-partition refusal (including zero-offset and
whole-extent children), mismatched lengths, incomplete inventory and graph/
cache/source drift. Preserve 32 and +1 backing-event and 2 MiB adjacent controls;
physical native allocation interception must distinguish payload from metadata.
No allocation after first-read may be claimed until the actual owning trace
proves that particular fixture's read phase. Unsupported public routes must
refuse before producing artifacts.

### Focused private execution

At [`03ac7a14b`](https://github.com/wavect/semaprax/commit/03ac7a14b), 33
focused tests passed across the private source/HIR owner, operation, interpreter,
native and paired Core-Wasm receivers. The direct interpreter source case
partitions one 12,288-byte backing into 4,096 children and settles the Vec before
publishing the scalar. Its owner tests also cover exact/one-short funding,
loan expiry, stale and foreign handles, popped-child survival, and Drop.

The private native source case passed at O0 and O2 with 4,096 children. The
paired Core-Wasm source case ran a 4,096-child consuming iterator with settlement.
Both backends exercised all four normalized operation failures, untouched
result publication and reentry. The operation tests exercised geometry and
diagnostic precedence, operand mutations and sticky work refusal.

These cases establish the private execution contours described below. A single
fixture, `tests/fixtures/byte-partition-private.spx`, is shared by the three
private receiver harnesses. At `f9849aaf2e`, the
[hosted Linux qualification](https://github.com/wavect/semaprax/actions/runs/43507327046)
passed all 14 selected tests and scoped production Clippy. It covers the
shared 4,096-child result, all four geometry faults, and bounded producer
failure before partition. The interpreter, native C11 at O0/O2, and paired
Core-Wasm also share the zero-child fixture
`tests/fixtures/byte-partition-private-empty.spx` in the owning selectors.
The owning selectors also use an 8,192-byte fixture to check the exact
8,192-child result and derive a matching 8,193-byte, 8,193-child refusal.
The [33-test hosted run at `bc4692d4f`](https://github.com/wavect/semaprax/actions/runs/43512449690)
passed the boundary cases, interpreter registry controls, repeated native and
Wasm success case, and scoped production Clippy. The
[36-test hosted run at `8d35815fb`](https://github.com/wavect/semaprax/actions/runs/43514391464)
also passed scoped production Clippy and the repeated interpreter source case,
including failures in each lexical scope and bounded producer refusal.
The [run at `19a7abc88`](https://github.com/wavect/semaprax/actions/runs/43515606457)
passed the same 36 selected tests and Clippy with the generated native and
Wasm repeated-source failure paths and the native result-generation preflight.
The [run at `dfd77d2a7`](https://github.com/wavect/semaprax/actions/runs/43517557123)
passed the 36-test selection with native stale-ticket rejection. The
[42-test run at `9a8bcab70`](https://github.com/wavect/semaprax/actions/runs/43520085520)
also passed scoped production Clippy. It adds the independent operation and
source/HIR operand proofs to the selection and covers Wasm empty-ticket
settlement, duplicate backing claims, and private source/cache round-trip
with refusal at ordinary graph and public target entry points.

The previous native registry kept a child-table debit after the table was
freed. The Wasm host previously admitted new partitions using individual
settled slots even when another child from the same table stayed live. Native
C11 at O0/O2 and the Wasm host producer now pass runtime controls for the
shared rule. With 8,192 children issued, a single retained child keeps the
whole table charged. After it settles, the next partition succeeds and the
retired carrier still refuses. Both runtime controls observe two backing events
and 8,193 cumulative payload bytes. The repeated source fixture exercises the
native and paired Wasm emitters. The interpreter registry drops the settled
table and leaves its context index vacant so later partitions cannot revive an
old receipt. Empty partitions and successful whole-backing moves also leave
vacant context indices and release their registry tables immediately. Cumulative
reservation debits remain monotonic. The direct interpreter source entry recognizes the repeated fixture's
two sequential lexical scopes on one account. The first scope settles before
the second allocation. Either partition can select its own failure, and a
failed bounded producer stops before its partition.

The single and repeated fixtures also exercise a scalar postcondition over
the staged result. The direct interpreter checks that the postcondition
failure exit uses the same ordered finalizers as success. Native O0/O2 and
paired Core Wasm return `semaprax.contract.v1` code 2 after settlement.
The native test keeps the caller result sentinel intact and checks each
backing free. The Wasm test checks that failure has no result and that the
managed account returns to its metadata baseline before reentry.
The [hosted run at `ed2103682`](https://github.com/wavect/semaprax/actions/runs/43522226242)
passed all 43 selected tests and scoped production Clippy, including the
interpreter, native and paired Wasm postcondition controls.

The shared private iterator fixture, `tests/fixtures/byte-partition-private-iterator.spx`,
consumes a 4,096-child Vec, takes the first child and returns its 16-byte
extent. The interpreter checks the owning match, binds its slice to the
current child's exact loan origin, and reads through the registry. The chosen
match edge and canonical loan end edges govern the view's lifetime before the
ordered child and iterator finalizers run. Its empty-input control takes the
Done arm. The local 43-test partition selection passed, including the
generated native C11 O0/O2 and paired Core-Wasm runs of the shared fixture.
Both runs verify one payload backing per invocation and settlement before
reentry.
If the interpreter's iterator step refuses before transferring a child, it
settles the untransferred suffix while preserving the selected refusal. After
a transfer, it attempts both finalizers in the checked arm order, including
when the scalar read refuses.
The [hosted run at `ab2dec272`](https://github.com/wavect/semaprax/actions/runs/43526593692)
passed all 43 selected tests and scoped production Clippy.

The full shared ownership matrix and activation through ordinary source,
workspace and public target routes remain qualification requirements.

## Separate construction-fusion obligation

This primitive does NOT admit the unchanged 4,096-row fixture that executes
bytes_zeroed(3usize) inside its constructor loop. It cannot reinterpret 4,096
source allocations as one simply by naming a partition primitive. The separate
[construction-fusion contract](OWNED-BYTE-CONSTRUCTION-FUSION-V1.md) for
[issue #747](https://github.com/wavect/semaprax/issues/747)
requires a versioned general bulk-construction lowering with exact induction/trip
count, capacity, payload extent, fresh row, same Vec destination, no intermediate
owner escape, left-to-right effects and failure/early-exit equivalence, independently
replayed from current source. No filename, stable-id or benchmark constant can
substitute for that proof. The original maximum fixture and all payload/assertion
requirements remain unchanged. #733 and #746 completion and any efficiency
measurement remain pending.

Owning existing contracts: [byte data](PORTABLE-INDEXED-BYTE-DATA-V1.md),
[owned collections](OWNED-LEAF-COLLECTIONS-V1.md),
[cleanup](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md),
[call settlement](RFC-0004-NATIVE-CALL-SETTLEMENT.md) and
[public owned data](PUBLIC-OWNED-DATA-API-V1.md).

## Private receiving implementation checkpoint

`construction_fusion::partition` independently matches a deliberately narrow
pending operation against held current source and HIR: one immutable ordinary
`bytes_zeroed` whole owner, literal count/width, exact `Own Bytes` / `Value usize`
operands and `Own Vec<Bytes>` result, and canonical monomorphic identities.
The borrowed proof exposes the actual left-to-right operands and checks the
executor's authenticated whole extent against the original producer before
geometry preflight. Source/HIR membership, scalar/name/identity comparison work
shares the caller's counter; refusal consumes the remaining allowance.

This is a lowering interface, not an admitted call or a validation lease. The
ordinary intrinsic registry still refuses `bytes_partition`; independent full
source/HIR validation, ordinary argument staging and atomic call commit, numeric
status assignment, capacity proof and all three executor mappings remain
required. Computed operands, imported/parameter owners, aliases and child inputs
are outside this first receiving shape. The executor must keep the explicit
invocation range context alive across creator, container and returned internal
child cleanup, precharge actual metadata allocations through its real account,
and preserve the original owner on preparation refusal. Copy proof data cannot
extend that lifetime or grant a public carrier. Authored tests use a checked
ordinary producer plus an explicitly unvalidated pending-call fixture; they
assert that full validation and default source admission remain closed.

Geometry retains width as the semantic unsigned 64-bit value. Zero count with
empty backing accepts every positive semantic width, including the maximum;
no target address-space conversion may add a 32-bit-only language fault. A
nonempty range's width is bounded by its authenticated extent. Empty execution
must not fabricate a child merely to force width into a target pointer word.

## Ordinary receiver checkpoint (not activated)

`construction_fusion/partition/operation.rs` classifies the fixed three-operand
source/HIR candidate and performs dynamic semantic-usize preflight. It is no
source/name-resolution grant, whole-owner certificate, or validation lease.
The ordinary builtin registry remains closed until the complete receiving join.

`interpreter/byte_partition.rs` supplies a safe invocation-owned registry of
exact boxed contexts. Affine child receipts authenticate the current invocation,
context, slot and generation; scoped loan callbacks expose only checked ranges.
The context survives creator-function and parent-container cleanup, so internal
returns and popped children remain usable. Handles never retain the registry
themselves: a public copy-out must create a separately ordinary-charged whole
owner and drop the provider child before invocation teardown. A host, async task
or later invocation cannot retain these private handles. Unsupported escape
routes stay closed.

The registry holds one caller runtime account. Each backing core receives only
a nonallocating handle to that same monotonic account; per-operation replacement
accounts are not accepted. Registry
headers, full replacement backing (including live overlap), boxed contexts and
complete output carriers are reserved before allocation; existing core tables
and identities retain their own reservations. These are conservative cumulative
requested-layout debits, including the core's existing frame charge, not a claim
of minimal heap bytes. Deep clone uses that same runtime's ordinary event/payload
account. Per-leaf settlement must be called at the actual canonical finalizer;
registry Drop is administrative abandonment, never canonical success.

The focused run includes `construction_fusion::partition::operation::tests::`
(two bodies) and `interpreter::byte_partition::tests::` (three bodies). They cover
dynamic preflight precedence, empty/maximal width, operand mutations, popped
children, disjoint stores, clone charges, real requested allocator layouts,
exact/one-short precommit reservations, replacement overlap, foreign/stale and
loan refusal, and abandonment. Full source/HIR/graph/replay installation, ordinary
Value consumer closure, native/Wasm execution and the public copy-out bridge
remain required; these controls do not qualify an ordinary admitted program.

### Ordinary loan receiver checkpoint

The pending ordinary interpreter receiver holds the actual fully validated
function and its immutable loan plan. Its separately charged local loan table
retains the existing 256-loan limit; the private fused Core's 64-view path remains
unchanged. A copied view descriptor carries no finalization ownership: one
registry entry retains the affine Core loan. Exact CFG `end_edges`, rather than
node-union kills, retire that entry and stale descriptors fail sticky. Header,
table, Core and replacement backing requests debit the same invocation account
before allocation; no teardown refund replenishes it.

The private loan-capacity constructor now receives only the immutable
original-source owner and an exact current function pointer, after its complete
compiler checks and metered graph proof. It has no ordinary Value installer.
The eventual installer must bind each runtime child to the current checked owner
root and drive transitions from actual execution. Public copy-out must precede
registry teardown. Its authored controls are not evidence of source admission
or complete native/Wasm/public partition support.

### Pending ordinary funding adapter

The private interpreter adapter borrows the actual current invocation's mutable
backing-event and cumulative-payload counters. Copyable account handles borrow
that same ledger; they neither snapshot nor reset it. Ordinary deep clone keeps
32 backing events, 2 MiB cumulative payload and the 128 KiB individual-owner
bound, and reserves its actual byte allocation before creating the new Box.

Its administrative forecast uses actual requested layouts and the existing
logical dimensions: at most one consuming partition per whole-backing event,
8192 children per result, 256 checked loans, and complete registry replacement
overlap. It includes the original Core tables, identities, context/carrier/loan
headers and potential ordinary clone payload. This is a conservative requested
layout forecast, not a new semantic byte ceiling or an RSS measurement. A joined
factory must prove whole-owner identity and one-time consumption before relying
on the event-derived context bound.

`Account::funded` is a private receiving adapter, not evidence that the host has
funded its argument. The actual entry must acquire that finite funding from its
host policy before installing the receiver. Computing the forecast never grants
source authority or automatically creates that funding. The ordinary producer,
Value installer, retained-frame/public SDK route and all-target activation remain
closed until their complete receiving obligations are joined.

### Affine interpreter carrier checkpoint

The closed interpreter carrier holds a partition child as an affine owner. The
central ordinary Bytes alias receiver returns a guard for this carrier rather
than copying its receipt. Legacy whole backing aliases and private fused aliases
retain their existing behavior. A partition Slice alias copies only an opaque
view descriptor; the checked loan table retains the single finalization owner.
Scoped reads authenticate that descriptor on every use, and actual loan-plan
end edges expire all its aliases before owner renewal.

The authored carrier control covers descriptor aliasing, exact edge expiry,
move/store after expiry, stale-view sticky refusal and leaf settlement. The
ordinary evaluator installer, borrowed Bytes parameter adapter, aggregate leaf
finalization, and cross-function execution remain closed pending the fresh
validated partition callgraph and all ordinary consumers. These private controls
do not qualify ordinary source admission or public carrier escape.

Borrowed Bytes uses the same nonowning view descriptor as Slice. Its central
alias operation copies only that descriptor; Bytes-to-Slice conversion retains
the exact checked loan instead of creating an Arc or another Core loan. Owner
move/store/finalizer adapters reject view carriers. Reads after the actual end
edge fail for both Bytes and Slice aliases. This receiving adapter remains
private and needs the joined installer to bind the actual source owner root.

The typed interpreter iterator retains the whole producer's logical allocation
identity in each transferred Bytes child. Its staged one-byte store checks the
authenticated child's own extent and maps bounds failure to the existing
byte-buffer status. The staged owner remains available for canonical cleanup.
A successful store renews the child lease without allocating a backing or
changing any sibling range.

### Flat container receiver checkpoint

The private affine container retains the original paid Vec<Owner> output; it
creates no second value array or Arc control block. Its actual header is charged
before partition publication. Consuming iteration moves that same array into
IntoIter<Owner> after preflight and an exact paid iterator header. Extracted
children survive container settlement in the held invocation registry. Clear,
Vec settlement and iterator abandonment preflight the remaining children before
the first finalizer. A loan on a later child preserves the earlier children.
Settlement uses ascending row order and stays available after sticky refusal
once live loans end.

These private receiving operations are not an ordinary vec_pop declaration or
a language IterStep constructor. Actual Value dispatch, current source/root
binding, canonical call/result handoff and language iterator publication still
require the sealed entry join. The authored controls are unexecuted.

The private funding upper also includes the exact flat container and iterator
headers per partition. Its eventual installer must prove at most one consuming
iterator transition per partition, as well as one consuming partition per whole
backing event. Additional container construction cannot reuse that header debit.
The bound still grants no host funding or source authority.

### Private current-source owner

The private compiler factory `hir::byte_partition::ValidatedPartitionProgram`
resolves the original immutable AST and retains its resulting HIR without
cloning or replacing either subject. Its read-only `program()` and `source()`
accessors do not let callers mint another owner. `compile_source` runs full
source/resolution/HIR, canonical cleanup/status and loan checks under the
ordinary bounded compiler policy. `revalidate_compiler` repeats the compiler
validation boundary; the private
availability selector only admits the exact compiler operation during those
checks. Ordinary source verification, HIR validation and backend entry remain
closed. The focused execution above uses this owner for the private receivers.

The initial emitted callgraph is monomorphic and acyclic, at most 512 functions,
with scalar, byte-array, Bytes, Slice and the demanded canonical Vec/Iter/IterStep
of Bytes and Option of u8 or Bytes types. Authored types, module imports,
effects, dynamic callable carriers and generic function instances are refused.
Unused compiler-prelude declarations are not a reason to reject a program;
actual signature and expression types determine this closed scope. `receive_current_graph` consumes only the caller's held graph/provenance
proof work, without a fresh receiver allowance. It meters actual type/opcode
identity and child/statement probes. Neither this counter nor its exact/short
tests pay for the separate source/resolver/full compiler passes, compiler
physical storage, or runtime allocations. Compiler acquisition errors poison
the extra graph counter and zero graph credit refuses before compilation.
Partition roots must resolve through immutable current let moves to a direct
whole Bytes allocation or copy. Own parameters, helper returns and children
are uncertain roots and refuse at compile time; interprocedural Whole/Child
provenance remains closed. Runtime funding and whole-owner provenance remain
separate mandatory receiver checks. Native and Wasm ordinary lowering retain
explicit refusal until their sealed receivers join this factory.

The interpreter now has distinct affine flat Vec and iterator Value carriers.
Central semantic copy refuses both; default legacy/fused finalizers reject them
before any physical action. Private exact consumer adapters retain the paid
array through Value construction and consuming conversion and route canonical
leaf/container settlement through its actual Loans receiver. This still does
not install a registry in the ordinary evaluator or authorize a raw Value as a
source proof; the sealed entry and language IterStep publication remain pending.

### Invocation-held function frame checkpoint

One ordinary interpreter registry remains alive through the complete invocation.
Entering a checked function moves its caller's actual loan table and CFG point
into a paid frame directory. A nonreused frame nonce distinguishes reused local
loan slots from stale descriptors. Returning retires only the callee's physical
loans and restores the caller table; affine returned children retain the same
registry and backing context.

The directory's complete replacement backing and overlapping transient header
are prepaid before allocation. A callee allocates only its actual local loan
slots; a scalar helper allocates none. Already-paid idle arrays and directory
capacity are reused without charging a nonexistent allocation. Actual new
requests remain cumulative through repeated calls, with no refund on return.
The existing funding forecast covers one base frame; it does not certify
arbitrary repeated frame allocations or replace the entry's finite account.

A borrowed parameter descriptor refers directly to its held caller loan.
Canonical Borrow-parameter-only helpers have empty loan plans: the receiver does
not invent a callee LoanId, start point or end edge. Same-invocation identity,
an actual held ancestor frame, exact current parameter pointer, Borrow mode and
Bytes/Slice type are checked before binding the descriptor. Only the caller's
real end edge retires its one physical loan. Actual argument-root correspondence
and call staging still belong to the closed installer.

Per-function 256-loan validation alone is not proof that a shared backing has at
most 256 simultaneous physical views across calls. The eventual entry must join
an independently checked current-callgraph view bound before using that Core
inventory. These frame and alias controls do not authorize ordinary entry,
public carrier escape, or a higher view quota.
### Private checked Core-Wasm producer checkpoint

The internal Core-Wasm receiver accepts only `ValidatedPartitionProgram` from
original-source compilation, repeats complete compiler validation, and debits
its current-graph receipt separately. It emits a module and independently
validates its complete import section together; no module-byte constructor or
raw import receipt can authorize the producer. Ordinary source and public
browser/provider routes remain closed pending the cross-backend join.

The private producer import has exact signature `(i64,i64,i64)->(i64,i32)`:
current whole owner, count, width; then result Vec handle and operation code.
The host reserves registry entries, the final retained Vec array, and its handle
before transferring the whole owner. Registry commit fills that same array and
retains the original backing; no child payload or carrier-array copy is made.
Allocation exceptions are administrative fail-stop. Language refusal leaves the
whole owner live for the unchanged canonical operation-failure cleanup. Only
success commits the staged owned argument.

Private emitted failure status is `0x7000 + operation_code`, where codes 1–4
remain the independently owned partition domain's width/overflow/extent/metadata
codes. The offset distinguishes them from ordinary Core-Wasm statuses; it does
not invent a new language failure. The graph receipt is not a runtime allocation
ledger. Fixed host registry backing and ordinary payload limits retain their
existing scope; JavaScript objects, Maps, carrier arrays and VM bookkeeping are
excluded. This checkpoint does not claim complete physical-account funding,
public installation, or completed OPT #746 qualification.

The private paired entry embeds its own compiler-produced module bytes and accepts
only a positive safe-integer managed-byte amount at installation. It accepts no
replacement module, imports, witness, or issuer. `invoke()` publishes either
`{outcome:"returned",result:"<signed decimal i64>"}` or
`{outcome:"language_failure",status:{domain,code}}` after canonical settlement.
Partition guest codes normalize to the owning partition domain and code, not a
new semantic domain. Unknown traps and account exhaustion produce an
administrative refusal with no result/status and poison subsequent entry use.

One installation-held account reserves the exact index, lease/backing typed
arrays and whole-token/invocation typed arrays before construction. It then
reserves each fresh payload at its original allocation boundary. Whole-owner
release or final-child settlement releases that payload charge exactly once;
transfer into a partition does not release or recharge it. The fixed requested
metadata is `16384*6 + 8192*22 + 32*11 + 32*8` bytes. Metadata remains held across
reentry. Existing 32 events, 2 MiB cumulative payload and individual extent limits
are unchanged, and no caller amount raises them. Zeroed payload allocates one
funded buffer directly rather than an unfunded temporary plus copy.

This is requested typed-array/live payload accounting. Carrier/reference arrays,
Maps, objects/tickets, module bytes, Wasm memory/engine, GC retention, allocator
headers and process memory are excluded. It is not complete physical funding or
an engine-wide memory ceiling. Managed private entries have no copy-out bridge;
public copy-out funding remains a separate prerequisite.

The source owner also supplies a receiving-only simultaneous-view bound for an
exact current entry-function pointer. Complete independent compiler validation
precedes this additional graph proof. The same caller graph-work counter pays
for current function/loan origin comparisons and the fixed-array acyclic graph
traversal. No HIR clone or new allowance is created. The conservative bound is
the maximum current call-path sum of each frame's total validated local loan
sites. Exact Borrow-parameter roots are inherited descriptors and contribute no
new Core loans. This count can overestimate actual lifetime overlap; it is an
upper bound, not permission to mint any loan or override NLL edges. Sequential
callee invocations take the maximum, rather than adding to simultaneous use.
An upper bound above 256 refuses before installation. The private Wasm receiver
runs this proof for its actual main. Native/interpreter receivers must retain
their own actual root/call/edge binding and physical funding checks.

### Private native source receiving checkpoint

The original-source owner also has a private native projection for whole Bytes
allocation, partition, Vec<Bytes> length/capacity reads, scalar expressions and
monomorphic source calls. It renews the complete compiler proof, checks the
current entry's conservative view bound, and independently refuses consumers
without a complete shared carrier binding. The canonical native emitter and
cleanup vectors remain unchanged: a versioned C binding fragment selects the
shared carrier/helper ABI only after ordinary runtime definitions and before
the sealed source functions. No caller can supply a raw program or closure.

Partition preparation leaves the canonical staged Bytes live on failure; the
actual operation status selects its existing exact failure exit. Successful
commit checks child and Vec result generation capacity before clearing that
carrier. The exhaustion probe verifies that the original owner, unpublished
reservation, and caller result slot survive the refusal. One registry
covers the entry and all helpers, and detachment requires zero loans, leases,
Vecs and pending reservations before result publication. The additional graph
counter does not fund compiler proof or runtime heap storage. Ordinary/default
and public callable routes remain closed. Borrow-parameter receiving and the
complete caller-funded native account remain pending.

The private native projection admits consuming `Iter<Bytes>` and
`IterStep<Bytes>` over the shared carrier. Generated C checks the 88-byte
step size and the exact tag, item and rest offsets on native64. It uses the
canonical match and finalizer paths to transfer the first child and settle
the remaining iterator. The generated-source O0/O2 check reads a 16-byte
child from a 4,096-child partition and checks one payload allocation and one
free on each invocation.
A nonempty step authenticates the child and obtains the successor iterator
generation before transferring the child or writing the outputs. Exhaustion
leaves the source and all outputs intact. The terminal Done step settles
the empty suffix without issuing a generation.

Two owning source bodies retain original 4096/count geometry at O0/O2 and actual
four-code failure/poison-result/reentry controls. Both bodies passed in the
focused run above.
The private native entry copies the normalized failure domain into caller-owned
storage before its local status arena expires. Success leaves the caller's
status outputs untouched. An AddressSanitizer probe on the retained generated
failure unit reproduced a stack-use-after-return with the original entry and
passed at C11 O0 and O2 with the copied domain and poisoned scalar output.

### Closed interpreter invocation account and whole backing receiver

The private interpreter receiver now lends one stack-held account into the
optional evaluator state and the invocation loan registry. The account borrows
the actual allocation-event and payload counters; no callee constructs a new
allowance or copies a counter snapshot. Its borrowed handle preserves the
account lifetime through a private object interface. Ordinary evaluators keep
None and their existing allocation behavior.

Selected zeroed, bounded-zeroed, copy, and explicit owned-leaf deep-clone
receivers debit that same account before each exact boxed payload allocation.
Both allocation-ID checks consult its current event count. A direct whole
backing carries a nonreused account identity and is affine: ordinary aliases
cannot clone it. Partition handoff moves its original Box without conversion,
payload copying, or another event debit; semantic precommit faults return that
same owner. Foreign-account whole carriers are administrative refusals rather
than operation statuses. Logical clone exhaustion retains the ordinary Vec
allocation failure; exhausted metadata funding remains administrative.

Preparation consumes the shared current-source view bound once at the exact
invocation entry and pays the invocation header remainder plus actual local
loan storage before allocation. Funding remains explicit caller input, not
an amount automatically minted from a forecast. The older funding::upper
describes the typed handle and one base frame only; it does not fund arbitrary
dynamic frames or certify this new invocation wrapper.

This is a closed receiving checkpoint, not ordinary source activation. The
actual root/argument/CFG installer, direct-whole view registry, every flat
Vec/IterStep publication, canonical leaf finalization, and final public copyout
must still join before installing the optional evaluator state. Effects,
resumption, imported foreign carriers, and unsupported view paths remain
outside the sealed profile. Receiver tests prepare a real source-held owner
but invoke individual receivers; they do not claim whole-program execution.

### Closed whole-view holding and move-out

The ordinary interpreter registry has a separate whole-holder constructor.
It takes the already allocated complete Box and prepays the actual context,
one owner slot, checked ordinary view table, temporary headers, fixed identity
boxes and registry replacement overlap before publication. It accepts an empty
whole owner without changing the partition operation's positive-width rule.
No payload allocation or ordinary event debit occurs while holding it.

The Core records that this context was formed for a whole owner. Move-out
requires its exact affine child, zero remaining views, one live complete slot,
and the current backing; it moves the original owner and invalidates the old
receipt. A partition child cannot pass this receiver, including the special
case where one child spans all bytes. Current checked loan end edges retire
views before move-out; administrative teardown does not certify canonical
settlement or success.

Two additional receiving bodies cover genuine current Own-root loans, exact
CFG expiration, original-pointer preservation, independent metadata request
sums, one-short refusal before allocation, empty whole views and child-to-whole
refusal. The focused run includes both private controls. Evaluator root binding,
call/return transitions and all canonical finalizers still must be joined
before ordinary source installation.

### Private direct source entry checkpoint

The interpreter now has a source-held direct receiver for an immutable main
body containing exactly a literal bytes_zeroed allocation, bytes_partition,
and i64_from_usize(vec_len(children)). It receives the original sealed program,
rechecks the current graph, and consumes the invocation view bound once.
Actual binding IDs and operation operands are matched before any allocation.
Receiving comparisons consume the same work counter. Caller-supplied finite
Account storage remains held through finalization; no default funding amount
or ordinary admission is introduced.

On the completed path, the affine Vec is finalized by its exact whole-value
action in the canonical scalar exit before publication. Three repeated calls
share one account and its cumulative event/payload counters. Source controls
retain the original 4096 children and 12288-byte backing, one child, and an
empty partition with maximal semantic width. Unsupported helper provenance,
additional statements, and work or physical shortage refuse before payload
effects.

For a geometry fault, the receiver matches the checked operation-failure exit
to the exact staged Bytes argument and its sole canonical drop action before
allocating. Preparation returns that same whole backing for finalization.
The geometry fault remains the selected status and publishes no scalar.
Administrative refusal keeps `cleanup_complete=false`.

The direct interpreter receiver also accepts literal requested and bound
operands to `bytes_zeroed_bounded`. It verifies the producer's exact
operation-failure exit has no owners to settle. An over-bound request selects
the existing buffer-capacity failure before any backing allocation or partition
geometry check. A successful request enters the same caller account and
partition path as `bytes_zeroed`. The private native projection admits bounded
zeroing through the existing status check and the same selected backing
allocator. The paired Wasm path already lowers that check ahead of its backing
import.

Local probes exercised the actual empty producer-failure exit and replayed
the bounded guard on retained native C11 and Core-Wasm partition artifacts.
The hosted qualification above exercised the focused original-source tests for
all three private receivers.
Ordinary activation still needs copy and computed-request execution, views,
iteration, helpers, contracts, general loan/frame settlement and public owned
results.

The private current-graph call whitelist includes exactly the checked
`i64_from_usize` conversion used by that scalar publication contour; other
numeric and String operations remain closed. The full compiler renewal still
authenticates the conversion signature and operands. Its owning controls
mutate the actual callee, argument type and ownership. The conservative
256/257 view-bound control counts actual `SliceView` and `BorrowedCall` sites;
`vec_len` borrows the container and contributes no Byte-root loan site. These
controls passed in the focused run above.
