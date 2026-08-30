# Workspace session host-policy CLI

Status: authored, unrun; no live protocol or Git publication test was executed.
Audience: local embedding hosts and semantic agent client authors.

```text
semaprax serve-workspace <manifest> <host-policy.json>
```

This command opens Image Agent Protocol v5 over bounded NDJSON stdin/stdout.
There is no startup banner. The trusted host selects the exact manifest and
reads one regular bounded policy file before requests begin. Relative manifest
paths are resolved against the host working directory; normal Project path
authentication still rejects aliases and unauthorized source shapes.

The policy is a closed JSON object, at most 64 KiB. A read-only example is:

```json
{
  "schema": "semaprax.workspace-host-policy.v1",
  "candidate_prepare": false,
  "diagnostics": false,
  "build_enabled": false,
  "test_policy": null,
  "git_commit": null
}
```

All v1 fields are required. Unknown fields reject. Diagnostics, testing, building
and committing require `candidate_prepare: true`. These flags are host choices,
not request arguments. `test_policy` is either null or the exact object
`{"max_steps":100000,"max_execution_bytes":65536,"max_report_bytes":262144}`;
values must pass the existing bounded CandidateTestPolicy constructor.
`build_enabled` grants only pathless compiler carrier generation and replay, not
filesystem artifact materialization, a native toolchain, package installation
or target execution.

The additive `semaprax.workspace-host-policy.v2` requires the same fields plus
`frontend_cache`, a boolean. `false` keeps the cold path. `true` selects
`VNextSession::open_with_frontend_cache` before any request and retains
compiler-created source ASTs for authenticated live refresh. V1 remains closed:
adding `frontend_cache` to a v1 policy rejects rather than silently enabling it.
Missing, null, string, or numeric cache selections in v2 also reject.

This selection changes frontend work only. It grants no methods, paths, store,
process, or publication authority; no request can turn it on or off. Cache hits
still require exact source bytes and complete semantic/link/profile admission.
There is no serialized HIR loading, cross-process warm reuse, filesystem cache
root, or measured speedup claim. See
[Workspace Frontend Cache v1](IMAGE-WORKSPACE-FRONTEND-CACHE-V1.md) for fresh
snapshot authentication, transactional cache adoption, and actual work reports.

Policy `semaprax.workspace-host-policy.v3` requires the v2 fields plus
`candidate_archives`, a bounded array of explicit host-selected immutable store
locators. Startup loads their complete source-backed candidates before frames
and before opening a Git provider. V1/v2 reject this added field. Recovery
retains historical candidates without replacing the live image, restoring
approvals or publishing source; it requires the candidate grant and the same
canonical manifest. See [Candidate Archive CLI v1](CANDIDATE-ARCHIVE-CLI-V1.md)
for exact fields, limits and required explicit rebase.

`git_commit` is null or a closed object containing `git_executable`, `repository`,
`reference`, `base_commit`, `project_prefix`, `author_name`, `author_email`,
`unix_seconds`, `message`, `max_commands`, `timeout_ms` and
`approved_candidate_digest`. Git policy values retain the requirements of
[Candidate Git Publication CLI v1](CANDIDATE-GIT-PUBLICATION-CLI-V1.md), including
an absolute trusted executable and bounded bare SHA1/SHA256 repository. The
approval digest is supplied independently by the host, never read from an RPC
or silently inferred from a candidate capsule. `source-commit/status` exposes a
public correlation handle for that existing approval; it does not grant one.

The supported CLI workflow is to prepare, inspect and export a candidate first,
then start a separate commit-enabled session with the exact host-approved
candidate digest, restore its exact history and invoke `candidate/commit` with
the retained candidate and approval handles. V5 host approvals remain restricted
to startup; requests cannot approve themselves or replace the fixed Git policy.
The process provider's existing lifetime deadline and command/I/O limits start
when it opens, so publication must occur within that bounded window. There is
no automatic deadline reset, repository replacement or authority refresh.

Git objects/ref publication does not rewrite raw checked-out source. The
existing independently replayed Git authority owns its one ref pivot, consumes
the selected approval on an attempted publication, and reports uncertain
post-pivot outcomes explicitly. Do not blindly retry uncertain publication.
Source/approval/candidate errors do not create a new approval or widen policy.

`tests/workspace_session_cli_v1.rs` authors host selection, NDJSON framing,
request-elevation rejection, and invalid closed-policy checks.
CLI cache-policy regressions preserve v1 rejection, compare cold/cached discovery
and image identities, and reject invalid v2 cache selections and RPC overrides.
Existing complete CLI help preservation retains the additive command line in its explicit
normalization list. No tests, client snippets, compiler gates or Git publication
commands were run for this implementation.
