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

`semaprax serve-workspace-mcp <manifest> <host-policy.json>` uses the same
startup loader and every closed policy version below, then wraps the configured
session in the optional [MCP stdio adapter](IMAGE-MCP-ADAPTER-V1.md). It changes
framing and discovery only; client capabilities cannot select another policy,
manifest, archive root or approval. The ordinary `serve-workspace` NDJSON wire
and argument contract remain unchanged.

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

The v2 AST-cache selection changes frontend work only. It grants no methods, paths, store,
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

Policy `semaprax.workspace-host-policy.v4` requires every v3 field plus the
required boolean `semantic_cache`. `true` requires `frontend_cache: true` and
selects `VNextSession::open_with_semantic_cache` before the first request. A
complete read-only example is:

```json
{
  "schema": "semaprax.workspace-host-policy.v4",
  "candidate_prepare": false,
  "diagnostics": false,
  "build_enabled": false,
  "test_policy": null,
  "git_commit": null,
  "frontend_cache": true,
  "candidate_archives": [],
  "semantic_cache": true
}
```

With both cache flags false the session remains cold. With only `frontend_cache`
true it retains the unchanged AST-only cache behavior. Missing, null, numeric,
string, or object `semantic_cache` values reject, as does enabling it while
disabling `frontend_cache`. V1, v2, and v3 remain closed and reject this field
even when false. Archive selectors retain all v3 validation and admission rules.

This additional selection reuses only compiler-created checked module HIR under
exact source, context, dependency, and complete synthetic-AST matching. It still
requires fresh filesystem/source authentication, full cross-file checks,
linking, and Project profile admission. It grants no methods, cache-root path,
file writes, build/test authority, or source approval. Refresh forks cache state;
preview and failure discard the fork, and only successfully rendered and finally
authenticated refresh adopts it. Neither requests nor recovered archives can
select the strategy. The existing Git startup-only approval guard is unchanged.

Semantic-cache refresh work uses the separate
`semaprax.project-semantic-cache-work.v1` schema with actual resolver-call and
checked-HIR reuse counts. AST-only work keeps its old schema and constant zero
checked-HIR hits; cold responses omit work accounting. Image identity and
authority discovery are unchanged by the cache flags. See
[Project Semantic Cache v1](PROJECT-SEMANTIC-CACHE-V1.md). This is not a
serialized-HIR loader, cross-process cache, general incremental verification,
backend shortcut, or measured performance claim.

Policy `semaprax.workspace-host-policy.v5` adds the required
`semantic_cache_entry` field to every v4 field. It is either null (the unchanged
v4 strategy) or exactly `{"root":"/absolute/private/root","entry_digest":"sha256:..."}`.
A selected entry requires both cache booleans true. V1–v4 reject this new field.
The root must already have been initialized through `semantic-cache-init` and
contain an entry written through `semantic-cache-persist`; neither startup nor
an RPC creates keys or writes cache entries. The separate store verifies the
MAC and current compiler-file binding before decoding, then the session opens
the fixed live manifest through ordinary source authentication. Source changes
invalidate affected restored entries. Bad keys, incompatible compiler files,
corruption or invalid current sources fail startup without widening authority.
There is no implicit fallback that labels a cold load as warm. Null entry or
older policies remain available for an explicit cold rebuild.

`VNextSession::retained_semantic_cache` is an embedding-host API for obtaining an
opaque historical cache through the live source boundary. It does not itself
write storage or change the startup-only Git approval guard. See
[Persistent Cache v1](PERSISTENT-SEMANTIC-CACHE-V1.md) and
[Cache Store v1](SEMANTIC-CACHE-STORE-V1.md) for the explicit trusted-host
requirements; compiler-file hashing is not loaded-code attestation.

Policy `semaprax.workspace-host-policy.v6` adds the required `draft_archives`
array to every v5 field. Each entry is exactly `root`, `archive_digest` and
`draft_digest`; at most sixteen unique drafts may be selected, and nonempty
selection requires candidate preparation. Startup loads these source-backed
archives through the explicit immutable store, authenticates the same canonical
manifest and retains only drafts before frames and before opening a Git provider.
The live source image remains current even when recovered drafts are historical.
V1–v5 reject this field. Store roots are never selected by RPC, and no approvals
or publication state are recovered. See [Typed-draft persistence](DRAFT-ARCHIVE-PERSISTENCE-V1.md)
for commands, exact bounds and authored/unrun regression evidence.

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
`tests/workspace_session_semantic_cache_cli_v1.rs` adds explicit v4 warm
checked-module reuse, cold/AST/semantic identity and authority equivalence, older
policy rejection, strict/dependent boolean selection, and RPC override rejection.
It also authors direct semantic-session source-drift recovery and verifies that
preview and failed expected-revision checks do not prime the retained cache.
Existing complete CLI help preservation retains the additive command line in its explicit
normalization list. No tests, client snippets, compiler gates or Git publication
commands were run for this implementation.
