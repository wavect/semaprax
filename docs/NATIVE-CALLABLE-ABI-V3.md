# Native callable ABI v3

Status: private physical tranche. `SPXNABI3` fixes the compiler/host descriptor
projection and seven bounded physical wire formats. Independent compiler and
host codecs are joined by a private dynamic-image loader with root-image
provenance checks, a private OS-seeded receipt authority and fixed-capacity
atomic ledger/facade, and graph-derived strict-C11 providers that execute all
14 authoritative normal scenarios at `-O0` and `-O2`. A narrower joint test
connects scalar discard-two and owned identity through provider, loader, and
host receipt commit. The other scenarios have not crossed that full boundary;
pending/pre-execute unwind fails closed, and post-`CallCommit` host evidence
decoding/replay still allocates. This exposes no public admission or iOS static
constructor and grants no general physical-finalizer or malicious-code
containment guarantee. Ordinary native resource compilation remains
`SPX-B104`.

## Scope and primitives

This ABI is the metadata boundary for [RFC 0004](RFC-0004-NATIVE-CALL-SETTLEMENT.md).
It binds one validated direct-trivial owned callable to its recovery graph,
future `execute`/`settle` entry points, exact descriptor capacities, and a
dynamic-image or iOS-static linkage role. The current emitter derives the
physical target from the compiler's own build target and exposes no cross-target
configuration. Android, iOS, and Windows cross-emission and runtime evidence
are absent. It is a new contract: v1, v2, settlement-proof v1, and v3 are
mutually incompatible and there is no negotiation or fallback.

All descriptor and graph integers are little-endian `u32`. A fingerprint is 32
raw SHA-256 bytes. Text is a `u32` byte length followed by non-empty, NUL-free,
well-formed UTF-8. Graph bytes are framed by a `u32` byte length. Counts are
dense, all arithmetic is checked, truncation and trailing bytes fail closed,
and the complete descriptor is at most 64 KiB. No native struct layout,
padding, pointer, handle, `size_t`, credential, loader path, or host secret is
part of the descriptor.

The linkage profile is closed:

| Tag | Profile |
| ---: | --- |
| `1` | Dynamic image on Linux, macOS, Windows, or Android |
| `2` | iOS static registration; no dynamic image open or unload |

The dynamic-image role has a private desktop loader on Unix and Windows; the
iOS-static role remains metadata only, not an implemented platform host.
Future iOS device, iOS simulator, and Mac Catalyst/macabi targets MUST retain
distinct target strings and MUST NOT share admission evidence merely because
they use the same static-registration linkage tag.

## Descriptor layout

The canonical descriptor is sequential; variable text, signature, and graph
fields make fixed byte offsets inappropriate. It is encoded in this exact
order:

| Field | Encoding |
| --- | --- |
| Magic | eight bytes `SPXNABI3` |
| Version | `u32 = 3` |
| Header size | `u32 = 20` |
| Total size | `u32`, exactly the complete descriptor length |
| Physical target | framed text |
| Linkage profile | `u32`, closed table above |
| Fingerprints | the 19 fingerprints below, each 32 bytes |
| Module identity | framed text |
| Function identity | framed text |
| Descriptor getter symbol | framed text |
| Execute symbol | framed text |
| Settle symbol | framed text |
| Call-ABI tag | `u32 = 3` |
| Required obligations | `u32 = 0x000003ff` |
| Capacities | the 15 `u32` values below, in listed order |
| Signature | canonical v2-shaped parameter/result transcript below |
| Settlement graph byte length | `u32` |
| Settlement graph | exactly that many canonical graph bytes |

The 19 fingerprints are ordered: descriptor schema, target, semantic module,
physical module, function template, execution/cleanup, event dictionary,
trace-path certificate, recovery contract, settlement graph, request schema,
execute-response schema, frame schema, decision schema, action-evidence schema,
candidate-receipt schema, committed-receipt schema, call ABI, and call contract.
Every fingerprint is nonzero.

