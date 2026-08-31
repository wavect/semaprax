# Image Retained Subjects v1

Status: additive implementation and regression sources authored, **unrun**.

Audience: embedding hosts and agents managing bounded v5 candidate-session
state.

`workspace/retained-subjects` is a compact inventory of handles currently held
by one live v5 session. It answers which candidates, drafts and
rejected attempts can still be addressed without probing every possible
digest. It does not serialize their full reports, revive discarded state or
make registry membership evidence of semantic validity.

## Selection and response

The route is selected only with the existing `candidate_prepare` startup grant.
Its sole parameter is the exact current `image_revision`; requests cannot
enable retention or change registry limits. Default sessions do not expose the
method. The closed response schema is `semaprax.image-retained-subjects.v1` and
is bounded to 65,536 bytes.

The root contains `schema`, `image_revision`, sorted `candidates`, `drafts` and
`attempts`, aggregate `retained_report_bytes`, fixed `limits`, four false
authority fields, and `nonclaims`. Limits disclose the existing maxima of 16
candidates, 16 drafts and 16 attempts, 268,435,456 retained report bytes, and
the 65,536-byte inventory bound. Rows are ordered lexically by their digest
keys, independent of creation order.

Candidate rows retain `candidate_revision`, final and base Project revisions,
the capacity-accounted retained report byte count, and existing detail/discard
method names. `has_retained_drafts` and `has_retained_attempts` are live registry-local
associations only. They do not say that a candidate is valid for a new action,
that all historical descendants are known, or that any source remains current.

Draft rows identify the draft, its source candidate, whether that candidate is
still a member of this same registry, capacity-accounted retained bytes, state
`incomplete` or `ready_to_complete`, unresolved-hole count, and existing
detail/discard methods. An orphaned source handle does not invalidate, complete or publish a retained
draft. Attempt rows similarly identify the rejected attempt, base candidate and
live-membership flag, base Project revision, diagnostic count, retained report
bytes, state `rejected`, and existing detail/discard methods. A rejected attempt
is never a checked candidate.

Retained byte counts are registry capacity accounting and can include a
retained predecessor. They do not predict detail-route payload or chunk size,
are not report digests, and do not authenticate cached bytes. Named methods are
navigation only: clients must call them with the exact handle and follow each
method's independent binding, chunking and stale-state contract.

## Lifecycle and authority

Opening or applying a candidate, opening a hole, and retaining a rejected
attempt add their ordinary handles. Explicit discard removes only the selected
subject. Discarding a candidate can leave a draft or attempt whose source/base
membership flag becomes false. No relationship is inferred across sessions.

`workspace/refresh` retains candidates under its existing contract and clears
drafts and attempts. A subsequent inventory binds the new image and reports
only the surviving live handles. Live source drift fails at the ordinary host
boundary and releases no inventory; the query does not bypass or repair an
absorbing session failure.

`source_authority`, `artifact_materialization`, `execution` and
`publication_authority` are false. The inventory does not expose or mutate
source; ordinary live-session
authentication still reads and checks exact source bytes. It runs no tests or
targets, applies no repair, completes no draft, commits, publishes, archives,
or grants approval. Omission is not proof that a subject never existed, was not archived elsewhere,
or is absent from another process or session. Membership is not liveness,
ownership, semantic compatibility, source freshness or external persistence.

Although the method is a read, it is intentionally excluded from
`workspace/read-batch`: it observes mutable session registry state rather than
an immutable detached subject. Generated TypeScript, Python and Rust clients
cover the closed request and response. MCP exposes the selected method as
`workspace__retained-subjects` without adding authority.

Authored, unrun evidence in `tests/image_retained_subjects_v5.rs` covers an
empty selected registry; candidate open/apply, draft and rejected-attempt
retention; deterministic order and fixed caps; registry-local association and
orphan flags; explicit discard; refresh clearing and candidate survival;
selected schemas and generated clients; MCP; batch rejection; live drift;
false authority and unchanged source. No tests, compiler executable, target or
application was run while authoring this tranche.
