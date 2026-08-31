# Image Workspace Protocol v5

Audience: embedding hosts, agent authors, protocol implementers, and reviewers.

Status: implementation and executable regression cases authored, **unrun**.
The user's instruction deliberately skips tests, compiler/interpreter/target
execution, and long local quality gates. This is not verified completion,
performance, publication, or complete-programme evidence.

V5 adds a host-configured session and explicit live-source refresh without
changing v1–v4 method lists or response bytes. Its envelope is
`semaprax.image-agent-result.v5`, with protocol
`semaprax.image-agent-protocol.v5`, exact `image_revision`, exact
`project_revision`, and a method-specific `payload`. Existing non-discovery
semantic payload schemas remain their owned versions. Discovery descriptions,
schemas, and generated clients derive from the methods actually enabled by the
fixed host policy; user payload strings are never rewritten to upgrade schemas.

## Host configuration

```rust
VNextSession::open(&absolute_manifest, VNextPolicy {
    candidate_prepare: true,
    diagnostics: true,
    test_policy: None,
    build_enabled: false,
})
```

The absolute manifest must equal its authenticated canonical Project path.
Defaults are semantic reads and explicit refresh only. Diagnostics, interpreted
tests, and pathless builds additionally require `candidate_prepare`; tests use
the host's existing fixed `CandidateTestPolicy`. No request changes these
booleans or limits. `serve_vnext(input, output, session)` accepts host-provided
streams and the already configured session; it discovers no paths or authority.

An embedding host can also attach an independently verified immutable package
graph before requests. This selects `package/summary` and `package/consumers`
without widening any execution, candidate or publication grant. They describe
an independent package subject, not a relationship to the current Project.
No RPC accepts package input paths or attaches/replaces a graph. See
[Package Semantic Graph](PACKAGE-SEMANTIC-GRAPH-V1.md).

The optional [MCP adapter](IMAGE-MCP-ADAPTER-V1.md) consumes that configured
session and exposes its selected methods as tools. It preserves exact v5
arguments and response bytes, requires a separate MCP initialization handshake,
and cannot add a grant or approve a candidate. Its larger outer response bound
accounts for escaping a complete v5 response before any operation runs.

An additive `open_with_frontend_cache` constructor keeps the same policy and
image identity while reusing source-exact compiler ASTs during authenticated
live refresh. The separate embedding-host `handle_read_batch` API runs selected
immutable image/discovery and retained semantic reads on bounded scoped workers.
Neither changes the
ordinary sequential NDJSON loop or grants new methods. See
[Live Frontend Cache](IMAGE-WORKSPACE-FRONTEND-CACHE-V1.md) and
[Parallel Reads](IMAGE-PARALLEL-READS-V1.md) and
[Parallel Retained Reads](IMAGE-PARALLEL-CANDIDATE-READS-V1.md) for their explicit
host choices and detached worker inputs.

The default read-only method set also includes `image/dependencies`: exact-image,
stable-ID declaration dependency reports delivered in bounded UTF-8 chunks.
It shares the immutable image index with candidate deltas and the host parallel
read path; no candidate grant or publication authority is needed or acquired.
See [Declaration Dependencies](SEMANTIC-IMAGE-DEPENDENCIES-V1.md).

`image/analysis-coverage` is another default exact-image semantic read. It
reports retained source facts alongside explicit uninspected deployment,
generated-file, external-API, runtime and consumer boundaries. It accepts no
external paths or execution authority; missing imports never prove missing
external dependencies. See [Analysis Coverage](SEMANTIC-IMAGE-ANALYSIS-COVERAGE-V1.md).

With candidate preparation, `candidate/analysis-coverage` applies that same
closed inventory to one exact fully admitted candidate revision. The outer
response remains bound to the live session image, while the payload image,
Project, graph and source identities describe the candidate. It preserves all
uninspected areas and creates no additional candidate or derived-image entry.
See [Candidate Analysis Coverage](PROJECT-CANDIDATE-ANALYSIS-COVERAGE-V1.md).

Agents can instead request `image/dependency-summary` and expand selected
`image/dependency-page` handles. These structured read-only methods expose
counts and bounded sites/callers/calls/members pages without transferring the
complete report. See [Dependency Navigation](SEMANTIC-IMAGE-DEPENDENCY-NAVIGATION-V1.md).

