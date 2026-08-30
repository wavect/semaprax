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
| Persistent, content-addressed derived HIR and graph snapshots | Partial. Image retains checked HIR, complete Project graph, indexes and canonical source facts in memory; exact image bytes can be replayed. Store persists canonical inputs and rebuilds, not trusted serialized HIR. Missing integrated persistent image lifecycle, cross-process warm HIR reuse and recovery evidence. |
| Identity binds compiler, graph/HIR compatibility, manifest, ordered paths/digests and profiles | Partial. Image binds package version, explicit image compatibility, manifest/profile bytes, source graph schemas and revisions. It does not claim exact compiler binary identity or independently versioned portable HIR ABI. Define and test cross-build invalidation before warm serialized-HIR reuse. |
| Derived, deletable, rebuildable, Git-excluded, revision-bound; no graph-only meaning | Authored/unrun Image/Store boundaries. Image replay reconstructs from admitted source revision; `.semaprax-images/` is ignored. Still require integrated cache deletion/recovery/corruption and stale-source lifecycle evidence. |
| Incremental invalidation/rechecking | Missing. Protocol invalidates after held-input drift and requires reopening; it does not incrementally update. Watchers may supply hints only; exact source authentication must remain authoritative. |
| Expanded symbol and reverse indexes | Partial, Facets authored/unrun. Stable-ID lookup and six-family indexes exist; HIR caller expansion adds local and cross-file direct callers. Remaining independent graph facets and generalized reverse dependencies are listed below. |
| Capability-negotiated discovery | Partial, authored/unrun. Host-selected read-only v1 and candidate-only v2 expose separate closed catalogues. Write/build/test/artifact profiles and their separate least-authority admission remain missing. |
| Compact summaries/references/facet expansion | Partial, Facets/Protocol authored/unrun. Handles/cursors bind image, target, facet and page size; summaries and paginated detail exist. Exact relevance reasons, broader stable session references, optional advisory ranking and measured context improvement remain open. |
| Diagnostics/repair metadata retained with the workspace | Partial. Existing compiler diagnostics and repair discovery are separate; invalid source does not produce an admitted Image. Missing integrated symbol-linked diagnostic/repair inventory and incremental refresh. |

## Phase 2: candidate model

| Requirement | Status and remaining evidence |
| --- | --- |
| Immutable overlays against one immutable base, branching and discard | Candidate authored/unrun. Applying returns a new value; siblings retain their base, dropping discards. Candidate-only v2 retains bounded candidate/draft registries and exposes discard. No durable registry or recovered branch lifecycle. |
| Versioned Semantic Change IR and mandatory constraints | Partial, Candidate authored/unrun for five closed intention kinds. Base revision, identities, exports, effects, permits, exact contract inventory changes and profile/core-target preservation are checked in that slice. General operation constraints and semantic-delta proof for all intention kinds remain open. |
| Typed expression/declaration constructors | Partial. Candidate body constructors cover a bounded scalar/parameter/operator/call surface. General expressions, owned values, declaration constructors and expected-type/effect/ownership-guided discovery are missing. |
| Ephemeral typed holes | Partial, authored/unrun Holes. Immutable body-hole drafts report expected type, parameter scope, effect budget, contracts, accessible calls and explicitly prior-body loan/cleanup facts; filling performs complete candidate admission. Unresolved drafts expose no candidate/source materialization API. General expression holes, recursive incomplete declarations and complete next-expression ownership guidance remain open. |
| Candidate ID, base/candidate revisions, semantic/source-diff digests, validation/diagnostics/gates | Partial, Candidate authored/unrun. Complete successful candidates carry digests, source changes, validation facts and required gates. Invalid/incomplete candidate state with queryable unresolved diagnostics is missing. |
| Candidate comparison, targeted validation and exact semantic replay | Partial. Candidate comparison is descriptive target overlap; source is formatted, reparsed and rebuilt with complete Project admission. Need semantic compatibility decisions, intended-delta verification across general transformations and selective invalidation/validation. |

## Phase 3: all eleven requested operations

