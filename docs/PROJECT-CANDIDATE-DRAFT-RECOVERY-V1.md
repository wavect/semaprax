# Typed-Hole Draft Recovery v1

Status: implementation and regression evidence authored, unrun. No compiler,
test, runtime or completion gate was executed for this change.

Audience: embedding hosts, agent client authors and compiler contributors.

Draft recovery saves pending body/expression selectors together with the exact
history of their last fully checked candidate. Restoring it rebuilds that valid
history from an independently admitted original source base, then recreates
every hole through the ordinary draft APIs. It never admits unfinished meaning
as source, HIR, candidate evidence or publication authority.

## Library and capsule

```rust
ProjectCandidateDraft::recovery_capsule(&self) -> Result<String, Vec<Diagnostic>>
ProjectCandidateDraft::restore(
    base: Arc<ProjectRevision>, expected_base: &str, bytes: &[u8]
) -> Result<ProjectCandidateDraft, Vec<Diagnostic>>
```

The canonical JSON capsule uses `semaprax.project-candidate-draft-recovery.v1`
and exactly these fields:

- `schema`, `compiler`, `base_revision`, `draft_schema`;
- `candidate_recovery`, the existing complete-candidate recovery object;
- `holes`, sorted by `hole_id`;
- `draft_digest`, the expected reconstructed draft identity;
- `capsule_digest`, the content digest of this capsule.

Compiler facts bind package, version and
`semaprax.project-candidate-draft-recovery-compatibility.v1`. The nested
[complete-candidate capsule](PROJECT-CANDIDATE-RECOVERY-V1.md) retains its own
unchanged compatibility, schema, history and replay checks. There is no new
source archive, serialized context, approval, session policy or trusted HIR.

Each body selector is exactly
`{kind:"function_body",hole_id,target}`. Each expression selector is exactly
`{kind:"expression",hole_id,target,expression_id}`. Expression IDs belong to the
reconstructed last-valid revision. After partial fills, export records the
remaining selectors already remapped by the normal fill route.

The additive [Contract Expression Holes](PROJECT-CANDIDATE-CONTRACT-HOLES-V1.md)
row is exactly `{kind:"contract_expression",hole_id,target,expression_id}`.
Its phase, predicate ordinal, scope and structural path are rederived from the
replayed source. It shares the same total sixteen-hole and selector bounds;
body/expression-only capsule bytes remain unchanged.

Object keys are sorted, array order is retained, and canonical bytes end with
one LF. The digest uses SHA-256 over the domain
`semaprax.project-candidate-draft-recovery.payload.v1\0`, the little-endian u64
payload length, and canonical payload bytes with `capsule_digest` omitted.
This identifies content; it establishes neither provenance nor approval.

Restore bounds input before allocating a JSON tree, checks closed shapes,
compiler compatibility, exact canonical bytes, original base and content digest,
then invokes ordinary `ProjectCandidate::restore` on the nested object. It opens
a fresh draft and replays `with_body_hole`, `with_expression_hole` or
`with_contract_expression_hole` for each
selector. Duplicate or overlapping holes, inaccessible targets and stale
expression identities retain their ordinary failures. Final draft identity
and the entire regenerated capsule must match exactly before returning.

The limit is 64 MiB, at most 16 pending holes and JSON depth 128. The raw node
bound is the existing complete-history preflight bound plus `16 * 16 + 32`
wrapper/selector nodes. The nested history separately retains its unchanged
32-change, byte, node, constructor and compiler limits. The enclosing capsule
can reject a near-limit history; it does not relax those limits. Export applies
the same outer preflight. Byte bounds do not establish total-memory or replay
time guarantees. G230 covers outer grammar/canonicality, G231 capacity, and
G232 stale/content/final identity mismatches; nested replay retains its own
diagnostics.

## Completion remains separate

Restoration returns only a draft. `complete` still rejects while any hole is
unresolved, and there is no public last-valid candidate accessor. Hole context
is rederived from rebuilt checked facts rather than loaded from the capsule.
Empty or fully filled drafts can recover as `ready_to_complete`, but still
require the ordinary completion call to release their candidate.

The nested valid history can independently reconstruct its prior valid
candidate, just as a caller may already hold that candidate before opening a
hole. That prior state does not represent or discharge pending intentions.
Recovery does not promise to hide prior valid history; it prevents unresolved
meaning from acquiring source or publication authority.

## Host-selected v5 transport

Only v5 sessions with `candidate_prepare` expose these methods:

| Method | Parameters beyond `image_revision` | Result |
| --- | --- | --- |
| `hole/recovery-export` | `draft_revision`, optional `offset`, `chunk_bytes` | Canonical capsule chunks. |
| `hole/recovery-restore` | Structured `capsule` | Existing draft handle. |

The export envelope is `semaprax.image-draft-recovery-chunk.v1`, binding the
draft, capsule schema, exact UTF-8 byte offsets and optional next offset; it
says `materializable:false` and `source_authority:false`. Restore must fit the
existing 64 KiB request frame; larger capsules can use the library with an
explicitly admitted base. Neither route reads a caller-selected path or writes
storage. Clients may explicitly save capsule bytes outside canonical source;
no automatic session checkpoint or new storage directory is introduced.

Restore uses the session's current original source revision and ordinary held
input authentication before and after preparation. Registry capacity and
response-overflow checks precede installation. Failure installs nothing.
Only the draft enters the registry; no complete-candidate entry, build grant,
test grant or approval is recovered. For a newly restored draft, the handle's
`source_candidate_revision` names its reconstructed last-valid candidate and
need not name a registered candidate. An already retained identical draft keeps
its existing session association. Query, fill and completion use the draft
handle; subsequent completion may register the fully filled candidate.

V1–v4 method sets are unchanged. V5 schemas and generated TypeScript/Python/Rust
clients describe only the host-selected methods. Explicit refresh still clears
drafts. A saved capsule can restore after an unchanged-source refresh, but a
changed original base rejects: this route does not implicitly rebase holes.
Historical source archives, entire registries, pending validation, cursors and
publication authority are not part of draft recovery.

The additive [Draft Archive v1](PROJECT-CANDIDATE-DRAFT-ARCHIVE-V1.md) wraps
this unchanged capsule with the original canonical source archive. Library
restore no longer needs a separately retained original revision. Historical
live-session imports remain host-only at startup; its separate RPC restore
requires the current original base and recovers no extra authority.

Authored, unrun evidence lives in
`tests/project_candidate_draft_recovery_v1.rs` and
`tests/image_transport_v5/draft_recovery.rs`, covering mixed holes, partial fills,
context regeneration, ready drafts, hostile and stale capsules, restart,
permission boundaries, registry association and source refresh.
