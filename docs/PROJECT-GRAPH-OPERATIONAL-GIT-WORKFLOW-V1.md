# Project graph-operational Git workflow v1

Status: integrated regression executed locally for exact subject
`474c481bf3c3561c144e077f0000460f61af55f2`; all three selected Git-workflow
tests passed. Full goal remains Partial.

Audience: compiler contributors, embedding hosts, and agent workflow integrators.

`tests/project_graph_operational_git_workflow_v1.rs` connects the real Image
Workspace Protocol v5 request dispatcher to complete candidate recovery and the
real `CandidateGitProcessAuthority`. It supplements the existing
[managed-generation workflow](PROJECT-GRAPH-OPERATIONAL-WORKFLOW-V1.md); it does
not replace the managed `ACTIVE` authority or rewrite a checked-out source tree.

The fixture starts from the three-file calculator Project. It adds a meaningful
precondition and postcondition plus a local checked caller to `calculator.add`.
An ordered signature intention moves `right` before `left`, renames them to
`rhs` and `lhs`, and appends an explicit Copy `i64` `offset` default. The
compiler substitutes the body and contracts and migrates the local, application
and test callers while staging their original arguments in their original
left-to-right evaluation order. A sibling candidate renames
`calculator.multiply` to `times` while preserving its declaration ID. Requests
contain compiler-supported intentions and revision handles, not replacement
source, paths, Git policy, or approval authority.

## Twelve connected steps

Both `twelve_step_v5_review_to_real_sha1_git_commit` and
`twelve_step_v5_review_to_real_sha256_git_commit` exercise this sequence:

| Step | Observed action |
| --- | --- |
| 1 | Open an authenticated v5 workspace and bind subsequent requests to its image revision. |
| 2 | Discover the actual stable target and its two parameters, precondition, and postcondition with `image/function-summary`. |
| 3 | Open a candidate and apply ordered `change_function_signature`; the compiler renames and reorders the retained parameters, adds `offset`, substitutes the body/contracts and reconstructs all three canonical sources. |
| 4 | Merge a sibling display rename through `candidate/merge`, retaining the original source base. |
| 5 | Read candidate impact plus bounded candidate/source-diff and semantic-delta chunks. Require signature, contracts, callers, ownership, and cleanup facets and exactly three migrated local/application/test calls. Inspect each staged call to prove its two original argument subtrees remain left-to-right while the final call uses them in the new parameter order. |
| 6 | Reject a competing signature with `SPX-G235`; require the previously reviewed candidate report to remain byte-identical and raw sources unchanged. |
| 7 | Request independent candidate validation and inspect the candidate's native-C11 and structurally validated Core-Wasm projections for entry and test closures. |
| 8 | Request the manifest test plan and interpreter tests through v5 with an explicit host policy: 100,000 steps, 65,536 execution bytes, and 262,144 report bytes. |
| 9 | Export exact recovery-capsule chunks, independently replay them, verify the exact semantic delta, and compare declaration identities, exports, parameter ownership, effects, contracts, module permits, and scalar cleanup inventory. Close the review session. |
| 10 | Construct a new, separately startup-approved commit session with one fixed bare repository, ref, old commit, metadata, and process authority. Restore the reviewed capsule through v5; reject both late session approval and a wrong request approval binding. |
| 11 | Request `candidate/commit`. The existing authority replays the exact candidate, authenticates original Project files, creates actual Git objects, and publishes through one expected-old-ref compare-and-swap. Read its bounded receipt. |
| 12 | Read the actual committed `.spx` blobs and manifest through Git. Compare every source with the replayed candidate, retain an unrelated executable tree entry, require the original parent commit, and assert raw files remain unchanged. Repeated publication is terminal and cannot advance the ref. |

The transport helper sends actual bounded JSON-RPC request bytes through
`VNextSession::handle_frame`; it does not call private registry mutation helpers.
Candidate construction, merge, validation, test execution requests, capsule
export/restore, and commit all traverse that dispatcher. Independent library
replay and HIR inspection supplement the protocol assertions; they do not
substitute for protocol commit. The publication provider is the real restricted
Git subprocess adapter, not a mock successful ref update.

