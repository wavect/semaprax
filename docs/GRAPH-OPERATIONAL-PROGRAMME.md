# Graph-operational programme: requirement and evidence ledger

Audience: compiler contributors, agent builders, and programme reviewers.

Status: **full programme remains Partial**. This ledger preserves the complete
requested objective: canonical human `.spx` source, persistent derived semantic
workspaces, typed intentions and candidate overlays, independently replayed
source materialization, and separate publication authority. Completing a
bounded image, query protocol, or append-parameter operation does not complete
that objective.

The [completion matrix](COMPLETION-MATRIX.md) continues to own product status.
This ledger tracks the detailed programme requirements, not a replacement
product denominator. **Authored/unrun** means code or regression cases exist
but this work has not executed their checks. **Partial** means a narrower
existing or newly authored mechanism leaves the stated requirement open.
**Missing** means no integrated implementation/evidence was identified for the
requested mechanism; a related utility is not equivalent evidence. No row here
asserts verified completion. The user explicitly requested skipping local
tests and long quality gates; consequently current changes provide no new
verified completion or hosted promotion evidence. That leaves validation work
outstanding without requiring another permission request now.

## Evidence owners

| Key | Source, specification, and executable evidence |
| --- | --- |
| Image | [image.rs](../src/project/image.rs), [Image v1](SEMANTIC-WORKSPACE-IMAGE-V1.md), [image evidence](../tests/semantic_workspace_image_v1.rs), [CLI evidence](../tests/project_semantic_image_cli_v1.rs) |
| Facets | [image_facets.rs](../src/project/image_facets.rs), [Facets v1](SEMANTIC-IMAGE-FACETS-V1.md), [facet evidence](../tests/image_facets_v1.rs) |
| Protocol | [image_transport.rs](../src/image_transport.rs), [Image Agent Protocol v1](IMAGE-AGENT-PROTOCOL-V1.md), [protocol evidence](../tests/image_transport_v1.rs) |
| Candidate | [candidate module](../src/project/candidate/mod.rs), [intent constructors](../src/project/candidate/intent.rs), [Candidates v1](PROJECT-CANDIDATES-V1.md), [candidate evidence](../tests/project_candidates_v1.rs) |
| Candidate protocol | [candidate transport](../src/image_transport/candidates.rs), [Candidate Protocol v2](IMAGE-CANDIDATE-PROTOCOL-V2.md), [protocol evidence](../tests/image_candidate_transport_v2.rs) |
| Holes | [draft module](../src/project/candidate/draft.rs), [Typed Holes v1](PROJECT-CANDIDATE-HOLES-V1.md), [hole evidence](../tests/project_candidate_holes_v1.rs) |
| Signature mapping | [signature engine](../src/project/candidate/signature.rs), [Signature Evolution v1](PROJECT-SIGNATURE-EVOLUTION-V1.md) |
| Expression changes | [expression module](../src/project/candidate/expression.rs), [Expression Change v1](PROJECT-EXPRESSION-CHANGE-V1.md), [expression evidence](../tests/project_candidate_expression_v1.rs) |
| Contract changes | [intent module](../src/project/candidate/intent.rs), [Contract Change v1](PROJECT-CONTRACT-CHANGE-V1.md), [candidate evidence](../tests/project_candidates_v1.rs) |
| Rebase | [rebase module](../src/project/candidate/rebase.rs), [Candidate Rebase v1](PROJECT-CANDIDATE-REBASE-V1.md), [rebase evidence](../tests/project_candidate_rebase_v1.rs) |
| Declaration creation | [declaration module](../src/project/candidate/declaration.rs), [Declaration Change v1](PROJECT-DECLARATION-CHANGE-V1.md), [declaration evidence](../tests/project_candidate_declaration_v1.rs) |
| Extraction | [extraction module](../src/project/candidate/extraction.rs), [Extraction v1](PROJECT-EXTRACTION-V1.md), [extraction evidence](../tests/project_candidate_extraction_v1.rs) |
| Recovery | [recovery module](../src/project/candidate/recovery.rs), [Recovery v1](PROJECT-CANDIDATE-RECOVERY-V1.md), [recovery evidence](../tests/project_candidate_recovery_v1.rs) |
| Declaration moves | [movement module](../src/project/candidate/movement.rs), [Declaration Move v1](PROJECT-DECLARATION-MOVE-V1.md) |
| Record-field changes | [field migration](../src/project/candidate/record_field.rs), [Record Field Change v1](PROJECT-RECORD-FIELD-CHANGE-V1.md) |
| HIR relationships | [relationship facets](../src/project/image_facets/relationships.rs), [HIR Relationships v1](SEMANTIC-IMAGE-HIR-RELATIONSHIPS-V1.md) |
| Candidate tests | [test planning/execution](../src/project/candidate/testing.rs), [Candidate Tests v1](PROJECT-CANDIDATE-TESTS-V1.md), [Test Protocol v3](IMAGE-CANDIDATE-TEST-PROTOCOL-V3.md) |
| Candidate diagnostics | [attempt/repair module](../src/project/candidate/diagnostics.rs), [Candidate Diagnostics v1](PROJECT-CANDIDATE-DIAGNOSTICS-V1.md) |
| Managed publication | [publication bridge](../src/project/candidate/publication.rs), [Candidate Publication v1](PROJECT-CANDIDATE-PUBLICATION-V1.md) |
| Image lifecycle | [image_store.rs](../src/project/image_store.rs), [Image Store v1](SEMANTIC-IMAGE-STORE-V1.md) |
| Semantic deltas | [delta.rs](../src/project/candidate/delta.rs), [Semantic Delta v1](PROJECT-CANDIDATE-SEMANTIC-DELTA-V1.md) |
| Diagnostic protocol | [Diagnostic Protocol v4](IMAGE-CANDIDATE-DIAGNOSTIC-PROTOCOL-V4.md) |
| Integrated managed workflow | [Workflow v1](PROJECT-GRAPH-OPERATIONAL-WORKFLOW-V1.md), [authored scenario](../tests/project_graph_operational_workflow_v1.rs) |
| Frontend reuse | [Frontend Cache v1](PROJECT-FRONTEND-CACHE-V1.md), [incremental.rs](../src/project/incremental.rs) |
| Live frontend reuse | [Workspace Frontend Cache v1](IMAGE-WORKSPACE-FRONTEND-CACHE-V1.md), [cached session cases](../tests/image_workspace_frontend_cache_v1.rs); exact authenticated source loading and transactional cache adoption |
| Parallel image reads | [Parallel Reads v1](IMAGE-PARALLEL-READS-V1.md), [batch cases](../tests/image_parallel_reads_v1.rs); embedding-host scoped workers, not a concurrent stdio server |
| Integrated canonical Git workflow | [Git Workflow v1](PROJECT-GRAPH-OPERATIONAL-GIT-WORKFLOW-V1.md), [authored real-provider scenarios](../tests/project_graph_operational_git_workflow_v1.rs); no executed publication evidence |
| Expression holes | [Expression Holes v1](PROJECT-CANDIDATE-EXPRESSION-HOLES-V1.md) |
| Typed repair change | [Diagnostic Change v1](PROJECT-DIAGNOSTIC-CHANGE-V1.md) |
| Static protocol conformance | [Static Protocol Conformance v1](STATIC-PROTOCOL-CONFORMANCE-V1.md), [static_protocol.rs](../src/static_protocol.rs) |
| Interface implementation | [Interface Change v1](PROJECT-INTERFACE-CHANGE-V1.md), [interface.rs](../src/project/candidate/interface.rs) |
| Conformance discovery | [Image Protocol Conformance v1](IMAGE-PROTOCOL-CONFORMANCE-V1.md); source-bound sidecar and v4 query/catalogue integration |
| Typed discovery | [Agent Discovery v5](IMAGE-AGENT-DISCOVERY-V5.md); runtime-selected request/envelope schemas, typed outer client parameters and explicitly opaque compiler-report schemas |
| Unified workspace protocol | [Workspace Protocol v5](IMAGE-WORKSPACE-PROTOCOL-V5.md), [startup CLI](WORKSPACE-SESSION-CLI-V1.md) |
| Protocol source commit | [Source Commit Protocol v5](IMAGE-SOURCE-COMMIT-PROTOCOL-V5.md); independently selected startup authority and exact approval |
| Target/artifact queries | [Target and Artifact Projections v1](IMAGE-TARGET-ARTIFACTS-V1.md); actual pathless Web/npm carrier construction and replay |
| Canonical Git publication | [Git Publication v1](PROJECT-CANDIDATE-GIT-PUBLICATION-V1.md), [explicit host CLI](CANDIDATE-GIT-PUBLICATION-CLI-V1.md) |
| Store | [project_revision_store.rs](../src/project_revision_store.rs), [Store v1](PROJECT-REVISION-STORE-V1.md), [Windows-entry v1](PROJECT-REVISION-STORE-WINDOWS-V1.md), [store evidence](../src/project_revision_store/tests.rs) |
| Analysis | [workspace_analysis.rs](../src/workspace_analysis.rs), [Workspace Analysis v1](WORKSPACE-ANALYSIS-V1.md); retained six-family typed indexes and existing Context/Impact/Review |
| Existing mutation | [semantic workspace operations](../src/semantic_workspace_operations.rs), [Operations v1](SEMANTIC-WORKSPACE-OPERATIONS-V1.md), [operation evidence](../tests/semantic_workspace_operations_v1.rs); [Project rename](PROJECT-RENAME-TRANSACTION-V1.md) and [rename evidence](../tests/project_agent_transport_rename_v1.rs) |
| Existing evidence | [patch evidence](../src/patch_evidence.rs), [workspace change](../src/semantic_workspace_change.rs), [target evidence](../src/target_evidence.rs), their versioned specifications and focused suites |
| Economics | [agent_economics.rs](../src/agent_economics.rs), [Agent Context Economics v1](AGENT-ECONOMICS-V1.md), [economics evidence](../tests/agent_economics.rs) |

