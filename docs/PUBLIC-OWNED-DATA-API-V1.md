# Public Owned Data API v1

Status: implementation and executable evidence authored; local and exact-head
hosted promotion are not claimed.

Audience: language users, generated-SDK consumers, tool authors, and compiler
contributors.

Public Owned Data API v1 defines one additive Project profile for calling a
closed set of stable-ID functions from JavaScript/TypeScript and safe Rust. It
extends the existing fixed-memory byte-data mechanism with controlled owned
byte results. Project Manifest v8, both generated consumer routes, the
reference-interpreter lane, and their focused evidence are authored in the
current source tree. This document does not claim those gates were executed at
the current head, that either package is published, or that hosted promotion is
complete.

The profile deliberately copies every successful owned result into host-owned
storage before publication. It does not expose a SEMAPRAX pointer, allocator,
arena token, or provider handle to application code.

## Fixed protocol identifiers

The following identifiers are exact and are not aliases:

| Layer | Identifier |
| --- | --- |
| Project schema | `semaprax.project.v8` |
| Project profile | `owned-data-api.v1` |
| Canonical API descriptor | `semaprax.public-owned-data-api.v1` |
| npm build carrier | `semaprax.project-npm-build.v7` |
| npm API metadata | `semaprax.owned-data-api.v1` |
| Rust SDK manifest | `semaprax.native-rust-owned-data-sdk.v1` |

Each identifier selects only the contract in this document. An earlier
Project schema cannot select `owned-data-api.v1`, and Project v8 cannot select
an earlier profile. Consumers must reject unknown identifiers rather than
fall back to a scalar, useful-data, command, or legacy SDK route.

## Canonical Project manifest

Project v8 has exactly eight assignments in this order and one terminal LF:

```toml
schema = "semaprax.project.v8"
name = "frame-payload"
version = "0.1.0"
profile = "owned-data-api.v1"
entry = "frame_payload.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["frame.payload", "frame.payload-maybe", "frame.payload-result"]
tests = ["frame_payload.tests"]
```

The existing canonical name, Semantic Versioning, module, source-path,
stable-ID, and ordering rules apply unchanged. `sources` contains 2–16
strictly sorted unique paths. `web_exports` contains 1–32 strictly sorted
unique stable IDs. There are no `command`, `input`, or `capabilities` fields.
Unknown, missing, extra, duplicated, or reordered assignments reject.

Project v8 enters the existing closed `ProjectProfile` dispatch as one distinct
variant. Schema or profile text is never converted into loose feature flags
from which downstream authority is inferred.

## Semantic admission

Admission is derived from validated, linked HIR. Source-shaped declarations,
generated metadata, target artifacts, or caller assertions are not semantic
authority.

Every selected export must satisfy all of the following:

- it is a source-authored function with an explicit persistent `@id` equal to
  the selected `web_exports` identity;
- it is monomorphic and is not the Project entry function;
- it has 0–8 parameters, each with one exact admitted parameter type;
- it has exactly one result with one exact admitted result type;
- the function and every function in its transitive closure are effect-free,
  import-free, and contract-free;
- the complete selected closure is acyclic; direct and mutual recursion both
  reject;
- the linked executable function inventory is nonempty and contains at most
  the existing public-export bound of 256 functions; and
- the closure passes the ordinary verifier, ownership checks, cleanup-plan
  construction, and independent cleanup-plan replay before descriptor or
  target generation.

All selected roots, including roots disconnected from the Project entry, are
linked under one authenticated held Project snapshot. The union of their
closures is checked once; a function reachable from more than one root is not
duplicated or charged twice.

### Admitted types

The parameter vocabulary is exactly:

- `i64`
- `bool`
- `borrow str`
- `borrow Slice<u8>`

The result vocabulary is exactly:

- `i64`
- `bool`
- `usize`
- `Bytes`
- `Option<Bytes>`
- `Result<Bytes, i64>`

`Option` and `Result` here are the authenticated compiler-owned variants.
They are admitted only in the exact unnested forms above. `Result::Err` is a
successful API invocation carrying a language value; it is not an adapter or
host-call failure.

Borrowed arguments are invocation-bounded snapshots. Their cumulative encoded
or byte length across one external call is at most 65,536 bytes. An owned byte
result has an exact length in `0..=65_536`; capacity-plus-one rejects before a
host value becomes observable.

