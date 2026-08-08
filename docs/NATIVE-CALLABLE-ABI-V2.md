# Native callable ABI v2

Status: connected private implementation. Feature-gated compiler emission
produces the complete guarded C11 provider, canonical descriptor v2, semantic
dictionary, and trace-path certificate. The unpublished native host
independently authenticates them, eagerly loads one exact byte-wire callable,
and connects strict request/response codecs to its authority and ownership
ledger. This is not a public or stable ecosystem ABI, and it does not open
`SPX-B104`.

## Scope

Callable ABI v2 binds one direct, monomorphic, `drop trivial` owned-resource
function from [Owned resource vertical slice v1](OWNED-RESOURCE-VERTICAL-V1.md).
The admitted signature contains `i64`, `bool`, and exact owned-resource
inputs, and returns either `i64` or one exact owned input. Unsupported
resources, imported finalizers, calls, aggregates, loops, callbacks, async work,
and allocation remain outside this ABI.

Descriptor v1 is permanently descriptor-only. A v1 blob cannot become callable
by placing a function beside it. Callable admission requires the distinct
`SPXNABI2` envelope and must reject v1 before loading.

## Canonical primitives

All integers are little-endian. A `u32` occupies four bytes, a `u64`
occupies eight bytes, and an `i64` is its two's-complement 64-bit
representation. A fingerprint is exactly 32 raw SHA-256 bytes, never hexadecimal
text. Text is a `u32` byte length followed by non-empty, NUL-free, well-formed
UTF-8. No C struct layout, padding, pointer, `size_t`, or native `bool` is
part of a wire.

Parameter indices and owned-resource ordinals begin at zero and are dense in
signature order. Semantic event ordinals begin at one; zero is reserved.
Unknown tags, noncanonical booleans, duplicate parameter value identities,
overflow, truncation, trailing data, and inconsistent counts or lengths fail
closed.

The descriptor is limited to 64 KiB. Request/response capacities and canonical
dictionary bytes are each nonzero and at most 1 MiB. The strict decoder
additionally limits parameters to 4,096 and event and dictionary-entry counts
to 65,536.

## Descriptor v2

The canonical pointer-free descriptor is encoded in this exact order:

| Field | Encoding |
| --- | --- |
| Magic | eight bytes `SPXNABI2` |
| Version | `u32 = 2` |
| Header size | `u32 = 20` |
| Total size | `u32`, exactly the complete descriptor length |
| Physical target | framed text |
| Schema fingerprint | 32 bytes |
| Target fingerprint | 32 bytes |
| Semantic-module fingerprint | 32 bytes |
| Physical-module fingerprint | 32 bytes |
| Function-template fingerprint | 32 bytes |
| Execution/cleanup fingerprint | 32 bytes |
| Event-dictionary fingerprint | 32 bytes |
| Trace-path-certificate fingerprint | 32 bytes |
| Request-schema fingerprint | 32 bytes |
| Response-schema fingerprint | 32 bytes |
| Call-ABI fingerprint | 32 bytes |
| Call-contract fingerprint | 32 bytes |
| Module identity | framed text |
| Function identity | framed text |
| Descriptor-getter symbol | framed text |
| Callable symbol | framed text |
| Call-ABI tag | `u32 = 1` |
| Required obligations | `u32 = 0x0000000f` |
| Maximum request bytes | `u32` |
| Maximum response bytes | `u32` |
| Maximum event count | `u32` |
| Dictionary byte length | `u32` |
| Dictionary entry count | `u32` |
| Parameter count | `u32` |
| Parameters | count entries in signature order |
| Result | one result entry |

The twelve fingerprints are ordered exactly as shown. All must be nonzero. The
host recomputes the schema, target, physical-module, request-schema,
response-schema, call-ABI, and call-contract fingerprints. The
semantic-module, function-template, execution/cleanup, event-dictionary, and
trace-path-certificate fingerprints are authenticated compiler inputs; all are
bound into the recomputed call contract and derived symbols.

The physical target binds architecture, operating system, environment, object
format, pointer width, endianness, and callable convention. Generated C carries
matching compile-time guards and fails closed when the C preprocessor cannot
prove those properties. Windows uses explicit C `__cdecl`; the MSVC endian path
does not depend on GNU byte-order builtins. Other admitted targets use the
platform C convention.