These owners identify review and validation work; their presence does not
assert current-head green results. Candidate and Protocol refer to the current
working implementation alongside the prior Image foundation.

## Phase 1: fast semantic workspace

| Requirement | Status and remaining evidence |
| --- | --- |
| Persistent, content-addressed derived HIR and graph snapshots | Partial. Image retains checked HIR, graph and indexes in memory. Image Store adds authored/unrun source-backed persist/load receipts and a retained-image refresh lifecycle. Cold load rebuilds source; cross-process warm HIR reuse and executed recovery evidence remain missing. |
| Identity binds compiler, graph/HIR compatibility, manifest, ordered paths/digests and profiles | Partial. Image binds package version, explicit image compatibility, manifest/profile bytes, source graph schemas and revisions. It does not claim exact compiler binary identity or independently versioned portable HIR ABI. Define and test cross-build invalidation before warm serialized-HIR reuse. |
| Derived, deletable, rebuildable, Git-excluded, revision-bound; no graph-only meaning | Authored/unrun Image/Store boundaries. Image replay reconstructs from admitted source revision; `.semaprax-images/` is ignored. Still require integrated cache deletion/recovery/corruption and stale-source lifecycle evidence. |
| Incremental invalidation/rechecking | Partial authored/unrun Frontend Cache. Source-exact retained canonical ASTs avoid parser/formatter calls for eligible unchanged modules; changed providers invalidate reverse import dependents. Direct owned-source refresh and opt-in v5 authenticated filesystem refresh now use this cache without a preliminary cold build. Every refresh still reruns all semantic checking/linking/profile admission. V5 stages cache and snapshot/image together, discards preview/failed work, preserves immutable candidates and clears drafts/attempts on success. Checked-HIR reuse remains missing. |
| Expanded symbol and reverse indexes | Partial, Facets authored/unrun. Stable-ID lookup and six-family indexes exist; HIR caller expansion adds local and cross-file direct callers. Remaining independent graph facets and generalized reverse dependencies are listed below. |
| Capability-negotiated discovery | Partial, authored/unrun. Host-selected read-only v1, candidate-only v2, fixed-policy test v3 and diagnostic v4 remain preserved. Additive v5 combines optional candidate/diagnostic/test/pathless-build capabilities and a separately attached startup-approved Git commit host. Artifact filesystem materialization and request-driven elevation remain absent. |
| Compact summaries/references/facet expansion | Partial, Facets/Protocol authored/unrun. Handles/cursors bind image, target, facet and page size; summaries and paginated detail exist. Exact relevance reasons, broader stable session references, optional advisory ranking and measured context improvement remain open. |
| Diagnostics/repair metadata retained with the workspace | Partial. Existing compiler diagnostics and repair discovery are separate; invalid source does not produce an admitted Image. Missing integrated symbol-linked diagnostic/repair inventory and incremental refresh. |