| Operation | Present scope and remaining requirement |
| --- | --- |
| `rename_declaration` | Partial. Existing managed declaration/import-alias operations and Project display rename; Candidate adds explicit monomorphic non-main function display rename. General types/fields/interfaces, candidate discovery and all dependent-reference cases remain. |
| `change_function_signature` | Partial, authored/unrun. Append scalar parameters with literal arguments, or retain/reorder/remove existing by-value built-in Copy parameters with hygienic left-to-right staging of every original argument. Parameter renaming, type/result changes, ownership-sensitive mapping, dependent declarations and external consumer migration remain missing. |
| `replace_expression` | Partial, authored/unrun. Body-expression selection uses actual revision-scoped HIR identity and unambiguous canonical AST provenance; replacement uses authenticated lexical scope, expected-type/ownership checks and full Project admission. Contract-region replacement, generic/synthetic selections and general typed constructors remain open. |
| `replace_function_body` | Partial, Candidate authored/unrun: bounded typed constructors for explicit monomorphic non-main functions followed by full source admission. General body/control/data/ownership shapes remain. |
| `extract_function` | Missing stable-ID synthesis, capture/parameter/ownership derivation, call substitution and replay evidence. |
| `add_declaration` | Missing typed declaration operation, placement/namespace/identity checks and dependency updates. Existing file creation with supplied source is not declaration synthesis. |
| `move_declaration` | Missing stable-ID move plus import/caller migration. Existing managed file moves are not semantic declaration moves. |
| `implement_interface` | Missing required-member discovery, typed implementation construction and contract/dispatch replay. |
| `add_record_field` | Missing constructor, match and projection migration with layout/ownership/target validation. |
| `add_contract` | Partial, authored/unrun. Append one typed requires/ensures predicate to an explicit monomorphic non-main function, preserving prior predicates and exact other invariants with full Project admission. General declaration contracts, proof of runtime satisfaction and external compatibility remain open. |
| `repair_diagnostic` | Partial. [Diagnostic Repair v1](DIAGNOSTIC-REPAIR-V1.md), [repair.rs](../src/repair.rs), and [repair evidence](../tests/diagnostic_repair_v1.rs) cover bounded ID assignment; generalized typed repairs and candidate integration are missing. |

`change/catalog <target>` now provides candidate-bound constructor discovery
in candidate-only v2 for the five supported intention classes. Unsupported
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
| Data access | Missing workspace read/write/move/field-projection index and candidate deltas. HIR contains facts but no integrated facet is claimed. |
| Interfaces | Missing workspace implementation/requirement/dispatch facet and candidate deltas. |
| Tests | Facets reports declared test module and linked-closure membership. Missing declaration/contract/diagnostic/profile coverage evidence and affected-test selection. |
| Targets | Facets reports existing Project profile admission only. Candidate derives C11/structurally validated Wasm facts; no execution. Missing generalized per-declaration admission/rejection reasons and package-profile coverage. |
| Artifacts | Facets identifies manifest-selected Web exports. Existing Native Rust/target evidence is separate. Missing unified npm/Rust/C/OpenAPI/Web stable-ID-to-artifact relationship and change-impact inventory. |
| Packages | Missing unified semantic consumer-interface relationships and cross-package migration evidence. Existing package tools are not that index. |
| Unsafe boundaries | Missing workspace unsafe entry/dependency/exposure facet and candidate-aware propagation. |
| Diagnostics | Existing source diagnostics/repair are separate. Missing workspace symbol-to-diagnostic/repair-class facet and invalid-candidate integration. |

Each generalized fact must bind source provenance, stable identity, revision,
edge kind, reason, evidence owner and authoritative/descriptive/advisory class.
Current Facets labels descriptive validated-HIR projections and source
revisions; this does not establish the missing families or their independent
replay. LoanPlan vectors remain proof data, not runtime liveness authority.

Cross-file candidate-aware impact is **Partial**: Candidate pairs base and
candidate six-family Impact reports, rather than a generalized semantic delta.
Source/semantic-diff binding and exact candidate replay are **Authored/unrun**
for the closed slice. Targeted tests plus policy-selected full gates remain
**unrun/missing integration**. Evidence-bound materialization through separate
commit authority is **Partial** in existing A0/managed Workspace routes and
**missing for the new candidate object**; a capsule cannot publish itself.

## Protocol, generated integrations and candidate lifecycle