The required-obligations word is one indivisible v2 profile. Its only canonical
value is `0x0f`; it is not a feature-negotiation mask. The call-ABI
fingerprint binds that value and the exact C signature, Windows convention,
one-shot behavior, and prohibitions on unwind, `longjmp`, retained pointers,
and callbacks.

### Parameter and result entries

Parameter tag `1` is scalar. Its entry is:

1. `u32 tag = 1`;
2. `u32 parameter_index`;
3. value identity as framed text; and
4. `u32 scalar_kind`, where `1 = i64` and `2 = bool`.

Parameter tag `2` is owned resource. Its entry is:

1. `u32 tag = 2`;
2. `u32 parameter_index`;
3. value identity as framed text;
4. `u32 owner_ordinal`;
5. resource identity as framed text;
6. lifecycle identity as framed text; and
7. `u32 payload_wire_kind = 1`, the opaque `u64` payload wire.

Result tag `1` is scalar `i64` and has no additional descriptor fields.
Result tag `2` is an owned input and is followed by `u32 parameter_index`,
the exact parameter value identity as framed text, and `u32 owner_ordinal`.
Those three fields must select the same preceding owned parameter.

### Capacities and dictionary binding

Maximum request bytes is derived exactly as:

```text
64
+ 16 for each i64 parameter
+ 12 for each bool parameter
+ 20 for each owned parameter
```

Maximum response bytes is:

```text
68
+ max(success payload bytes, failure payload bytes)
+ 4 * maximum event count
```

The success payload is 12 bytes for scalar result (`u32 result tag + i64`) or
8 bytes for owned result (`u32 result tag + u32 owner ordinal`). The failure
payload is one `u32` selected-failure semantic ordinal.

The semantic dictionary is not embedded in the descriptor, request, or
response. The descriptor carries only its fingerprint, canonical byte length,
entry count, and maximum emitted event count. The authenticated dictionary is
the separately built `semaprax.semantic-event-dictionary.v1` canonical JSON
projection `{schema,function,entries:[{ordinal,event}]}`. It assigns explicit,
deterministic dense nonzero ordinals in first-occurrence order and contains
semantic identities only—never payloads, addresses, handles, ledger slots,
generations, credentials, loader paths, or target identities.

The dictionary is only a vocabulary. The compiler separately compiles every
valid path of the replay-validated cleanup CFG into
`semaprax.trace-path-certificate.v1`, a canonical trie-DFA whose accepting
states bind the exact ordinal sequence and terminal outcome. Descriptor v2
binds its fingerprint independently. Admission verifies schema, function,
dictionary fingerprint, certificate fingerprint, state count, and maximum path
length. Response validation walks this DFA without allocation before event
materialization, so omitted finalizers, duplicate pairs, reordered transfers,
selection after cleanup, wrong publication, and incomplete paths fail closed.

Generated code must emit ordinals from its actual executed cleanup control flow.
The host materializes those ordinals through the exact fingerprint-bound
dictionary. It may not infer missing events, reorder or repair them, or use a
shadow cleanup interpreter. Zero, an unknown ordinal, too many ordinals, or a
dictionary-size/count mismatch fails closed after execution.

## Exported C surface

The admitted image has exactly one immutable descriptor getter and one callable:

```c
const uint8_t *SPX_CALL spx_descriptor_symbol(void);

uint32_t SPX_CALL spx_callable_symbol(
    const uint8_t *request,
    uint32_t request_len,
    uint8_t *response,
    uint32_t response_capacity
);
```

`SPX_CALL` is `__cdecl` on Windows and the ordinary C calling convention
elsewhere. The symbols are distinct, non-empty valid C identifiers, at most
1,024 bytes, and deterministically derived from the physical-module,
function-template, execution/cleanup, event-dictionary,
trace-path-certificate, request-schema, response-schema, call-ABI, and call-contract
fingerprints.

The request and response ranges are non-null, complete, disjoint, and valid only
for one synchronous call. The callable reads only the request range, writes only
the response range, retains neither pointer, performs no callback or delayed
lookup, and does not unwind, `longjmp`, trap, terminate, or start asynchronous
work. The loader exposes no generic symbol lookup, raw handle, raw pointer, or
manual close.

The callable's `u32` return is a physical adapter result, not a SEMAPRAX
normalized status. Its private, call-ABI-fingerprinted namespace is exact:

- `0`: one complete canonical response;
- `1`: invalid request;
- `2`: incorrect or insufficient response capacity; and
- `3`: internal provider failure.