## Phase 2: candidate model

| Requirement | Status and remaining evidence |
| --- | --- |
| Immutable overlays against one immutable base, branching and discard | Candidate authored/unrun. Applying returns a new value; siblings retain their base, dropping discards. Candidate-only v2 retains bounded candidate/draft registries and exposes discard. No durable registry or recovered branch lifecycle. |
| Versioned Semantic Change IR and mandatory constraints | Partial, Candidate authored/unrun for all eleven requested operation classes, including replayed diagnostic repair and static protocol implementation. Base revision, exact identity additions/relocations, exports, effects, permits, exact contract inventory changes and profile/core-target preservation are checked in that slice. General interface behavior, broader constraints and complete semantic-delta proof remain open. |
| Typed expression/declaration constructors | Partial. Candidate constructors cover bounded scalar/parameter/operator/call expressions and monomorphic function declarations with limited ownership modes. General expressions/declarations and expected-type/effect/ownership-guided discovery remain missing. |
| Ephemeral typed holes | Partial, authored/unrun. Body holes and disjoint authored expression holes expose checked lexical context, expected type/ownership and prior-body facts. Filling performs complete candidate admission and remaps surviving selections; unresolved drafts cannot materialize. Contract-region holes, recursive incomplete declarations and complete next-expression liveness guidance remain open. |
| Candidate ID, base/candidate revisions, semantic/source-diff digests, validation/diagnostics/gates | Partial, Candidate authored/unrun. Complete candidates carry digests, diffs, validation facts and gates. V4 adds bounded rejected-attempt lifecycle and repair discovery without invalid source/image access. General incomplete-state diagnostics remain missing. |
| Candidate comparison, targeted validation and exact semantic replay | Partial. Candidate comparison is descriptive target overlap; source is formatted, reparsed and rebuilt with complete Project admission. Need semantic compatibility decisions, intended-delta verification across general transformations and selective invalidation/validation. |

