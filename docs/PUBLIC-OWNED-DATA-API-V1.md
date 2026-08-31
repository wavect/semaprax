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

The additional `tests/public_api_descriptor_v1/semantic_replay.rs` cases
distinguish retained-HIR binding from framing and digest rejection. Each pair
derives two authentic descriptors from separately checked source, first
replays each against its own HIR, then cross-replays both directions using
their correct digests and requires the exact retained-subject diagnostic.
V8 and v10 cover parameter types, names, order and arity, scalar and owned
result types; v10 additionally distinguishes `Bytes` from owned UTF-8. Explicit
fact assertions keep these cases from degenerating into unrelated mutations.

The fixture deliberately holds synthetic revision facts constant to isolate
signature replay. It does not claim an edited real Project retains its revision,
that a descriptor authenticates source provenance, or that equal descriptors
prove equal function behavior. Body-only and function-display-name controls
preserve descriptor bytes; parameter presentation names are included facts.
Parameter IDs derive from function identity and ordinal, so no independent
source-authored parameter-ID mutation is claimed. These added cases are
authored and unrun; production code, schemas and existing golden bytes remain
unchanged.

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

The Node evidence requires actual shared and resizable buffers plus transfer
support; missing prerequisites fail the selected gate rather than omit cases.
Buffer construction and capability checks happen outside rejection assertions.
The direct-Bytes fixture and shared v8/v9/v10 input-admission fixture require the
actual facade's `TypeError` and fixed-input diagnostic, followed by healthy
same-instance calls. An assertion that a forbidden input was accepted must not
be swallowed by a capability-detection catch. The direct/variant v8 and UTF-8
consumer tests also require Node execution instead of returning success when
Node cannot start. These evidence corrections are authored and unrun; generated
runtime bytes, schemas, and support status are unchanged.

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

Filesystem materialization uses the shared
[Unix npm publication boundary](PROJECT-MANIFEST-V2.md): final parent binding
compares held/reopened filesystem identities as well as canonical path text.
Same-path parent replacement must not turn a package retained in a displaced
directory into reported success. The added physical regressions are unrun;
no Windows routing change or atomic-publication claim follows.

The separately approved [Windows owned npm publication](WINDOWS-OWNED-NPM-PUBLICATION-V1.md)
change routes Project v8–v10 npm/Web filesystem effects through `semaprax-full`
and the existing held-handle platform authority. Standalone Windows publication
rejects before output effects; inline carriers and generated bytes are unchanged.
The private route requires an existing parent and an admitted ASCII output leaf.
Its source-drift, no-clobber and settlement evidence is authored but unrun.

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

### Generated Cargo build-script path boundary

The shared private build-script renderer for v8, v9, and v10 validates
`CARGO_MANIFEST_DIR` before printing any Cargo instruction. After the existing
target-mismatch check, it requires a present Unicode path containing neither
CR nor LF. It does not canonicalize, require existence, or reject spaces and
other ordinary Unicode characters. Accepted paths retain the exact three
archive-change, native-search, and static-link instructions in their original
order. This follows Cargo's [line-oriented build-script output contract](https://doc.rust-lang.org/cargo/reference/build-scripts.html#outputs-of-the-build-script):
a path must not introduce another instruction line.

This correction intentionally changes generated `build.rs` bytes and their
manifest integrity bindings, including standalone owned-data evidence packages.
It does not change descriptors, provider C/archive bytes, generated safe/FFI
Rust, public signatures, package schemas, or the existing scalar SDK. Missing,
non-Unicode, and CR/LF paths fail before any stdout; target mismatch retains
precedence and its original message.

The lower package's `build_script::tests` renders both families for all five
target identities and authors standalone build-script subprocess checks for
the current host. The two executables are reused across valid-path, CR/LF,
missing-variable, target-precedence, and platform-specific non-Unicode cases.
These regressions are unrun. They test the generated instruction boundary, not
Cargo's downstream execution, archive linking, pathname authority, or a build
sandbox; the separate real package consumers remain required.

Focused gate (not executed in this tranche):

```sh
cargo test --locked -p semaprax-native-rust-owned-data-package build_script::tests
```

### Raw Wasm call boundary

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