The 15 capacities are ordered: request bytes, execute-response bytes, frame
bytes, decision bytes, action-evidence bytes, candidate-receipt bytes, maximum
event count, dictionary bytes, dictionary entries, resource count, checkpoint
count, graph work units, active frames, quarantined frames, and reserved
instance bytes. Their exact derivation is:

```text
request             = 104 + sum(i64: 16, bool: 12, owned: 20)
execute_response    = 156 + 4 * maximum_event_count
frame               = 388 + 12 * resource_count
decision            = 172
action_evidence     = 196
candidate_receipt   = 372 + 12 * resource_count

resource_count      <= 4_096
checkpoint_count    <= 65_536
graph_work_units     = resource_count * checkpoint_count <= 1_000_000
active_frames        = 256
quarantined_frames   = 64
reserved_instance_bytes
  = (256 + 64)
    * (request + execute_response + frame + 172 + 196
       + candidate_receipt + 524)
  <= 64 MiB
```

The first six byte capacities are nonzero and at most 1 MiB. Event and
dictionary bounds retain their authenticated compiler-derived values and must
fit their `u32` fields. No decoder may truncate to a bound or repair a count.
The obligations word is one indivisible v3 profile. Its only canonical value is
`0x000003ff`; it is not a feature-negotiation mask.

Signature entries are exactly the v2 canonical shape. Parameter tag `1` is
`u32 tag`, dense `u32 index`, value-identity text, and scalar kind `1 = i64` or
`2 = bool`. Parameter tag `2` additionally carries dense owner ordinal,
resource-identity text, lifecycle-identity text, and payload kind `1 = opaque
u64`. Result tag `1` is scalar `i64`. Result tag `2` carries the selected owned
parameter index, its exact value identity, and owner ordinal. The result must
refer to one preceding owned parameter.

## Settlement graph

The embedded pointer-free graph is encoded as:

1. `u32 version = 3`;
2. function-identity text;
3. recovery-contract, execution/cleanup, and trace-certificate fingerprints;
4. `u32 resource_count` and `u32 checkpoint_count`;
5. each dense checkpoint: ordinal, state count, one state tag per resource,
   admitted outcome, abort cleanup count and ordinals, then accept cleanup count
   and ordinals;
6. start count and dense start ordinals; and
7. edge count followed by `from`, `to`, and one typed action.

State tags are `1 Live`, `2 ProvisionalResult`, `3 Finalizing`, `4 Dead`, and
`5 Published`. Outcome tag `0` is absent, `1` scalar success, `2` semantic
failure, and `3` owned success followed by its owner ordinal. Edge action tag
`1` is `Finalize(owner)` and `2` is `StageOwnedResult(owner)`. Tag `3` is:

```text
CertifyOutcome(
  trace_evidence: 32 bytes,
  ordinal_count: u32,
  ordinals: ordinal_count * u32,
  trace_outcome: u32
    1 = scalar success
    2 = owned success
    3 = failure, followed by selected_ordinal: u32
)
```

The evidence digest is nonzero and the host recomputes it exactly as:

```text
SHA256(
  "semaprax.native-recovery-trace-evidence.v1\0"
  || trace_path_certificate_fingerprint[32]
  || ordinal_count_as_u64_le
  || each_ordinal_as_u32_le
  || trace_outcome_as_one_byte
  || selected_ordinal_as_u32_le_if_failure
)
```

This binds one canonical ordinal/outcome witness to the separately carried
trace-certificate fingerprint. It is not independent host acceptance,
reconstruction, or walking of the trace-path trie-DFA certificate itself. The
graph must satisfy RFC 0004 density, reachability, forward-DAG,
state-transition, cleanup-order, and terminal-outcome rules. `Finalizing` and
`Published` are closed vocabulary values but are never admissible checkpoint
states. Canonical bytes are never sorted or repaired by the host.

## Frozen runtime wires

Every wire begins with the same 20-byte envelope: its eight-byte magic,
`u32le version = 3`, `u32le header_size = 20`, and `u32le total_size`. Except
for execute-response storage, total equals the supplied slice; a response total
declares its canonical prefix and every remaining capacity byte is zero. All
remaining integers are little-endian;
all arithmetic, count conversions, and buffer ranges are checked. Prefixes,
trailing bytes, unknown tags, noncanonical booleans, capacity disagreement,
overlapping provider buffers, and zero required identities fail closed.
Decoders never sort, truncate, repair, negotiate, or fall back.