## Phase 3: all eleven requested operations

All eleven requested classes now have bounded **authored/unrun** slices. This
counts represented operation classes, not completed general operations, runtime
interface support, passing tests or completion-matrix promotions. Each row keeps
its broader requirement open.

| Operation | Present scope and remaining requirement |
| --- | --- |
| `rename_declaration` | Partial. Existing managed declaration/import-alias operations and Project display rename; Candidate adds explicit monomorphic non-main function display rename. General types/fields/interfaces, candidate discovery and all dependent-reference cases remain. |
| `change_function_signature` | Partial, authored/unrun. Scalar append and ordered Copy mapping now include hygienic parameter display renaming and conservative direct owning-Bytes retention/reordering. Owners cannot be dropped or duplicated. Type/result changes, broader borrow/resource mapping, dependent declarations and external consumer migration remain missing. |
| `replace_expression` | Partial, authored/unrun. Body-expression selection uses actual revision-scoped HIR identity and unambiguous canonical AST provenance; replacement uses authenticated lexical scope, expected-type/ownership checks and full Project admission. Contract-region replacement, generic/synthetic selections and general typed constructors remain open. |
| `replace_function_body` | Partial, Candidate authored/unrun: bounded typed constructors for explicit monomorphic non-main functions followed by full source admission. General body/control/data/ownership shapes remain. |
| `extract_function` | Partial, authored/unrun. Actual HIR expression selection, immutable built-in Copy capture derivation, fresh caller-selected function identity, in-place call substitution and exact source replay. Owned/mutable captures, unsafe-boundary relocation and general control/data forms remain excluded. |
| `add_declaration` | Partial, authored/unrun. Typed monomorphic function append in an anchor's module with global identity/namespace/effect checks and exact invariant extension. General declaration kinds, named types, recursive construction and placement remain open. |
| `move_declaration` | Partial, authored/unrun. Explicit monomorphic scalar function relocation between existing modules preserves identity, migrates stable-ID call/import bindings and replays exact source. Fixed exports, general named/owned types and audit-bearing relocation remain excluded. |
| `implement_interface` | Partial, authored/unrun. Compiler-derived required-member discovery and a typed mapping bind an explicit local monomorphic record to a same-module protocol and eligible existing functions. Exact member coverage/signature matching and canonical source replay are checked. General interface contracts, cross-module implementations, generic/owned receiver forms, runtime witnesses and dynamic dispatch remain missing. |
| `add_record_field` | Partial, authored/unrun. Appends one i64/bool field to an eligible monomorphic Copy record, migrates constructors and exact nested patterns, preserves existing projections and revalidates complete Project/layout/target admission. Owned/generic/class/variant fields and broader evolution remain open. |
| `add_contract` | Partial, authored/unrun. Append one typed requires/ensures predicate to an explicit monomorphic non-main function, preserving prior predicates and exact other invariants with full Project admission. General declaration contracts, proof of runtime satisfaction and external compatibility remain open. |
| `repair_diagnostic` | Partial. Existing ID repair remains separate. Candidate Diagnostics and v4 retain rejected attempts; the new typed wire rederives the exact compiler-admitted integer-literal repair and preserves it in replayable history. Its selector binds the exact predecessor and rejects rebase/reminting. General repair classes remain missing. |