With candidate preparation, `candidate/dependency-summary` and
`candidate/dependency-page` expose the same compact views over one exact
retained candidate revision. Their handles and cursors bind the candidate, so
base-image and sibling-candidate selectors cannot be reused. The queries derive
and discard an image from the already admitted candidate, create or retain no
additional candidate or derived-image registry entry, and are eligible for
detached authenticated read batches. See
[Candidate Dependency Navigation](PROJECT-CANDIDATE-DEPENDENCY-NAVIGATION-V1.md).

Candidate preparation also selects `candidate/impact-summary` and
`candidate/impact-page`. These methods recompute the existing bounded reverse
impact artifact for the exact candidate and expose its affected, dependency-edge
and frontier inventories through artifact/query-bound handles. Page items use a
self-identifying unbundled envelope whose `value` is the unchanged compiler row.
Truncation and budget evidence remain visible; pagination adds no omitted facts,
authority, execution or completeness. See
[Candidate Impact Navigation](PROJECT-CANDIDATE-IMPACT-NAVIGATION-V1.md).

Startup-only archive handoff can preload complete historical candidates from
independently replayed source-backed archives. It retains the current image and
fixed policy, requires the same canonical manifest, and grants no approvals or
publication authority. No archive/store root is accepted through an RPC frame.
See [Workspace Archive Recovery v1](IMAGE-WORKSPACE-ARCHIVE-RECOVERY-V1.md).

Candidate preparation also selects `hole/recovery-export` and
`hole/recovery-restore` for [Typed-Hole Draft Recovery](PROJECT-CANDIDATE-DRAFT-RECOVERY-V1.md).
These methods recover pending selectors over source-replayed valid history,
retaining only a draft and no authority. Unresolved holes still block completion.
Restore requires the current exact original base and the existing request-frame
limit; it cannot silently rebase a draft after source changes. Refresh continues
to clear drafts, and v1–v4 gain no recovery methods for unfinished work.

Candidate preparation also selects the read-only `candidate/merge-preview`
query. It authenticates two retained candidate selectors, attempts ordinary
semantic merge in both orders and reports directional admission plus exact
resulting source comparison. Temporary merged candidates are not installed in
the registry. Diagnostic excerpts require no rejected-attempt grant and cannot
be used as attempt handles. See [Merge Preview](PROJECT-CANDIDATE-MERGE-PREVIEW-V1.md).

Candidate preparation also selects `candidate/contract-expression-catalog` and
`hole/open-contract-expression`. These expose existing pre/postcondition
subtrees and open [Contract Expression Holes](PROJECT-CANDIDATE-CONTRACT-HOLES-V1.md)
over exact candidate/draft revisions. Existing fill, query, completion and
recovery routes handle these drafts; unresolved contract holes block completion
alongside body holes. Older body-expression methods remain body-only.

Semantic conformance reads and target-admission projections are available
independently of diagnostic permission. Candidate preparation adds current
candidate, expression-hole, interface-discovery, and semantic-delta operations.
Rejected-attempt operations require `diagnostics`. Runtime test execution and
artifact projection have separate host selections. A build projects compiler
artifacts into bounded reports; it does not publish files or execute a native
or Wasm target. Source publication requires a separately attached Git host.

Candidate preparation also grants `candidate/interface-delta`, a read-only,
whole-candidate comparison of static interface declarations and their bound
functions. Diagnostic selection grants `candidate/symbol-diagnostics`, which
associates retained rejected attempts only with their exact predecessor and
intention target. It never attributes a rejected-source span to verified HIR.
Both use bounded report chunks under current image and candidate expectations;
diagnostic continuations additionally require the exact report revision because
the retained attempt inventory can change. Parallel retained reads select an
immutable snapshot of that inventory inside source authentication. Neither
method grants repair application, execution, or publication. Their
report contracts are [Interface Delta](PROJECT-CANDIDATE-INTERFACE-DELTA-V1.md)
and [Symbol Diagnostics](PROJECT-CANDIDATE-SYMBOL-DIAGNOSTICS-V1.md).

Candidate preparation also grants `candidate/contract-delta`, the additive
[Contract Delta](PROJECT-CANDIDATE-CONTRACT-DELTA-V1.md) read. It compares all
contract-bearing functions against the candidate's original base, including
static helper dependencies behind unchanged predicates. It takes no target
selector and returns immutable bounded UTF-8 chunks under exact image/candidate
expectations. It performs no target projection or execution, grants no additional
authority. The parallel retained-read extension admits the same pure handler.