The shared [checked `usize` multiplication correction](PORTABLE-INDEXED-BYTE-DATA-V1.md#checked-multiplication-correction)
is an explicit correctness exception to the artifact-byte preservation below:
Wasm modules emitting this operation and their derived integrity bindings change
across affected profiles, not just v8. It restores multiplication by zero and
preserves genuine overflow cleanup; schemas, source semantics, descriptors and
native code are unchanged. Its cross-target regressions are authored and unrun.

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

The frame-payload native corpus additionally has a separate allocator-observed
O0/O2 lane, alongside the unchanged plain-provider lane. The test wraps the
exact generated provider with the existing calibrated allocator observer and
checks each call: nonempty payload allocation and release, empty-owner
`free(NULL)`, allocation-free inactive `None`/`Err`, exact copy-out, and stale
drop rejection without another free. Both live-pointer and provider-slot
inventories must be empty between calls. Existing callers select the isolated
module and both authenticated baseline/display-renamed Project subjects.
The observer and its static context are private fixtures, not generated hooks
or public ABI. These checks remain authored and unrun; successful handle-drop
statuses alone are not physical deallocation evidence, and this lane does not
replace sanitizers, OOM handling, or the separate generated Rust consumer.

The separate frame-format supplement in
`tests/frame_payload_product_v1/adversarial.json` retains the existing corpus
schema without changing any of the three committed nine-case corpora. Its 72
rows cover every bit of the four-byte declared length, valid power-of-two
payload sizes, all incomplete header lengths with good and bad magic prefixes,
complete-header magic/error precedence, and unsigned 32-bit declaration edges.
Expected validity and errors are literal fixture facts, checked by a separate
host-side inventory oracle rather than by the SEMAPRAX decoder. In particular,
ignoring either upper length byte must no longer pass the conformance corpus.

The original native/raw-Wasm checkpoints run before the supplement. The 72
additional cases make 160 calls with 48 nonempty owned results, and reuse the
existing native executables and raw-Wasm instance. Actual baseline and renamed
Project subjects also use the same published npm/Rust packages and unchanged
external consumer source. The additional Rust consumer has a fresh source/input
path and shares only the compilation cache, avoiding reliance on timestamps
for an in-place `include_str!` input change. The provisioned Chromium fixture
passes the test-owned supplement to the unchanged same-origin corpus runner
after checking the original host-served corpus; it does not require a changed
host corpus or claim authentication of Project source derivation. All of these
additional execution checks remain authored and unrun.

The provisioned private toolchain gate `project_owned_tuple_sdk_v1` adds real
published v8/v9 Rust consumers for one borrowed UTF-8 string plus two borrowed
byte slices. It checks cumulative 65,535/65,536/65,537-byte tuples, Unicode byte
length rather than character count, unused arguments, and v8 inactive
`None`/`Err` branches. Accepted cases include maximum `Some`/`Ok` outputs;
rejections are followed by successful calls on the same SDK object. Separate
objects, input/output mutation, and retained outputs after object drop check
host-owned copies. Each seven-file package is reopened against its canonical
descriptor and regenerated provider binding before the locked/offline consumer
runs. The gate is authored but unrun and ignored until tools are provisioned:

```sh
cargo test --locked -p semaprax-toolchain --test project_owned_tuple_sdk_v1 -- --ignored
```

This is consumer-level capacity and ownership evidence, not an invocation-entry
counter, allocation trace, or proof of persistent native-context reuse. It does
not replace the separate sanitizer, browser, or exact-head promotion gates.

The npm counterpart `tests/project_owned_tuple_npm_v1.rs` derives both real
Project subjects and reopens each six-file publication against its verified
inline carrier and descriptor. Its Node consumer spells the same tuple corpus
independently, checks exact variant/record results, and retains independent
copies across two runtime objects. A test-only wrapper counts entry into the
actual selected Wasm exports: accepted calls calibrate one entry, and rejected
tuples must leave the count unchanged before a successful recovery call. This
observes JavaScript preflight before selected-export entry, not Wasm-internal
allocation, native-context lifetime, browser execution, or TypeScript checking.
The fixture remains authored and unrun.

`tests/native_owned_tuple_admission_v1.rs` uses the same authenticated Project
subjects to generate the actual native v8/v9 providers. Its separate C fixture
observes the provider's existing post-validation invocation counter, owner
slots, issuance serial, and instrumented `malloc`/`calloc`/`free` call counts.
Successful calls calibrate entry and allocation observations; oversized tuples
and malformed UTF-8 must preserve output sentinels and context bytes without
entry, allocation calls, or ownership changes, including inactive v8 branches.
Recovery uses the same physical context. Empty active `Bytes` results still own
a handle; their drop observes `free(NULL)` without a payload allocation. These
authored O0/O2 checks remain unrun; test-only allocator instrumentation is not a public
ABI, an OOM-recovery proof, or a sanitizer substitute.

The provisioned [Owned Data Browser v1 fixture](../platform-tests/owned-data-browser-v1/README.md)
now imports an actual generated direct-Bytes package in the existing three
browser projects instead of trusting an injected global and skipping a missing
URL. Its fixed Project and separate compiler-side carrier test bind the fixture
signatures; the browser cases cover capacity, hostile inputs, copy independence,
Wasm authentication before engine instantiation, and same-instance recovery
after genuine pre/post-owned-staging failures. Required shared/resizable/transfer
features fail closed when unavailable. These cases remain authored and unrun;
host-provisioned package provenance, raw-carrier fault coverage and physical
cleanup traces are separate obligations. No production artifact or schema
changes follow from completing this evidence fixture.

### Authored offline installed-package gate

`tests/frame_payload_product_v1/npm_installation.rs` adds an explicitly
provisioned v8 gate for both baseline and display-renamed frame projects. It
publishes the real six-file package, checks it against the retained verified
inline carrier, and runs offline `npm pack`. An independent SHA-512 of the
actual tarball must match both the pack report and the local dependency's
integrity in a closed, two-package-row lockfile before offline `npm ci`.
The lockfile must remain unchanged and all six installed files must equal the
retained compiler artifacts.

The consumer imports the installed package by name, resolves its exported Wasm
and metadata assets, and runs the unchanged nine-case corpus and 72-case
supplement. Strict TypeScript accepts the same package-name import and rejects
wrong argument types and unguarded Result access. Provision `NODE` as an
absolute Node executable, `NPM_CLI` as an absolute `npm-cli.js`, and `TSC_CLI`
as an absolute TypeScript 5.8.3 `tsc.js`; invoking both JavaScript tools through
Node avoids platform-specific command shims. The existing full-toolchain build
prerequisites also apply, including Windows native Cargo/linker provisioning.

```sh
cargo test --locked -p semaprax --test frame_payload_product_v1 npm_installation::installed_owned_npm_package_resolves_and_runs_without_compiler -- --ignored --exact
```

Missing tools fail this selected gate instead of skipping it. Installation and
consumption follow package generation without invoking SEMAPRAX or a native
compiler; TypeScript is used separately for declaration checking. Offline npm
flags, disabled lifecycle scripts, private cache and cleared npm configuration
are not OS-level network confinement. This gate is authored and unrun, covers
v8 only, and does not establish registry publication or hosted promotion.

### Authored same-source Result-extrema gates

`tests/support/owned_result_product.rs` supplies one canonical two-source v8
Project to the interpreter/native/npm fixture and the private Rust SDK fixture.
Its single stable-ID export returns `Err(0)`, `Err(i64::MIN)` and
`Err(i64::MAX)` for three input lengths, successful copied Bytes for two
others, and a genuine division failure after staging Bytes for a sixth.
The extrema stay exact integers; JavaScript expectations use decimal strings
converted to `BigInt`, never JSON numbers.

`tests/project_owned_result_extrema_v1.rs` checks the retained Project HIR in
the reference interpreter, actual native providers at O0/O2, and the real
six-file npm publication. The native and raw-Wasm observations distinguish
successful Err status/tag/payload bits from invocation failure, calibrate real
owner allocation/consumption, and require untouched failure outputs and
recovery on the same native context or Wasm instance. Native language failure
is normalized to status 1; the interpreter and raw Wasm retain division status
4. Interpreter cleanup events describe boundary copy-out, not a physical
allocator trace. Provision Clang and Node plus the existing full-toolchain
prerequisites for Windows npm publication:

```sh
cargo test --locked -p semaprax --test project_owned_result_extrema_v1 same_source_result_extrema_match_interpreter_native_and_npm -- --exact
```

`crates/semaprax-toolchain/tests/project_owned_result_extrema_sdk_v1.rs`
publishes the same source through the real Rust builder and reopens all seven
files. It checks the retained descriptor, all parsed manifest facts, inventory
hashes and regenerated provider-source binding before running a separately
locked/offline, unsafe-forbidden Rust consumer. That consumer distinguishes
`Ok(Err(...))` from an outer `CallError::SemanticFailure`, reuses two SDK
objects, and retains independent output vectors after both objects are
dropped. This is not a new package verifier or evidence that an SDK object
retains one physical native context. Provision absolute `CLANG` and
`SEMAPRAX_ARCHIVER`, native Cargo, and the existing Windows MSVC environment:

```sh
cargo test --locked -p semaprax-toolchain --test project_owned_result_extrema_sdk_v1 provisioned_result_extrema_publish_and_run -- --ignored --exact
```

Both gates remain authored and unrun. They add no production ABI, schema,
artifact or completion-status change and do not replace sanitizer or exact-head
hosted evidence.

### Authored mixed-parameter arity gates

`tests/support/owned_mixed_arity_product.rs` supplies one canonical v8 Project
with nine selected exports, covering every arity from zero through eight. The
parameter prefix repeats `i64`, `bool`, `borrow str`, and `borrow Slice<u8>`;
distinct scalar values and borrowed lengths distinguish both positions of each
repeated type. Each export returns owned `ok` or `bad` bytes according to its
present arguments. Consumer literals are independent of the source predicates.

The descriptor fixture `tests/public_api_descriptor_v1/mixed_arity.rs` checks
exact parameter order, names, identities, ordinals, result types and replay,
including separate seven/eight-parameter selections. A selected ninth parameter
must yield the exact `SPX-J113` arity diagnostic. The real nine-parameter Project
must reject before exposing an authenticated callback and preserve its input
files. This callback check is not a physical tool-execution counter.

`tests/project_owned_mixed_arity_v1.rs` retains the same Project descriptor for
actual native providers at O0/O2 and a verified, published six-file npm package.
Independent C and Node consumers exercise every arity, wrong-value controls for
each present position, same-type argument swaps and healthy recovery. The native
fixture separately observes owned allocation/copy/drop and empty context slots;
Node checks host-copy independence across runtime objects. These tests establish
only their observed argument positions and lengths when executed, not general
evaluation order, complete borrowed-content equivalence or browser support.

The private toolchain's `project_owned_mixed_arity_sdk_v1` gate publishes the
same Project through the real Rust builder, reopens the exact seven-file package
and binds its descriptor and provider source. An unsafe-forbidden, locked/offline
external consumer calls all nine methods, checks each eight-argument mutation
and recovery, and retains independent outputs across two SDK objects and after
their disposal. SDK object reuse is not proof of one persistent native context.

Focused gates, all authored and unrun:

```sh
cargo test --locked -p semaprax --test public_api_descriptor_v1 mixed_arity
cargo test --locked -p semaprax --test project_owned_mixed_arity_v1
cargo test --locked -p semaprax-toolchain --test project_owned_mixed_arity_sdk_v1 provisioned_mixed_arity_publish_and_run -- --ignored --exact
```

Native/npm execution requires Clang and Node, plus the existing full-toolchain
prerequisites for Windows npm publication. The selected Rust gate requires
absolute `CLANG` and `SEMAPRAX_ARCHIVER`, native Cargo and the existing Windows
MSVC environment. Missing prerequisites fail; no test silently skips. These
fixtures leave bounded temporary evidence trees for inspection. They do not
change production code, generated artifacts, schemas or completion status, and
do not replace strict TypeScript, installed-package, sanitizer or exact-head
hosted gates.

## Nonclaims

Public Owned Data API v1 does not claim public records or authored variants,
nested algebraic data, resources, owned inputs, borrowed outputs, owned UTF-8
strings, mutable/shared values, multiple returns, general generics, lifetime
inference, zero-copy transfer, allocator interoperability, callbacks, imports,
effects, contracts, async, reentrancy, threads, shared memory, memory growth,
WIT/Component Model support, C/C++/Swift/Kotlin bindings, native executable
support for Project v8, package resolution, registry publication, signing,
provenance, or general public aggregate ABI stability.