`change/catalog <target>` now provides candidate-bound constructor discovery
in candidate-only v2 for supported intention classes. Unsupported
targets expose no operations; each payload still requires full admission.
Discovery of fully proven legal transitions remains open; this catalogue does
not advertise the aspirational table above. Existing [hygienic generation](HYGIENIC-GEN-V1.md) is related typed
synthesis, not an implementation of the missing change operations.

## Phase 4: generalized impact, facets and evidence

| Required facet family | Present evidence and remaining work |
| --- | --- |
| Contracts | Facets exposes actual pre/postcondition expressions. Missing independently verified dependency edges to invariants and candidate contract deltas. |
| Ownership | Facets exposes parameter modes and structural slots. Missing general creation/transfer/settlement relationships and candidate ownership deltas. |
| Cleanup | Facets reuses complete ordered CleanupPlan projection. Missing generalized reverse expression/field-to-obligation queries and candidate cleanup deltas. |
| Data access | Partial, authored/unrun HIR Relationships. Bounded function facets expose actual ValueId reads/writes, field projections and consumption-context facts with provenance. General reverse field indexes and candidate deltas remain missing. |
| Interfaces | Partial, authored/unrun source-sidecar conformance facts and v4 `protocol/conformance` / `candidate/interface-catalog` discovery. Facts bind source revisions and stable receiver/protocol/member/function identities; they do not add dispatch edges to the runtime Graph. General interface dependency facets, candidate deltas and dispatch remain missing. |
| Tests | Partial, authored/unrun Candidate Tests selects relevance from exact transitive test-root HIR calls with conservative fallback for non-call facts; explicit execution runs the full declared interpreter test closure. Dynamic coverage, multi-root selection and native/Wasm evidence remain missing. |
| Targets | Partial, authored/unrun. V5 `image/target-admission` derives actual native-C11/Core-Wasm emission and structural validation facts for complete entry/test closures and checks selected-function membership. Whole-closure failure is not blamed on that function. Runtime execution, general declaration-level diagnosis and broader package-profile coverage remain missing. |
| Artifacts | Partial, authored/unrun. Web/npm projections actually build and replay existing pathless carriers and bind files, source inputs and manifest export identities. V5 host-enabled `candidate/build` restores the candidate history first and returns bounded report chunks, not materialized files. Rust/C/OpenAPI carriers, generalized consumer migration and artifact filesystem authority remain missing. |
| Packages | Missing unified semantic consumer-interface relationships and cross-package migration evidence. Existing package tools are not that index. |
| Unsafe boundaries | Partial, authored/unrun HIR Relationships projection machinery. Current Project admission excludes unsafe-bearing sources, so its public unsafe inventory remains empty. Active unsafe-source coverage, transitive dependency/exposure analysis, broader import-bearing Project admission and candidate deltas remain missing. |
| Diagnostics | Existing source diagnostics/repair are separate. Missing workspace symbol-to-diagnostic/repair-class facet and invalid-candidate integration. |