### Execute request: `SPXNRQ03`

The fixed prefix is 104 bytes: the common envelope, call contract, nonzero
`u64` invocation, nonzero `u64` frame generation, nonzero 32-byte challenge,
and argument count. Arguments follow the exact signature order. Scalar tag `1`
carries the dense parameter index and either `i64` as eight bytes (16 bytes
total) or canonical Boolean `u32` zero/one (12 bytes total). Owned tag `2`
carries dense parameter index, dense owner ordinal, and opaque `u64` payload
(20 bytes total). Payload zero is valid and never means dead.

### Execute response: `SPXNEX03`

Its capacity is `156 + 4 * maximum_event_count`; a canonical response uses an
exact prefix and leaves the unused preallocated tail zero. In order it contains
the envelope; call contract; invocation and frame generation; challenge;
request digest; certified checkpoint; outcome tag and detail; result payload
`u64`; event count; and exact nonzero event ordinals. Outcome tag `1` is scalar
success (`detail = 0`, payload contains the canonical `i64` bits); tag `2` is
semantic failure (`detail` is the selected ordinal, payload zero); tag `3` is
owned success (`detail` is the owner ordinal and payload exactly equals its
frame payload).

The C return is physical adapter evidence, never semantic status. Only zero
makes response bytes eligible for parsing. A nonzero execute return permits
`Abort(PhysicalResult(code))` only when the independent frame remains valid at
a certified checkpoint; otherwise the exact instance is quarantined.

### Recovery frame: `SPXNFR03`

The mutable caller-owned frame has capacity `388 + 12 * resource_count`. Its
exact order is:

| Field | Encoding |
| --- | --- |
| Envelope | common 20-byte header |
| Call, recovery, graph fingerprints | three 32-byte values |
| Invocation, frame generation | two nonzero `u64` values |
| Challenge | nonzero 32 bytes |
| Request, response-storage, semantic-trace digests | three 32-byte values |
| Execute return tag | `1 Pending`, `2 Returned` |
| Execute return code | `u32`; zero while pending |
| Certified checkpoint and phase | two `u32` values |
| Locked-decision digest | 32 bytes; zero only before decision commit |
| Next action index, record count, active finalizers | three `u32` values |
| Resource count | `u32` |
| Resource cells | state `u32`, exact opaque payload `u64` per owner |
| Action-chain digest | 32 bytes |
| Pre-candidate frame digest | final 32 bytes |

Resource tags are `1 Live`, `2 ProvisionalResult`, `3 Finalizing`, `4 Dead`,
and `5 Published`. Provider phases are `1 Executing`, `2 DecisionLocked`,
`3 ActionInProgress`, and `4 ProviderSettled`. Host-only phases are
`5 ReceiptCommitted` and absorbing `6 Quarantined`; provider output can never
assert either. The host initializes the all-live start and payload cells before
`CallCommit`. The provider binds the request digest, cells, generation,
invocation, challenge, and graph before any effect.

`Finalizing` is written before entering a physical finalizer and `Dead` only
after normal return. A returned or unwound frame with `Finalizing`, a nonzero
active-finalizer count, or state/checkpoint disagreement is uncertain and can
never be retried.

### Settlement decision: `SPXNDC03`

The fixed 172-byte decision contains the envelope; call, recovery, and graph
fingerprints; invocation and generation; challenge; decision tag; and one
`u32` detail. Tags are `1 AcceptScalar`, `2 AcceptSemanticFailure`,
`3 AcceptOwned(detail = owner ordinal)`, `4 AbortPhysical(detail nonzero)`,
`5 AbortMalformedResponse`, `6 AbortTraceRejected`, and `7 AbortHostUnwind`.
Every tag except `3` and `4` requires zero detail.

### Action evidence: `SPXNAC03`