### Exact exclusions

The public boundary rejects all of the following, even when the shape exists
inside an otherwise valid SEMAPRAX program:

- owned parameters;
- resource parameters or results;
- fixed arrays or any other arrays as public values;
- borrowed results;
- authored records;
- authored variants;
- nested `Option` or `Result` values;
- `Result` error types other than exactly `i64`;
- callbacks or callable imports;
- async functions or async values;
- shared values;
- public mutation or mutable borrowed arguments;
- multiple returned values or unit results;
- host allocator adoption or zero-copy allocator transfer; and
- direct raw pointer, arena-token, provider-handle, or context exposure.

The profile also grants no filesystem, process, environment, clock, random,
network, thread, UI, registry, signing, or publication authority.

## Canonical public API descriptor

The compiler derives one canonical descriptor from the authenticated Project
revisions, Project graph digest, validated HIR, selected stable IDs, and fixed
limits. Its schema is `semaprax.public-owned-data-api.v1`. It contains, in
strict stable-ID order:

- the Project schema, Project revision, Workspace revision, and Project graph
  digest;
- every export stable ID;
- every parameter's stable identity, source display name, ordinal, and one
  closed parameter-type tag;
- one closed result-type tag for each export; and
- the exact export, parameter, closure-function, borrowed-input, and
  owned-output limits.

The descriptor has deterministic canonical bytes and a domain-separated
digest. Parsing is closed and bounded. Independent replay must reconstruct the
same facts from the same retained Project subject and reject truncation,
insertion, deletion, reordering, duplication, unknown fields or tags, changed
revisions, changed graph digest, or any byte mutation.

The lower native package applies its existing nonempty, 1 MiB, terminal-LF
and NUL-free byte guard before JSON parsing, including the preliminary schema
lookup that selects the v8/v10 digest domain. Digest discovery and full replay
share that guard; reaching the exact byte limit does not establish canonical
or semantic validity. Provider-validation precedence and accepted descriptor
bytes remain unchanged. The focused regressions in
`crates/semaprax-native-rust-owned-data-package/src/descriptor_input_tests.rs`
cover exact/plus-one input framing, canonical replay, domain separation and
early rejection through the public builder. They are authored but unrun.

The shared native provider-binding check also compares borrowed text rather
than formatting a copy of the caller-supplied digest. It retains the exact
single-binding-line predicate, including rejection of duplicate or malformed
lines, without imposing a new digest grammar or changing error precedence.
The same unrun regression module compares the old and new predicates on
hostile line/digest combinations and an oversized digest mismatch.

The lower native package reader enforces parameter-identity uniqueness within
each export, matching the compiler's descriptor parser. Correctly recomputing
the digest does not authorize two parameter ordinals to share an identity.
This shared v8/v10 rejection leaves canonical compiler-produced bytes and
identifier grammars unchanged. It is a structural replay check, not independent
proof of provider semantics. Authored, unrun regressions live in
`tests/public_api_descriptor_v1/parameter_identity.rs` and
`crates/semaprax-native-rust-owned-data-package/src/descriptor/tests.rs`:
the root cases retain real HIR and exact canonical framing while reminting
duplicate-ID mutations; lower cases cover both schemas and valid parameter
boundaries as well as duplicates. Structural uniqueness is per export, not a
new cross-export identity rule.

This descriptor is the sole semantic API source for JavaScript bindings,
TypeScript declarations, npm metadata, native provider descriptors, and the
safe Rust SDK. A target may add authenticated target-layout facts, but it may
not rediscover or reinterpret a source signature. If two targets require
different semantic descriptors, the profile must remain inactive.

Display names are presentation only. Generated APIs are keyed by, or derive
their collision-checked host identifier from, the persistent stable ID. A
source display rename that preserves the stable ID must preserve descriptor
meaning and the external API.

## Host type mappings

| SEMAPRAX | TypeScript | Rust |
| --- | --- | --- |
| `i64` | `bigint` | `i64` |
| `bool` | `boolean` | `bool` |
| `usize` | `bigint` | `u64` |
| `borrow str` | `string` | `&str` |
| `borrow Slice<u8>` | `Uint8Array` | `&[u8]` |
| `Bytes` | `Uint8Array` | `Vec<u8>` |
| `Option<Bytes>` | `Uint8Array \| null` | `Option<Vec<u8>>` |
| `Result<Bytes, i64>` | `SemapraxResult<Uint8Array, bigint>` | `Result<Vec<u8>, i64>` |

