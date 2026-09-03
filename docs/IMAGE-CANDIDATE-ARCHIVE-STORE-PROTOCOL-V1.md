# Candidate Archive Store Protocol v1

Status: **Partial; implementation and regressions authored, unrun.**

This contract adds one opt-in v5 operation for persisting an already retained,
complete candidate archive. It composes the existing source-backed candidate
archive, immutable candidate archive store, and authority-neutral retention
lifecycle without transferring their roots or policies into request data.

## Startup selection

An embedding host opens `VNextSession` with candidate preparation enabled and
may call `with_candidate_archive_store(root)` exactly once before any protocol
frame. `root` is an explicit, pre-existing, current-euid-owned `0700` directory
accepted under the candidate archive store's existing supported-Unix rules.
The session holds its authenticated directory chain for its lifetime and uses
that held root for every store operation. A request cannot provide, discover,
replace, or enumerate the root.

The host may separately attach `RetentionLifecycleCoordinator` before frames.
The archive store and retention registry are distinct held roots and distinct
authorities. Selecting either one does not select the other.

## Operation

`candidate/archive-store` is exposed only when the archive store was selected
at startup. Its closed request has exactly:

- the current `image_revision`; and
- one exact retained complete `candidate_revision`.

There is no path, archive byte string, policy, cursor, store locator, restore
selector, approval, or publication input. Before touching the store, the
session authenticates live source, resolves the exact retained candidate, and
prepares its canonical `semaprax.project-candidate-archive.v1`. The immutable
store independently restores and replays that complete archive before its
no-replace publication pivot. Existing destinations are not adopted or
overwritten.

## Successful response and retention accountability

The closed success payload schema is
`semaprax.image-candidate-archive-store.v1`. It returns:

- exact image, candidate, archive, and base Project revisions;
- exact canonical stored byte count;
- `store_status: "immutable_archive_stored"`;
- false source, approval, publication, restore, and GC authority flags; and
- a closed `retention_lifecycle` selection/outcome object.

If no retention lifecycle was selected, the archive store still succeeds and
the response states `not_selected_before_frames` with a null outcome. If it was
selected, the route passes the typed `CandidateArchiveStoreReceipt` to the
existing coordinator only after the immutable store returned success. The
complete canonical `semaprax.semantic-retention-lifecycle-report.v1` is then
returned, including `advanced`, stale, failed, publication-uncertain,
recovery-uncertain, or poisoned status.

Registry failure never changes `store_status`, deletes or rolls back the
archive, or turns the successful archive receipt into a store failure. Recovery
and reopening remain host startup responsibilities. A candidate archive store
failure remains an ordinary operation error; a post-pivot store diagnostic can
state publication uncertainty and must not be blindly retried.

## Bounds and discovery

The archive retains its existing 128 MiB canonical byte bound. The store keeps
its existing 32-entry inventory, 4,096-byte normalized absolute-root bound,
64-component depth bound, single-link `0600` file rule, one-stage rule, and
cooperative exclusive root lock. The v5 request remains under 64 KiB and the
response under 1 MiB; the nested lifecycle report remains under 65,536 bytes
with at most 96 receipt projections and 64 diagnostic codes. This operation
supplies exactly one candidate receipt.

Capabilities, query catalogues, schemas, generated clients, and MCP expose the
method only in the startup-selected session. Generated clients and MCP encode
the same two digest selectors and do not perform host I/O or acquire either
root.

## Nonclaims

This operation does not restore an archive, make a candidate current, retain a
new in-memory candidate, refresh source, mutate canonical source, approve or
publish source, discover or enumerate storage, overwrite an archive, delete a
subject, apply a GC plan, select freshness, inspect deployment state, or grant
filesystem authority beyond the startup-held immutable store operation.

The implementation and hostile cases are authored and intentionally unrun.
No execution, platform, durability, crash-recovery, generated-client, MCP, or
completion evidence is claimed by this document.
