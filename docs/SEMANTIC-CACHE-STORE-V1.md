# Authenticated semantic cache store v1

Status: implementation and focused regressions authored; unrun and unverified.

Audience: trusted compiler hosts, cache integrators, and compiler maintainers.

This additive store retains compiler-created checked-module cache state across
processes. It authenticates the complete selected payload **before** entering
the private HIR decoder. It is separate from source-backed candidate archives,
Project Revision Store entries, semantic images, Git publication, and the
existing cold/AST-only/in-process semantic cache constructors.

## Trust contract

The host provisions and exclusively controls a dedicated private root and its
signing key for trusted compiler use. No RPC, archive, or untrusted client may
choose key bytes, sign raw payload bytes, or construct decoded checked HIR. The
public writer accepts only opaque `ProjectFrontendCache` state; the encoder,
envelope signer, and decoder are crate-private implementation details.

The host must keep its currently executing **static compiler installation**
immutable from process execution through every cache operation, including its
path and ancestors. The implementation hashes the actual bounded file located
through `current_exe`, holds its file and directory descriptors, and rechecks
path identity and exact bytes. This binds the observed compiler installation
file. It does **not** prove which bytes were already mapped before the first
observation, attest dynamic libraries, detect every runtime injection, or verify
that the executable is statically linked. Static installation integrity is an
explicit host precondition, not a claim inferred from a version string or hash.

Likewise, HMAC proves possession of the protected key, not compiler provenance
independently of that custody contract. Hostile same-principal code, key theft,
key replacement, ACL grants outside the checked Unix mode boundary, malicious
mount changes, administrators, and kernel compromise are excluded. A same-user
process able to read the key can forge cache entries. There is no signature
service, hardware attestation, arbitrary-host security claim, key-import API,
or general secret-management facility.

## Public interface

```rust
pub fn initialize(root: &Path) -> Result<(), Vec<Diagnostic>>;
pub fn persist(
    root: &Path,
    cache: &ProjectFrontendCache,
) -> Result<SemanticCacheReceipt, Vec<Diagnostic>>;
pub fn load(
    root: &Path,
    expected_digest: &str,
) -> Result<ProjectFrontendCache, Vec<Diagnostic>>;
pub fn evict(
    root: &Path,
    expected_digest: &str,
) -> Result<SemanticCacheEvictionReceipt, Vec<Diagnostic>>;
```

`initialize` requires an existing, empty, host-selected directory. It obtains a
fresh 32-byte key from OS entropy and publishes it create-new; it does not import
or adopt a caller-provided key, create the root, overwrite a key, or rotate one.
`persist` accepts only semantic-cache mode, obtains bytes through the private
compiler snapshot encoder, authenticates the local key/compiler binding, and
publishes one immutable envelope. AST-only caches reject.

`SemanticCacheReceipt` exposes borrowed `entry_digest()` and
`compiler_digest()` plus `payload_bytes()`. It carries no key, root, descriptor,
approval, or reusable authority. The entry digest identifies the whole envelope
and is distinct from project, image, candidate, and compiler-file identities.

`load` reads the selected envelope, verifies its requested content address and
MAC, checks its exact compiler/context binding, and only then calls
`project::incremental::decode_snapshot`. The private decoder reconstructs source
ASTs, checks stored synthetic-AST matches, performs the normal cross-file/link/
profile checks using authenticated checked modules, and compares retained graph
and revision evidence. It returns an opaque cache, not public raw HIR. Loading
does not authenticate the current raw project files; the session constructor
must independently read and authenticate current source before deciding whether
any restored entry is reusable. A source/dependency/context mismatch cannot be
converted into a cache hit merely because a MAC verified.

`evict` removes exactly one digest-selected completed entry under an exclusive
store lock. It authenticates the held root, complete inventory and selected
file identity before the handle-relative unlink, syncs the directory, then
rechecks the surviving inventory and root binding. Its receipt reports the
removed entry digest, envelope bytes and number of completed entries remaining.
It carries no key, source handle, retry permission or publication authority.
Absence and selector disagreement fail before the namespace pivot; any failure
after unlink is `SPX-I363` uncertainty and must not be retried blindly.

## Deterministic lifecycle telemetry

The public CLI adds:

```text
semaprax semantic-cache-lifecycle <manifest> <empty-store-root>
```

The command composes existing authority boundaries into one bounded authored
scenario: initialize the caller-provisioned private root, perform a cold
source-authenticated semantic open, persist and authenticate one entry, restore
it into another source-authenticated session, perform an unchanged explicit
refresh, evict that exact entry, then rebuild cold. The canonical JSON report
uses `semaprax.semantic-cache-lifecycle.v1` and includes the existing exact
compiler-work report for cold open, restored open, refresh and post-eviction
rebuild; payload/envelope bytes; final entry count; and exact Project/image/work
equivalence assertions. The command fails unless cold admission resolves at
least one module with zero checked-HIR hits and both restored open and refresh
reuse the complete resolved module inventory with zero resolutions. Successful
completion leaves only the initialized key.

The report is deterministic evidence of compiler operations and retained-byte
counts, bounded to 512 KiB. It deliberately contains no elapsed time, RSS,
allocator, model-token or cross-process claim. The command executes in one
process, never mutates source, executes target code or grants publication
authority. It does not automatically remove partial store effects after a
failure; each underlying operation retains its existing settlement contract.

## Exact envelope

All integer fields are unsigned little-endian. There is no optional field,
compression, JSON header, or trailing data outside the MAC:

