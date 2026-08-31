# Source-backed Typed-Draft Archive v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: compiler contributors, embedding hosts and agents recovering unfinished work.

This archive makes typed-hole recovery independent of the continued existence
of the original source checkout. It combines the last valid candidate's
self-contained source archive with the draft's pending selectors. It does not
turn holes into source, restore unchecked HIR or grant publication authority.

## Library API and exact recovery

`ProjectCandidateDraftArchive::prepare(draft, expected_draft)` creates an
archive for an exact immutable draft. `to_json()`, `archive_digest()`,
`draft_digest()` and `base_revision()` expose its bytes and content bindings.
`ProjectCandidateDraftArchive::restore(bytes, expected_archive, expected_draft)`
returns a `ProjectCandidateDraft` without a supplied Project revision or any
filesystem access.

The closed `semaprax.project-candidate-draft-archive.v1` object contains
`schema`, `compiler`, `base_revision`, `candidate_digest`,
`candidate_archive_digest`, `candidate_archive`, `draft_recovery_capsule`,
`draft_digest`, `source_authority`, `approval_authority`, `trusted_hir` and
`archive_digest`. The three authority/trust fields are exactly false. Compiler
metadata binds package, version and
`semaprax.project-candidate-draft-archive-source-replay.v1` compatibility; it
does not claim a compiler binary identity.

The archive nests the existing
[complete-candidate source archive](PROJECT-CANDIDATE-ARCHIVE-V1.md) and
[draft recovery capsule](PROJECT-CANDIDATE-DRAFT-RECOVERY-V1.md) as exact
canonical strings. Their schemas, limits and source/history replay rules remain
unchanged. The candidate archive includes the canonical original manifest and
all canonical sources; the draft capsule includes the remaining body,
expression and contract-expression selectors after any partial fills.

Restore validates outer grammar, compiler compatibility, resource bounds,
canonical bytes and expected identities before trusting any nested meaning.
It then independently restores the complete candidate archive through ordinary
Project admission and full intention-history replay. The existing draft
recovery API rebuilds the pending selectors against that reconstructed original
base. The last valid candidate must agree with the independently archived
candidate, and the final draft, capsule and regenerated archive must match
exactly.

Matching or recomputing public hashes does not admit invented source facts,
hole contexts, overlap rules or expression identities. Context is regenerated
from the rebuilt checked revision. No serialized source root, file handle,
session permission, approval, cursor or trusted HIR is restored.

## Pending meaning and completion

Recovery returns only a draft. Its unresolved body, expression and contract
holes remain unresolved, and ordinary `complete` still rejects them. A draft
whose holes were all filled can restore as ready to complete, but the explicit
completion call remains necessary. Filling any restored hole uses the ordinary
typed constructor, canonical source rebuild, invariant and target-admission
checks.

The archive contains prior valid source and history, just as callers may
already retain the valid candidate from which they opened a hole. Those bytes
are not the unfinished candidate's intended meaning. The draft API adds no
public accessor for its private last-valid candidate; archive recovery does
not install that candidate in a session registry.

Source files may be edited or removed after export. Library restore operates
on the archived owned bytes and does not recreate or overwrite those files.
A host may persist the archive separately from canonical source; this change
adds no automatic disk store or background checkpoint process.

## Host startup and in-session recovery

`VNextSession::restore_draft_archive(bytes, expected_archive, expected_draft)`
is an embedding-host API available only before the first nonempty frame and
only when the host selected candidate preparation. Historical source is allowed,
but the archived original manifest must exactly equal the live canonical
manifest, including source paths, entry, test module and profile. The current
live image is not replaced or relabeled as historical source.

The host operation authenticates live inputs before and after complete archive
replay. Registry capacity and handle serialization must succeed before the
single draft insertion. Failure inserts no draft or candidate. No build, test,
commit or approval state is restored, and an invalid or ordinary first frame
does not reopen the startup window.

`VNextSession::export_draft_archive(expected_image, expected_draft)` lets an
authorized live host export a retained draft before or after requests. It binds
the current image and reauthenticates live source around archive construction.
The caller owns any persistence decision.

V5 additionally exposes candidate-granted RPC methods:

| Method | Parameters beyond `image_revision` | Result |
| --- | --- | --- |
| `hole/archive-export` | `draft_revision`, optional `offset`, `chunk_bytes` | Exact archive chunks. |
| `hole/archive-restore` | `archive_revision`, `draft_revision`, structured `archive` | Ordinary draft handle. |

RPC restore requires the exact current original Project revision as well as
the canonical manifest. It cannot import a historical base after startup.
This prevents an empty or eventually completed draft from bypassing the
existing startup-only historical-candidate archive boundary. Historical work
uses the host startup API; neither route implicitly rebases pending selectors.

Both RPC methods preserve the existing held-source checks. Restore prepares a
draft mutation, checks response and registry bounds, then installs only that
draft after authentication. The handle's `source_candidate_revision` identifies
the reconstructed last valid candidate; it need not identify a registered
candidate. Existing identical drafts retain their prior session association.

The request frame stays at 64 KiB. Larger archives can use the library or host
API; chunked export does not introduce chunked request uploads, caller-selected
filesystem paths or an authority-bearing continuation. Both methods remain
outside immutable parallel-read batches. V1–v4 methods and older recovery
capsule bytes are unchanged.

## Bounds, evidence and remaining work

Outer archives have a fixed byte limit and a raw syntax-depth/node preflight
before JSON allocation. Nested archive/capsule strings keep their existing
limits, and escaping contributes to the outer cap. Near-limit nested inputs
can therefore fail the wrapper rather than widening source, constructor,
history or hole limits. No bound is a process-memory or latency guarantee.

The outer input/output limit is 128 MiB, with raw nesting at most 16 and at
most 1,024 potential JSON nodes. The nested candidate archive remains bounded
to 128 MiB and draft capsule to 64 MiB; their source/history limits and the
shared sixteen-hole limit are unchanged. Canonical JSON uses sorted object
keys and one terminal LF. `archive_digest` hashes
`semaprax.project-candidate-draft-archive.payload.v1`, a NUL, the little-endian
u64 payload byte length and canonical payload including LF with
`archive_digest` omitted. This is content addressing, not provenance or approval.

`SPX-G340` owns outer grammar, canonicality and compatibility; `SPX-G341` owns
capacity; `SPX-G342` owns selector, source-base, manifest and replay mismatches.
Nested replay diagnostics propagate. Host lifecycle/policy rejection remains
`SPX-G303`; stale host export image remains `SPX-G282`. Ordinary registry,
transport-frame and held-source failures retain their existing diagnostics.

Authored cases in
[library evidence](../tests/project_candidate_draft_archive_v1.rs) and
[transport evidence](../tests/image_draft_archive_transport_v5.rs) cover missing
original source, partial fills, context regeneration, ready and unresolved
drafts, altered content, host startup rules, current-base RPC recovery and
unchanged authority. Tests and compiler checks were not run; no completion
row is promoted.

Automatic durable registries, complete session checkpoints, pending validation,
continuation recovery, approval recovery and measured cross-process performance
remain outside this archive. The full graph-operational programme remains
incomplete.
