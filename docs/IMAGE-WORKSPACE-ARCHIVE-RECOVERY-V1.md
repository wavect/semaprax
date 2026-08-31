# Image Workspace Archive Recovery v1

Status: Partial; implementation and focused regression evidence authored, unrun.

Audience: workspace hosts, agent session integrators, and compiler contributors.

The v5 host can retain a historical candidate from a self-contained Project
Candidate Archive before accepting its first frame. Recovery restores checked
in-memory candidate state. It does not restore source publication authority,
Git approvals or receipts, drafts, attempts, filesystem roots, or session policy.
Existing frame-based recovery capsules and v1–v4 transports remain unchanged.

## Host API

`VNextSession::restore_candidate_archive(bytes, expected_archive,
expected_candidate)` returns ordinary `semaprax.image-candidate-handle.v1` JSON.
The host must already have selected `candidate_prepare`; restoration is rejected
after the first nonempty frame, including invalid frames and notifications, or
after the session becomes terminal. Empty ignored frames do not start a session.

Restoration authenticates the live host-bound Project snapshot, independently
rebuilds the archive's original canonical manifest and sources, replays its
candidate history, and checks exact archive/candidate expectations. It then
requires the same canonical Project manifest as the live session and obtains
ordinary bounded registry admission. The second live snapshot authentication
must succeed before insertion. Errors preserve the candidate registry; observed
physical source drift invalidates the held snapshot through its existing
absorbing authentication rule.

`retain_archived_candidate(candidate, expected_candidate)` provides the same
startup-only installation for an opaque compiler-created `ProjectCandidate`,
including one returned by the independently replaying archive store. It does
not accept serialized AST/HIR or claim archive provenance for arbitrary typed
candidates. Hosts must use the archive/store verifier to load serialized state.
Both paths share the exact same expectation, manifest, live authentication, and
registry admission checks. Duplicate candidate identities reuse their existing
entry; the existing limit is 16 candidates and 256 MiB aggregate retained reports.

`export_candidate_archive(expected_image, expected_candidate)` returns a
`ProjectCandidateArchive`. Export requires the candidate-preparation policy,
the current exact image expectation, a retained candidate, and a nonterminal
session. It works before or after requests and authenticates live source before
and after archive construction. Export itself does not write an archive or any
source file; the host explicitly owns persistence.

## Historical sources and authority

The archived base source revision may differ from current live source. A restart
in an identical-manifest sibling checkout can therefore recover work after manual
edits without guessing or adopting the old source as current. The live image and
its digest remain unchanged. Historical candidates remain queryable and can be
explicitly rebased onto a newly opened current candidate. No automatic rebase,
merge, approval restoration, or source replacement occurs.

Existing source commit checks still require a candidate based on the current
held source revision and a separate startup-selected Git host and approval.
Archive retention grants neither. The host APIs introduce no RPC archive bytes,
request-selected filesystem root, background scanning, or ambient persistence.
The self-contained archive owns its byte bounds, source checks, and independent
replay; the session adds lifecycle, authentication, and registry bounds.

The additive [Draft Archive](PROJECT-CANDIDATE-DRAFT-ARCHIVE-V1.md) introduces
separate host `restore_draft_archive` and `export_draft_archive` methods. The
historical restore keeps this startup-only, same-manifest fence and installs
only a draft. Its distinct in-session RPC accepts only the current original
base; completing a ready draft cannot bypass historical-candidate startup
admission. Neither mechanism restores approvals or changes the live image.

## Diagnostics and evidence

`SPX-G303` rejects lifecycle/policy misuse and a foreign canonical Project
manifest. Existing archive grammar/capacity/replay diagnostics remain intact;
`SPX-G224` covers unknown or mismatched candidate expectations, `SPX-G282` stale
image expectations, and `SPX-G223` registry admission. Live Project drift retains
its ordinary diagnostic and absorbing behavior. Explicit refresh can recover
the live snapshot; it does not make startup-only restoration available again.

`tests/image_workspace_archive_recovery_v1.rs` authors sibling-root recovery after
the original root is removed, historical query and explicit rebase, startup and
read-only denials, tamper and typed expectation failures preserving registry
contents, live drift rejection, explicit refresh, and raw source byte preservation.
These tests were not run. No compiler, test, interpreter, generated client, or
target executable was run for this tranche. Full durable workspace orchestration
and universal source/publication recovery remain outside this foundation.