`candidate/ownership-delta` uses the same candidate-preparation grant and bounded
chunk parameters for [Ownership Delta](PROJECT-CANDIDATE-OWNERSHIP-DELTA-V1.md).
It compares checked signatures, structural inventories and complete ordered
loan/cleanup plans across the candidate. It takes no target and gains no target
execution, plan mutation or physical ownership authority.

The existing build grant also enables `candidate/artifact-delta`, which compares
actual base/candidate Web or npm carriers through
[Artifact Delta](PROJECT-CANDIDATE-ARTIFACT-DELTA-V1.md). It replays the candidate
before either build, compares file/export/source bindings, and returns chunks.
The request cannot change the fixed build limit or gain artifact filesystem
materialization. Candidate preparation alone does not enable this method.

The same grant additively selects `candidate/analysis-artifact-evidence`. It
composes exact candidate analysis coverage with one freshly replayed Web, npm,
OpenAPI or C artifact delta and returns the heterogeneous report in bounded
UTF-8 chunks. The closed chunk binds the candidate and kind and includes plain
SHA256 of the exact full report bytes for consistent reassembly. The route is a
build-class operation, is absent without `candidate_build`, and is excluded
from parallel reads and `workspace/read-batch`. Only `generated_artifacts`
becomes partial for the selected pathless carrier; no materialization,
deployment, native compilation, target execution or external-consumer evidence
is inferred.

All available method names, closed request parameters, payload schema references,
capabilities, and generated clients come from the selected catalogue. Optional
parameters are omitted; null is accepted only where the declared schema permits
it. The schema bundle explicitly lists payload schemas that remain opaque and
links bundled constructor schemas. Generated clients only construct/validate
messages; they have no filesystem, network, process, approval, or session-policy
authority.

## Explicit refresh and stale state

After a manual edit, call `workspace/refresh-preview` with the current image
digest. It independently reads the same fixed manifest and returns observed
Project/image revisions without replacing the current image or snapshot,
clearing registries, or reviving an invalidated session. Its v5 envelope remains
bound to the old session image; the
`semaprax.image-workspace-refresh-preview.v1` payload explicitly labels
`observed_project_revision` and `observed_image_revision`. The payload also
records `old_image_revision`, the observed Workspace revision,
`manifest_changed:false`, `current_state_replaced:false`,
`requires_explicit_refresh:true`, and `source_authority:false`.

Use that observed Project revision in the explicit refresh request below. If
sources change again between preview and refresh, the exact expectation fails;
preview is an observation, not permission to accept a different source subject.

```json
{"jsonrpc":"2.0","id":"refresh","method":"workspace/refresh","params":{"image_revision":"sha256:<current-image-hex>","expected_new_project_revision":"sha256:<independently-observed-project-hex>"}}
```

Refresh has no path, source, manifest, policy, or force parameter. The caller
supplies the current image digest and the independently observed new Project
revision. The session loads the one host-bound manifest afresh, requires its
canonical configuration to match the current session, checks the caller's new
revision expectation, and independently derives the image. A manifest/profile,
source-inventory, entry, or export-configuration change requires a new session.

Ordinary requests still authenticate the held current snapshot before preparing
their result and afterward before retaining any mutation. Observed drift is
absorbing for that snapshot; restoring old file text does not implicitly revive
it. Refresh deliberately does not authenticate or revive that old snapshot: it
creates and authenticates a new one. It fully prepares a bounded response and
rechecks the fresh snapshot before replacing current session state. A rejected
refresh leaves the old image, registries, and old snapshot's drift state intact.
Malformed requests, unknown parameters, invalid IDs, notifications, and stale
current-image expectations cannot trigger refresh.

Successful refresh retains complete immutable candidates and their exact
historical bases. No intent is implicitly replayed onto the new source state.
Call `candidate/open` after refresh to retain the current base, then explicitly
`candidate/rebase` a historical candidate onto that new base. Historical
candidate reports remain independently bound to their own subjects; the current
image token is still required on requests. Source commit additionally requires
a candidate based on the current held Project revision.

Every successful explicit refresh clears drafts and rejected attempts, including
an unchanged-image refresh. It never silently remaps expression selections or
repairs rejected intentions. The report lists retained candidate handles and
exact cleared counts. An identical freshly derived image retains the old image
`Arc` after byte equality checking, but the source snapshot is still replaced.
The default constructor performs a complete cold Project load. The opt-in
frontend constructor instead stages source-exact AST reuse through the same
filesystem authentication and full semantic/link/profile rebuild. It adds an
optional `frontend_work` report to preview/refresh only; failed preparation and
preview do not install it. Neither route claims incremental semantic checking
or warm cross-process compilation.

