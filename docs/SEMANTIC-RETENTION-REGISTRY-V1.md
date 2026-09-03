# Semantic retention registry v1

Status: **Partial, authored/unrun**. The library implementation and hostile
interruption regressions are authored. Only compile and static checks have run;
the registry has no CLI or session integration and executes no garbage
collection.

Audience: embedding hosts that need one durable current selector over bounded,
authority-neutral image, candidate and draft retention metadata.

## Explicit storage boundary

`semantic_retention_registry` composes the existing receipt planner and immutable
retention metadata store. The caller selects one existing absolute registry root
owned by the current effective user with exact mode `0700`. That root must
already contain one current-user-owned `0700` directory named `metadata`. The
registry neither creates nor discovers either directory.

The root inventory is closed to `metadata`, an optional `CURRENT` file and at
most one interrupted `.CURRENT-stage`. `metadata` remains the unmodified
`semantic_retention_store` root, so it continues to admit only its fixed
immutable checkpoint/plan envelopes. The registry holds and rechecks the root,
ancestor and metadata-directory identities. Pair persistence and restoration
remain descriptor-relative to that held `metadata` directory for the complete
registry operation; a same-owner path replacement cannot redirect a nested
store open. `CURRENT` and its stage must be current-user-owned, single-link
regular files with exact mode `0600`; the cursor is at most 4,096 bytes.
Absolute paths are bounded to 4,096 bytes and 64 components.

The root is an explicit metadata-selection capability. It is not a subject
locator, source root, archive root or proof that any retained subject still
exists. The returned state retains no path, file descriptor or reusable store
handle.

## Receipt-driven lifecycle

`initialize(root, policy, receipts)` requires at least one successful typed
`RetentionReceipt`. Image, candidate and draft stores construct those receipts
only after their ordinary immutable publication succeeds. Receipt values carry
the exact typed subject identity and logical stored-byte accounting; they carry
no path or store authority.

Initialization derives sequence one through `checkpoint_receipts`, persists its
canonical checkpoint and exact companion plan in `metadata`, reloads that pair
through the ordinary canonical restorers, and only then publishes `CURRENT`.
The cursor binds:

- the nonzero sequence;
- checkpoint, predecessor and plan digests;
- the fixed retention policy; and
- `authority: "none"` plus closed nonclaims.

`recover(root)` reads the one canonical `CURRENT` selected by the explicit root,
loads exactly its named immutable pair and verifies sequence, predecessor,
policy, checkpoint and plan equality. It restores metadata only. It does not
load an image, candidate, draft, source file or archive.

`advance(root, expected_cursor_digest, receipts)` is a compare-and-swap
transition. It authenticates the current cursor and pair under the caller's
exact expected cursor digest, derives exactly sequence plus one from a nonempty
successful-receipt inventory, and reuses the policy authenticated by the current
checkpoint. Stale cursors fail before creating the next pair. Callers cannot
advance time or age retained identities with an empty generation.

Receipt order cannot affect checkpoint or plan bytes. The existing policy still
ranks only exact generation numbers and subject identities; the registry reads
no clock, mtime or access frequency.

## Durable ordering and interruption

Every transition durably publishes its immutable pair before staging the next
cursor. Generation one uses a no-replace `CURRENT` rename. Later generations
atomically exchange the staged cursor with `CURRENT` while holding the exclusive
registry lock, validate that the exchanged old cursor is the expected exact
value, remove that obsolete cursor stage and settle the directory.

If the exact derived pair already exists after an interrupted or uncertain
attempt, retry loads it by the newly derived checkpoint, predecessor and plan
selectors. Only exact canonical equality permits reuse; the registry does not
enumerate or adopt an unrelated pair. A remaining cursor stage must parse as an
exact canonical cursor, select an authenticated immutable pair through the held
metadata directory, and be either the consecutive next cursor over `CURRENT` or
the exact checkpoint predecessor exchanged out by `CURRENT`. The next exclusive
transaction rebinds the stage's identity and file facts immediately before
removing and settling it. Recovery rejects malformed or unrelated stages rather
than silently ignoring them. A linked, foreign-owned, oversized or wrongly
permissioned stage also fails closed.

Failure before the cursor pivot leaves the prior `CURRENT` authoritative. A
failure after the atomic pivot reports explicit uncertainty: the caller must run
`recover` against the same explicit root before deciding whether to retry. A
blind retry with the old cursor digest fails the compare-and-swap check.

## Bounds and authority

The registry inherits all semantic-retention limits: at most 96 observations,
128 MiB logical accounting per subject, 8 GiB total policy bytes and 0 through
32 protected generations. Checkpoint and plan JSON remain capped at 1 MiB each.
The unchanged metadata store admits at most 32 immutable pairs. Capacity failure
stops the transition; the registry does not delete an old pair to make room.

The implementation is available only on the existing supported Unix set:
Linux, Android, Apple Unix targets and Redox. Other targets fail without opening
a registry. Diagnostics `SPX-G464` through `SPX-G468` cover invalid cursor data,
capacity, binding, stale/CAS and filesystem or post-pivot uncertainty failures.

`RetentionRegistryState::authority()` and its restored metadata remain
`RetentionAuthority::None`. The registry does not:

- execute, approve, repair or delete a GC-plan subject;
- delete immutable checkpoint/plan pairs;
- scan image, candidate or draft stores or infer a subject locator;
- restore or make current any source, image, candidate, draft or approval;
- infer workspace freshness, validation, review, runtime or deployment state;
- grant source, filesystem-subject, GC, publication or deployment authority; or
- add a command, protocol method, daemon route or automatic session hook.

Removing an obsolete private cursor stage is transaction recovery for registry
metadata. It is not application of the GC plan and never removes a retained
subject or immutable checkpoint/plan pair.

## Authored evidence

The module regression authors initialization, exact recovery, a pre-persisted
pair left before cursor publication, a stale cursor stage, successful consecutive
CAS advancement, stale-CAS rejection and preservation of both immutable pairs.
It also rejects an empty receipt generation, malformed/unrelated/unbacked cursor
stages and a same-owner replacement of the held `metadata` child. These cases
are authored and unrun; there is no executed interruption, cross-process,
cross-platform, CLI, session, subject-replay, GC or quality-gate evidence.