TypeScript uses this exact result definition:

```ts
export type SemapraxResult<T, E> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly error: E };
```

JavaScript accepts only ordinary, attached, fixed-length `Uint8Array` byte
inputs. It rejects shared, resizable, detached, differently typed, `DataView`,
and implicitly coercible inputs. Strings are encoded as checked UTF-8. Every
borrowed argument is snapshotted before public scratch is mutated, and the
complete call is synchronous and non-reentrant.

For safe Rust, adapter, semantic, or host-call failure is an outer SDK error.
The table's `Result<Vec<u8>, i64>` is the successful SEMAPRAX language value
inside that outer result. Generated safe API code forbids unsafe code and
exposes no raw provider handle or context.

## Result representation and lifetime

The target-neutral variant discriminants are fixed:

```text
Option: None = 0, Some = 1
Result: Ok = 0, Err = 1
```

Any other tag is an invariant failure before payload access. `None` and `Err`
own no byte carrier. `Some` and `Ok` own exactly one active byte carrier.
Inactive payload storage is never read, copied, or dropped. A tag/liveness
disagreement fails before publication.

Before a generated API call returns to JavaScript or safe Rust, it must
complete this sequence in order:

1. authenticate the SEMAPRAX owned result and its active-case liveness;
2. query and check its exact length against the authenticated carrier and the
   65,536-byte output bound;
3. allocate exact host-owned storage and copy exactly that many bytes;
4. consume or drop the SEMAPRAX carrier exactly once;
5. prove the invocation's provider or arena is settled, with no live result,
   provisional result, or cleanup obligation remaining; and
6. only then publish the host-owned `Uint8Array`, `Vec<u8>`, `null`, or error
   branch to application code.

The host allocation is never adopted by SEMAPRAX, and SEMAPRAX allocation is
never adopted by the host. Empty owned bytes still carry real ownership until
settled. Failure before result publication preserves the caller-visible result
slot and exposes no partial bytes. Failure after staging but before
publication settles the staged owner exactly once. If exact settlement cannot
be proven, the invocation fails closed and publishes no language value.

The Wasm wrapper never exposes its raw carrier. It clears or poisons temporary
result storage after settlement and requires an empty arena before success.
The native provider uses an opaque, provider-owned handle only inside the
private FFI sibling; safe Rust queries length, copies, and settles through an
internal owner guard. A separate invocation guard then proves the entire
context settled through the existing context-close operation before returning
any value or recoverable error, including scalars, `None`, and language `Err`.
On Rust unwind the owner guard runs before the invocation guard. Uncertain
owner or context settlement retains the existing fail-stop policy; no retry or
later provider effect is permitted. Unknown tags or forged inactive handles
never authorize length, copy, or drop operations. A panic may not cross the
FFI boundary. No allocator pointer is converted directly into a `Vec<u8>`.

The authored v8/v9/v10 Rust correction reinitializes a proven-closed context
only on the next invocation. This resets its private invocation counter, not
the linked provider's nonreused handle issuer. No live result or provider
obligation may cross that reset. A rejected initialization preserves its
existing error and permits no further provider operation in that invocation
or its cleanup; a later explicit invocation may attempt initialization again.
Only a successfully initialized invocation owns a context-close obligation.
Public Rust signatures, provider C and ABI,
descriptors, and manifest schemas stay unchanged; generated safe/private Rust and
its integrity bindings intentionally change. Hostile-provider protocol tests
are authored but unrun; this is not a safety claim against arbitrary malicious
native machine code.

The subsequent [native internal String correction](NATIVE-OWNED-DATA-STRING-SETTLEMENT-V1.md)
settles String allocations that the standalone descriptor already admits inside
function bodies. Provider context close cannot observe those inline allocations
and is not their cleanup proof. V8/v9 provider emission now shares ordinary
length-header helpers and a per-function String ledger. Full translation units
with Strings, including unselected functions, intentionally change native C and
dependent artifact bindings; String-free output remains exact. Public types,
descriptors, schemas, v10 output, and activated Project admission are unchanged.
Physical allocator and external-consumer evidence is authored but unrun.

