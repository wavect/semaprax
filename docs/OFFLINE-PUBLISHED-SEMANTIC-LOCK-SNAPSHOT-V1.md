# Offline Published Semantic Lock Snapshot v1

Status: additive implementation and executable evidence authored, unrun,
unpublished, and unpromoted.

Audience: package-tooling, compiler, and platform-authority contributors.

## Purpose and authority boundary

Offline Published Semantic Lock Snapshot v1 turns one exact caller-owned
[Resolver v1](OFFLINE-PACKAGE-RESOLVER-V1.md) input/evidence pair into three
independently replayable byte strings. The compiler layer is authority-free.
The separate safe lower crate can publish those three strings only into one
fresh directory through the existing held, no-replace package-publication
state machine.

The snapshot is not an updateable lockfile, registry, cache, `ACTIVE` pivot,
signature, provenance record, build, sandbox, or target-execution result.
Evidence and digests carry facts, never filesystem authority.

## Public pure API and exact input capsule

The additive API is:

```text
ResolutionSnapshot {
    input_json: String,
    resolution_evidence_json: String,
    lock_json: String,
}
generate(&ResolutionInput, &ResolutionOptions, &str)
    -> Result<ResolutionSnapshot, Diagnostic>
verify(&ResolutionSnapshot) -> Result<VerifiedResolution, Diagnostic>
```

Generation first runs unchanged Resolver-v1 verification. `lock_json` is the
exact Lock-v2 returned by that replay and `resolution_evidence_json` is the
exact submitted Resolver-v1 evidence. Neither is re-rendered.

The only new wire schema is
`semaprax.offline-package-resolution-input.v1`. Its wrapper order is
`schema,digest,bytes,payload`. Its payload order is
`schema,requirements,target,allowed_capabilities,subjects,resolution_max_bytes,
limits,nonclaims`. The digest transcript is the schema plus NUL byte, the
little-endian `u64` payload length, and exact payload bytes.

Requirements and capabilities retain Resolver-v1's strict canonical order.
Subjects are embedded as raw canonical Subject-v2 JSON objects, never as
quoted strings and never through a `serde_json` re-render. Their array is
strictly sorted by exact bytes. Input catalog order therefore has no semantic
or wire effect. Every raw subject is authenticated through the unchanged
Subject-v2 source replay before rendering and after parsing.

Verification rejects an invalid wrapper, depth, duplicate/unknown/missing key,
noncanonical string/integer, trailing byte, reordered or duplicated subject,
and raw-subject boundary disagreement. It reconstructs the exact
`ResolutionInput` and `ResolutionOptions`, reruns Resolver-v1, exact-compares
the returned Lock-v2, regenerates the complete snapshot, and exact-compares all
three submitted strings. A self-consistent digest remint cannot bypass replay.

Diagnostics are `SPX-PK501` input/options, `PK502` nested Resolver/Subject
association, authentication, or policy, `PK503` bounds, `PK504` submitted
wire, and `PK505` exact replay. Resolver `PR501/505/506/507` map monotonically
to those corresponding families; other Resolver failures map to `PK502`.
Nested messages and codes do not escape this surface.

## Frozen bounds

Resolver-v1 remains the source of truth for 64 subjects, 17 MiB per subject,
128 MiB cumulative catalog bytes, and 16 MiB resolution evidence. Lock v2
retains its 16 MiB output maximum. This surface does not narrow those limits.

The input framing ceiling is exactly derived as:

| Component | Maximum bytes |
| --- | ---: |
| Closed wrapper, fixed payload members, maximum-width integers, limits, and nonclaims | 1,114 |
| Four requirement rows, including delimiters | 1,255 |
| 256 quoted maximum-length capabilities, including delimiters | 66,047 |
| 64 raw-subject delimiters | 63 |
| Total framing | 68,479 |

Package identities, ranges, targets, and capabilities use Resolver-v1's ASCII
grammar, so that calculation requires no additional JSON-escape expansion.
`MAX_INPUT_BYTES` is 128 MiB plus 68,479 bytes. The cumulative input renderer
is three final-input ceilings plus two framing ceilings: the three exact raw
catalog copies are the subject join, payload, and wrapper, while the two extra
framing ceilings cover canonical row/quote joins and fixed-member temporaries.
`MAX_SNAPSHOT_BYTES` is the checked sum of the
input, 16 MiB Resolver evidence, and 16 MiB Lock-v2 ceilings. Every runtime sum
is checked and charges exact final bytes plus canonical delimiters; it is not
an allocator-heap or wall-clock meter. No output is truncated.

## Fixed three-file publication

The lower `semaprax-offline-wasm-package` crate adds:

```text
publish_lock_snapshot(output, snapshot)
    -> Result<PublishedOfflinePackageLockSnapshot, PublicationError>
```

The visible inventory is exactly, in write order:

1. `semaprax.package-resolution-input.json`
2. `semaprax.package-resolution.evidence.json`
3. `semaprax.lock.json`

There is no visible manifest, metadata, temporary, advisory-lock, or terminal
newline file. The public API cannot select filenames. An internal sealed
inventory enum preserves the existing build-v1/v2 names, order, bytes, and
failure messages.

The facade owns output/success allocations and fully replays the snapshot
before acquiring filesystem authority. The shared authority state machine
stages create-new files, authenticates each held identity and exact byte string,
runs a second complete snapshot replay immediately before the no-replace
publication attempt, settles held handles, and authenticates the exact
published inventory. Previsibility cleanup can remove only its authenticated
stage inventory. Rename/close or postpublication uncertainty fails stop;
foreign bytes are never replaced or removed and the first failure remains
sticky.

Every supported platform retains the publisher's host precondition excluding
uncooperative mutation of the destination, parent, ancestors, and stage.
Unix/macOS additionally requires the current-euid-owned exact-mode-`0700`
parent. Darwin ACL authority remains a host precondition. Windows reparse and
identity rejection remains owned by the unchanged lower platform authority.

## Canonical nonclaims

The input capsule binds, in order: no registry/network/discovery/fetch; fresh
output only with no mutable update or `ACTIVE`; no cache/index/GC; no trusted
signature, publisher identity, provenance, license, or SBOM; no build script,
external tool, or target execution; no capability enforcement or hermetic
sandbox; evidence/digest is not authority; no source/Git/editor/commit
mutation; and no change to Report v1/v2, Subject/Lock v1/v2, Resolver v1,
Capsule v1, Build v1/v2, Project, Graph, or Cleanup contracts.

## Evidence and promotion

Authored focused evidence covers catalog permutation, exact replay, mutation,
truncation, insertion, cross-pairing, reminting, raw-subject parsing, exact and
plus-one component/cumulative length and preallocation-count guards, exact
framing derivation, public component-plus-one rejection, exact inventory,
existing output, staged substitution,
pre-effect races, foreign-byte preservation, replay disagreement, cleanup,
uncertainty, platform preconditions, and build-v1/v2 preservation.
The length-guard tests avoid allocating maximum-size capsules; they do not
claim a successful maximum-size valid-catalog replay or measured memory use.

The focused commands are:

```sh
cargo test --locked -p semaprax --test offline_package_resolution_snapshot_v1
cargo test --locked -p semaprax-offline-wasm-package --test lock_snapshot_publication
cargo test --locked -p semaprax-offline-wasm-package --lib authority::tests
```

This documentation audit did not run those commands. No local-green, hosted,
cross-platform, public-support, completion, or promotion claim is made.
