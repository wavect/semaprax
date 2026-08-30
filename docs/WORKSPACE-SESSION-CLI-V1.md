# Workspace session host-policy CLI v1

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

All fields are required. Unknown fields reject. Diagnostics, testing, building
and committing require `candidate_prepare: true`. These flags are host choices,
not request arguments. `test_policy` is either null or the exact object
`{"max_steps":100000,"max_execution_bytes":65536,"max_report_bytes":262144}`;
values must pass the existing bounded CandidateTestPolicy constructor.
`build_enabled` grants only pathless compiler carrier generation and replay, not
filesystem artifact materialization, a native toolchain, package installation
or target execution.

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
request-elevation rejection, and invalid closed-policy checks. Existing complete
CLI help preservation retains the additive command line in its explicit
normalization list. No tests, client snippets, compiler gates or Git publication
commands were run for this implementation.
