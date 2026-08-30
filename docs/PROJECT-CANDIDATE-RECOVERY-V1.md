# Complete Candidate Recovery v1

Audience: compiler contributors and agents restoring complete candidates.

Status: additive implementation with focused authored, unrun regression tests.
No executed validation, target execution, or platform-completion claim.

`ProjectCandidate::recovery_capsule()` produces canonical compact JSON plus one
LF, with schema `semaprax.project-candidate-recovery.v1`. Its closed fields bind
the compiler package/version and explicit compatibility identifier, candidate
and change schemas, exact original Project revision, ordered complete semantic
changes, expected final candidate digest and Project revision, and capsule digest.
No source text, serialized HIR, disk paths, approvals, or authority are imported.

The capsule digest is SHA-256 over the domain
`semaprax.project-candidate-recovery.payload.v1\0`, the little-endian u64 byte
length, and canonical payload bytes including LF, with `capsule_digest` omitted.
It identifies content; it is not a signature, trusted provenance, or approval.
A caller may construct another self-consistent valid history, but recovery still
requires full compiler admission and exact final identity.

`ProjectCandidate::restore(admitted_base, expected_base, bytes)` requires the
independently authenticated original source revision. It bounds input before
JSON allocation, checks the closed schema/compiler/canonical bytes/content hash,
then admits each complete change and sequentially applies it through ordinary
candidate validation. It compares the final candidate and Project identities,
then regenerates and exactly compares the entire capsule before returning the
candidate. Unknown/duplicate fields, alternate whitespace or extra LF, changed
compiler compatibility, stale bases, and rehashed incorrect final identities fail.

The capsule limit is 64 MiB and history limit is 32 changes. Each change retains
its 1 MiB input and ordinary structural/constructor limits. A raw preflight caps
whole-capsule nesting at 128 and potential JSON nodes at `32 * (2 * 8192 + 128) + 256`;
the ordinary serde recursion limit also applies. This permits bounded multiple
changes without applying one change's node budget to the entire history.
Serialized byte bounds are not total-memory or replay-time bounds.

Diagnostics G236 cover capsule grammar, compatibility and canonical spelling;
G237 covers capsule resource bounds; G238 covers original-base/content/final
identity disagreements. Ordinary change/compiler diagnostics remain authoritative
when a replayed intention fails semantic validation.

## Explicit CLI storage

```
semaprax project-candidate-export <manifest> <change.json>
semaprax project-candidate-restore <manifest> <capsule.json>
```

Export admits and applies the explicit canonical change, then writes the capsule
to stdout. Restore emits the full candidate report only after successful source
authentication and replay. Both use bounded explicit regular-file reads and
held-input authentication before and after processing. Symbolic links/reparse
points and nonregular files are rejected; no source publication occurs.

Callers may choose the Git-ignored `.semaprax-candidates/` directory to retain
capsules across sessions. Creating that directory, redirecting stdout into a
chosen file, and selecting the file to restore are explicit caller actions;
neither the library nor CLI creates directories or writes a cache implicitly.
The original base source must still exist exactly when restored. A capsule is
not a replacement for source control or an incremental compiler cache.

Image Candidate Protocol v2 adds `candidate/recovery-export` chunk queries and
`candidate/recovery-restore` for structured capsules fitting its 64 KiB request
frame. Existing candidate-registry and response-before-mutation bounds apply.
The read-only v1 profile gains no methods or authority. Drafts, unresolved holes,
private last-valid candidates, warm HIR, and entire sessions are not persisted.