Each generalized fact must bind source provenance, stable identity, revision,
edge kind, reason, evidence owner and authoritative/descriptive/advisory class.
Current Facets labels descriptive validated-HIR projections and source
revisions; this does not establish the missing families or their independent
replay. LoanPlan vectors remain proof data, not runtime liveness authority.

Cross-file candidate-aware impact is **Partial**: Candidate pairs base and
candidate six-family Impact reports. Semantic Delta adds authored/unrun selected
declaration before/after facts, function facets, reverse field accesses, test
relevance and whole-closure target artifacts with exact recomputation. This
narrows the missing delta work above; general interface/package/artifact families,
behavioral equivalence and runtime coverage remain open.
Source/semantic-diff binding and exact candidate replay are **Authored/unrun**
for the closed slice. Targeted tests plus policy-selected full gates remain
**unrun/missing integration**. Evidence-bound materialization through separate
commit authority is **Partial** in existing A0/managed Workspace routes and
**Partial for candidates** through an authored/unrun separate managed Workspace
bridge. It replays under the existing lock before `ACTIVE` publication and leaves
original raw Git paths unchanged. A separate authored/unrun host route now
constructs canonical Git objects and publishes a branch by expected-old ref
update in an explicitly selected Unix bare SHA1 or SHA256 repository. SHA1
adds exact staged-object readback and a SHA256 observed/prepared-content binding,
without collision-detection or modern SHA1 integrity claims. Broader hosts and
checkout integration remain missing; a capsule cannot publish
itself or select that authority. V5 can invoke this fixed authority only when the
host attached it and approved an exact candidate before the first request.
Review/export first, then restore/commit in a separate host-approved session;
there is no in-session RPC or later-startup approval shortcut.

## Protocol, generated integrations and candidate lifecycle

| Requested surface | Current status |
| --- | --- |
| `protocol/capabilities`, `protocol/schemas` | Authored/unrun Protocol. Host-selected capabilities and catalogue-driven request/success/error envelopes. Additive `protocol/constructor-schemas` supplies self-contained closed typed-expression/intent/change schemas. V5 adds runtime-selected schemas and typed clients with concrete candidate comparison/reconciliation/catalog/test-plan and frontend-work payloads. Client generation rejects unsupported validation assertions and includes only response-reachable documents. Heterogeneous candidate/HIR report interiors and executed client/schema compatibility remain missing. |
| `workspace/open`, `workspace/status`, `workspace/refresh-preview`, `workspace/refresh`, `query/catalog` | Authored/unrun. Host binds the manifest; requests cannot select a new path. V5 preview discovers the currently observed revision without replacing state. Cold and opt-in frontend-cache refresh authenticate the same source set with exact expected image/new Project revisions, retain candidate handles and clear incomplete drafts/attempts only after successful bounded response preparation. Failed refresh preserves session/cache state; incremental HIR reuse is not claimed. |
| `protocol/conformance`, `candidate/interface-catalog` | Authored/unrun additive v4 read-only source-sidecar queries and candidate-bound static required-member discovery. Existing v1–v3 method sets and runtime Graph contracts remain unchanged; no runtime interface or publication authority is granted. |
| `change/catalog`, `validation/catalog` | Authored/unrun candidate-only v2. Target-specific constructor discovery and independent candidate replay are available; arbitrary payload validity requires apply. General legal-transition discovery and execution routes remain open. |
| Read-only, candidate-only, source-commit, build-enabled, test-enabled, artifact-materialization-enabled sessions | V5 composes host-selected read-only/candidate/diagnostic/test/pathless-build capabilities and optionally startup-approved fixed Git publication. Older profiles remain unchanged. No artifact filesystem materialization, arbitrary tool execution or request authority elevation is granted. |
| Version-matched agent instructions | Authored/unrun catalogue-derived instructions cover only methods granted by the selected v5 host policy and preserve older profile instructions. Executed instruction/client compatibility evidence remains open. |
| TypeScript, Python, Rust clients | V5 authors typed TypeScript/Python/Rust outer request parameters, enum choices, builders and result decoders from runtime-selected catalogues. Concrete transport handles/chunks are described; nested constructor objects stay server-validated and opaque compiler-report schemas remain explicitly unbundled. Independently executed cross-language validation, complete typed payloads and client ergonomics remain open. |
| Optional MCP/editor adapters | Missing for this new protocol. |
| Machine-readable operation catalogue | Read query and target-specific constructor catalogues authored/unrun; general change catalogue remains partial. |
| `candidate/open`, `candidate/apply-intent` | Candidate library and candidate-only v2 methods authored/unrun; bounded immutable registry with no publication authority. |
| `candidate/query`, `candidate/validate`, `candidate/impact` | Authored/unrun v2 methods: bounded report chunks, complete independent replay and six-family impact. General incomplete-candidate validation and generalized impact remain open. |
| `candidate/test` | Authored/unrun library and explicitly host-selected v3 route: exact candidate replay precedes fixed-policy execution of the full declared interpreter test closure. Static relevance is not coverage; native/Wasm and policy-selected full gates remain separate. No tests were run in this implementation work. |
| `candidate/compare`, `candidate/discard` | Descriptive library and v2 lifecycle methods authored/unrun. Semantic compatibility proof remains missing. |
| `candidate/commit` | Partial separate host bridges to managed `ACTIVE` and canonical Git branch publication, each with exact candidate approval/replay. The Git adapter admits Unix bare SHA1/SHA256 repositories and never rewrites raw checkouts. V5 adds `candidate/commit` only with separately attached fixed authority and exact startup approval, consumed on invocation; success/uncertainty is terminal for that commit host. Ordinary checkout integration, broader Git interoperability and executed end-to-end publication evidence remain missing. |

