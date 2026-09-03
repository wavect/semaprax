# Persistent Semantic Cache v1

Status: Partial; implementation and focused regression evidence authored, unrun.

Audience: compiler contributors and hosts managing trusted compiler installations.

This opt-in host store preserves compiler-created checked-module HIR across
processes. A fresh process authenticates the complete private cache, parses its
canonical source again, and rebuilds the linked Project and graph while reusing
checked module HIR. This is actual resolver reuse, with bounded private decoding;
it is not a portable source archive or a general incremental compiler.

## Host surface

`semantic_cache_store::initialize(root)` initializes an existing, empty, dedicated
host directory with an operating-system-generated secret key. `persist(root,
&ProjectFrontendCache)` accepts only opaque compiler-created semantic caches;
there is no public arbitrary-byte signer or raw checked-HIR constructor.
`load(root, expected_entry_digest)` returns a fully replayed semantic cache.
Its `restored_work()` describes the successful warm Project replay.

The explicit CLI adapters are:

```
semaprax semantic-cache-init <store-root>
semaprax semantic-cache-persist <manifest> <store-root>
semaprax semantic-cache-load <store-root> <entry-digest>
semaprax semantic-cache-evict <store-root> <entry-digest>
```

Initialization emits `semaprax.semantic-cache-initialized.v1`; persistence emits
`semaprax.semantic-cache-receipt.v1` with `entry_digest`, `compiler_digest`, and
`payload_bytes`. Neither receipt grants source authority, current-source
admission, or commit approval. Load emits the existing
`semaprax.project-semantic-cache-work.v1` report. Each command can run in a fresh
process using the same compiler executable and host-protected store.

Eviction removes one exact digest-selected entry under the same held-root and
exclusive-lock discipline. It emits
`semaprax.semantic-cache-eviction.v1`, binding the removed digest and envelope
byte count plus the remaining completed-entry count. The operation hashes the
held bytes before unlink, rejects absence or digest disagreement, settles the
directory, and reports post-unlink failures as `SPX-I363` uncertainty. It does
not remove the store key, another entry, canonical source, host policy, or any
publication state. Because eviction does not require the selected envelope to
match the running compiler, a host can remove an obsolete but exactly selected
entry after an upgrade. Persisting an unchanged admitted project again rebuilds
the same deterministic entry; a changed project produces a different entry.

Workspace host policy `semaprax.workspace-host-policy.v5` preserves the v4
fields and requires `semantic_cache_entry`, either null or the closed object
`{ "root": <absolute host path>, "entry_digest": <canonical SHA256> }`.
A selected entry requires both `frontend_cache: true` and `semantic_cache: true`.
Older policy schemas reject this field, including null. Selection happens
before the first frame; RPC parameters cannot select cache files, roots, or
keys. Existing candidate archive and source-commit grants remain separate.

A historical load does not inspect the original source paths. Starting a live
workspace from that cache still authenticates the host-bound manifest and all
current source files. Edited inputs invalidate the affected module and reverse
import closure through the ordinary cache path. A historical cache cannot
restore old source into a live workspace or suppress held-input drift checks.
Deleting the store or explicitly evicting an entry leaves canonical source
intact; explicit cold startup and source-derived rebuild remain available. A
selected corrupt cache fails closed rather than silently falling back to a
differently authenticated state.

## Authentication and trust boundary

The store binds the exact SHA256 of the current executable, package name/version,
OS, architecture, endianness, pointer width, and checked-module compatibility.
Different executables from the same package version are incompatible. The host
must trust its static compiler installation and keep it immutable from process
execution through each operation. Hashing the executable file does not attest
already-loaded instructions, dynamically loaded libraries, or a hostile host.
Same-principal tampering, stolen keys, malicious compiler installations, and
compromised operating systems are outside this boundary.

Each envelope contains `SPXSHC01`, the 32-byte compiler digest, a little-endian
u32 context length and compatibility context, a little-endian u64 payload length,
the payload, and a 32-byte HMAC-SHA256 tag. The MAC input is the domain
`semaprax.semantic-cache-store.authenticated-envelope.v1` followed by NUL and
all envelope bytes preceding the tag. The public entry digest hashes the entire
envelope, including the tag. Reminting that public digest after an edit cannot
replace the secret-key MAC. MAC verification precedes payload-controlled
allocation and private HIR decoding; exact compiler/context binding precedes
adoption.

The private codec covers source AST, complete resolved HIR, cleanup inventories,
cleanup plans, and loan plans. It rejects unknown enum tags and static tokens,
noncanonical map/set ordering, duplicate entries, trailing bytes, excessive
lengths, depth, and allocation accounting. It is not a public deserialization
contract or a way for an agent to submit graph facts as canonical meaning.

After decoding authenticated state, load independently parses/formats every
stored canonical source and rederives each synthetic resolver input, including
imports, declarations, IDs, and spans. Checked reuse requires exact equality.
It reruns HIR validation, cross-file/stub checks, linking, Project-profile
admission, and graph generation, then requires exact stored project/workspace
revisions and graph bytes. Every module must be a checked-HIR hit; unexpected
cold resolution rejects restoration. The work report counts this final warm
build, not the preceding independent source parsing or authentication work.
This does not claim runtime equivalence or target execution.

## Filesystem and resource limits

Supported Unix hosts require an absolute normalized root, held directory-chain
checks, an owner-only 0700 dedicated root, and owner-only 0600 regular single-link
key/entry files. Symlink adoption and implicit key rotation are rejected.
The executable must be a bounded, executable, regular single-link file without
group/other write permission. Held paths, file identities, inventory, key, and
compiler bytes are rechecked around operations. Immutable entry publication
uses the existing exclusive staging/install discipline; no canonical source or
Git reference is written.

The payload is bounded to 128 MiB, envelope overhead to 4096 bytes, store inventory
to 32 entries, and compiler executable to 256 MiB. Private codec limits include
128 MiB allocation accounting, one million nodes, depth 256, and exact EOF.
Existing source, AST, checked-module prebounds and Project limits remain in
force. Accounting is not a peak heap/RSS guarantee; input, decoded state, staged
cache, and rebuilt linked representations may coexist.

`SPX-G304/G305` report private codec grammar/capacity rejection. Store diagnostics
use `SPX-G306` for invalid requests, `SPX-G307` for capacity, `SPX-G308` for
binding/compiler mismatch, and `SPX-G309` for failed MAC authentication.
Filesystem failures retain `SPX-I362`; `SPX-I363` represents publication
uncertainty and must not be presented as proof that no cache entry was installed
or removed.
Ordinary source and Project diagnostics propagate.

## Authored evidence

`tests/semantic_cache_store_cli_v1.rs` authors separate-process warm load with
three checked-HIR hits and zero resolver calls; historical loading after source
edits; live startup admission and unchanged refresh; request-level cache and
commit authority rejection; exact entry eviction, source preservation and
deterministic warm rebuild; explicit cold startup after deletion; reminted
public digest with invalid MAC; exact compiler mismatch; and closed older/new
startup policy validation. The compiler-mismatch case applies only when both
executables satisfy the supported 256 MiB bound.

Private codec regressions additionally cover full HIR with nonempty cleanup and
loan plans, canonical reencoding, malformed containers, allocation limits,
unknown tags/tokens, and truncation. Store-local regressions cover private key
initialization, hostile filesystem shapes, and authentication before decoding.
These tests were authored but not run. No compiler, interpreter, CLI fixture,
generated client, target, or long gate was executed for this change. Hosted
executable evidence and measured cross-process performance remain outstanding.