### Mapping to the requested vertical slice

The chronological test steps above group review and approval separately. The
original twelve requirements map as follows:

| Requested requirement | Assertion and precise scope |
| --- | --- |
| Immutable snapshot | `workspace/open`, exact image-bound requests, independent capsule replay. |
| Explicit stable-ID selection | The function summary selects `calculator.add`. |
| Signature change | The typed ordered mapping renames and reorders both retained parameters and adds one explicit scalar default. |
| All authenticated callers migrated | Exactly three migrated local/application/test calls; original argument subtrees stage left-to-right before the reordered final call; all three source differences and full candidate replay are checked. |
| Stable and exported identity | Complete declaration-fact map and exact manifest Web export inventory comparison. |
| No new effects or capabilities | Exact candidate/base manifest capability inventory, function effect, and module permit comparisons for this scalar, effect-free fixture. This is a bounded invariant check, not a new general proof system. |
| Contracts, ownership, cleanup | Nonempty predicates are scope-aware renamed with the parameters, their structure remains checked, parameter modes remain exact, and ordinary validation, selected facets and empty scalar cleanup inventories are checked. |
| Affected tests | Explicit-policy v5 test request for the full manifest test closure; its passing assertion executed in the exact local subject bundle. |
| Native/Wasm admission | All four entry/test native-C11/Core-Wasm evidence rows admitted; no target execution. |
| Semantic impact and source diff | Real `candidate/impact` request, exact semantic-delta replay, and per-file human source differences. |
| Concurrent change handling | Sibling candidate merge, competing signature rejection, and a real concurrent bare-ref advancement rejected before publication. Manual raw-source refresh/rebase and a mid-CAS race are not this fixture's coverage. |
| Separate commit authority | A new startup-approved session, exact restored candidate, fixed real Git provider, and actual committed-source verification. |

The new commit session has candidate preparation and the separately attached
source-commit capability. It does not inherit test or build authority from the
review session. Approval is installed before its first request. This preserves
the existing startup-only approval boundary: review/export in one session,
trusted-host approval before another session, then exact restore and commit.
Requests cannot grant or renew approval. The adapter is opened after review so
its fixed 60-second provider lifetime is not spent during review; no deadline is
silently extended.

## Hostile cases and boundaries

Each successful-format scenario first sends a syntactically valid but incorrect
approval digest. `SPX-G286` must leave the branch and separately granted pending
approval unchanged. After success, `SPX-G287` must leave the published ref
unchanged and the approval slot empty.

`competing_real_git_ref_consumes_approval_without_overwriting_the_other_commit`
uses real Git to install another commit after session startup and before the
publication request. The fixed expected-old-base check must reject with
`SPX-G265`, preserve that other commit, consume the attempted publication's
approval, and reject reuse of the consumed binding. This authors a stale-ref
preflight case, **not** a deterministic race inside the final CAS. It neither
mocks nor claims physical lost-acknowledgment, post-pivot uncertainty, process
crash, power failure, or hostile concurrent filesystem mutation coverage.

`real_git_ref_update_with_lost_response_is_terminal_and_requires_inspection`
wraps the real restricted Git provider. It delegates the actual compare-and-swap
and, only after Git reports `Updated`, injects loss of that result. The test
requires the ref to contain the new commit, `publication_uncertain` status,
consumed approval, no commit report, and terminal rejection of both report and
retry routes without a second compare-and-swap. This is deterministic provider
result-loss coverage, not evidence of an operating-system crash, remote Git,
power loss, or socket-level lost acknowledgment.

All Git objects and fixture files live under a fresh temporary directory.
`SEMAPRAX_TEST_GIT`, when explicitly set by the future runner, selects the trusted
Git executable; otherwise the fixture uses `/usr/bin/git`. These Unix-only tests
require a Git installation with both selected object formats. The adapter's
bare-repository, no-hook, no-network, bounded-process, and storage-indirection
restrictions remain those of
[Git publication v1](PROJECT-CANDIDATE-GIT-PUBLICATION-V1.md). SHA1 is a legacy
Git compatibility format, not a modern collision-resistance claim. The exact
local evidence invocation executed these fixture Git commands for the recorded
subject; no hosted or cross-platform execution follows from that run.

