# Image Source Commit Protocol v5

Audience: trusted host integrators and agent-client authors.
Status: authored optional v5 extension with unrun regression cases. No compiler,
Git adapter process, test execution or hosted promotion is claimed for this batch.

This extension connects retained complete candidates to the existing real local
[Git publication authority](PROJECT-CANDIDATE-GIT-PUBLICATION-V1.md). Startup
selection and exact independent host approval are required. Read-only and earlier
candidate protocol versions gain no source authority. The extension creates
canonical Git blobs/trees/commit and pivots one fixed bare-repository branch;
original working-tree paths, indexes and managed `ACTIVE` remain unchanged.

## Startup selection and approval

The trusted host creates `GitCommitHost::new(manifest, target, metadata,
Box<dyn CandidateGitAuthority>)`. The fixed absolute manifest, repository/ref,
expected old Git commit, Project prefix, author/committer identity, timestamp and
message are selected outside the request protocol. The supplied process adapter
supports the existing restricted Unix bare SHA1/SHA256 profiles; an injected
provider is independently trusted authority, not request data.

The host calls `approve(candidate_digest)` separately. It accepts one exact
canonical SHA256 candidate digest and returns an `approval_revision`. Approval
lives in one private slot; a request containing the same digest cannot create,
replace, or replenish it. A second approval while one is pending is rejected.
The binding is a deterministic, domain-separated digest over the host manifest,
repository identity, candidate digest and monotonically increasing local approval
sequence. It is public correlation, not a secret credential or a signature. The
private pending slot and independently created host are the authority boundary.

`VNextSession::with_git_commit_host` attaches the host before any request. The
framework also exposes a guarded startup-only `approve_git_commit`; no RPC
selects or modifies policy, adds the `source_commit` capability or grants approval.
A session without the startup host does not advertise these methods. A client
can discover the already-approved candidate and binding through host status,
but discovery itself grants nothing.

The supported review workflow uses two sessions: prepare/review/export a complete
candidate first; then the trusted host independently approves that exact digest
before starting a commit-capable session, which restores the exact capsule and
commits it. Approval cannot be added to an already active protocol session.

The complete approved candidate may be restored or prepared through ordinary
candidate methods. Its digest must equal the separately selected approval; its
original base must equal the live session's held Project revision. Candidate
selection does not admit drafts, rejected attempts, arbitrary source text or a
new path. Applying, changing or repairing an approved candidate creates a new
digest and requires a new independent approval. A pending approval cannot be
replaced by an agent request.

## Closed methods

All methods require the current `image_revision` and appear only in the
startup-selected v5 method catalogue. The same descriptors own admission and
request-schema generation.

| Method | Other parameters | Result |
| --- | --- | --- |
| `candidate/commit` | Required `candidate_revision`, `approval_revision` | Compact `semaprax.image-source-commit-handle.v1` |
| `source-commit/status` | None | `semaprax.image-source-commit-status.v1` |
| `candidate/commit-report` | Required `report_revision`; optional `offset`, `chunk_bytes` | `semaprax.image-source-commit-report-chunk.v1` |

Commit parameters contain no repository, ref, path, metadata, approval boolean,
capability toggle or process option. The capability catalogue labels the extension
`source_commit`; a request cannot elevate a different host profile.

Status reports host publication state, the optional already-granted approval,
retained receipt handle and bounded last diagnostic codes. It is a host-state
observation, not a fresh claim about Project inputs or Git repository contents.
Receipt queries return historical publication evidence with
`current_source_admission:false`. These observations remain useful after the
publication host becomes terminal; they do not re-enable mutation.

## Publication and failure boundary

The framework first authenticates its held inputs, resolves the candidate and
checks its original base. That read-only boundary finishes **before** calling
the publication host. The host requires both the selected candidate and the
pending approval binding to match, then consumes approval immediately before
invoking `apply_candidate_git_publication`.

The existing library freshly loads/authenticates all original Project inputs,
independently replays the full approved candidate, authenticates the selected
Git base tree, prepares bounded immutable objects and receipt, and performs one
expected-old ref update. It owns final input checks and uncertain-pivot
classification. The transport must not surround this call with an ordinary
post-request source check that could turn an already-published or uncertain
result into a misleading ordinary read failure.

Approval is consumed on every invocation of that publication API, including a
definite pre-pivot error. A malformed request, mismatched approval or failed
initial held-input selection does not invoke publication. Successful publication
makes the host terminal. `SPX-G267` also makes it terminal: inspect the fixed ref
and prepared commit from the diagnostic; never retry blindly. Other pre-pivot
failures leave its host state available but without approval. The v5 framework
currently requires approval before the first request, so a further session attempt
requires newly configured startup authority; direct trusted-host library users
can explicitly reapprove an available host. An agent request cannot. No automatic
retry or rollback is performed.

The underlying host provider retains its own lifetime limits. In particular,
the supplied Git process adapter's deadline starts when that adapter is opened,
not when a review finishes. Startup approval and publication must occur
within that provider window. Out-of-band approval never resets its deadline or
silently renews held filesystem/process authority. An expired provider requires
a newly and explicitly configured host/session; long-lived interactive Git
publication beyond this fixed window is not claimed.

Publishing into a bare repository does not change the original raw Project
sources. Nevertheless, the old branch base is fixed, and the one-shot success
state forbids a second publication from that host. Refreshing a session image
cannot revive consumed approval or a terminal publication host.

## Bounded replies and retained receipts

A successful library receipt may be up to 1 MiB. It is retained in the host and
bound by `report_revision`, a SHA256 digest of its exact bytes under a dedicated
receipt domain. The immediate commit response contains only fixed-size digest
handles, byte length, state and receipt-method name. It never embeds the full
receipt: JSON string escaping must not overflow a response after authority has
already pivoted. An unexpected library receipt-size violation is classified as
`SPX-G267`, not as an ordinary failed publication.

Only one receipt is retained. Report chunks are 1–32,768 source bytes, default
16,384; offsets must select UTF-8 boundaries and output ends at a complete code
point. The framework's ordinary response bound remains in force, and receipt
chunks fit after JSON escaping. Status retains at most eight diagnostic codes
of at most 32 characters each. Approval storage is one candidate/binding pair.
These are report/state bounds, not a total retained HIR memory claim.

`SPX-G284` rejects invalid host or request shape, `SPX-G285` reports extension
bounds, `SPX-G286` rejects absent/stale approval or receipt selection, and
`SPX-G287` rejects terminal publication reuse. Existing source/candidate and
Git `SPX-G263`–`SPX-G267` diagnostics are preserved.

## Authored evidence and limits

`src/image_transport/vnext/commit/tests.rs` contains injected-authority cases for
request self-approval rejection, preservation of an independently pending slot,
one-shot success and receipt retrieval, unchanged raw source, consumed approval
on definite preflight failure, and terminal state after a simulated actual pivot
whose acknowledgment is lost. An additional v5 frame-level scenario checks
startup-only capability/approval, exact capsule restore, commit and historical
status after source drift. They are authored and unrun. The independent
Git library's real bare-repository regressions remain unrun in this batch too.

This extension does not implement interactive RPC approval, signed approval
services, remote push, checked-out branch updates, arbitrary process execution,
source editing, draft publication, build/artifact publication, or atomic raw
multi-file filesystem writes. Git process trust, SHA1 compatibility nonclaims,
cooperative filesystem-race limits and uncertain outcomes remain exactly those
of the underlying publication authority.