The authored native correction uses a 13-bit one-based slot (`1..=4096`)
and a nonreused 51-bit issuance serial within one statically linked provider
runtime. One private atomic strong compare-exchange reserves a serial without
retry. Contention or permanent exhaustion rejects before owner or output
publication; the caller settles its unpublished result. The issuer survives
context disposal and storage reinitialization. It does not establish uniqueness
across separate provider images or reloads, pointer secrecy, shared-context
thread safety, or guaranteed successful progress under contention. The same
private runtime correction applies to v9/v10; generated C/object/archive bytes
and integrity bindings intentionally change, not public signatures or schemas.

The authored v8/v9/v10 JavaScript correction admits the entire argument tuple,
including exact bounded UTF-8 lengths, before payload snapshot allocation.
Captured intrinsic view checks reject detached/shared/resizable and wrong-brand
inputs without invoking caller constructor/species hooks. All snapshots still
precede Wasm scratch writes and arena entry. Module-byte inputs have their
separate 16 MiB admission bound before copy/hash. Explicit v9/v10 routing now
reuses the byte-identical v8 input helper; v8 JavaScript remains unchanged by
that extension. V9/v10 JavaScript and dependent artifact bindings intentionally
change, not descriptors, Wasm, or TypeScript declarations. Their record-field
authentication, scalar validation, and consume-before-UTF-8-decoding rules
remain in force. Real-package hostile-input regressions are authored but
unrun, not promotion evidence; their owner is
[`tests/project_owned_input_admission_v1.rs`](../tests/project_owned_input_admission_v1.rs),
alongside the preserved v8 renderer known answers.

The subsequent [owned npm invocation correction](OWNED-NPM-INVOCATION-V1.md)
reserves busy state before preflight and distinguishes reusable input rejection
from post-entry uncertainty. Only a locally authenticated checked-status error
may recover after settlement. Caught reentry, malformed output and unexpected
host exceptions poison the instance even with an empty arena; the first thrown
value is preserved without truthiness checks. This correction intentionally
changes v8/v9/v10 JavaScript and its dependent integrity bindings, including
the bounded-renderer known answers. Historical unselected renderers, Wasm,
descriptors, declarations and v1-v7 artifacts remain unchanged. Its separate
real-package failure matrix is authored but unrun.

## Generated artifacts and carriers

The npm output inventory is exactly:

1. `app.wasm`
2. `semaprax.js`
3. `semaprax.bindings.js`
4. `semaprax.bindings.d.ts`
5. `semaprax.api.json`
6. `package.json`

`semaprax.api.json` uses `semaprax.owned-data-api.v1` and binds the canonical
API descriptor, Wasm digest, fixed limits, exact target call shapes, ordered
artifact inventory, and owned-result settlement policy. The surrounding
context-bound npm build carrier uses `semaprax.project-npm-build.v7` and binds
the retained Project subject plus the exact ordered artifacts, bytes, digests,
and payload digest. Inspection or replay proves consistency only; neither the
metadata nor carrier grants build or publication authority.

The owned-data Rust package uses manifest
`semaprax.native-rust-owned-data-sdk.v1`. It binds the same canonical API
descriptor and exact generated package/provider inventories. It is distinct
from, and does not reinterpret, the existing scalar Rust SDK manifest. The
safe package must build locked and offline without repository source or a
workspace dependency. Unsafe FFI remains quarantined outside the root
`semaprax` crate and outside the generated safe API.

The lower builder shares the scalar SDK's
[compiled-ABI admission](NATIVE-RUST-INTEROP-V1.md): five supported
GNU/Linux, Apple/macOS, and x86-64 MSVC/Windows target identities, without
substituting another libc or ABI. Unsupported targets retain the existing
tool error before staging; v8/v9/v10 descriptors and supported-target package
bytes are unchanged by this selection rule.

The raw Wasm owned-result call shape is profile-specific:

```text
(parameters..., result_out: i32) -> status: i32
```

The adapter validates alignment and the complete result-out range before
semantic execution. It writes one authenticated carrier only after semantic
success and cleanup-plan result publication. Status and out-of-band adapter
failure remain distinct from a successful language `Result::Err`.

