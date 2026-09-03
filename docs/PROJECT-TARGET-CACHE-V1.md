# Project target cache v1

Status: Partial; one scalar Web target lane and focused regression sources are
authored. No target execution, benchmark, test suite, or completion gate was run.

Audience: compiler contributors and embedding hosts that already hold an
admitted immutable Project revision.

`ProjectTargetCache` is a caller-owned, single-entry cache for the existing
scalar Web inline carrier. It accepts only a private-constructor
`ProjectRevision`; source verification, HIR validation, workspace linking and
Project-profile admission therefore precede the cache boundary. Every request
also invokes the revision's held-input check. The cache grants no source,
filesystem, process, execution, or publication authority.

## Exact key and replay

The key binds compiler package/version and
`semaprax.project-scalar-web-target-work.v1`, canonical manifest bytes, Project
revision, workspace revision, Project graph digest, entry module, ordered Web
exports, and the requested target byte limit. A mismatch is a miss rather than
an approximate hit.

On a miss, the ordinary `build_web_inline` path emits the target. On an exact
hit, the cache skips only deterministic target emission and carrier assembly.
It clones the in-memory carrier, reruns its independent integrity verification,
parses the envelope, and checks the complete subject and limit binding against
the admitted revision before returning it. A failed miss or replay does not
replace the prior successful entry.

The cache does not deserialize untrusted target bytes or persist an entry.
Compiler executable identity is not claimed; same-process ownership and the
closed compatibility key bound this first lane. npm, Native, non-scalar,
cross-revision, cross-process, and partial target-work reuse remain open.

## Work report

Each result carries canonical
`semaprax.project-target-cache-work.v1` JSON. It records the exact target key,
whether target emission was reused, zero or one emitter call, one carrier replay
call, retained payload digest and artifact bytes. Validation fields distinguish
the admission completed before revision construction from the exact target
subject replay performed on this request. The report is bounded to 32 KiB and
makes no allocator, RSS, elapsed-time, execution, or authority claim.

Module-local authored regressions cover a cold miss, exact hit with identical
carrier, and an incompatible-profile failure that leaves the retained scalar
entry reusable. They were not run. Before any performance claim, this lane needs
executed cold/warm evidence with observed time and memory, broader target
profiles, exact compatibility matrices, and integration into the measured
agent lifecycle.
