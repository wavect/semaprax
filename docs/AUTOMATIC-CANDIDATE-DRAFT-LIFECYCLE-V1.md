# Automatic durable candidate/draft lifecycle v1

Status: additive host-library implementation and focused regressions authored.

Audience: embedding hosts and agent-workspace integrators.

`AutomaticRetentionLifecycle` composes one host-selected held candidate archive
root with one distinct host-selected held semantic-retention registry root.
Neither root can enter through a request, archive, receipt, generated client or
environment lookup. Construction authenticates both roots and rejects aliasing.

For a candidate or draft, the operation independently replays and publishes the
immutable typed archive first. Only its successful typed receipt can enter the
retention coordinator. The existing registry derives the canonical checkpoint
and pending GC plan, persists their exact immutable pair, then compares and
swaps `CURRENT` against the startup-selected cursor. Stale and uncertain
outcomes keep the archive success intact. A registry failure blocks further
attempts until explicit recovery and reopen; there is no adoption, retry,
rollback, deletion or cleanup.

If interruption occurs after archive publication, a freshly opened lifecycle
uses `resume_candidate` or `resume_draft` with the independently retained exact
archive/content/base selectors. The held store replays the existing archive and
reconstructs its typed receipt without writing, adopting, or weakening ordinary
`persist` no-clobber behavior. Resume authenticates `CURRENT` before acting. If
the exact subject is already selected, it returns an `already_checkpointed`
outcome without creating a generation. If absent, it makes exactly one registry
attempt. A same-archive cross-kind or selector conflict fails closed.

After a confirmed pivot, the library independently recovers the selected pair
and derives a bounded canonical replay receipt for the transaction, binding the prior and new cursors,
checkpoint, plan, archive/content/base identities, closed candidate/draft kind,
and deterministic registry-local branch identity. The transaction is
accountability data with independent exact-byte replay, not an additional durable object or capability; the
authenticated checkpoint/plan/CURRENT generation is the durable record.

At startup, `restore_pending` reads only `CURRENT`, authenticates the selected
checkpoint/plan pair, and independently restores candidate/draft archives
through the already-held archive root. It skips image subjects. Restored values
contain archived history and pending selectors only. They are not current-source
admission, approval, Git state, publication, GC permission or trusted/warm HIR.
Original checkout files are not read.

The archive/store, checkpoint/plan and registry cursor v1 bytes are unchanged.
Limits remain 32 shared archive entries, 96 retention subjects, 128 MiB per
archive, and a 65,536-byte transaction projection. Unsupported platforms fail
closed through the existing stores.