The payload schema `semaprax.image-workspace-refresh.v1` contains old/new image
and Project revisions, the new Workspace revision, `image_arc_reused`, sorted
`retained_candidates`, `cleared_drafts`, `cleared_attempts`,
`manifest_changed:false`, `source_authority:false`, and
`recovery:"explicit_fresh_snapshot"`, plus explicit nonclaims. Old image, facet,
draft, and attempt handles cannot be mistaken for current ones.

## Preparation, bounds, and publication

V5 reuses existing typed candidate preparers and the bounded registry: at most
16 complete candidates, 16 drafts, 16 attempts, and 256 MiB of accounted retained
report bytes. Historical candidates occupy the same registry; refresh does not
create an unbounded history. Hosts can explicitly discard historical candidates
to make room for a new base. Registry accounting is not a complete HIR/RSS bound.

Ordinary handlers prepare payloads and mutations without modifying the registry,
admit capacity, render the complete response, and perform final source
authentication before committing a mutation. Response overflow discards the
prepared mutation. The transport accepts at most 64 KiB per frame and produces
at most 1 MiB per response, using the existing strict NDJSON framing/JSON-RPC
codec. Oversized input ends the stream; notifications remain silent and do no
semantic work. Query/report owners retain their own smaller limits.

`with_git_commit_host` attaches a fixed manifest-matching Git authority only
before the first frame. `approve_git_commit` is a separate host API, never an
RPC method. In this implementation approval also must precede the first frame.
An attempted relaxation to approve within an already active session was rejected
by the environment's automatic security review as weakening that temporal
authorization boundary, and was not applied. The supported review workflow is
to review/export a candidate first, then open a separate host-approved commit
session and restore its exact source-backed capsule. Requests cannot self-approve
or replace the fixed Git target, executable, repository, or commit metadata.

Commit preparation checks the current held source and candidate base before
calling the dedicated Git publication authority. That authority owns independent
replay, immutable objects, one expected-old ref pivot, and uncertain post-pivot
diagnostics. A generic request wrapper does not reinterpret its post-publication
failure as an ordinary failed preparation. Commit status and immutable receipt
chunks remain inspectable after uncertain outcomes without asserting current
source admission. They still require the session image expectation.

The final session boundary retains source authentication. If it fails after a
terminal commit outcome, `SPX-G287` explicitly preserves the host's `published`
or `publication_uncertain` classification and includes the source diagnostics;
it does not erase a known successful commit or advise blind retries. See the
separate Git-publication and source-commit protocol specifications for authority
and repository constraints.

`serve_vnext` retains final authentication failures as `VNextSessionFailure`
inside `io::Error`. Embedding hosts can downcast `error.get_ref()` and copy its
`diagnostics()` without flattening publication codes into a generic transport
error. Stream read/write failures after a terminal Git outcome likewise preserve
an explicit `SPX-G287` published/uncertain classification. Final authentication
still runs after stream failure, and its diagnostics are retained alongside the
outcome and bounded I/O-kind description. An ordinary pre-publication stream
failure keeps its original I/O error when final authentication succeeds.

## Evidence and diagnostics

`SPX-G280` rejects invalid host/session configuration;
`SPX-G281` covers refresh response capacity;
`SPX-G282` rejects stale image/new-source expectations or a wrong commit base;
`SPX-G283` rejects configuration changes or inconsistent unchanged image facts.
Existing source, candidate, draft, diagnostic, test, target, and publication
codes remain intact. Invalid JSON-RPC parameters and unavailable methods use
the existing transport error codes.

`tests/image_workspace_transport_v5.rs` authors preview without state revival,
absorbing-drift recovery,
historical-candidate rebase, explicit transient invalidation, failed-refresh
preservation, notification/parameter rejection, unchanged-image refresh,
manifest-change refusal, selected capabilities, and v1/v2/v4 compatibility
regressions. Existing v3 tests and separate v5 discovery, artifact, and commit
tests own their domains. These tests have not been executed in this change.
Verified full workflow, latency/memory benchmarks, exhaustive race testing,
cross-process warm reuse, and all remaining programme requirements are still
outstanding evidence, not implied by the v5 label.