## Phase 5: multi-agent operation

| Requirement | Status and remaining evidence |
| --- | --- |
| Semantic rebase/merge | Partial, authored/unrun Rebase. Stable-ID target/dependency conflict classification, introduced-identity collision checks, display-normalized call facts, canonical source replay and same-root history merge cover supported intentions. General moves/declarations, ownership-sensitive reconciliation and cross-manifest merging remain open. |
| Conflict cases | Focused Rebase cases are authored/unrun: unrelated body/display rename, body/postcondition revalidation, competing signature conflicts and deleted call dependencies. Execute the matrix, expand to all intended operations and preserve expected-expression remapping before claiming full coverage. |
| Candidate branching | Partial in-memory immutable siblings and bounded candidate-only v2 registry. Complete histories have authored/unrun recovery capsules; durable branch registries and incomplete-draft recovery remain missing. |
| Parallel read-only requests | Partial, authored/unrun. The embedding-host batch API uses at most four scoped workers for sixteen immutable image/discovery reads, restores input order and authenticates held source before/after the entire joined batch. No candidate, refresh, test, build or commit authority enters workers. StdIO remains sequential; general concurrent transport scheduling, candidate reads, cancellation and executed isolation/throughput evidence remain missing. |
| Session recovery and content-addressed candidate persistence | Partial, authored/unrun Recovery. Explicit complete-history capsules bind content digests and reconstruct candidates by exact source replay through library/CLI/candidate-only protocol. No automatic storage, warm HIR, cursors, incomplete drafts, pending validation or authority recovery. `.semaprax-candidates/` is Git-excluded. |
| Manual edits and stale recovery | Partial held-input absorbing invalidation, exact base rejection and library rebase onto a separately admitted revision. V5 reloads the fixed manifest through cold or source-exact frontend reuse and preserves historical candidates for inspection/rebase; drafts and attempts clear on successful refresh. Complete session recovery, checked-HIR/incremental semantic reload and recovery benchmarks remain open. |

## Required twelve-step signature demonstration