The fixed 196-byte record contains the envelope; call, recovery, and graph
fingerprints; invocation and generation; challenge; dense zero-based action
index; boundary tag; owner ordinal; exact payload `u64`; before and after state;
and certified checkpoint. Boundary tags are `1 FinalizeStart`,
`2 FinalizeComplete`, and `3 Publish`. Start records the transition to
`Finalizing`; completion requires `Finalizing -> Dead`; publish requires
`ProvisionalResult -> Published`. Records feed the action chain in execution
order and are evidence, never finalizer authority.

### Candidate receipt: `SPXNCR03`

The provider candidate has capacity `372 + 12 * resource_count`. It contains
the envelope; call, recovery, and graph fingerprints; invocation and
generation; challenge; request, response-storage, semantic-trace,
pre-candidate-frame, decision, and action-chain digests; outcome tag and detail;
canonical `active_finalizers = 0`; resource count; and one disposition-state
`u32` plus exact payload `u64` per owner. Disposition tag `1 Dead` or
`2 Published` is a receipt-only table. Candidate outcome tags are `1 Scalar`,
`2 SemanticFailure`, `3 Owned(detail = owner ordinal)`, and
`4 Abort(detail = 0)`. Owned accept has exactly the selected owner published
and all others dead.

Identical-decision replay from `ProviderSettled` re-encodes byte-identical
candidate bytes with no action or finalizer effect. A provider candidate has no
ledger authority.

### Host committed receipt: `SPXHRP03`

This fixed 524-byte role is host-only. In order it contains the envelope;
32-byte exact-instance binding; call, recovery, and graph fingerprints;
invocation and generation; challenge; request, response-storage, semantic,
frame, decision, action-chain, candidate, ledger-before, and ledger-after
digests; publication tag and `u32` detail; then a 32-byte HMAC. The body is
bytes `0..492`; the HMAC occupies `492..524`.

Only an independent host parser/replay gate may construct it. One exact
64-byte OS-random fill is separated into a nonzero 32-byte receipt key and
nonzero 32-byte instance binding. There is no retry, deterministic fallback,
or capability-token-key reuse. The HMAC is:

```text
HMAC-SHA256(
  K_receipt,
  "semaprax.native-callable-host-receipt-auth.v3\0"
  || F(receipt[0..492])
)
```

Publication tag `1` means no owner publication and requires zero detail; tag
`2` names one published owner ordinal. The host must make the authenticated
receipt cache and exact ledger transition visible as one atomic
`ReceiptCommit`. Replay returns the cached result without a second mutation. A
postcommit conflict quarantines while preserving the original receipt and
publication.

### Digest DAG

Let `F(x)` mean `u64` big-endian length followed by `x`:

```text
RQ = SHA256("semaprax.native-callable-request-digest.v3\0" || F(request))

RS = SHA256(
  "semaprax.native-callable-execute-response-storage-digest.v3\0"
  || execute_return_u32le
  || F(full_preallocated_response_storage)
)

DD = SHA256("semaprax.native-callable-decision-digest.v3\0" || F(decision))

A[0] = SHA256(
  "semaprax.native-callable-action-chain-seed.v3\0"
  || DD
  || expected_semantic_action_count_u64le
)

A[j + 1] = SHA256(
  "semaprax.native-callable-action-chain-step.v3\0"
  || A[j]
  || j_u64le
  || F(action_record)
)

FD = SHA256(
  "semaprax.native-callable-pre-candidate-frame-digest.v3\0"
  || F(frame[0..total_size-32])
)

CD = SHA256("semaprax.native-callable-candidate-digest.v3\0" || F(candidate))
```

The semantic digest is the existing nonzero recovery trace-evidence digest for
an accepted response and exact terminal graph edge; abort uses 32 zero bytes.
`FD` excludes only the final self-digest field. `CD` is host-computed and is
not written into the provider frame, keeping the graph acyclic. The host ledger
digests are exactly:

```text
LB = SHA256(
  "semaprax.native-callable-ledger-before.v3\0"
  || instance_binding32
  || call_contract32
  || invocation_u64le
  || frame_generation_u64le
  || owner_count_u32le
  || each owner in ascending ordinal order as
     ordinal_u32le || slot_u64le || generation_u64le
     || state_u32le(1 = InInvocation)
)

LA = SHA256(
  "semaprax.native-callable-ledger-after.v3\0"
  || LB
  || candidate_digest32
  || owner_count_u32le
  || each owner in ascending ordinal order as
     ordinal_u32le || slot_u64le || generation_after_u64le || state_u32le
)
```

Ledger-after state `1 Retired` requires generation-after zero. State
`2 Published` requires the checked predecessor generation plus one; only an
owned accept has exactly one published owner. Provider bytes never choose
these transcripts.

## Hash DAG and symbols

Let `F(x)` mean `u64` big-endian byte length followed by `x`. Domain strings
include their displayed terminal NUL. SHA-256 inputs are concatenated exactly;
numeric transcript fields are raw little-endian `u32` values.

The descriptor schema and frozen request, execute-response, frame,
decision, action, candidate-receipt, and committed-receipt role fingerprints
use respectively:

```text
semaprax.native-callable-descriptor-schema.v3\0
semaprax.native-callable-request-schema.v3\0
semaprax.native-callable-execute-response-schema.v3\0
semaprax.native-callable-frame-schema.v3\0
semaprax.native-callable-decision-schema.v3\0
semaprax.native-callable-action-schema.v3\0
semaprax.native-callable-candidate-receipt-schema.v3\0
semaprax.native-callable-committed-receipt-schema.v3\0
```

Each is the hash of its domain plus one canonical ASCII statement. The complete
tables, tags, digests, bounds, and rejection rules above are normative with
these compact identities. Their literal bytes have no terminal NUL:

```text
SPXNABI3;u32le;header=20;sequential-no-offsets-no-trailing;target;linkage-profile;19-fingerprints;module;function;getter;execute;settle;abi-tag;obligations;15-capacities;signature;graph-len;graph
SPXNRQ03;v3;u32le;header20;total-exact;call32;invocation-u64;generation-u64;challenge32;argc;args[tag,index,payload];scalar-tag1;i64-8;bool-u32-0-or-1;owned-tag2-owner-u32-payload-u64;no-trailing
SPXNEX03;v3;u32le;header20;total-declared;zero-tail-to-capacity;call32;invocation-u64;generation-u64;challenge32;request-digest32;checkpoint;outcome;detail;payload-u64;event-count;ordinals;outcomes1-scalar-2-semantic-3-owned
SPXNFR03;v3;u32le;header20;total-exact;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;request32;response32;semantic32;return-tag;return-code;checkpoint;phase;decision32;next-action;record-count;active-finalizers;resource-count;cells[state-u32,payload-u64];action-chain32;pre-candidate-frame32
SPXNDC03;v3;u32le;header20;total172;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;decision-tag;detail;tags1-scalar-2-semantic-3-owned-4-physical-5-malformed-6-trace-7-unwind
SPXNAC03;v3;u32le;header20;total196;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;action-index;boundary-tag;owner;payload-u64;before-state;after-state;checkpoint;tags1-start-2-complete-3-publish
SPXNCR03;v3;u32le;header20;total372-plus-12r;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;request32;response32;semantic32;frame32;decision32;action32;outcome;detail;active-finalizers-zero;disposition-count;cells[disposition-u32,payload-u64]
SPXHRP03;v3;u32le;header20;total524;host-only;instance32;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;request32;response32;semantic32;frame32;decision32;action32;candidate32;ledger-before32;ledger-after32;publication;detail;hmac32;separate-receipt-key;atomic-ledger-and-cache
extern-C;getter=const-u8-ptr(void);execute=u32(const-u8-ptr,u32,u8-ptr,u32,u8-ptr,u32);settle=u32(u8-ptr,u32,const-u8-ptr,u32,u8-ptr,u32);windows-cdecl;synchronous;same-thread;no-unwind;no-longjmp;no-callbacks;no-retained-pointers;no-reentrancy
```

For any current one-statement identity, its fingerprint is
`SHA256(domain || F(statement))`. The target fingerprint is
`SHA256("semaprax.native-callable-target.v3\0" || F(target_utf8))`. The
physical-module fingerprint is SHA-256 over
`semaprax.native-callable-physical-module.v3\0`, then length-framed descriptor
schema, target, and semantic-module fingerprints, length-framed module UTF-8,
and the raw little-endian linkage word. The settlement-graph hash is:

