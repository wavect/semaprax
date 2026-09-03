# Semantic retention host lifecycle v1

Status: **Partial, authored/unrun**. The typed library coordinator and
regressions are authored. Library compilation is the only intended executable
gate for this tranche; no session, protocol, CLI, GC, or hosted evidence is
claimed.

Audience: embedding hosts that already publish immutable semantic images,
candidate archives, or draft archives and explicitly choose durable retention
metadata coordination at startup.

## Startup boundary

`RetentionLifecycleCoordinator::open` accepts only:

- one caller-selected existing private retention-registry root;
- one bounded `RetentionPolicy`; and
- either an exact canonical expected cursor digest or an explicit expectation
  that the registry is uninitialized.

The constructor authenticates that expectation through ordinary registry
recovery. An initialized registry must match both the expected cursor and fixed
policy. An uninitialized expectation accepts only the registry's exact
not-initialized result; a busy, malformed, substituted, or initialized root
fails. The root and expectation come from host startup code, never a request,
receipt, image, candidate, draft, or generated client.

The coordinator opens and retains the authenticated registry-root and metadata
directory identities for its complete lifetime. Every recovery, initialize,
or compare-and-swap operation uses those held directories, validates the
startup-selected path when the operation begins, and revalidates it immediately
before a `CURRENT` pivot. A same-owner path substitution cannot redirect writes
to a replacement directory or make a stranded immutable pair current. A race
after the initial validation may leave an unselected pair in the displaced held
metadata directory. Substitution after the final pre-pivot validation,
including at or after the pivot, retains the registry's ordinary post-pivot
uncertainty and explicit-recovery requirement. It receives no image,
candidate, or draft store root and cannot reopen a subject. Its
`RetentionAuthority::None` result means no subject restore/delete, GC, source,
approval, or publication authority; the startup-selected registry root remains
the host's explicit metadata-write capability.

## Successful receipt checkpointing

`checkpoint` accepts a closed typed slice containing only:

- `SuccessfulRetentionReceipt::Image(&ImageStoreReceipt)`;
- `SuccessfulRetentionReceipt::Candidate(&CandidateArchiveStoreReceipt)`; or
- `SuccessfulRetentionReceipt::Draft(&CandidateDraftArchiveStoreReceipt)`.

These values exist only after their ordinary immutable store operations return
success. The coordinator cannot manufacture them or treat a failed store as a
successful receipt. One call admits 1 through 96 receipts. It derives only each
receipt's closed typed identity fields, subject digest, and exact logical
stored-byte accounting. Image rows also retain the image receipt digest;
candidate and draft rows retain their exact archive/content/base selectors.
Canonical report rows sort by a total key of subject digest, kind, and exact
canonical projection bytes, independent of receipt order even if equal digest
keys are presented.

For an explicitly uninitialized root, the first accepted batch calls registry
`initialize`. Otherwise the coordinator calls registry `advance` with the exact
startup/current cursor. A successful result updates the coordinator's private
expected cursor to the returned digest. Registry policy remains fixed; changing
it requires a new explicitly authenticated startup coordinator.

The registry persists the canonical checkpoint/plan pair before `CURRENT` and
does not apply the plan. This lifecycle route never deletes an evicted subject,
deletes metadata, scans a store, restores a subject, selects a source revision,
changes current source, approves a candidate, or publishes anything.

## Outcome and failure semantics

After a nonempty typed receipt slice is supplied, `checkpoint` always returns a
`RetentionLifecycleOutcome`; it does not return a plain error that could be
misread as store rollback. Every ordinary outcome includes
`subject_store_status: "successful_receipts_precede_registry_attempt"`, the
successful receipt count, and bounded authority-neutral receipt projections.
Registry failure never denies, removes, or rolls back those already published
immutable subjects.

`registry_cursor_status` is a closed enum:

- `advanced`;
- `registry_cursor_not_advanced`;
- `registry_cursor_not_advanced_stale`;
- `registry_cursor_not_advanced_pair_publication_uncertain`;
- `registry_cursor_uncertain_recovery_required`;
- `registry_attempt_blocked_reopen_required`;
- `no_registry_attempt_invalid_receipt_inventory`;
- `no_registry_attempt_receipt_capacity_exceeded`;
- `no_registry_attempt_receipt_projection_failed`;
- `registry_cursor_advanced_report_unavailable`; or
- `registry_outcome_report_unavailable`.

An empty call alone uses
`subject_store_status: "no_successful_receipt_batch_accepted"`. A nonempty
over-capacity batch still states that successful receipts preceded the rejected
registry attempt; it does not project all rows beyond the 96-receipt bound.
Receipt-projection failure uses
`subject_store_status: "successful_typed_store_receipt_was_supplied"` and makes
no registry attempt. The complete bounded typed inventory is projected before
that decision: successful rows are retained in total canonical order and every
projection failure is selected in exact typed-identity order, independent of
caller order. Empty/over-capacity outcomes permit a corrected bounded batch,
and projection failure requires inspection of the already successful typed
receipt. They do not poison a previously usable coordinator because no registry
mutation was attempted. Once a registry attempt has failed, blocked
status dominates every later nonempty call, including an over-capacity
inventory or a receipt whose projection cannot be completed. The outcome still
states that successful receipts precede the blocked no-attempt result; a bounded
call includes every projection it can authenticate, while an over-capacity call
does not materialize rows beyond the fixed bound. Only an empty call retains
the empty-specific input status.

`SPX-G467` maps to a known stale/not-advanced cursor. Metadata-pair publication
uncertainty (`SPX-I371`) is distinguished from cursor uncertainty. `SPX-G468`
requires explicit registry recovery because `CURRENT` may have advanced. Once
any registry attempt fails, the coordinator is blocked: later successful
receipts receive a no-attempt outcome until the host recovers the explicit root
and opens a new coordinator with an exact expectation. It never retries blindly.

If canonical outcome rendering were to fail after a successful cursor pivot,
the in-memory outcome retains `registry_advanced() == true`, reports
`registry_cursor_advanced_report_unavailable`, and carries `SPX-G484`. The host
must recover and reopen; it must not infer that either subject storage or cursor
publication failed.

## Report schema and bounds

The closed schema is
`semaprax.semantic-retention-lifecycle-report.v1`, canonical compact JSON with
one terminal LF and a 65,536-byte cap. Its exact fields are:

- `schema`;
- `successful_receipt_count`;
- `successful_store_receipts`;
- `subject_store_status`;
- `registry_cursor_status`;
- nullable `sequence` and `cursor_digest`;
- `diagnostic_codes`;
- `next_action`;
- `authority`; and
- `nonclaims`.

At most 64 diagnostic codes are retained. `SPX-G480` owns lifecycle input
grammar, `SPX-G481` capacity, `SPX-G482` startup cursor/policy binding,
`SPX-G483` blocked-coordinator reuse, and `SPX-G484` outcome encoding. Registry,
planner, and immutable-store diagnostics retain their existing codes.

The report is status metadata. It is not a receipt for subject publication, a
proof that a subject remains present, a freshness claim, a store locator, a GC
approval, or a runtime/deployment compatibility result.

## Authored evidence

The existing semantic store harness authors a mixed real image/candidate/draft
receipt batch, startup-uninitialized coordination, exact per-family receipt
projections, generation-one and consecutive generation-two cursors, reopen by
the returned exact selector, and a second coordinator's stale then poisoned
failure without deleting any stored subject. A focused substitution regression
holds an uninitialized root, replaces its pathname with another private root,
and requires failure before either directory receives `CURRENT`; restoring the
original binding permits ordinary reopen. Equal-digest synthetic projection
rows pin total canonical ordering under reversed input. These additions are
authored and unrun. No test target, broader filesystem interruption matrix,
CLI/session route, GC, subject restoration, parallel host, or hosted gate is
claimed.