| Step | Current evidence or outstanding gate |
| --- | --- |
| 1. Open immutable Project snapshot | Image/Project foundation; current-head preservation gates unrun. |
| 2. Select explicit stable-ID function | Image/Facets and Candidate closed selection; authored/unrun. |
| 3. Change signature | Candidate append-scalar and ordered built-in Copy mapping subsets authored/unrun; general evolution open. |
| 4. Migrate every authenticated caller | Authored bounded append and hygienic mapped-argument migration; local/cross-file/contract-region evidence must run. No external/dynamic migration claim. |
| 5. Preserve stable ID and exported identity | Candidate identity/manifest checks authored; external ABI compatibility is not implied. |
| 6. Prove no new effects/capabilities | Candidate invariant checks plus re-admission authored; execute success and hostile cases. |
| 7. Revalidate contracts/ownership/cleanup | Candidate complete source rebuild/replay authored; execute exact preservation/negative cases. |
| 8. Run affected tests | Selection and explicit candidate interpreter-test API/transport authored, unrun. No execution evidence for this demonstration yet; broader test/target gates remain open. |
| 9. Verify native/Wasm admission | Candidate C11/structural Wasm projection authored; runtime conformance and broader package targets remain open. |
| 10. Return semantic impact and human source diff | Paired Impact, digest-bound source diff and selected source-bound semantic delta replay are authored/unrun; complete generalized facet families remain open. |
| 11. Reject or semantically rebase concurrent source change | Retained-base stale rejection and bounded source-replayed semantic rebase are authored/unrun; live candidate publication race evidence and general conflict reconciliation remain open. |
| 12. Commit only through separate authority | Authored/unrun bridges support managed Workspace and a Unix bare-SHA1/SHA256 Git ref authority. The latter publishes canonical source in Git objects without touching checkouts. Integrated v5 scenarios now author separate startup-approved review/restore/commit, actual Git object inspection, wrong approval and stale-ref rejection. Executed source-commit/hostile evidence and broader Git support remain incomplete. |

An integrated canonical Git scenario is authored in
`tests/project_graph_operational_git_workflow_v1.rs`: real v5 requests select a
cross-file signature change, merge an unrelated sibling, reject competing
signatures, review exact source/delta evidence, invoke the explicit interpreter
test policy, and export/replay the complete candidate. Separate startup-approved
sessions then use the actual bare Git SHA1/SHA256 process provider, compare
committed source objects and reject wrong approval or a stale fixed ref base.
The scalar fixture checks preserved pre/postconditions, effects, exports and
empty owned cleanup; it does not establish general resource behavior. Native
and Wasm evidence is compiler emission/structural validation, not target execution.
The scenario is **unrun** and therefore does not complete the demonstration.

An integrated managed-generation precursor is also authored in
`tests/project_graph_operational_workflow_v1.rs`: it combines signature migration,
unrelated merge, competing-signature rejection, deltas, explicit test policy and
separate managed publication with stale rejection. It is unrun and deliberately
leaves canonical raw Git source unchanged. The demonstration is not complete
until one integrated executable scenario
covers all twelve steps, including the separate commit boundary and its hostile
cases, with evidence tied to the exact commit and required target matrix.

## Guardrails and evaluation gates

All phases retain canonical human source and ordinary Git review; persistent
public `@id` declarations; exact-base intentions; canonical source diffs;
source-to-intended-candidate replay; cache/daemon-independent rebuilds;
deterministic authoritative output; manual-edit invalidation or verified
rebase; host-selected least authority; evidence/review distinct from commit;
and no ambient filesystem/network authority in semantic reasoning. Existing
A0 and managed `ACTIVE` publication semantics remain distinct.

Prohibited shortcuts remain explicit: canonical graph databases in Git;
arbitrary agent writes to graph fields; unrepresentable graph-only states;
hidden cache mutations; approval inferred from proof/review; authoritative
ML relevance ranking; daemon-required compilation; and graph diffs replacing
human-readable source review. Optional deterministic ranking must stay advisory
and outside source/graph identity, proof and commit decisions.

Economics provides offline bytes/lexical-unit/relevance evidence only; its
small corpus produced context larger than source. Still missing are measured
end-to-end workloads comparing source-first and semantic workflows across:
model tokens, tool calls, invalid attempts, stale recovery, correctness,
validation cost and human review time. Add warm/cold/persistent/incremental and
multi-agent conflict/recovery cases; bind corpus, compiler, prompts/models,
source revisions and correctness/coverage criteria. Run generated-client,
protocol preservation, hostile-image/candidate, backend and policy-selected
quality gates before any completion or comparative productivity assertion.