```text
SHA256("semaprax.native-callable-settlement-graph.v3\0" || F(graph_bytes))
```

Each `CertifyOutcome` witness is therefore sealed twice for different purposes:
the trace-evidence digest binds the trace-certificate fingerprint plus exact
ordinal/outcome transcript, while the graph fingerprint binds its complete
wire encoding and topology. Recomputing the outer graph and call-contract
hashes after mutating a nonzero witness or digest does not make the descriptor
valid; independent host recomputation of the trace-evidence digest must still
match.

The call-ABI fingerprint uses
`semaprax.native-callable-c-abi.v3\0` and binds the exact ABI statement,
including its Windows calling convention and synchronous
no-unwind/no-`longjmp`/no-retained-pointer/no-callback rules. Its fingerprint is
`SHA256(domain || F(statement))`; the ABI tag and obligations are bound
separately by the call contract.

The six execute arguments are `(request, request_len, frame, frame_len,
response, response_capacity)`. The settle arguments are `(frame, frame_len,
decision, decision_len, candidate, candidate_capacity)`. Request and decision
are read-only; frame is mutable; response and candidate are disjoint mutable
outputs. Every pair of supplied ranges must be disjoint.

The call-contract fingerprint uses
`semaprax.native-callable-contract.v3\0`. It length-frames, in order, target,
the first 18 descriptor fingerprints (everything before the call contract),
module identity, and function identity; appends raw linkage, ABI, obligations,
and all 15 capacity words; then appends the canonical signature transcript.
The call contract does not hash symbols or the descriptor containing itself.
This makes the dependency graph acyclic.

These frozen replacements intentionally change the earlier private metadata
fingerprints, symbols, and known answers. Independent encoders, parsers,
canonical re-encoders, known answers, mutation tests, cross-binding tests, and
version-confusion tests are required before the bytes count as evidence.

After the contract exists, the symbol seed under
`semaprax.native-callable-symbol-seed.v3\0` length-frames physical module,
function template, recovery contract, settlement graph, the seven wire-schema
fingerprints, call ABI, and call contract. Role hashes use the distinct domains
`semaprax.native-callable-getter.v3\0`,
`semaprax.native-callable-execute.v3\0`, and
`semaprax.native-callable-settle.v3\0`. Each symbol is `spx_`, the first 24
digest bytes as lowercase hexadecimal, and suffix `descriptor_v3`,
`execute_v3`, or `settle_v3`. Symbols are distinct valid C identifiers bounded
to 1,024 bytes.

```text
schema statements ----> schema fingerprints ---+
target ----------------> target fingerprint ----+--> physical module
semantic HIR ----------> semantic/template/execution/dictionary/trace/recovery
settlement graph bytes -> graph fingerprint -----+
all preceding identities + capacities + signature --> call contract
physical/template/recovery/graph/wires/ABI/contract --> symbol seed --> symbols
```

## Runtime role separation

Six frozen roles are provider-visible:

| Role | Magic | Descriptor capacity |
| --- | --- | --- |
| Execute request | `SPXNRQ03` | request bytes |
| Execute response | `SPXNEX03` | execute-response bytes |
| Recovery frame | `SPXNFR03` | frame bytes |
| Settlement decision | `SPXNDC03` | decision bytes |
| Action evidence | `SPXNAC03` | action-evidence bytes |
| Candidate receipt | `SPXNCR03` | candidate-receipt bytes |

The provider must never emit a committed receipt. `SPXHRP03` identifies the
host-only committed-receipt role created only after
independent candidate parsing, exact-instance/frame-generation replay, and host
authentication. It is not provider output, is not covered by a provider buffer
capacity, and is the only role eligible to accompany public ledger
`ReceiptCommit`. The private joint scalar-discard and owned-identity paths now
exercise this boundary through exact loader admission and host authentication;
the remaining corpus and failure paths are not yet joint evidence.

