# Semantic retention protocol session v1

Status: **Partial, authored/unrun**. The v5 embedding-host integration and its
regression are authored. Library compilation is the only executed gate. No wire
method, immutable subject store, test target, hosted run, GC, or restoration is
claimed.

Audience: embedding hosts, compiler contributors, and protocol reviewers.

## Startup selection

`VNextSession::with_retention_lifecycle` is an opt-in host API used before the
session accepts any protocol input. It accepts one explicit existing private
registry root, one bounded `RetentionPolicy`, and either an exact expected
cursor digest or the explicit uninitialized expectation. It constructs the
existing `RetentionLifecycleCoordinator`, so the authenticated registry root
and metadata directory file descriptors and identities remain held for the
session lifetime. Request parameters, generated clients, MCP tools, receipts,
and source cannot select or replace that root, policy, or cursor expectation.

Attaching twice or after protocol input fails. The ordinary coordinator startup
checks still reject stale policy/cursor bindings, substituted paths, malformed
registry state, and a mismatched initialized/uninitialized expectation. This
attachment grants only the already explicit registry metadata-write capability;
it grants no subject-store, source, candidate, draft, image, approval, commit,
publication, network, process, or secret authority.

## Successful store boundary

`checkpoint_successful_retention_receipts` accepts only the existing closed
`SuccessfulRetentionReceipt` families for image, candidate, and draft receipts.
Those typed receipts are returned only after their separate immutable stores
have succeeded. The method forwards one bounded batch to the retained
coordinator and stores the returned `RetentionLifecycleOutcome` as the session's
latest accountability outcome.

Subject storage precedes this method. A registry stale, uncertain, capacity,
binding, or poisoned outcome cannot deny or roll back that successful store and
never deletes a subject or metadata pair. `retention_lifecycle_outcome` exposes
the exact latest canonical outcome, including successful receipt projections,
cursor status, sequence/digest when known, diagnostic codes, next action,
authority `none`, and nonclaims. An unattached-session error explicitly states
that the successful receipt remains valid.

The coordinator preserves its existing fail-closed behavior: a failed registry
attempt blocks later attempts until an embedding host explicitly recovers the
same root and opens a new session with an exact cursor expectation. The session
does not retry, infer freshness, scan a store, restore a subject, make a subject
current, apply a pending GC plan, approve deletion, or publish source.

## Current integration boundary

Project Agent Transport v5 currently creates no `ImageStoreReceipt`,
`CandidateArchiveStoreReceipt`, or `CandidateDraftArchiveStoreReceipt`. Its
candidate and draft archive methods export canonical bytes; refresh derives a
new in-memory image. None is an immutable subject-store success. Therefore no
wire request can safely trigger retention checkpointing in this version.

The deepest coherent integration is the session-owned startup coordinator plus
the typed post-store host method. An embedding host that separately invokes an
existing immutable store immediately supplies that returned receipt to the
session method. Adding automatic wire-level checkpointing requires a future
protocol operation that itself owns an explicit subject-store root and returns
a typed successful receipt; it must preserve the same store-success versus
registry-outcome split. This v1 does not add such filesystem authority or
mislabel archive export or refresh as storage.

## Authored evidence

The existing semantic store harness authors a real successful candidate archive
store followed by two v5 sessions holding the same explicitly uninitialized
registry. The first session records generation one. The second returns the
exact stale cursor outcome while the archive remains present. The regression
also pins retained latest-outcome bytes and verifies that an unattached session
reports configuration failure without denying or removing the prior store.
These cases are authored and intentionally unrun.