| Requested surface | Current status |
| --- | --- |
| `protocol/capabilities`, `protocol/schemas` | Authored/unrun Protocol. Host-selected capabilities and catalogue-driven request/success/error envelopes. Additive `protocol/constructor-schemas` supplies self-contained closed typed-expression/intent/change schemas. Complete bundled response schemas and executed client/schema compatibility remain missing. |
| `workspace/open`, `workspace/status`, `query/catalog` | Authored/unrun Protocol. Host binds the manifest; open returns the retained image handle and cannot select a new path. |
| `change/catalog`, `validation/catalog` | Authored/unrun candidate-only v2. Target-specific constructor discovery and independent candidate replay are available; arbitrary payload validity requires apply. General legal-transition discovery and execution routes remain open. |
| Read-only, candidate-only, source-commit, build-enabled, test-enabled, artifact-materialization-enabled sessions | Explicit read-only v1 and candidate-only v2 authored/unrun. Source-commit, build, test and artifact profiles remain missing; no agent authority elevation is implied. |
| Version-matched agent instructions | Authored/unrun `protocol/instructions` for supported read-only and candidate-only methods. Instructions for remaining authority profiles remain open. |
| TypeScript, Python, Rust clients | Authored/unrun `protocol/client` emits small I/O-free request/result helpers. Missing independently executed cross-language compatibility suites and complete typed payload clients. |
| Optional MCP/editor adapters | Missing for this new protocol. |
| Machine-readable operation catalogue | Read query and target-specific constructor catalogues authored/unrun; general change catalogue remains partial. |
| `candidate/open`, `candidate/apply-intent` | Candidate library and candidate-only v2 methods authored/unrun; bounded immutable registry with no publication authority. |
| `candidate/query`, `candidate/validate`, `candidate/impact` | Authored/unrun v2 methods: bounded report chunks, complete independent replay and six-family impact. General incomplete-candidate validation and generalized impact remain open. |
| `candidate/test` | Missing candidate-specific host-authorized execution/affected-test route; tests are explicitly not run. |
| `candidate/compare`, `candidate/discard` | Descriptive library and v2 lifecycle methods authored/unrun. Semantic compatibility proof remains missing. |
| `candidate/commit` | Missing integration with separate evidence-gated source authority. Read-only protocol intentionally cannot perform it. |

## Phase 5: multi-agent operation

| Requirement | Status and remaining evidence |
| --- | --- |
| Semantic rebase/merge | Partial, authored/unrun Rebase. Stable-ID target/dependency conflict classification, display-normalized call facts, canonical source replay and same-root history merge cover supported intentions. General moves/declarations, ownership-sensitive reconciliation and cross-manifest merging remain open. |
| Conflict cases | Focused Rebase cases are authored/unrun: unrelated body/display rename, body/postcondition revalidation, competing signature conflicts and deleted call dependencies. Execute the matrix, expand to all intended operations and preserve expected-expression remapping before claiming full coverage. |
| Candidate branching | Partial in-memory immutable siblings and bounded candidate-only v2 registry. Persistence and recovery remain missing. |
| Parallel read-only requests | Immutable revisions are reusable, but the new NDJSON loop is sequential. Missing bounded concurrent request scheduling and deterministic isolation evidence. |
| Session recovery and content-addressed candidate persistence | Missing. Existing Store reconstructs source revisions; it does not restore candidate intentions, cursors, pending validation or authority. |
| Manual edits and stale recovery | Partial held-input absorbing invalidation, exact base rejection and library rebase onto a separately admitted revision. Candidate-only protocol selects retained revisions only; live disk refresh/session recovery and recovery benchmarks remain open. |

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
| 8. Run affected tests | Not performed; affected-test selection/integration still missing. |
| 9. Verify native/Wasm admission | Candidate C11/structural Wasm projection authored; runtime conformance and broader package targets remain open. |
| 10. Return semantic impact and human source diff | Candidate paired six-family Impact, digest-bound source diff authored; generalized semantic deltas remain open. |
| 11. Reject or semantically rebase concurrent source change | Retained-base stale rejection exists; live candidate publication race evidence and semantic rebase remain open. |
| 12. Commit only through separate authority | Candidate commit integration missing. Existing rename/A0/Workspace authorities cannot be inferred from a candidate report. |

The demonstration is not complete until one integrated executable scenario
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