| Field | Size / encoding |
| --- | --- |
| Magic/version | 8 bytes, ASCII `SPXSHC01`. |
| Compiler installation SHA256 | 32 raw bytes. |
| Compatibility byte count | `u32`, at most 2,048. |
| Compatibility | Six `u32`-length-prefixed UTF-8 strings: package, package version, OS, architecture, endianness, and checked-module compatibility; then pointer width as `u32`. |
| Payload byte count | `u64`, at most 128 MiB. |
| Payload | Exact private compiler snapshot bytes. |
| Authentication tag | 32-byte HMAC-SHA256. |

The HMAC input is the fixed domain
`semaprax.semantic-cache-store.authenticated-envelope.v1` followed by one NUL,
then every envelope byte preceding the tag. The content address is
`sha256:` plus lowercase SHA256 of the entire envelope, including its tag.
Compiler package/version or compatibility strings alone are never accepted as
an exact-build identity.

Before MAC verification the loader enforces the total fixed envelope bound,
checks the public requested digest, and locates only the fixed terminal tag.
It performs no HIR decode or header-driven allocation. After the constant-time
HMAC verification it validates magic, compiler digest, framed compatibility,
payload length, and exact end position. A tamperer who recomputes the public
content address still fails MAC verification. A correctly signed entry from a
different observed compiler installation rejects before decoding.

## Filesystem behavior and limits

The root must be normalized and absolute, at most 4,096 bytes and 64 normal path
components, current-effective-user-owned, and exact `0700`. Relative, dot,
parent, repeated-separator, trailing-slash, filesystem-root-only, and symlinked
root spellings reject. All ancestor directory identities remain held and are
checked against their names and a fresh absolute reopening.

The complete inventory is:

```text
compiler-cache.key       # exact 32-byte secret, regular single-link 0600
<64 lowercase hex>.bin   # at most 32 completed authenticated envelopes
.stage-<64 lowercase hex> # at most one retained failed entry stage
.stage-key               # alternative retained initialization stage
```

At most one stage of either kind is permitted. All objects must be regular,
single-link, current-effective-user-owned exact `0600` files; unexpected names,
symlinks, hard links, special objects, directories, and excess entries reject.
Completed envelopes must be nonempty. The total envelope bound is 128 MiB plus
4,096 bytes; key stages are at most 32 bytes. Unselected envelopes receive only
bounded identity/mode/owner/size inventory checks, not content authentication.

Each invocation acquires a nonblocking advisory root lock: exclusive for key
initialization, publication and eviction, shared for load. It coordinates cooperating callers;
it is not protection against a malicious peer ignoring it. A stage is created
once with exclusive/no-follow flags, written, synced, reread exactly, checked
against held identity and complete inventory, and published by no-replace
rename. The root is synced and the published path, bytes, key, and compiler
file are rechecked before success. Receipt allocation precedes publication.

The compiler file must be a nonempty regular single-link executable, no larger
than 256 MiB, with no group/other write bits. Its ancestors are held without
following links after canonical resolution; installation roots need not be
private cache roots. The complete canonical executable path is at most 4,096
bytes and its parent chain has at most 64 normal directory components. SHA256 is streamed through a 16-KiB buffer, with bounded
size and held/path metadata checks and full rehashes before/after publication
or after load. A change during an operation rejects. The filesystem checks do
not independently establish the static-installation trust precondition above.

Load keeps the key, entry, lock, and compiler handles while decoding and replaying
the snapshot, then rereads selected bytes and key and rechecks the compiler.
It performs no store writes. Publication never adopts an existing entry, even
if its bytes match, and never removes failed stages. A failed stage blocks
future publication but may coexist with a readable completed entry. Failed key
initialization does not become an implicitly adopted key on retry.

Any failure after a successful publication rename or eviction unlink is
`SPX-I363`, including final readback, key/compiler disagreement, directory
settlement, or lock-release failure. Publication may already exist or the
selected entry may already be absent. There is no rollback, automatic retry, cleanup,
key rotation, automatic eviction policy, or garbage collection. A selected entry may be inspected
only through ordinary authenticated `load`; failed initialization requires
explicit host intervention, never silent adoption. Sync ordering is not a
power-loss, NFS, overlay, hardware-durability, or physical-crash guarantee.

The route is enabled only for the supported Unix handle-relative/no-replace
target families (Linux, Android, Apple, Redox). Other targets fail closed.
There is no Windows store-support claim. Byte and construction limits are not
RSS or total allocator guarantees; bounded envelope/input buffers use fallible
reservation, but no general out-of-memory recovery claim follows.

## Diagnostics and evidence

| Code | Meaning |
| --- | --- |
| `SPX-G306` | Invalid selector/configuration or non-semantic cache selection. |
| `SPX-G307` | Envelope/payload/header/inventory capacity exceeded. |
| `SPX-G308` | Root/key/file/compiler/context/content-address binding disagreement, existing destination, or uninitialized/busy root. |
| `SPX-G309` | HMAC authentication failure before private decode. |
| `SPX-I362` | OS entropy/filesystem failure before confirmed publication or during load. |
| `SPX-I363` | Failure after successful publication or eviction namespace pivot; outcome is uncertain. |

Private tests author envelope success, recomputed-address tampering, wrong key,
wrong compiler digest, incompatible authenticated context and oversized header;
real filesystem tests author private create-new key initialization, mode
rejection, repeated initialization, key symlink rejection, tampered invalid
payload rejection before decoding, and immutable no-replace publication. These
tests do not expose public key/signing APIs and remain unrun. CLI evidence also
authors exact eviction and the five-stage lifecycle receipt, source-byte
preservation, warm-work facts and cold reconstruction equality. Cross-process
recovery evidence is separate. No tests, compiler gates, executable fixtures,
or generated clients ran while authoring this implementation. Cross-process
cache reuse is not measured performance evidence or full-goal completion.