## Phases, finalizer uncertainty, and lifetime

The authoritative post-`CallCommit` phases are `Executing`, `DecisionLocked`,
`ActionInProgress`, `ProviderSettled`, host `ReceiptCommitted`, and absorbing
`Quarantined`. A provider candidate is evidence, not the host-committed phase.
Unwind before decision lock selects `Abort(HostUnwind)`; after lock it resumes
the exact decision. A resource records `Finalizing` before physical effect and
`Dead` only after normal return. Interruption while `Finalizing` is uncertain,
must quarantine, and must never retry the finalizer.

The future ownership relation is an acyclic lifetime DAG:

```text
module instance / static registration
  -> nonreused frame generation and invocation
     -> preallocated request, response, frame, decision, action, candidate,
        and host committed-receipt storage
        -> locked decision and provider evidence
           -> independently authenticated host committed receipt
              -> one ledger outcome

quarantine retains the module/static registration, every preallocated buffer
including host committed-receipt storage, decision/evidence, owners/results,
callbacks, and finalizer pins
```

Dynamic images become only unload-eligible after draining and release of every
frame, owner, result, credential, callback, and finalizer pin. The iOS static
profile resolves an admitted registration table at link/bootstrap time and has
no `dlopen`/unload claim, but it must preserve the same logical instance,
generation, settlement, draining, and quarantine rules.

## Threat boundaries and nonclaims

Descriptor/wire equality and hash validation do not authenticate code provenance,
make malicious native code memory-safe, observe omitted side effects, recover a
process crash, or make an interrupted non-idempotent finalizer retryable. This
private tranche executes all 14 authoritative normal scenarios in generated
providers, but only scalar discard-two and owned identity through the joint
desktop dynamic-loader and host path. It does not implement static-registration admission, callbacks,
async work, concurrency, fork/hot reload, imported finalizers, cross-target
emission, mobile execution, public adoption, or ecosystem FFI. The emitter is
bound to its own build target; there is no Android/iOS/Windows cross-emission
evidence, and v3 Windows runtime CI remains unobserved. Legacy loader
constructors still reject `SPXNABI3`; the separate private v3 constructor admits
only exact root-provenance images.

## Mandatory gates

Before any runtime or public claim, all of these must pass together:

- deterministic compiler bytes and fixed fingerprints plus an independently
  implemented host parser and canonical re-encoder;
- all seven frozen runtime roles independently encoded, parsed, canonically
  re-encoded, and covered by exact byte, tag, digest, and host-HMAC known answers;
- every-prefix, trailing-byte, every-byte mutation, hostile count/text/tag,
  overflow, cap, graph-topology, cross-module/target/trace, and rehashed
  substitution rejection;
- exact trace-evidence digest recomputation, nonzero digest enforcement, and
  resealed witness/digest mutation rejection without claiming independent
  trace-path DFA certificate acceptance;
- v1/v2/proof/v3 version-confusion rejection, unchanged v2/proof known answers,
  default-consumer hiding, and loader pre-open rejection for both constructors;
- all 14 authoritative corpus cases, dynamic and iOS-static profile fixtures,
  exact candidate replay, host-only committed-receipt authentication, and
  duplicate/stale/cross-bound rejection;
- physical finalizer order/counters, every interruption boundary, quarantine,
  draining/unload or static-retention, sanitizers, and Linux/macOS/Windows plus
  Android/iOS evidence; and
- the complete repository formatting, strict Clippy, test, doctest, rustdoc,
  package, dependency-policy, example, and documentation-link gates.

Until the physical and public gates pass, `SPX-B104` remains closed.

Current bounded evidence covers all 14 normal scenarios inside generated
strict-C providers and connects only scalar discard-two and owned identity
through dynamic loader plus host receipt commit. Pending/pre-execute
`AbortHostUnwind` deliberately returns a nonzero settle failure with no frame,
candidate, or physical-effect mutation until its canonical response-storage
and execute-return transcript is specified. Host evidence decoding and replay
also remain allocating after `CallCommit`; panic is absorbed into exact
pre-reserved quarantine, but allocator-failure hardening is not claimed.