## What this does not establish

The fixture deliberately uses Copy scalar parameters and empty effect sets.
Contract and ownership facts are scope-aware rebuilt, and scalar cleanup
inventories are checked; this is not owned-resource settlement, effectful
execution, or general signature migration evidence. Stable Web export IDs
remain unchanged, but reordering, renaming and appending an exported parameter
changes its external signature: no external ABI or consumer compatibility is
claimed.

Native-C11 emission and Core-Wasm structural validation are target projections,
not native or Wasm execution. The candidate report and Git receipt continue to
say `tests: not_run` for their own operations; separately requested interpreter
test evidence is a different report bound to the candidate digest. The exact
local bundle observes that separate interpreter assertion passing; it does not
turn the Git receipt's `tests: not_run` field into a different fact.

This scenario uses sibling merge, not live-source refresh/rebase. It does not
exercise every operation class, generated client, backend, failure race,
transport framing mode, or quality gate. It supplies no measured time, token,
success-rate, or task-level benchmark. Publication changes a bare Git branch,
not raw checkout files, a remote repository, managed `ACTIVE`, or a deployment.
Neither this exact-subject result nor these bounded regressions promote a later
repository head to verified or the
[graph-operational programme](GRAPH-OPERATIONAL-PROGRAMME.md) to complete.

## Focused exact-commit evidence runner

[Graph-operational Execution Evidence v1](GRAPH-OPERATIONAL-EXECUTION-EVIDENCE-V1.md)
defines one local exact-commit runner for this integration binary:

```sh
python3 scripts/graph-operational-evidence.py
```

It selects exactly the two twelve-step real-provider tests and the real stale-ref
preflight test, all nonignored. The SHA-1 and SHA-256 workflows can export their
canonical `semaprax.agent-task-economics.v1` values as
`agent-task-economics-sha1.json` and
`agent-task-economics-sha256.json`; an outer
`semaprax.graph-operational-execution-evidence.v1` envelope binds those artifacts,
the Cargo transcript, exact commit, source manifest and lock state. The reviewed
[local bundle](evidence/graph-operational/474c481bf3c3561c144e077f0000460f61af55f2/5269b6acba08a197e6a8411ba95ccdec6e6a4ff724d35681344b5260087cb2e8/evidence.json)
records 3 passed, 0 failed and 0 ignored selected tests for exact subject
`474c481bf3c3561c144e077f0000460f61af55f2` on Darwin arm64.

The ignored managed-generation test is deliberately outside the runner and is
recorded as `not_selected`. The runner likewise does not select generated-client,
MCP, editor, native-runtime, Wasm-runtime, hosted, or complete-programme gates.
Those dimensions remain independent even if all three selected Git tests pass.

[Graph-operational Execution Evidence v2](GRAPH-OPERATIONAL-EXECUTION-EVIDENCE-V2.md)
extends that runner with the real post-CAS result-loss case, the four managed
publication boundary regressions, and the integrated managed-generation
workflow. Its exact-subject execution is pending.

## Task-economics observation

The same workflow authors a bounded
[`semaprax.agent-task-economics.v1`](AGENT-TASK-ECONOMICS-V1.md) observation.
Its wrapper records exact v5 request/response traffic, review-material sizes,
scripted control rejections and assertion-backed twelve-step criteria without
changing protocol bytes. Frame-to-criterion associations are logical rather
than a second chronological step sequence, and one signature operation covers
both signature change and caller migration. Compiler protocol calls are not
model tool calls; sibling-candidate reconciliation is not stale recovery;
target admission is not target execution. Model tokens, latency, validation
time and human review time remain explicitly unobserved. Commit, commit-report
and source-commit-status route hashes are host-bound invocation evidence rather
than portable snapshots; recovery traffic remains separately labeled. Both
format-specific task reports were executed and authenticated for the exact
subject above; they are protocol traffic observations, not comparative
task-performance measurements.
