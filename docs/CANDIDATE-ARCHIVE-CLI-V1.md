# Candidate archive CLI v1

Status: implementation and regression cases authored, unrun.
Audience: local hosts and agent workspace integrators.

```text
semaprax project-candidate-persist <manifest> <capsule.json> <store-root>
semaprax project-candidate-load <store-root> <archive-digest> <candidate-digest>
```

Persistence reads one bounded regular recovery-capsule file through the existing
CLI reader and independently replays it against the manifest's live authenticated
original source revision. It prepares a self-contained source-backed archive and
finishes live-source authentication before invoking the explicitly selected
archive store. The store then independently replays the archive before its first
filesystem effect. This stores a historical source-and-intention subject; it
does not keep the raw checkout locked or claim it stays at that revision.

The host must create the store root first and satisfy the private-root and
same-principal exclusion requirements of [Archive Store v1](CANDIDATE-ARCHIVE-STORE-V1.md).
The command never discovers or creates a cache root. It does not overwrite,
adopt, remove or clean an existing entry or failed stage. The source-backed
archive uses the bounds and compatibility contract in
[Candidate Archive v1](PROJECT-CANDIDATE-ARCHIVE-V1.md); recovery capsules retain
their existing 64 MiB input limit.

Successful persistence emits one canonical compact JSON line with schema
`semaprax.candidate-archive-store-receipt.v1`, `archive_digest`,
`candidate_digest`, `base_revision`, `historical_source_snapshot:true`,
`current_source_admission:false`, `source_authority:false`, and
`commit_approval:false`. This output is prepared before the filesystem pivot;
the receipt contains no path or authority. Store post-pivot uncertainty remains
`SPX-I361`, never an ordinary successful or safely retryable publication.
If stdout is lost after success, use the independently retained archive digest
and exact load; never infer that missing output means no entry exists.

Load requires the explicit root and both exact expected digests. It replays
archived source and intentions and emits the ordinary complete candidate report.
The original checkout and recovery-capsule file can be absent. Loading does not
recreate those files, restore a current workspace image, replace source, or grant
Git/managed-workspace publication authority. No native executable or interpreter
is run by archive persistence or loading.

## Startup policy v3

`semaprax serve-workspace <manifest> <host-policy.json>` accepts additive policy
`semaprax.workspace-host-policy.v3`. It requires every v2 field, including the
`frontend_cache` boolean, plus `candidate_archives`:

```json
{
  "schema": "semaprax.workspace-host-policy.v3",
  "candidate_prepare": true,
  "diagnostics": false,
  "build_enabled": false,
  "frontend_cache": true,
  "test_policy": null,
  "git_commit": null,
  "candidate_archives": []
}
```

Each array entry is an exact object with `root`, `archive_digest`, and
`candidate_digest`. Roots must be absolute and selected by the host. Digests
must use canonical lowercase SHA256 syntax. At most sixteen distinct candidate
digests may be selected; duplicate selections, unknown fields, null arrays and
missing fields reject. A nonempty array requires `candidate_prepare:true`.
The existing registry's aggregate retained-report bound also applies. V1 and
v2 policies remain closed and reject this added field.

Before the first frame, the CLI loads each archive through the real store and
hands its opaque, independently checked candidate to
`retain_archived_candidate`. The session requires the same canonical manifest
as its current live Project but permits historical source revisions. Source
authentication and registry admission occur before each insertion. Startup
failure exits without serving requests or publishing source. Archives load
before any deadline-bound Git provider opens. No store write, automatic scan,
checkpoint, or background recovery occurs during the session.

The host must explicitly rebase historical candidates before current-source
publication. Startup recovery cannot restore drafts, tests, approvals, Git
policy, publication receipts or a wider session capability. Any optional Git
host/approval still comes separately from the host policy and retains its
startup-only boundary. No RPC parameter can request archive roots or restore
the policy. See [Workspace Archive Recovery](IMAGE-WORKSPACE-ARCHIVE-RECOVERY-V1.md).

## Evidence and limitations

`tests/candidate_archive_cli_v1.rs` authors real CLI persist/load after removal
of the original sources, duplicate publication preservation, startup recovery
against edited live source, authority exclusion, strict policy/version/count
checks, and RPC rejection. Historical complete CLI help snapshots remain pinned
after explicit removal of the two additive command lines. These tests are
unrun; no compiler or store fixture execution occurred during implementation.

This supplies explicit candidate persistence and startup recovery, not a warm
cross-process HIR cache, automatic durable registry, complete session checkpoint,
eviction/GC, runtime verification, or measured performance improvement.
