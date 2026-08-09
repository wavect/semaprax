# Native callable ABI v3

Status: private metadata contract. `SPXNABI3` fixes the current compiler/host
descriptor projection. Its seven future physical-wire strings and fingerprints
are provisional bounded role/schema reservations, not complete wire codecs.
No v3 provider, runtime wire codec, loader admission, settlement host,
finalizer, or public compiler surface exists. The current loader rejects v3
metadata before path or image access. Ordinary native resource compilation
remains `SPX-B104`.

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

These tags describe private metadata roles, not implemented platform hosts.
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
execute_response    = 124 + 4 * maximum_event_count
frame               = 208 + 4 * resource_count
decision            = 172
action_evidence     = 188
candidate_receipt   = 264

resource_count      <= 4_096
checkpoint_count    <= 65_536
graph_work_units     = resource_count * checkpoint_count <= 1_000_000
active_frames        = 256
quarantined_frames   = 64
reserved_instance_bytes
  = 256 * (request + execute_response + frame + decision
           + action_evidence + candidate_receipt)
    + 64 * (frame + candidate_receipt)
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

## Hash DAG and symbols

Let `F(x)` mean `u64` big-endian byte length followed by `x`. Domain strings
include their displayed terminal NUL. SHA-256 inputs are concatenated exactly;
numeric transcript fields are raw little-endian `u32` values.

The descriptor schema and current provisional request, execute-response, frame,
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

Each is currently the hash of its domain plus one ASCII statement. The
descriptor statement is part of the private metadata layout. The seven runtime
role statements are provisional reservations: they intentionally omit complete
byte layouts, tag namespaces, digest transcripts, and—for the host receipt—the
exact HMAC input/authentication transcript. Their current literal bytes, with no
terminal NUL, are:

```text
SPXNABI3;u32le;header=20;sequential-no-offsets-no-trailing;target;linkage-profile;19-fingerprints;module;function;getter;execute;settle;abi-tag;obligations;15-capacities;signature;graph-len;graph
SPXNRQ03;u32le-envelope;call-contract32;invocation-u64;frame-generation-u64;provider-challenge32;argument-count;ordered-indexed-arguments;scalar-or-owned-u64-payload
SPXNEX03;u32le-envelope;call-contract32;invocation-u64;frame-generation-u64;provider-challenge32;checkpoint;outcome;result-payload;event-count;event-ordinals
SPXNFR03;u32le-envelope;call-contract32;recovery-contract32;settlement-graph32;invocation-u64;frame-generation-u64;provider-challenge32;checkpoint;phase;resource-count;resource-states;pre-candidate-digest32
SPXNDC03;u32le-envelope;call-contract32;recovery-contract32;settlement-graph32;invocation-u64;frame-generation-u64;provider-challenge32;decision-tag;decision-detail
SPXNAC03;u32le-envelope;call-contract32;recovery-contract32;settlement-graph32;invocation-u64;frame-generation-u64;provider-challenge32;action-index;action-tag;owner-ordinal;before-state;after-state;checkpoint
SPXNCR03;u32le-envelope;call-contract32;recovery-contract32;settlement-graph32;invocation-u64;frame-generation-u64;provider-challenge32;pre-candidate-frame-digest32;decision-digest32;action-evidence-digest32;candidate-outcome
SPXHRP03;u32le-envelope;host-only-HMAC-SHA256;exact-instance-capability;call-contract;invocation;frame-generation;provider-challenge;candidate-digest;ledger-before;ledger-after;decision;action-evidence-digest;publication-result;atomic-ledger-and-receipt-visibility
extern-C;getter=const-u8-ptr(void);execute=u32(const-u8-ptr,u32,u8-ptr,u32);settle=u32(u8-ptr,u32,const-u8-ptr,u32,u8-ptr,u32);windows-cdecl;synchronous;same-thread;no-unwind;no-longjmp;no-callbacks;no-retained-pointers;no-reentrancy
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

The call-contract fingerprint uses
`semaprax.native-callable-contract.v3\0`. It length-frames, in order, target,
the first 18 descriptor fingerprints (everything before the call contract),
module identity, and function identity; appends raw linkage, ABI, obligations,
and all 15 capacity words; then appends the canonical signature transcript.
The call contract does not hash symbols or the descriptor containing itself.
This makes the dependency graph acyclic.

Before provider or runtime admission, each of the seven provisional role
statements MUST be replaced by and frozen against a complete normative codec,
including every byte, tag, digest, authentication input, bound, and failure
rule. Independent encoder/parser tests and known answers MUST cover those
codecs. Because the private call contract binds these fingerprints, that
replacement may intentionally change private v3 fingerprints, symbols, and
descriptor known answers. No compatibility promise attaches to the current
private KATs.

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

## Provisional runtime role/schema reservations

This milestone reserves bounded roles and current fingerprint inputs, not
complete runtime schemas, encoders, or provider behavior. The six
provider-visible future roles are:

| Future role | Reserved string | Descriptor capacity |
| --- | --- | --- |
| Execute request | `SPXNRQ03` | request bytes |
| Execute response | `SPXNEX03` | execute-response bytes |
| Recovery frame | `SPXNFR03` | frame bytes |
| Settlement decision | `SPXNDC03` | decision bytes |
| Action evidence | `SPXNAC03` | action-evidence bytes |
| Candidate receipt | `SPXNCR03` | candidate-receipt bytes |

The provider must never emit a committed receipt. `SPXHRP03` provisionally
identifies a future host-only committed-receipt role created only after
independent candidate parsing, exact-instance/frame-generation replay, and host
authentication. It is not provider output, is not covered by a provider buffer
capacity, and is the only role eligible to accompany public ledger
`ReceiptCommit`. None of these seven runtime codecs is complete, frozen, or
implemented by this metadata milestone.

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
     -> preallocated request, response, frame, decision, action, candidate
        -> locked decision and provider evidence
           -> independently authenticated host committed receipt
              -> one ledger outcome

quarantine retains the module/static registration, frame, buffers,
decision/evidence, owners/results, callbacks, and finalizer pins
```

Dynamic images become only unload-eligible after draining and release of every
frame, owner, result, credential, callback, and finalizer pin. The iOS static
profile resolves an admitted registration table at link/bootstrap time and has
no `dlopen`/unload claim, but it must preserve the same logical instance,
generation, settlement, draining, and quarantine rules.

## Threat boundaries and nonclaims

Descriptor equality and hash validation do not authenticate code provenance,
make malicious native code memory-safe, observe omitted side effects, recover a
process crash, or make an interrupted non-idempotent finalizer retryable. This
contract does not implement provider symbols, physical wire codecs, loader or
static-registration admission, callbacks, async work, concurrency, fork/hot
reload, imported finalizers, cross-target emission, mobile execution, public
adoption, or ecosystem FFI. The emitter is bound to its own build target; there
is no Android/iOS/Windows cross-emission evidence. The existing dynamic loader
rejects `SPXNABI3` before canonicalization,
image load, or symbol lookup; no v3 loader constructor exists.

## Mandatory gates

Before any runtime or public claim, all of these must pass together:

- deterministic compiler bytes and fixed fingerprints plus an independently
  implemented host parser and canonical re-encoder;
- complete replacement and freeze of all seven provisional runtime role
  statements as independently encoded/parsed byte, tag, digest, and host-HMAC
  transcripts, accepting deliberate changes to private v3 known answers;
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
