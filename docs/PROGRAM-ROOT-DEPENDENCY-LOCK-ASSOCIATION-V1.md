# ProgramRoot Dependency Lock Association v1

Status: additive SEG-02 fact bundle; focused evidence passes locally.

Audience: compiler contributors, package-tooling authors, semantic-service
implementers, and reviewers of exact ProgramRoot dependency associations.

This contract associates exact, already-admitted Project Lock v1 bytes with
one [ProgramRoot v1](PROGRAM-ROOT-V1.md). It does not redefine the Canonical
Semantic Workspace Revision's `dependency_lock_digest`, which remains the
digest of its local admitted dependency-closure projection. It also does not
change Project Lock v1's legacy `program_root` field, whose value remains the
`ProjectRevision` identity.

## Derivation

`ProgramRootDependencyLockAssociation::derive` accepts a held authenticated
`ProjectSnapshot`, an already-derived ProgramRoot, its expected digest, and
caller-supplied lock bytes. It:

1. freshly derives the snapshot's canonical workspace and ProgramRoot;
2. exact-compares the selected ProgramRoot and expected digest;
3. delegates the submitted lock to existing `verify_project_lock`, including
   its source, manifest, compiler, interface, target, capability, and Project
   revision checks; and
4. emits a compact canonical association without retaining a path or file
   handle.

The schema is
`semaprax.program-root.dependency-lock-association.v1`. It binds the exact
ProgramRoot digest, canonical workspace revision, distinct canonical workspace
dependency-closure digest, Project revision, Project Lock schema/payload
digest/legacy program root, exact whole-lock byte count, and an independently
domain-separated digest of the complete lock bytes.

The typed association privately retains those exact admitted lock bytes and
exposes them read-only through `project_lock_bytes()`. This permits a later
exact context to invoke replay without reacquiring or rereading a lock. The
canonical association JSON records only their count and digest; it does not
embed them or turn them into a dependency closure.

The association digest uses domain
`semaprax.program-root.dependency-lock-association.digest.v1\0` over the exact
canonical document without its self-referential `association_digest` field.
The whole-lock byte digest uses domain
`semaprax.program-root.dependency-lock-association.lock-bytes.digest.v1\0`.
Both use `domain || u64le(byte_length) || exact_bytes`.

## Optional segment and retention hook

`program_root_segment()` returns a normal content-addressed ProgramRoot segment
descriptor with kind `project_lock_association`, this association schema as its
node schema, the association digest as node digest, and the exact association
byte length. This extension segment is deliberately not inserted into the
default nine-segment ProgramRoot manifest, preserving all existing bytes and
identities. A semantic service may retain the typed association and segment
beside its immutable canonical workspace generation after the same admission;
it must not silently acquire or reread a lock.

The exact selector hooks for a future service/query/transaction surface are:
ProgramRoot digest, canonical workspace revision, Project revision,
association digest, Project Lock payload digest, and whole-lock byte digest.
Every selector must exact-match one retained immutable association. No default
service or transaction wire changes in this badge.

## Replay, bounds, and diagnostics

`replay` validates exact canonical association bytes, authenticates the embedded
association digest, exact fixed limits and nonclaims, cross-field Project
revision association, and the submitted lock byte count/schema/digests before
freshly repeating ProgramRoot and Project Lock verification,
and exact-compares the complete result. Association bytes are capped at 64 KiB;
Project Lock's existing 1 MiB bound applies to submitted lock bytes.

Malformed or internally inconsistent association material uses `SPX-G550`;
stale ProgramRoot selection or exact replay uses `SPX-G551`. Project Lock
admission retains its existing `SPX-J123` stale and `SPX-J124` malformed
diagnostics and precedence.

The operation performs no filesystem read/write, resolution, registry access,
network request, process execution, cache mutation, commit, or publication.
It proves exact association only, not provenance, signature, license, SBOM,
revocation, vulnerability status, acquisition, or dependency availability.

Focused tests in `tests/workspace/program_root_dependency_lock.rs` pass in the
existing Workspace harness, including exact replay, hostile fixed-field and
digest mutations, cross-snapshot binding, both byte bounds, and preservation of
default ProgramRoot and Project Lock bytes.
