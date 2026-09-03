# Semantic retention checkpoint v1

Status: additive authority-neutral lifecycle planner authored; compile-only
validation performed. Filesystem cleanup and integrated recovery evidence remain
open, so the Graph-Operational Programme remains Partial.

Audience: embedding hosts, persistent workspace integrators and compiler
contributors.

## Scope and authority boundary

`semantic_retention` provides one deterministic lifecycle policy for disposable
Semantic Workspace Image, complete-candidate archive and incomplete-draft
archive identities. It records no source, HIR, graph, archive, approval or store
root. It cannot read a clock, inspect a filesystem, validate a candidate, make a
historical subject current or publish source.

Each subject is a closed typed identity:

- an image binds the image digest, Project Revision Store entry digest and
  Project revision;
- a candidate binds its archive digest, candidate digest and base Project
  revision;
- a draft binds its archive digest, draft digest and base Project revision.

All identities use canonical lowercase SHA-256 syntax. A subject digest binds
the complete typed identity under
`semaprax.semantic-retention-subject.digest.v1\0`; it is metadata identity, not
a locator, capability or existence proof. Every later use must invoke the
ordinary image/revision-store or candidate/draft archive loader and perform its
complete source/history replay.

## Deterministic policy

`RetentionPolicy::new(max_subjects, max_bytes, protected_generations)` admits:

- 1 through 96 retained subjects;
- 1 through 8 GiB of declared stored bytes;
- 0 through 32 protected generations.

One subject declares 1 through 128 MiB. Accounting is an exact policy input,
not observed filesystem size. Re-observing the same immutable identity with a
different byte count fails closed.

`checkpoint` takes an optional exact predecessor checkpoint and digest, the
next consecutive nonzero sequence, a policy and at most 96 observations. It
merges at most 96 old survivors and 96 new observations, rejects repeated
subjects, updates only exact identities and ranks entries by:

1. most recent observed sequence;
2. most recent first-observed sequence; and
3. ascending subject digest.

Entries observed within the protected generation window must fit. The
transition rejects rather than silently evicting one when the policy cannot
hold all protected entries. Remaining entries are retained in rank order while
both caps fit; skipped entries become the exact GC plan. Checkpoint entries and
evictions are emitted in ascending subject-digest order, independent of input
order, wall time, mtime or access frequency.

Changing policy is explicit in the next checkpoint. It does not reinterpret an
old checkpoint or mutate a store.

## Durable metadata recovery

The checkpoint schema is `semaprax.semantic-retention-checkpoint.v1`. It binds:

- the consecutive sequence and exact predecessor checkpoint digest;
- the complete selected policy;
- every survivor's typed identity, subject digest, fixed stored bytes and
  first/last observed generation;
- total retained bytes; and
- the closed authority nonclaims.

Canonical compact JSON has one terminal LF and a 1 MiB cap. Its digest uses
`semaprax.semantic-retention-checkpoint.digest.v1\0`, the `u64_le` byte length
and exact bytes.

The companion `semaprax.semantic-retention-plan.v1` binds the predecessor,
result checkpoint, sequence, retained counts, and every exact eviction. Its
digest uses the same length-delimited construction and the distinct
`semaprax.semantic-retention-plan.digest.v1\0` domain.

A durable host writes both exact outputs before attempting cleanup. On restart,
`restore_checkpoint` requires the independently retained expected checkpoint
and expected predecessor digests. `restore_plan` requires the authenticated
checkpoint and independently retained plan digest. Both require canonical exact
bytes. This rejects accidental rollback, cross-checkpoint plan substitution,
reordered or reminted metadata and altered accounting when the host preserves
those expected selectors.

The APIs do not choose where or how metadata is durably stored. Retaining an
old selector is host responsibility; checkpoint bytes alone cannot establish
that they are the newest bytes ever created.

Restored checkpoints and plans expose `authority()` as the closed
`RetentionAuthority::None` value. This makes the absence of action authority an
API fact; no variant can name a store, mutate source, approve or publish.

## Applying garbage collection

The returned plan has effect `none_metadata_plan_only`. It performs no delete,
adoption, overwrite, repair or store discovery. A host may apply its exact
subjects only through a separately selected store authority after durably
settling the checkpoint and plan. Store-specific code must reauthenticate the
selected root, current entry identity and cooperative lock and must own
post-effect uncertainty. Missing entries, failed stages and foreign inventory
are not silently treated as successful cleanup by this module.

This ordering permits recovery of a pending plan after interruption without
turning checkpoint restoration into authority. Completing cleanup does not
make any survivor fresh, current, approved or publishable.

## Diagnostics and evidence status

| Diagnostic | Meaning |
| --- | --- |
| `SPX-G420` | Malformed policy, subject, checkpoint or plan grammar. |
| `SPX-G421` | Fixed subject, inventory, byte, sequence or output bound exceeded. |
| `SPX-G422` | Immutable identity, accounting, canonical bytes or plan/checkpoint binding disagrees. |
| `SPX-G423` | Expected predecessor, sequence or companion plan is stale. |

Authored unit regressions cover input-order determinism and eviction order,
protected-capacity failure, stale predecessor and rollback selectors, tampered
checkpoint/plan rejection, and the explicit no-authority API result. They were
not executed. The library target compiled locked and offline after authoring;
tests and long quality gates were intentionally not run. Required follow-up
evidence includes store-specific idempotent cleanup, interruption at every
durable/effect boundary, image/candidate/draft replay after survival, absence
after cleanup, parallel-coordinator serialization, measured checkpoint/recovery
cost and hosted execution. Cross-platform filesystem support remains owned by
the underlying stores.