The authored raw-Wasm correction excludes the complete reachable private
shadow-stack interval, not merely the wrapper's temporary output. The shared
validated-HIR call index and the actual lowerer's frame sizes derive the
maximum simultaneous call-path extent for selected exports only. Missing,
cyclic, overflowing, or over-capacity selected extents reject; unrelated
declarations do not change this admission. Full and partial overlap guards run
before UTF-8 imports or semantic execution and preserve the fixed borrowed and
public-result scratch reservation. This shared v8/v9/v10 correction intentionally
changes their Wasm bytes and dependent artifact bindings, without changing
descriptors, public call signatures, or v1-v7 artifacts. Its real-engine poison,
nested-helper, settlement, and re-entry regressions are authored but unrun.

## Compatibility

Project v8 and every protocol above are additive. Project v1–v7 parsing,
diagnostic selection, canonical manifest bytes, linked meaning, generated
artifacts, npm carrier schemas and bytes, Rust SDK v1 schemas and bytes, scalar
Web exports, command packages, and target known answers must remain unchanged.

The v8 implementation must be reachability- and profile-gated. When Project v8
is absent, it emits no owned-data descriptor, metadata, helper, arena/provider
operation, target branch, artifact, or runtime state. Earlier schemas reject
the new profile, and v8 rejects all earlier profile names. There is no migration
that silently rewrites an earlier manifest as v8.

Stable-ID display rename compatibility requires the same selected identity,
signature, descriptor semantics, host API identity, and generated consumer
behavior before and after rename. A signature, ownership, effect, contract,
import, or stable-ID change is not a display rename and must reject or produce
a separately reviewed compatibility change.

## Completion gates

This implementation may be described as locally evidenced only when all
applicable gates below pass at the same commit. Completion-matrix or public
promotion additionally requires the exact hosted gate in item 12. Authored but
unexecuted repository tests do not satisfy either condition.

1. **Parser and canonical manifest:** exact v8 assignment/order/LF/profile
   acceptance; every malformed, capacity, ordering, schema/profile-confusion,
   source-count, and export-count rejection; byte-for-byte v1–v7 manifest and
   diagnostic preservation.
2. **HIR admission:** success for every admitted parameter/result type and
   their mixed 0–8-parameter signatures; rejection of every exact exclusion;
   stable-ID selection, non-entry, monomorphic, effect/import/contract-free,
   acyclic closure, 256-function bound, and hostile-HIR validation.
3. **Descriptor replay:** canonical bytes and digest; zero-export rejection;
   one and 32 exports; zero, one, and 8 parameters; strict ordering and
   uniqueness; display rename; revision/graph binding; independent replay;
   byte mutation, truncation, insertion, deletion, foreign-subject,
   unknown-tag, and budget rejection; one descriptor driving every target
   generator.
4. **Interpreter/native/Wasm equivalence:** the same source corpus produces
   equal scalar, byte, `Option`, and language-`Result` values and normalized
   failures through interpreter, native O0/O2, and Core-Wasm, including empty,
   embedded-NUL, invalid-UTF-8, `0xff`, `None`, `Some`, `Ok`, and minimum/zero/
   maximum `Err(i64)` cases.
5. **JavaScript and TypeScript consumption:** compiler-free installed package
   use; exact declarations and result union; stable-ID access; string and byte
   input validation; snapshot isolation; repeated calls; digest and metadata
   authentication; ordinary fresh `Uint8Array` results; no raw carrier access;
   locked offline pack/install.
6. **Safe Rust consumption:** exact descriptor/manifest replay; generated API
   with unsafe forbidden; locked offline external consumer; exact `Vec<u8>`,
   `Option`, inner language `Result`, and outer call-error behavior; no raw
   handle/context; panic containment; O0/O2 equality.
7. **Hostile carriers:** malformed Wasm/provider/artifact digest; invalid or
   unaligned result pointer; zero, stale, foreign, or duplicate handle/token;
   wrong length; invalid tag; inactive forged payload; tag/liveness mismatch;
   wrong destination length; double consume/drop; provider copy/drop failure;
   arena/provider non-settlement; tampered re-digested metadata.
8. **Cleanup exactness:** independent cleanup-plan replay and target traces for
   success, pre-publication failure, post-staging failure, inactive cases,
   postcondition failure, copy failure, settlement failure, repeated entry,
   token/handle rotation, first-failure stickiness, exact-once active-owner
   settlement, and no partial result publication.
