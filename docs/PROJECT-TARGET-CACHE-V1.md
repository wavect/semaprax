# Project target cache v1

Status: Partial; exact scalar Web, pathless native-C11 and pathless npm target
lanes and focused regression sources are authored. No target execution,
benchmark, test suite, or completion gate was run.

Audience: compiler contributors and embedding hosts that already hold an
admitted immutable Project revision.

`ProjectTargetCache` is a caller-owned cache with one exact entry for the
existing scalar Web inline carrier and one for the existing pathless native-C11
carrier plus one for the existing pathless npm carrier. It accepts only a private-constructor
`ProjectRevision`; source verification, HIR validation, workspace linking and
Project-profile admission therefore precede the cache boundary. Every request
also invokes the revision's held-input check. The cache grants no source,
filesystem, process, execution, or publication authority.

## Exact key and replay

Each key binds compiler package/version and either
`semaprax.project-scalar-web-target-work.v1` or
`semaprax.project-native-c11-target-work.v1` or
`semaprax.project-pathless-npm-target-work.v1`, canonical manifest bytes, Project
revision, workspace revision, Project graph digest, entry module, ordered Web
exports, and the requested target byte limit. A mismatch is a miss rather than
an approximate hit.

On a miss, the ordinary Web, C or npm path emits the selected target. On an
exact hit, the cache skips only deterministic target
emission and carrier assembly. The Web lane reruns its independent integrity
verification. The C lane replays canonical payload digest, exact
revision/manifest/source/export bindings, artifact hex/count/SHA inventory and
every embedded C-header envelope, then compares separately retained digest and
byte facts. The npm lane invokes `ProjectNpmBuild::verify` to replay its closed
schema, semantic recipe and exact ordered file bytes/digests, then matches the
manifest/package and source-bound cache key plus separately retained digest and
byte facts. A failed miss or replay does not replace the prior successful
entry, and a miss in one lane does not evict another.

The cache does not deserialize untrusted target bytes or persist an entry.
Compiler executable identity is not claimed; same-process ownership and the
closed compatibility keys bound these lanes. C compilation, linking and
execution are absent. Package-manager installation and npm runtime execution
are absent. Wasm-package, non-scalar, cross-revision,
cross-process, and partial target-work reuse remain open.

## Work report

Each result carries canonical
`semaprax.project-target-cache-work.v1` JSON. It records the exact target key,
whether target emission was reused, zero or one emitter call, one carrier replay
call, retained payload digest and artifact bytes. Validation fields distinguish
the admission completed before revision construction from the exact target
subject replay performed on this request. The report is bounded to 32 KiB and
makes no allocator, RSS, elapsed-time, execution, or authority claim.

Module-local authored regressions cover Web cold/exact/incompatible behavior;
C cold/exact replay, lane isolation and max-byte key drift; and fail-closed C
carrier tampering with recovery after restoring the exact entry. npm adds exact
hit, C/npm lane isolation, limit drift and altered-cache-fact rejection/recovery.
They were not run. Before any performance claim, these lanes need
executed cold/warm evidence with observed time and memory, broader target
profiles, exact compatibility matrices, and integration into the measured
agent lifecycle.