Every other value is reserved and invalid. The response is decoded only after
physical result `0`. Any non-completed result, malformed response, or provider
contract violation occurs after ownership commit and is an executed adapter
failure, never a call rejection.

## Request wire v1

The request has a 64-byte fixed prefix and then signature-ordered arguments:

| Offset | Field | Encoding |
| ---: | --- | --- |
| 0 | Magic | eight bytes `SPXNREQ1` |
| 8 | Version | `u32 = 1` |
| 12 | Header size | `u32 = 20` |
| 16 | Total size | `u32`, exactly `request_len` |
| 20 | Call-contract fingerprint | 32 bytes |
| 52 | Invocation ID | nonzero `u64` |
| 60 | Argument count | `u32`, exactly the descriptor parameter count |
| 64 | Arguments | ordered entries below |

There is no module-instance nonce in the wire. A prepared call is bound to its
exact loader instance by private host/lease state, not by unauthenticated
provider bytes.

Every argument begins with its descriptor parameter tag and `u32
parameter_index`:

- scalar `i64`: `u32 tag = 1`, index, then `i64` (16 bytes total);
- scalar `bool`: `u32 tag = 1`, index, then canonical `u32 0` or `u32 1`
  (12 bytes total);
- owned: `u32 tag = 2`, index, `u32 owner_ordinal`, then opaque `u64`
  payload (20 bytes total).

The descriptor supplies the scalar kind, so the request scalar tag remains
`1` for both scalar encodings. Owner credentials, authority secrets, slots,
generations, and pointers never cross the wire. An opaque owned payload may be
zero or `u64::MAX`; its bits carry no liveness meaning.

## Response wire v1

The response has a 68-byte fixed prefix, followed by one outcome payload and
then the semantic event ordinals:

| Offset | Field | Encoding |
| ---: | --- | --- |
| 0 | Magic | eight bytes `SPXNRSP1` |
| 8 | Version | `u32 = 1` |
| 12 | Header size | `u32 = 20` |
| 16 | Total size | `u32`, no greater than the descriptor capacity |
| 20 | Call-contract fingerprint | 32 bytes |
| 52 | Invocation ID | exact request value as `u64` |
| 60 | Outcome | `u32` success-or-failure discriminant |
| 64 | Event count | `u32`, no greater than the descriptor maximum |
| 68 | Outcome payload | result payload or selected-failure ordinal |
| dynamic | Semantic events | event-count nonzero `u32` ordinals |

A successful outcome payload starts with the descriptor result tag. Scalar
success is `u32 result_tag = 1` followed by `i64`; owned success is `u32
result_tag = 2` followed by the exact published input's `u32 owner_ordinal`.
A failure outcome payload is the one nonzero `u32` semantic ordinal that
identifies the selected sticky failure. The private, response-schema-
fingerprinted outcome namespace is `1 = success` and `2 = semantic failure`;
zero and every other value are invalid. Success and failure must be decoded
only by that exact codec.

The event ordinal vector immediately follows the outcome payload. Its order is
the executed semantic order, and each value must exist in the bound dictionary.
On success, the sequence and result payload must agree with the exact
result-commit event. On failure, the selected-failure ordinal, emitted
select-failure event, absence of publication, and cleanup sequence must agree.
The declared total length must equal the fixed prefix plus the selected payload
and `4 * event_count`; trailing bytes inside the preallocated capacity are not
part of the response.

## Admission and ownership order

The following order is security-significant:

1. The private, feature-gated callable-admission derivation validates HIR and
   the attached cleanup plan, admits the complete resource shape, builds the
   deterministic event dictionary and trace-path certificate, derives all
   twelve fingerprints and both symbols, serializes descriptor v2, and
   independently round-trips the canonical fields. Ordinary compiler preflight
   still derives and discards descriptor v1 only. Failure emits no v2 artifact.
2. Before loading, the host bounds and strictly parses the expected descriptor,
   checks the current target, recomputable fingerprints, exact `0x0f`
   obligations, capacities, signature/result mappings, symbol derivation, and
   absence of trailing bytes. A v1 descriptor is rejected here.
3. The unsafe loader accepts only an already canonical absolute path and the two
   bounded distinct symbols. It opens the trusted image with eager local
   resolution (`RTLD_NOW | RTLD_LOCAL` on Unix), resolves and calls the getter,
   compares exactly the expected descriptor bytes, eagerly resolves the one
   callable, and only then allocates its process-local instance identity.
