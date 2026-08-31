# Candidate symbol diagnostics v1

Status: bounded integration and regression cases authored; unrun and unverified.

Audience: semantic agent authors, embedding hosts, and compiler maintainers.

The additive v5 query `candidate/symbol-diagnostics` associates an existing
admitted candidate symbol with rejected intentions retained in the current
session. It does not insert diagnostics into a checked image, manufacture an
invalid candidate, or change existing image facets/handles, v4 methods, attempt
report bytes, repair semantics, or source authority.

## Selection and provenance

Requests bind `image_revision`, `candidate_revision`, and `target`, with optional
`offset`, `chunk_bytes`, and `expected_report_revision`. The selected target must
exist in the admitted candidate. An attempt matches only if its exact retained
predecessor candidate digest and its exact intention `target` both match. A
different candidate history or another symbol's rejection is not included.
The host must select the existing diagnostic capability; no request grants it.

The canonical aggregate report has schema
`semaprax.project-candidate-symbol-diagnostics.v1`. It binds the current image
and Project revision separately from the selected candidate and its Project
revision. Target provenance identifies the stable ID, kind, identity origin,
owner, source path/module, source revision, and source digest from the admitted
candidate's semantic index. Historical candidate provenance never becomes a
claim that its source is the current filesystem revision.

Each matching entry, produced by the additive library method
`ProjectCandidateAttempt::symbol_diagnostics(expected_attempt,
expected_candidate, target)`, uses schema
`semaprax.project-candidate-symbol-diagnostic-attempt.v1`. It binds its immutable
attempt revision and predecessor, carries the same verified target provenance,
and provides all ordered diagnostic indices, codes, and severities. Exact full
messages, paths, spans, and help remain accessible through `attempt/query` at
those indices. They are not duplicated or silently shortened in this compact
query.

The association is **the rejected intention's target, not diagnostic
causality**. An error may describe a constructor or cross-file failed candidate
operation. Its span cannot be resolved as a verified predecessor expression.
Entries remain `state: rejected`, `checked_image: false`, and
`materializable: false`; no invalid source or HIR is exposed.

An admitted candidate has no retained diagnostic/warning inventory in this API.
The report says `candidate_diagnostic_inventory: not_retained`. An empty
`attempts` array means `no_matching_retained_rejected_attempts`, never absence
of every diagnostic, warning, or potential issue. Refresh clears session
attempts under the existing lifecycle; they are not persisted or remapped onto
the new image.

## Actual repair facts and bounds

Each matched entry embeds the existing exact `repair_catalog` result. Available
repair classes are not guessed from an error code or a target type. Discovery
uses the compiler-derived integer-literal retag or direct owned-byte field
borrow proposal and requires
normal full candidate admission before advertising it. Unsupported cases keep
the existing empty repair array and explicit availability reason. Querying does
not select a repair or create a new registry candidate; explicit existing
`attempt/repair-apply` or the admitted `repair_diagnostic` intention is still
required. Tests are not run by discovery.

The aggregation considers at most 16 retained attempts and allows at most four
matching repair catalogue evaluations, each with at most one full candidate
apply. A fifth match rejects with `SPX-G242` **before any repair discovery**;
clients can inspect individual attempts instead. The complete canonical report
is bounded to 1 MiB, including its final LF. Diagnostic and individual attempt
bounds remain unchanged. No partial report or silently truncated attempt list
is returned. Work counts and their upper bounds are explicit; this is not a
constant-time or measured-low-latency query.

## Independent single-attempt replay

The library-only `ProjectCandidateAttempt::verify_symbol_diagnostics` accepts
the expected attempt, predecessor candidate, target, and exact single-attempt
report bytes (at most 2 MiB). It replays the predecessor's complete intention
history from its retained admitted original base, reapplies the exact rejected
intention, checks the reproduced attempt digest, regenerates actual repair
discovery, and compares every report byte including the terminal LF. Its
`semaprax.project-candidate-symbol-diagnostic-verification.v1` receipt binds a
domain-separated report digest and states `source_authority: false` and
`execution: false`. It neither authenticates current filesystem paths nor
replays a session aggregate inventory. No new RPC verifier is exposed.

## Mutable-session chunk binding

Responses use `semaprax.image-symbol-diagnostics-chunk.v1` and carry the report
schema, image/candidate/target selectors, `report_revision`, byte offset, total
bytes, chunk text, and nullable next offset. Chunk sizes are 1,024–65,536 bytes
and boundaries preserve UTF-8. The report revision hashes the exact compact
canonical JSON report plus its single terminal LF with domain
`semaprax.project-candidate-symbol-diagnostics.report.v1`, one NUL, and a
little-endian `u64` byte length. The report itself has no circular self-digest.

Offset zero may omit `expected_report_revision`; every nonzero offset must
supply the revision returned by the first chunk. When supplied at any offset,
the expected revision must match. Adding/discarding attempts, clearing them on
refresh, or changing included work facts can change the report. A continuation
then rejects with `SPX-G243` instead of mixing snapshots. Each request recomputes
the bounded report and its actual repair availability; continuations can repeat
the same bounded admission work. No new mutable report registry is introduced.

The existing v5 held-source pre/post authentication and render-before-mutation
boundary applies. This query always returns `Mutation::None`; digest selectors
confer no repair, source, test, build, Git, or persistence authority.

`SPX-G241` covers invalid target/offset/chunk selection, `SPX-G242` aggregation
capacity, and `SPX-G243` mismatched attempt association or stale/missing report
continuation bindings. Existing stale image/candidate and compiler diagnostics
remain unchanged.

`tests/image_symbol_diagnostics_v1.rs` authors exact predecessor/target matching,
actual supported and unsupported repair facts, empty-scope nonclaims, absent
symbols and missing host grant, continuation invalidation, refresh clearing,
the fifth-match capacity failure, and single-attempt replay with changed bytes,
extra LF, and wrong predecessor rejection. All tests remain unrun. This closes a
bounded discovery connection, not general diagnostic localization, general
repair synthesis, executed validation evidence, or full-goal completion.
