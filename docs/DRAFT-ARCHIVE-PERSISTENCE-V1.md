# Typed-draft archive persistence v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: embedding hosts, local agent workspace integrators and compiler contributors.

Unfinished typed drafts can be stored independently of the original checkout
and explicitly selected when a workspace session starts. Storage retains the
[source-backed draft archive](PROJECT-CANDIDATE-DRAFT-ARCHIVE-V1.md), including
its canonical original sources, valid intention history and pending selectors.
It does not persist unchecked HIR, publish source or complete unresolved holes.

## Typed store API

The existing `semaprax::candidate_archive_store` module adds:

```rust
pub fn persist_draft(
    root: &Path,
    archive: &ProjectCandidateDraftArchive,
) -> Result<CandidateDraftArchiveStoreReceipt, Vec<Diagnostic>>;

pub fn load_draft(
    root: &Path,
    expected_archive: &str,
    expected_draft: &str,
) -> Result<ProjectCandidateDraft, Vec<Diagnostic>>;
```

The receipt has private fields and borrowed `archive_digest()`, `draft_digest()`
and `base_revision()` getters. It contains no root, file handle, approval or
reusable authority. Persistence fully replays the archive before opening the
root and prepares receipt allocations before publication. Loading restores the
draft while the selected store input is held, then authenticates its exact
bytes and inventory again before returning the draft.

The implementation shares the existing private archive-store byte transport.
Its [root, lock, inventory and publication contract](CANDIDATE-ARCHIVE-STORE-V1.md)
is unchanged: an explicitly selected, existing, normalized absolute `0700`
current-user-owned directory; held no-follow ancestors; `0600` single-link
regular files; at most 32 completed entries and one inert failed stage; an
exclusive create-new stage and no-replace rename. The host must exclude
uncooperative mutation by the same principal and any additional ACL authority.
The advisory lock only serializes cooperating callers.

Complete candidates and drafts may share a root and its aggregate 32-entry
limit. Both use `<archive-digest-hex>.json`; their distinct canonical schemas and
hash domains bind the content kind. The selected typed loader rejects an archive
of the wrong kind. Unselected entries receive metadata checks only, not semantic
validation. Draft and complete-candidate files retain the same 128 MiB limit.
There is no schema sniffing that silently converts one subject into the other.

Persistence never adopts or overwrites an existing destination, even if the
bytes match. Failed stages are neither resumed nor removed. `SPX-I361` still
means the rename occurred but subsequent observation or settlement failed;
resolve that uncertainty using exact `load_draft` and independently retained
digests, never a blind persist retry. Existing `SPX-G300`–`SPX-G302` and
`SPX-I360` store failures remain unchanged; archive replay diagnostics propagate.
Supported Unix targets and fail-closed behavior elsewhere are unchanged. Local
sync ordering is not a universal power-loss or distributed-filesystem guarantee.

## Explicit host commands

```text
semaprax project-draft-persist <manifest> <draft-capsule.json> <store-root>
semaprax project-draft-load <store-root> <archive-digest> <draft-digest>
```

Persist reads one bounded regular draft-recovery capsule, restores it against
the manifest's exact authenticated original Project revision and constructs a
self-contained draft archive. Live source authentication finishes before any
store effect. The store independently replays the archive again before opening
its root. The capsule remains bounded to 64 MiB; JSON escaping and the nested
source archive must also fit the 128 MiB archive bound.

The successful receipt has schema
`semaprax.candidate-draft-archive-store-receipt.v1`, `archive_digest`,
`draft_digest`, `base_revision`, `historical_source_snapshot:true`,
`current_source_admission:false`, `source_authority:false` and
`commit_approval:false`. Its canonical stdout bytes are prepared before
persistence. Loss of stdout is not evidence that no file was published.
The command does not discover or create roots, change permissions, or clean
failed stages. `.semaprax-candidates/` remains the Git-excluded conventional
directory name; hosts must explicitly choose and initialize their private root.

Load requires both expected identities, independently replays archived source,
history and pending holes, and emits the ordinary draft summary. The original
manifest, sources and capsule may be absent. Load does not recreate files or
expose the private last-valid candidate as the unfinished draft's meaning.
To continue editing, select that exact archive through startup policy.

## Workspace host policy v6

`semaprax.workspace-host-policy.v6` requires all v5 fields and a required
`draft_archives` array. Each entry is a closed object with `root`,
`archive_digest` and `draft_digest`. Roots are host-selected absolute paths;
digests use canonical lowercase SHA-256 syntax. At most sixteen distinct draft
digests may be selected. Missing/null arrays, duplicate draft selections,
unknown fields and nonempty selections without `candidate_prepare:true`
reject. V1–v5 remain closed and reject the additive field.

Startup loads complete candidates and drafts through the typed store APIs
before opening any deadline-bound Git provider or accepting a frame. Recovered
drafts pass the same startup-only lifecycle, exact digest, canonical manifest,
live-source authentication and registry-capacity checks as direct host archive
recovery. Only draft entries are retained; their private last-valid candidates,
tests, validation attempts, approvals and source authority are not installed.
Startup failure serves no requests and publishes no source.

Embedding hosts can pass the store's opaque compiler-created draft to
`VNextSession::retain_archived_draft(draft, expected_draft)` without serializing
and replaying it a second time. It shares the same admission helper as
`restore_draft_archive`; it accepts no raw HIR and grants no additional authority.

A sibling checkout with the same canonical manifest may have changed source.
Recovery retains the historical base without replacing the current image.
Filling holes and completing the draft remain explicit operations; historical
completion still requires explicit rebase before current-source publication.
Alternatively, [typed-draft rebase](PROJECT-CANDIDATE-DRAFT-REBASE-V1.md) can
move valid history and pending selectors to the current revision before filling
the remaining holes. It returns a draft and does not implicitly complete it.
The original Git startup approval boundary is unchanged. No RPC can choose a
store root, change startup policy, write the store or approve a candidate.

## Authored evidence and remaining work

`tests/semantic/draft_archive_store.rs` and `tests/semantic/draft_archive_cli.rs` author
source-loss recovery, partial-hole continuation, exact typed selection,
hostile storage inputs, publication preservation and strict startup policies.
Tests, compiler checks, CLI executions and long quality gates were not run.
No completion-matrix row is promoted.

This adds explicit durable draft selection. Automatic registry checkpoints,
branch naming, cursors, pending validation recovery, eviction, cross-platform
storage support and measured recovery performance remain open. Stored archives
are disposable recovery inputs; canonical `.spx` remains the source of truth.