9. **Stable-ID display rename:** a multi-module fixture is built and consumed
   before and after a display-only rename; Project graph facts, canonical API
   identity, npm and Rust method identity, and consumer behavior remain bound
   to the unchanged stable ID, while presentation facts may change.
10. **Capacity boundaries:** minus-one, exact, and plus-one cases for 32
    exports, 8 parameters, 256 linked functions, 65,536 cumulative borrowed
    input bytes, 65,536 owned output bytes, every descriptor/carrier byte
    budget, Wasm result storage, native destination length, and handle/arena
    capacity.
11. **Compatibility preservation:** known-answer bytes for every Project v1–v7
    manifest, existing npm carrier, scalar and useful-data Web artifact,
    command package, and scalar Rust SDK remain exact; profile-absent builds
    contain no v8 runtime or metadata; all earlier mismatch diagnostics remain
    stable.
12. **Cross-platform hosted promotion:** all focused gates, hostile tests,
    external JS/TypeScript and Rust consumers, native O0/O2, browser execution,
    sanitizers, deterministic inventories, and compatibility KATs pass on one
    exact commit in blocking Linux, macOS, and Windows jobs, with every claimed
    browser/runtime and minimum-Rust-version lane named by the release gate.
    Skipped, cancelled, diagnostic-only, retried, or allowed-failure jobs do
    not count.

The current source tree contains the authored Project v8 implementation behind
its exact manifest/profile and CLI routes. That source wiring is not execution
or promotion. Until all twelve gates are executed and the applicable
exact-head jobs are green, neither generated package is a published supported
SDK and no completion-matrix status is promoted. The upstream baseline run
linked by the completion matrix predates this work and supplies no Project v8
promotion evidence.

The frame-payload product's authored binding fixtures now reopen each real
baseline/display-renamed Project revision and replay the exact descriptor from
both generated packages against that subject. They compare regenerated npm
artifacts and independently reconstruct the native manifest's canonical
inventory and descriptor/provider-source bindings from reopened bytes. This
is a test-specific manifest observation, not a newly exposed package verifier
or an independent proof of the archive's machine-code semantics.

The lower native package builder preserves the safe platform facade's
[archive settlement state](NATIVE-RUST-INTEROP-V1.md) before projecting its
existing publication error. An `Uncertain` failure stops before owned-stage
discard, later authority rechecks, or outer package publication. The inert
stage remains for reconciliation; ordinary held handles are still released.
Settled failures retain exact-inventory cleanup and sticky primary-error
precedence. This shared boundary also serves the additive v9/v10 packages.
Private regressions inject closed archive failures and exercise real held-stage
inventory preservation, including foreign bytes. They remain unrun and do not
prove physical archiver settlement or process quiescence.

The seven-file outer package has the same one-way publication boundary:
exact-inventory cleanup is available only during preparation. Starting
`settle_for_publish` consumes that cleanup authority, including when settlement,
rename, or a later path/replay check fails. Failures retain the inert or
published tree for reconciliation; moving a published tree back to its former
staging name cannot restore deletion authority. Private held-filesystem
regressions cover preparation cleanup, retained post-transition stages, a real
no-clobber collision, and Unix post-rename displacement back to the stage name.
They remain unrun and do not establish hostile same-principal isolation or
permanent pathname binding.

The same retained HIR supplies the interpreter and native O0/O2 corpus checks.
A separate raw-Wasm ABI observer uses the unchanged production arena/core
templates, observes actual mint/drop/copy-out and empty settlement, and checks
alignment rejection before imports with preserved poisoned output. It is an
independent consumer, not an independent arena implementation or a complete
internal destruction trace. All these new fixtures remain unrun.

## Nonclaims

Public Owned Data API v1 does not claim public records or authored variants,
nested algebraic data, resources, owned inputs, borrowed outputs, owned UTF-8
strings, mutable/shared values, multiple returns, general generics, lifetime
inference, zero-copy transfer, allocator interoperability, callbacks, imports,
effects, contracts, async, reentrancy, threads, shared memory, memory growth,
WIT/Component Model support, C/C++/Swift/Kotlin bindings, native executable
support for Project v8, package resolution, registry publication, signing,
provenance, or general public aggregate ABI stability.