4. Before a call, the host checks its thread and draining state, exact lease,
   scalar count and kinds, every owner credential, request/response/event
   capacities, invocation exhaustion, and all serialization bounds. It
   allocates the complete request and response, response-decode/event buffers,
   normalized failure values, owned-result credentials, and the detached ledger
   plan before changing ownership.
5. One ledger transaction consumes every owner in parameter order or none. Safe
   wrappers become consumed only at this atomic ingress commit. Any rejection
   before it leaves the exact wrappers live and reusable; no rejection is
   possible after it.
6. The host invokes the one-shot prepared call once. Only physical completion
   permits strict response decoding; the certificate accepts the complete
   ordinal path and terminal outcome before semantic materialization. Success
   publishes only the admitted result, while authenticated semantic failure
   consumes every committed input and publishes nothing. A physical failure or
   malformed response becomes an adapter failure and retires the logical ledger
   state, but the general canonical fallback cleanup/finalizer trace and
   physical quiescence guarantee remain an explicit `SPX-B104` blocker.

Every allocation, capacity check, generation advance, invocation reservation,
wire construction, and result reservation belongs before commit. A provider
failure or malformed response after commit is normalized as executed adapter
failure and cannot be relabeled as rejection. The current direct-trivial host
logically abandons the committed ledger transaction, but does not claim that
this is a general semantic cleanup path for future physical finalizers.

## Trust and lifetime model

Callable admission is an explicit `unsafe` trusted-native-code boundary. Its
caller must establish the exact root image, all selected dependencies,
same-root provenance and exact ABI of both symbols, immutable getter storage,
stable module directory and dependency namespace, and every no-escape callable
obligation for the full lifetime of all leases, owners, active calls, results,
callbacks, and finalizers.

Descriptor equality proves none of those claims. A canonical path is diagnostic
metadata, not file identity. This is not a sandbox, code-signing protocol,
malicious-plugin boundary, or proof that arbitrary native code is memory safe.
Callable leases remain non-`Send`, non-`Sync`, non-cloneable, and
non-formattable. The image becomes only eligible for release after every pin and
active operation is quiescent; immediate physical unmapping is not promised.

## Quality evidence and `SPX-B104`

The current implementation proves deterministic provider, descriptor,
dictionary, and certificate derivation; compile-time target guards; independent
strict parsing and mutation rejection; allocation-free postcommit decoding and
certificate walking; exact loader-instance admission; atomic ledger
integration; safe scalar/owned calls; status and owner-result reconciliation;
draining; and cross-instance rejection. Real generated shared libraries execute
all 14 authoritative cases through the host at O0/O2 and match the reference
trace, outcome, publication, owner rotation, and final logical liveness exactly.

`SPX-B104` remains closed until all of these are green together:

- a general, traceable fallback cleanup/finalizer and quiescence protocol for
  physical provider failure or malformed response after ledger commit;
- a green public run of the configured Linux job that loads ASan/UBSan-
  instrumented generated providers through the loader, authority, ledger, and
  callable host, plus sanitizer instrumentation of the Rust host itself (the
  configured linker flags make the provider runtimes available but do not
  instrument Rust code);
- a green public Windows CI run of the generated callable corpus plus the
  dependency-collision fixture and hardened search assertions;
- Android device/runtime admission and an iOS-compatible static-link profile,
  with representative device or simulator evidence;
- the ordinary public compiler build/preflight path emitting and admitting this
  exact slice while every excluded shape preserves its stable diagnostic; and
- the complete all-feature MSRV, formatting, strict Clippy, tests, docs,
  package, cargo-deny, examples, sanitizer, and platform matrix in public CI.

Only that joint evidence may replace `SPX-B104` for this exact slice. It does
not open excluded resource shapes or imply completion of records, variants,
generics, imported finalizers, borrowing across FFI, callbacks, async,
concurrency, fork recovery, hot reload, signed admission, application
frameworks, or the broader SEMAPRAX goal.

## Platform nonclaims

Unix eager local relocation is implemented in the private loader. Windows uses
`LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS`, excluding
legacy current-directory/PATH lookup, but a real callable ownership run and a
malicious dependency-collision fixture are not yet evidence. Android is a
compile target, not device execution evidence. iOS dynamic loading is not
claimed and may require a later static-link admission profile. There is no
present claim of Android/iOS device execution, cross-thread calls, concurrency,
callback/finalizer quiescence, fork recovery, hot reload, signed-code admission,
independent same-root symbol provenance authentication, or public ABI stability.
