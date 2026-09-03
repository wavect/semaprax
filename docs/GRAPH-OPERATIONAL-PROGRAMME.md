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
asserts verified completion. The user requested skipping long quality gates;
the ledger therefore distinguishes the five archived focused local executions
from unselected full-profile and hosted evidence. No focused result
promotes a complete row, and broader validation remains outstanding.

## Evidence owners

| Key | Source, specification, and executable evidence |
| --- | --- |
| Image | [image.rs](../src/project/image.rs), [Image v1](SEMANTIC-WORKSPACE-IMAGE-V1.md), [image evidence](../tests/workspace/semantic_image.rs), [CLI evidence](../tests/project/semantic_image_cli.rs) |
| Facets | [image_facets.rs](../src/project/image_facets.rs), [Facets v1](SEMANTIC-IMAGE-FACETS-V1.md), [facet evidence](../tests/image_protocol/facets_v1.rs) |
| Concrete generic-function navigation | [Function Instances v1](SEMANTIC-IMAGE-FUNCTION-INSTANCES-V1.md); source-template-bound retained-instance pages and exact instance facets, callers and closure relationships, authored/unrun; no new instantiation or execution |
| Protocol | [image_transport.rs](../src/image_transport.rs), [Image Agent Protocol v1](IMAGE-AGENT-PROTOCOL-V1.md), [protocol evidence](../tests/image_protocol/transport_v1.rs) |
| Candidate | [candidate module](../src/project/candidate/mod.rs), [intent constructors](../src/project/candidate/intent.rs), [Candidates v1](PROJECT-CANDIDATES-V1.md), [candidate evidence](../tests/project_candidate/candidates.rs) |
| Candidate protocol | [candidate transport](../src/image_transport/candidates.rs), [Candidate Protocol v2](IMAGE-CANDIDATE-PROTOCOL-V2.md), [protocol evidence](../tests/image_protocol/candidate_transport_v2.rs) |
| Holes | [draft module](../src/project/candidate/draft.rs), [Typed Holes v1](PROJECT-CANDIDATE-HOLES-V1.md), [hole evidence](../tests/project_candidate/holes.rs) |
| Signature mapping | [signature engine](../src/project/candidate/signature.rs), [Signature Evolution v1](PROJECT-SIGNATURE-EVOLUTION-V1.md) |
| Computed signature arguments | [Argument Expressions v1](PROJECT-SIGNATURE-ARGUMENT-EXPRESSIONS-V1.md); explicit scalar and stable-ID Copy nominal computations over staged original parameters, caller-local annotations and rebuilt checked-signature admission, authored/unrun |
| Expression changes | [expression module](../src/project/candidate/expression.rs), [Expression Change v1](PROJECT-EXPRESSION-CHANGE-V1.md), [expression evidence](../tests/project_candidate/expression.rs) |
| Immutable lexical bindings | [Lexical Binding Constructor v1](PROJECT-LEXICAL-BINDING-CONSTRUCTOR-V1.md); scoped initializer reuse through ordinary candidate admission, authored/unrun |
| Compiler-owned byte calls | [Builtin Call Constructor v1](PROJECT-BUILTIN-CALL-CONSTRUCTOR-V1.md); stable-ID selection of seven existing operations, shared schemas/discovery and complete source replay, authored/unrun; direct authenticated field places remain open |
| Contract changes | [intent module](../src/project/candidate/intent.rs), [Contract Change v1](PROJECT-CONTRACT-CHANGE-V1.md), [candidate evidence](../tests/project_candidate/candidates.rs) |
| Rebase | [rebase module](../src/project/candidate/rebase.rs), [Candidate Rebase v1](PROJECT-CANDIDATE-REBASE-V1.md), [rebase evidence](../tests/project_candidate/rebase.rs) |
| Declaration creation | [function declaration module](../src/project/candidate/declaration.rs), [type declaration module](../src/project/candidate/type_declaration.rs), [Declaration Change v1](PROJECT-DECLARATION-CHANGE-V1.md), [function evidence](../tests/project_candidate/declaration.rs), [type evidence](../tests/project_candidate/type_declarations.rs) |
| Extraction | [extraction module](../src/project/candidate/extraction.rs), [Extraction v1](PROJECT-EXTRACTION-V1.md), [extraction evidence](../tests/project_candidate/extraction.rs) |
| Recovery | [recovery module](../src/project/candidate/recovery.rs), [Recovery v1](PROJECT-CANDIDATE-RECOVERY-V1.md), [recovery evidence](../tests/project_candidate/recovery.rs) |
| Typed-hole restart recovery | [draft recovery module](../src/project/candidate/draft_recovery.rs), [Draft Recovery v1](PROJECT-CANDIDATE-DRAFT-RECOVERY-V1.md), [library evidence](../tests/project_candidate/draft_recovery.rs), [v5 evidence](../tests/image_transport_v5/draft_recovery.rs) |
| Self-contained draft recovery | [Draft Archive v1](PROJECT-CANDIDATE-DRAFT-ARCHIVE-V1.md); canonical original sources plus valid history and pending selectors, source-independent library restore, startup-only historical host restore and current-base RPC restore, authored/unrun |
| Durable typed drafts | [Draft Persistence v1](DRAFT-ARCHIVE-PERSISTENCE-V1.md); shared immutable archive store with independent typed replay, explicit persist/load commands and closed startup-policy v6 selection, authored/unrun |
| Draft semantic rebase | [Draft Rebase v1](PROJECT-CANDIDATE-DRAFT-REBASE-V1.md); checked-history rebase, source-region conflicts and authenticated remapping of pending body/expression/contract holes, without implicit completion, authored/unrun |
| Draft semantic merge | [Draft Merge v1](PROJECT-CANDIDATE-DRAFT-MERGE-V1.md); common-base history merge, opposing-write checks, independent pending-selector rebinding and bounded union without inferred completion, authored/unrun |
| Candidate archive persistence | [Archive v1](PROJECT-CANDIDATE-ARCHIVE-V1.md), [immutable Archive Store](CANDIDATE-ARCHIVE-STORE-V1.md), [CLI](CANDIDATE-ARCHIVE-CLI-V1.md); independently rebuilt original source and exact complete history |
| Startup archive recovery | [Workspace Archive Recovery](IMAGE-WORKSPACE-ARCHIVE-RECOVERY-V1.md); host-only historical candidate retention with live authentication and unchanged approval boundary |
| Declaration moves | [movement module](../src/project/candidate/movement.rs), [Declaration Move v1](PROJECT-DECLARATION-MOVE-V1.md) |
| Nominal declaration rename | [Nominal Rename v1](PROJECT-NOMINAL-RENAME-V1.md); shared authenticated occurrence planning for explicit source records/variants, full candidate replay and conservative nominal rebase, authored/unrun |
| Member rename | [Member Rename v1](PROJECT-MEMBER-RENAME-V1.md); shared cross-file occurrence planning for explicit record fields, variant cases and payload fields, full source replay and conservative owner-shape conflicts, authored/unrun |
| Record-field changes | [field migration](../src/project/candidate/record_field.rs), [Record Field Change v1](PROJECT-RECORD-FIELD-CHANGE-V1.md) |
| HIR relationships | [relationship facets](../src/project/image_facets/relationships.rs), [HIR Relationships v1](SEMANTIC-IMAGE-HIR-RELATIONSHIPS-V1.md) |
| Declaration dependencies | [shared index](../src/project/image_dependencies.rs), [Dependencies v1](SEMANTIC-IMAGE-DEPENDENCIES-V1.md); lazy immutable-image access and caller indexes shared with candidate deltas, authored/unrun |
| Compact declaration navigation | [Dependency Navigation v1](SEMANTIC-IMAGE-DEPENDENCY-NAVIGATION-V1.md); bounded summaries and reference-bound sites/callers/calls/members pages over the shared index, authored/unrun |
| Cross-session function references | [Function Reference v1](SEMANTIC-IMAGE-FUNCTION-REFERENCE-V1.md); canonical exact-image function/facet selectors with source provenance, fresh summary/handle resolution after an identical rebuild, closed v5 reads and detached batch support, authored/unrun |
| Candidate tests | [test planning/execution](../src/project/candidate/testing.rs), [Candidate Tests v1](PROJECT-CANDIDATE-TESTS-V1.md), [Test Protocol v3](IMAGE-CANDIDATE-TEST-PROTOCOL-V3.md) |
| Candidate diagnostics | [attempt/repair module](../src/project/candidate/diagnostics.rs), [Candidate Diagnostics v1](PROJECT-CANDIDATE-DIAGNOSTICS-V1.md) |
| Managed publication | [publication bridge](../src/project/candidate/publication.rs), [Candidate Publication v1](PROJECT-CANDIDATE-PUBLICATION-V1.md) |
| Image lifecycle | [image_store.rs](../src/project/image_store.rs), [Image Store v1](SEMANTIC-IMAGE-STORE-V1.md) |
| Semantic deltas | [delta.rs](../src/project/candidate/delta.rs), [Semantic Delta v1](PROJECT-CANDIDATE-SEMANTIC-DELTA-V1.md) |
| Contract deltas | [Contract Delta v1](PROJECT-CANDIDATE-CONTRACT-DELTA-V1.md); whole-candidate predicate and static callable-dependency comparisons, exact replay and v5 chunks, authored/unrun |
| Ownership deltas | [Ownership Delta v1](PROJECT-CANDIDATE-OWNERSHIP-DELTA-V1.md); checked signatures, structural inventories and complete ordered loan/cleanup comparisons with exact replay, authored/unrun |
| Cleanup dependencies | [Cleanup Dependencies v1](SEMANTIC-IMAGE-CLEANUP-DEPENDENCIES-V1.md); reverse type/case/field selection over actual retained inventory/cleanup/loan facts, original plan coordinates and candidate before/after review, authored/unrun |
| Artifact deltas | [Artifact Delta v1](PROJECT-CANDIDATE-ARTIFACT-DELTA-V1.md); actual base/candidate Web/npm, [OpenAPI](IMAGE-OPENAPI-ARTIFACTS-V1.md) and [C source](IMAGE-C-ARTIFACTS-V1.md) file and stable export comparisons after full replay, under the existing build grant, authored/unrun |
| Candidate artifact boundary evidence | [Candidate Analysis Artifact Evidence v1](PROJECT-CANDIDATE-ANALYSIS-ARTIFACT-EVIDENCE-V1.md); exact candidate coverage plus one independently replayed selected pathless carrier delta, changing only generated artifacts to partial; library and v5 build-granted chunk evidence authored/unrun |
| Candidate runtime boundary evidence | [Candidate Analysis Runtime Evidence v1](PROJECT-CANDIDATE-ANALYSIS-RUNTIME-EVIDENCE-V1.md); exact candidate coverage plus one independently replayed, policy-bounded reference-interpreter test closure, changing only runtime environment to partial; library evidence authored/unrun |
| Candidate function facets | [Candidate Function Facets v1](PROJECT-CANDIDATE-FUNCTION-FACETS-V1.md); final-candidate compact summaries and all nine existing HIR facet pages with candidate-bound handles and cursors, authored/unrun |
| Diagnostic protocol | [Diagnostic Protocol v4](IMAGE-CANDIDATE-DIAGNOSTIC-PROTOCOL-V4.md) |
| Integrated managed workflow | [Workflow v1](PROJECT-GRAPH-OPERATIONAL-WORKFLOW-V1.md), [authored scenario](../tests/project_graph_operational_workflow_v1.rs) |
| Focused canonical-Git execution evidence | [Execution Evidence v1](GRAPH-OPERATIONAL-EXECUTION-EVIDENCE-V1.md); exact subject `474c481bf3c3561c144e077f0000460f61af55f2` passed the locked/offline local three-test selection with authenticated SHA-1/SHA-256 economics exports; managed, generated-client, MCP, target-runtime, hosted and completion dimensions remain unselected or unclaimed |
| Same-subject Phase 0 aggregate evidence | [Phase 0 Evidence v1](GRAPH-OPERATIONAL-PHASE0-EXECUTION-EVIDENCE-V1.md); exact subject `d85566f0682df0d236f7df3023479dc0ea50d450` freshly passed the canonical-Git, generated-client/MCP v2, real Visual Studio Code host and independent Python MCP SDK 1.27.0 components under one recursively authenticated bundle; managed `ACTIVE`, exact tag, remote/later head, hosted cross-platform, target runtime, full quality and programme completion remain unclaimed |
| Frontend reuse | [Frontend Cache v1](PROJECT-FRONTEND-CACHE-V1.md), [incremental.rs](../src/project/incremental.rs) |
| Checked-module reuse | [Semantic Cache v1](PROJECT-SEMANTIC-CACHE-V1.md), [authored cases](../tests/project/semantic_cache.rs); exact synthetic AST/HIR retention within one compiler process |
| Persistent checked-module reuse | [Persistent Cache v1](PERSISTENT-SEMANTIC-CACHE-V1.md), [authenticated store](SEMANTIC-CACHE-STORE-V1.md); private complete codec, compiler-file/key binding before decoding, source reparse and warm full-Project replay |
| Live frontend reuse | [Workspace Frontend Cache v1](IMAGE-WORKSPACE-FRONTEND-CACHE-V1.md), [cached session cases](../tests/image_protocol/workspace_frontend_cache_v1.rs); exact authenticated source loading and transactional cache adoption |
| Parallel image reads | [Parallel Reads v1](IMAGE-PARALLEL-READS-V1.md), [batch cases](../tests/image_protocol/parallel_reads_v1.rs); embedding-host scoped workers, not a concurrent stdio server |
| Parallel retained reads | [Parallel Retained Reads v1](IMAGE-PARALLEL-CANDIDATE-READS-V1.md); selected immutable candidate/draft/attempt inputs, shared sequential payload handlers and authored/unrun parity evidence |
| Host-selected protocol batches | [Parallel Read Protocol v1](IMAGE-READ-BATCH-PROTOCOL-V1.md); explicit v7 startup worker selection, unchanged aggregate wire caps, generated schemas/clients and MCP method selection, authored/unrun |
| Retained session subjects | [Retained Subjects v1](IMAGE-RETAINED-SUBJECTS-V1.md); bounded deterministic candidate/draft/rejected-attempt handles and registry-local associations, explicitly outside immutable batches, authored/unrun |
| Checked hole-fill suggestions | [Fill Suggestions v1](PROJECT-HOLE-FILL-SUGGESTIONS-V1.md); exact type/effect-guided place/call enumeration with ordinary source fill replay, no retained previews or intent/runtime-contract proof, authored/unrun |
| Integrated canonical Git workflow | [Git Workflow v1](PROJECT-GRAPH-OPERATIONAL-GIT-WORKFLOW-V1.md), [real-provider scenarios](../tests/project_graph_operational_git_workflow_v1.rs); exact local subject evidence passed SHA-1, SHA-256 and stale-ref cases, with broader provider/platform evidence still open |
| Supported product workflow | [Product Workflow v1](IMAGE-SUPPORTED-PRODUCT-WORKFLOW-V1.md), [Response Accountability v1](IMAGE-SUPPORTED-WORKFLOW-RESPONSE-ACCOUNTABILITY-V1.md), and [Phase 1 Product Workflow Evidence v1](GRAPH-OPERATIONAL-PHASE1-PRODUCT-WORKFLOW-EXECUTION-EVIDENCE-V1.md); exact-subject local evidence passed the complete Python, Rust, and explicitly provisioned TypeScript compositions over isolated local Unix bare SHA-256 repositories, including ten closed hostile transitions. Later focused work adds typed v5 application diagnostics and closed per-step authority/blind-spot contracts, without transferring the earlier execution result. This is one bounded scalar workflow only; a packaged workflow driver, general intentions and ownership, MCP/editor transport, cancellation, hosted/cross-platform, target-runtime, full-quality, and programme evidence remain open |
| Expression holes | [Expression Holes v1](PROJECT-CANDIDATE-EXPRESSION-HOLES-V1.md) |
| Contract expression holes | [Contract Expression Holes v1](PROJECT-CANDIDATE-CONTRACT-HOLES-V1.md); source-replayed pre/postcondition subtree changes, shared draft/recovery lifecycle and v5 discovery, authored/unrun |
| Typed repair change | [Diagnostic Change v1](PROJECT-DIAGNOSTIC-CHANGE-V1.md) |
| Static protocol conformance | [Static Protocol Conformance v1](STATIC-PROTOCOL-CONFORMANCE-V1.md), [static_protocol.rs](../src/static_protocol.rs) |
| Interface implementation | [Interface Change v1](PROJECT-INTERFACE-CHANGE-V1.md), [interface.rs](../src/project/candidate/interface.rs) |
| Conformance discovery | [Image Protocol Conformance v1](IMAGE-PROTOCOL-CONFORMANCE-V1.md); source-bound sidecar and v4 query/catalogue integration |
| Typed discovery | [Agent Discovery v5](IMAGE-AGENT-DISCOVERY-V5.md); runtime-selected request/envelope schemas, typed outer client parameters and explicitly opaque compiler-report schemas |
| Typed responses | [Typed Response Clients v1](IMAGE-TYPED-RESPONSE-CLIENTS-V1.md); exact local subject evidence executes Python, Rust and provisioned TypeScript response consumers for the selected bounded schemas; heterogeneous compiler reports remain open |
| Typed requests | [Typed Request Clients v1](IMAGE-TYPED-REQUEST-CLIENTS-V1.md) and [Client/MCP Evidence v2](GRAPH-OPERATIONAL-CLIENT-MCP-EXECUTION-EVIDENCE-V2.md); one exact local subject executes Python, offline compiled Rust and provisioned TypeScript request construction against real compiler admission, including an exact hostile unbound-place rejection; packaged SDK and exhaustive generated-method coverage remain open |
| Compact hole navigation | [Hole Navigation v1](PROJECT-HOLE-NAVIGATION-V1.md); typed summaries and context-bound scope/call/obligation/constructor pages for all three hole kinds, authored/unrun; full contexts and pending-state authority unchanged |
| Unified workspace protocol | [Workspace Protocol v5](IMAGE-WORKSPACE-PROTOCOL-V5.md), [startup CLI](WORKSPACE-SESSION-CLI-V1.md) |
| MCP integration | [MCP Adapter v1](IMAGE-MCP-ADAPTER-V1.md), [Client/MCP Evidence v2](GRAPH-OPERATIONAL-CLIENT-MCP-EXECUTION-EVIDENCE-V2.md), and [Phase 0 Evidence v2](GRAPH-OPERATIONAL-PHASE0-EXECUTION-EVIDENCE-V2.md); one exact local subject passes the authored adapter/stdio gates and an independent provisioned Python `mcp` SDK 1.27.0 interoperability profile including bounded catalogue paging and notification nonexecution; full MCP certification, HTTP/cancellation and hosted cross-platform evidence remain open |
| Editor source review | [Source Review v1](PROJECT-CANDIDATE-SOURCE-REVIEW-V1.md), [VS Code Adapter v1](VSCODE-SAVED-SOURCE-ADAPTER-V1.md), and [focused host evidence](GRAPH-OPERATIONAL-VSCODE-HOST-EXECUTION-EVIDENCE-V1.md); exact local subject `2888f84f123b7caa44aa6807388d98f851d4beaf` executes the compiler-backed typed rename, verified virtual diff and dirty-buffer invalidation without saved-source writes. Packaging, manual UI, minimum-version, hosted and cross-platform evidence remain open |
| Editor typed holes | [VS Code Adapter v1](VSCODE-SAVED-SOURCE-ADAPTER-V1.md#typed-hole-workflow); three-kind hole planning, compact facets, explicit checked-suggestion selection into bound fill scratches and separate completion, authored/unrun; editor-host execution remains open |
| Editor diagnostic repair | [VS Code Adapter v1](VSCODE-SAVED-SOURCE-ADAPTER-V1.md#diagnostic-attempts-and-explicit-repair); separate rejected attempts, exact-byte report inspection and explicit compiler repair selectors, authored/unrun with mock controller regressions; richer repair UX and actual editor-host execution remain open |
| Draft expression discovery | [Draft Expression Catalogue v1](PROJECT-DRAFT-EXPRESSION-CATALOG-V1.md); last-valid body/contract selection after fills, exact draft bindings, typed v5 discovery and editor integration, authored/unrun; no implicit candidate release |
| Protocol source commit | [Source Commit Protocol v5](IMAGE-SOURCE-COMMIT-PROTOCOL-V5.md); independently selected startup authority and exact approval |
| Target/artifact queries | [Target and Artifact Projections v1](IMAGE-TARGET-ARTIFACTS-V1.md), [OpenAPI Artifacts v1](IMAGE-OPENAPI-ARTIFACTS-V1.md) and [C Artifacts v1](IMAGE-C-ARTIFACTS-V1.md); actual pathless source-bound carrier construction and replay |
| Canonical Git publication | [Git Publication v1](PROJECT-CANDIDATE-GIT-PUBLICATION-V1.md), [explicit host CLI](CANDIDATE-GIT-PUBLICATION-CLI-V1.md) |
| Store | [project_revision_store.rs](../src/project_revision_store.rs), [Store v1](PROJECT-REVISION-STORE-V1.md), [Windows-entry v1](PROJECT-REVISION-STORE-WINDOWS-V1.md), [store evidence](../src/project_revision_store/tests.rs) |
| Analysis | [workspace_analysis.rs](../src/workspace_analysis.rs), [Workspace Analysis v1](WORKSPACE-ANALYSIS-V1.md); retained six-family typed indexes and existing Context/Impact/Review |
| Existing mutation | [semantic workspace operations](../src/semantic_workspace_operations.rs), [Operations v1](SEMANTIC-WORKSPACE-OPERATIONS-V1.md), [operation evidence](../tests/workspace/semantic_operations.rs); [Project rename](PROJECT-RENAME-TRANSACTION-V1.md) and [rename evidence](../tests/project/agent_transport_rename.rs) |
| Existing evidence | [patch evidence](../src/patch_evidence.rs), [workspace change](../src/semantic_workspace_change.rs), [target evidence](../src/target_evidence.rs), their versioned specifications and focused suites |
| Economics | [agent_economics.rs](../src/agent_economics.rs), [Agent Context Economics v1](AGENT-ECONOMICS-V1.md), [economics evidence](../tests/agent_economics.rs) |

These owners identify review and validation work; their presence does not
assert current-head green results. Candidate and Protocol refer to the current
working implementation alongside the prior Image foundation.

## Phase 1: fast semantic workspace

| Requirement | Status and remaining evidence |
| --- | --- |
| Persistent, content-addressed derived HIR and graph snapshots | Partial, authored/unrun. A separate authenticated store now retains complete checked module HIR, synthetic inputs, canonical sources and graph projections. Fresh processes can decode only after MAC and compiler-file checks, then reparse source and require zero-resolver full-Project replay with exact graph equality. Ordinary Image Store remains cold. Executed cross-process, corruption, recovery, profile breadth and measured performance evidence remain missing; no complete session/checkpoint recovery claim. |
| Identity binds compiler, graph/HIR compatibility, manifest, ordered paths/digests and profiles | Partial, authored/unrun. Image identity stays unchanged. Persistent envelopes additionally bind exact compiler executable bytes, codec/header compatibility, host width/endianness/platform and the complete source/manifest/HIR payload. This requires an immutable static installation from exec and protected host key custody; it does not attest loaded code, dynamic libraries or a hostile same-principal process. Cross-build rejection cases still require execution. |
| Derived, deletable, rebuildable, Git-excluded, revision-bound; no graph-only meaning | Authored/unrun Image/Store boundaries. Image replay reconstructs from admitted source revision; `.semaprax-images/` is ignored. Still require integrated cache deletion/recovery/corruption and stale-source lifecycle evidence. |
| Incremental invalidation/rechecking | Partial authored/unrun. Exact synthetic AST, imported stubs and manifest context govern checked-module reuse, including explicitly restored caches. Changed providers invalidate reverse import dependents. Eligible hits skip source resolution, but HIR validation, cross-file checks, linking, graph and profile admission rerun. Owned-source ImageWorkspace and authenticated v5 refresh share transactional adoption; preview/failed work is discarded. Persistent load reparses original source and rebuilds graph/indexes while reusing HIR. Function-level reuse, target-work reuse and executed equivalence/performance evidence remain missing. |
| Expanded symbol and reverse indexes | Partial, authored/unrun. Stable-ID lookup and six-family indexes exist; the shared immutable-image dependency index adds source-bound field/type/case use sites and local/cross-file direct caller closure, exposed through a read-only v5 query and reused by candidate deltas. General package/artifact consumers and measured index benefits remain open. |
| Capability-negotiated discovery | Partial, authored/unrun. Host-selected read-only v1, candidate-only v2, fixed-policy test v3 and diagnostic v4 remain preserved. Additive v5 combines optional candidate/diagnostic/test/pathless-build capabilities and a separately attached startup-approved Git commit host. Artifact filesystem materialization and request-driven elevation remain absent. |
| Compact summaries/references/facet expansion | Partial, authored/unrun. Function facets and declaration-dependency summaries expose revision-bound handles and paginated detail. Generic-function navigation adds source-template-bound retained instances with exact instance facets, caller identities and entry/test closure joins without inventing instantiations or execution evidence. Dependency pages add source-bound caller relevance reasons and bind page size/output limits without embedding complete reports in summaries. Candidate-bound dependency pages reuse those four views over changed and introduced declarations with history-isolated handles/cursors and no retained derived image. Typed-hole summaries add exact-context references and bounded scope/call/obligation/constructor pages while retaining full prior proof contexts separately. A live v5 retained-subject inventory now recovers bounded candidate/draft/rejected-attempt references and registry-local associations without replaying their reports; it stays outside immutable batches. Exact-revision function references now carry one stable-ID/facet selector across processes and resolve only against an identical rebuilt image, deriving the current summary and handle anew. Cross-revision migration, type/candidate/draft references, cursor continuation, optional advisory ranking and measured task-level context improvement remain open. |
| Diagnostics/repair metadata retained with the workspace | Partial. Existing compiler diagnostics and repair discovery are separate; invalid source does not produce an admitted Image. Missing integrated symbol-linked diagnostic/repair inventory and incremental refresh. |

## Phase 2: candidate model

| Requirement | Status and remaining evidence |
| --- | --- |
| Immutable overlays against one immutable base, branching and discard | Candidate authored/unrun. Applying returns a new value; siblings retain their base, dropping discards. Candidate-only v2 retains bounded candidate/draft registries and exposes discard. Explicit draft capsules now recover pending selectors and prior valid history through source replay, retaining no complete candidate until completion. Automatic durable registry and complete recovered branch lifecycle remain open. |
| Versioned Semantic Change IR and mandatory constraints | Partial, Candidate authored/unrun for all eleven requested operation classes, including replayed diagnostic repair and static protocol implementation. Base revision, exact identity additions/relocations, exports, effects, permits, exact contract inventory changes and profile/core-target preservation are checked in that slice. General interface behavior, broader constraints and complete semantic-delta proof remain open. |
| Typed expression/declaration constructors | Partial, authored/unrun. Candidate constructors cover bounded scalar/parameter/operator/call expressions, stable-ID record and variant construction with explicit direct-scalar generic arguments, authenticated Option/Result cases, record-field value projection, direct field places with authenticated nominal roots, subset record updates with base-first evaluation, exhaustive variant value matching with exact-owner typed staging and arm-local payload bindings, monomorphic function declarations with limited ownership modes and stable-ID Copy nominal parameters/returns, and explicit monomorphic record/variant declaration creation with checked resource-free data fields. Fields can select direct scalar/String/Bytes types or already visible nominal owners; no new import or borrowed/resource storage is introduced. Field places compose with compiler-owned byte calls under the existing direct owned-field borrow profile, without staging a root temporary. Catalogue/hole discovery exposes bindings, checked templates, field/case owners and compiler provenance; full candidate replay, exact added identity inventories and checked type facts own admission. Nested/named generic arguments, general nested borrowing, general/ownership-aware patterns, generic/resource type creation, and general declarations remain missing. Bounded place/direct-call hole suggestions now use exact type/effect prefilters followed by ordinary full fill replay, dropping preview drafts; broader constructor search, runtime contract/intent proof and executed guidance evidence remain open. |
| Ephemeral typed holes | Partial, authored/unrun. Body, body-expression and contract-expression holes expose checked lexical context, expected type/ownership and last-valid proof facts. Filling performs complete candidate admission and remaps surviving selections; unresolved drafts cannot materialize. Contract holes distinguish phase/predicate/subtree, enforce pure predicates and coexist with body regions. Explicit recovery rebuilds prior valid history and re-creates pending holes under exact draft/capsule identities. Recursive incomplete declarations, general incomplete-state diagnostics, next-expression liveness guidance and executed evidence remain open. |
| Candidate ID, base/candidate revisions, semantic/source-diff digests, validation/diagnostics/gates | Partial, Candidate authored/unrun. Complete candidates carry digests, diffs, validation facts and gates. V4 adds bounded rejected-attempt lifecycle and repair discovery without invalid source/image access. General incomplete-state diagnostics remain missing. |
| Candidate comparison, targeted validation and exact semantic replay | Partial. Original comparison is descriptive target overlap; additive read-only merge preview performs full merge replay in both orders, reports admission/rejection, and compares actual accepted canonical source without retaining a candidate. Source is formatted, reparsed and rebuilt with complete Project admission. General semantic compatibility decisions, intended-delta verification across general transformations, selective invalidation/validation and executed preview evidence remain open. |

The additive [literal constructors](PROJECT-LITERAL-CONSTRUCTORS-V1.md) cover
bounded owned string contents and explicit fixed byte arrays through ordinary
source replay. The [scalar literal extension](PROJECT-SCALAR-LITERAL-CONSTRUCTORS-V1.md)
adds exact character and finite IEEE encodings to recursive expressions and
both signature-default forms, completing the eight built-in Copy scalars while
leaving record defaults, diagnostic repair and computed argument selectors on
their explicit narrower grammars. Both extensions are authored/unrun; source
ownership/provenance and target admission remain unchanged, and repeat arrays
and general constructor search remain outside their scope.
The existing seven String intrinsics are also selectable through compiler-owned
typed builtin calls, with exact parameter ownership and separate byte/string
evidence owners. Discovery and eligible declaration movement share those
identities. This authored, unrun extension does not widen String target
profiles, source imports or runtime authority.

## Phase 3: all eleven requested operations

All eleven requested classes now have bounded **authored/unrun** slices. This
counts represented operation classes, not completed general operations, runtime
interface support, passing tests or completion-matrix promotions. Each row keeps
its broader requirement open.

| Operation | Present scope and remaining requirement |
| --- | --- |
| `rename_declaration` | Partial, authored/unrun. Candidate supports explicit monomorphic non-main functions, source record/variant display renames and explicit record fields/variant cases/payload fields through the shared authenticated Operations occurrence collector. Cross-file member labels migrate while stable identities and type import aliases remain unchanged. Generic/owned nominal shapes retain ordinary source/profile admission; unsupported reference pairs fail closed. Catalogue discovery, owner-shape conflict guards and conservative test relevance are included. Interface and other declaration renames, broader reference forms, complete merge normalization and executed evidence remain open. |
| `change_function_signature` | Partial, authored/unrun. Scalar append and ordered mapping include hygienic parameter display renaming, checked nominal Copy record/variant retention/reordering/removal, direct Bytes and checked resource-free String/nominal owner retention/reordering, and fresh scalar or stable-ID Copy nominal parameters computed from staged original arguments. Provider and caller type bindings are resolved independently. Exact rebuilt HIR TypeFacts govern new named eligibility, retained facts govern original parameters, and nominal identities participate in rebase conflicts. Owners cannot be dropped or duplicated, including bare String parameters whose owning mode is implicit in source. Type/result conversion, broader borrow/resource mapping, dependent declarations and external consumer migration remain missing. |
| `replace_expression` | Partial, authored/unrun. Body-expression selection uses actual revision-scoped HIR identity and unambiguous canonical AST provenance; replacement uses authenticated lexical scope, expected-type/ownership checks and full Project admission. A separate `replace_contract_expression` intention supports pre/postcondition subtrees with exact requested-source reconstruction and conservative rebase, leaving body-only behavior unchanged. Generic/synthetic selections, general typed constructors and executed evidence remain open. |
| `replace_function_body` | Partial, Candidate authored/unrun: bounded typed constructors for explicit monomorphic non-main functions followed by full source admission. General body/control/data/ownership shapes remain. |
| `extract_function` | Partial, authored/unrun. Actual HIR expression selection, immutable scalar and checked Sized Copy nominal capture/result derivation, whole-root capture for field reads, fresh caller-selected function identity, in-place call substitution and exact source replay. A nested-block lane retains internal resource-free owners inside their original lexical cleanup boundary under a fresh helper root; source/HIR correspondence checks local values and stable identities. Nominal values and bindings use retained compiler TypeFacts under the existing shared type/byte bounds. Owned results, owned/borrowed/mutable captures, root-body relocation with owners, unsafe-boundary relocation and general control/data forms remain excluded. |
| `add_declaration` | Partial, authored/unrun. Typed monomorphic function append and explicit monomorphic record/variant creation in an anchor's module with global identity/namespace checks and exact invariant extension. New data types bind every owner/case/field identity and admit direct scalar/String/Bytes fields or existing stable-ID nominal fields through checked sized/resource-free facts. Nominal field dependencies participate in intermediate rebase checks; later intentions can evolve and use those types. Function creation composes with local owning nominal helpers and bare String signatures through requested-mode and rebuilt HIR checks. Copy nominal signatures retain direct-scalar local generic instances and authenticated prelude types; owning nominal parameters are monomorphic, sized, non-Copy and resource-free with cleanup. Imports and target profiles remain unchanged. General declaration kinds, generic/resource type creation, broader borrowing modes, recursive construction and placement remain open. |
| `move_declaration` | Partial, authored/unrun. Explicit monomorphic scalar, String and checked resource-free data planning preserves identity, migrates stable-ID call/type import bindings and aggregate syntax, and replays exact source plus rebuilt semantic identities. Internal owned byte work uses authenticated compiler operations without new staging. Existing import rules still reject owning parameter/type imports; planning an owned nominal move does not establish cross-module admission. Fixed exports, borrowed signatures, generic source-type imports and audit-bearing relocation remain excluded. |
| `implement_interface` | Partial, authored/unrun. Compiler-derived required-member discovery and a typed mapping bind an explicit local monomorphic record to a same-module protocol and eligible existing functions. Exact member coverage/signature matching and canonical source replay are checked. Conservative rebase and same-root merge retain an admitted intention only while the exact compiler-owned receiver, protocol, method and selected-function conformance facts remain unchanged and the implementation identity and receiver/protocol pair remain vacant; full replay still decides admission. General interface contracts, cross-module implementations, generic/owned receiver forms, behavioral compatibility, runtime witnesses and dynamic dispatch remain missing. |
| `add_record_field` | Partial, authored/unrun. Appends one inert i64/bool/i32/u8/usize field to an existing checked sized resource-free record, including owned storage beyond flat Bytes when ordinary source admission permits it. It migrates constructors and admitted exact patterns, preserves old field identities and owning bindings, and revalidates complete Project/layout/loan/cleanup/target admission. Generic/class/variant targets, new owning fields, broader pattern/borrow profiles and general record evolution remain open. |
| `add_contract` | Partial, authored/unrun. Append one typed requires/ensures predicate to an explicit monomorphic non-main function, preserving prior predicates and exact other invariants with full Project admission. General declaration contracts, proof of runtime satisfaction and external compatibility remain open. |
| `repair_diagnostic` | Partial, authored/unrun. Existing ID repair remains separate. Candidate Diagnostics and v4 retain rejected attempts; typed wire rederives integer-literal retagging and direct owned-byte field borrow repairs and preserves them in replayable history. Byte-field repair requires actual SPX-T266 rejection and complete candidate admission without weakening source borrowing rules. Selectors bind the exact predecessor and reject rebase/reminting. General repair classes remain missing. |

`change/catalog <target>` now provides candidate-bound constructor discovery
in candidate-only v2 for supported intention classes. Unsupported
targets expose no operations; each payload still requires full admission.
Discovery of fully proven legal transitions remains open; this catalogue does
not advertise the aspirational table above. Existing [hygienic generation](HYGIENIC-GEN-V1.md) is related typed
synthesis, not an implementation of the missing change operations.

## Phase 4: generalized impact, facets and evidence

| Required facet family | Present evidence and remaining work |
| --- | --- |
| Contracts | Partial, authored/unrun. Facets expose actual pre/postcondition expressions. Whole-candidate Contract Delta compares ordered predicates and their static callable dependencies with exact candidate replay and source provenance, including helper changes behind unchanged predicates. General invariant dependency graphs, logical implication/satisfaction, runtime behavior and executed evidence remain missing. |
| Ownership | Partial, authored/unrun. Facets expose parameter modes and structural slots. Whole-candidate Ownership Delta compares checked ownership signatures, complete structural inventories and retained instance facts with exact source replay. General creation/transfer/settlement relationships, runtime liveness and executed evidence remain missing. |
| Cleanup | Partial, authored/unrun. Facets reuse complete ordered CleanupPlan projection. Ownership Delta compares whole-candidate loan and cleanup plans without rewriting their vectors or claiming behavioral equivalence. Cleanup Dependencies adds reverse source type/case/field selection over retained storage shapes, cleanup and loan plan facts with original coordinates, an image-local lazy index, and candidate before/after review through the same collector. General lifetime/alias reasoning, broader expression-to-obligation queries, physical execution and executed evidence remain missing. |
| Data access | Partial, authored/unrun. Function facets expose actual ValueId reads/writes, field projections and consumption-context facts with provenance. A shared lazy index supports bounded reverse field/type/case queries and candidate relationship deltas. Whole-value leaf expansion, general alias analysis, runtime liveness and executed evidence remain missing. |
| Interfaces | Partial, authored/unrun source-sidecar conformance facts and v4 `protocol/conformance` / `candidate/interface-catalog` discovery. The additive whole-candidate [Interface Delta](PROJECT-CANDIDATE-INTERFACE-DELTA-V1.md) compares complete affected-member inventories, bound functions and static-call dependencies with exact candidate replay; v5 exposes chunks. Facts bind source revisions and stable receiver/protocol/member/function identities; they do not add dispatch edges to the runtime Graph. Cross-module/generic interface admission, dynamic dispatch and executed evidence remain missing. |
| Tests | Partial, authored/unrun Candidate Tests selects relevance from exact transitive test-root HIR calls with conservative fallback for non-call facts; explicit execution runs the full declared interpreter test closure. Dynamic coverage, multi-root selection and native/Wasm evidence remain missing. |
| Targets | Partial, authored/unrun. V5 `image/target-admission` derives actual native-C11/Core-Wasm emission and structural validation facts for complete entry/test closures and checks selected-function membership. Whole-closure failure is not blamed on that function. Runtime execution, general declaration-level diagnosis and broader package-profile coverage remain missing. |
| Artifacts | Partial, authored/unrun. Web/npm projections build and replay existing pathless carriers; OpenAPI adds actual per-source documents and C adds real linked native source with exact header prototypes or explicit exclusions. Shared renderers and complete Project source replay preserve ordinary admission, native linkage and status conventions. All bind files, source inputs and manifest export identities. Artifact Delta compares actual base/candidate file inventories and exports after complete candidate replay, separating content from carrier metadata. V5 requires the existing build grant and returns chunks, not materialized files. Rust carriers, compiled C libraries/public FFI, broader schema admission, installed consumer relationships, generalized consumer migration, artifact filesystem authority and executed evidence remain missing. |
| Packages | Partial, authored/unrun [Package Semantic Graph](PACKAGE-SEMANTIC-GRAPH-V1.md). Exact source-capsule replay supplies coordinate-qualified interface, import and cross-package call relationships with revision-bound consumer queries. Explicit host attachment exposes a separate immutable package subject, never an inferred Project dependency. [Candidate Package Consumer Replay](PROJECT-CANDIDATE-PACKAGE-CONSUMER-REPLAY-V1.md) independently authenticates an explicit candidate-era provider report/source/capsule and projects only that final-candidate source onto its known imports and static call sites. General package profiles, installed-consumer discovery, compatibility, whole-Project package association and cross-package source migration remain missing. |
| Analysis boundaries | Partial, authored/unrun [Analysis Coverage](SEMANTIC-IMAGE-ANALYSIS-COVERAGE-V1.md) plus its [candidate projection](PROJECT-CANDIDATE-ANALYSIS-COVERAGE-V1.md): exact retained image or final-candidate input and declared interface-import facts alongside explicit deployment, generated provenance, external API behavior, runtime and consumer blind spots. [Candidate Analysis Evidence](PROJECT-CANDIDATE-ANALYSIS-EVIDENCE-V1.md) composes one explicit authenticated candidate-era package corpus and changes only external consumers to partial. [Candidate Analysis Artifact Evidence](PROJECT-CANDIDATE-ANALYSIS-ARTIFACT-EVIDENCE-V1.md) composes one independently replayed selected pathless carrier delta and changes only generated artifacts to partial. [Candidate Analysis Runtime Evidence](PROJECT-CANDIDATE-ANALYSIS-RUNTIME-EVIDENCE-V1.md) composes the exact policy-bounded reference-interpreter test report and changes only runtime environment to partial. No ambient external-input ingestion, installed discovery, native/Wasm or deployed execution, dynamic/path coverage, environment observation, materialization/deployment proof, conformance/compatibility proof, before/after coverage improvement or completeness percentage. |
| Unsafe boundaries | Partial, authored/unrun HIR Relationships projection machinery. Current Project admission excludes unsafe-bearing sources, so its public unsafe inventory remains empty. Active unsafe-source coverage, transitive dependency/exposure analysis, broader import-bearing Project admission and candidate deltas remain missing. |
| Diagnostics | Partial, authored/unrun [Symbol Diagnostics](PROJECT-CANDIDATE-SYMBOL-DIAGNOSTICS-V1.md) joins session-retained rejected attempts to their exact predecessor and intention target, with actually admitted repair classes and report-bound continuations. It never treats a rejection as a checked image or attributes its spans to verified HIR. General diagnostic causality, retained compiler warning inventories, broader repairs and executed evidence remain missing. |

Each generalized fact must bind source provenance, stable identity, revision,
edge kind, reason, evidence owner and authoritative/descriptive/advisory class.
Current Facets labels descriptive validated-HIR projections and source
revisions; this does not establish the missing families or their independent
replay. LoanPlan vectors remain proof data, not runtime liveness authority.

Cross-file candidate-aware impact is **Partial**: Candidate pairs base and
candidate six-family Impact reports. Additive [Impact Navigation](PROJECT-CANDIDATE-IMPACT-NAVIGATION-V1.md)
provides candidate/query-bound compact summaries and exact ordered pages over
the final candidate artifact without expanding truncated rows. Semantic Delta adds authored/unrun selected
declaration before/after facts, function facets, reverse field accesses, test
relevance and whole-closure target artifacts with exact recomputation. This
narrows the missing delta work above; package/artifact families, broader interface admission,
behavioral equivalence and runtime coverage remain open.
Source/semantic-diff binding and exact candidate replay passed for the bounded
`474c481b` Git-workflow fixture; generalized facet families remain
**Authored/unrun**. The fixture's explicit interpreter request passed, while
policy-selected full gates remain **unrun/missing integration**.
Evidence-bound materialization through separate
commit authority is **Partial** in existing A0/managed Workspace routes and
**Partial for candidates** through an authored/unrun separate managed Workspace
bridge. It replays under the existing lock before `ACTIVE` publication and leaves
original raw Git paths unchanged. A separate host route constructs canonical
Git objects and publishes a branch by expected-old ref update in an explicitly
selected Unix bare SHA1 or SHA256 repository; the bounded local fixture executed
both formats. SHA1
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
| `workspace/open`, `workspace/status`, `workspace/refresh-preview`, `workspace/refresh`, `query/catalog` | Authored/unrun. Host binds the manifest; requests cannot select a new path. V5 preview discovers the currently observed revision without replacing state. Cold and opt-in frontend/checked-module cache refresh authenticate the same source set with exact expected image/new Project revisions, retain candidate handles and clear incomplete drafts/attempts only after successful bounded response preparation. Failed refresh preserves session/cache state; general incremental rechecking and measured reuse remain open. |
| `protocol/conformance`, `candidate/interface-catalog` | Authored/unrun additive v4 read-only source-sidecar queries and candidate-bound static required-member discovery. Existing v1–v3 method sets and runtime Graph contracts remain unchanged; no runtime interface or publication authority is granted. |
| `change/catalog`, `validation/catalog` | Authored/unrun candidate-only v2. Target-specific constructor discovery and independent candidate replay are available; arbitrary payload validity requires apply. General legal-transition discovery and execution routes remain open. |
| Read-only, candidate-only, source-commit, build-enabled, test-enabled, artifact-materialization-enabled sessions | V5 composes host-selected read-only/candidate/diagnostic/test/pathless-build capabilities and optionally startup-approved fixed Git publication. Older profiles remain unchanged. No artifact filesystem materialization, arbitrary tool execution or request authority elevation is granted. |
| Version-matched agent instructions | Authored/unrun catalogue-derived instructions cover only methods granted by the selected v5 host policy and preserve older profile instructions. Executed instruction/client compatibility evidence remains open. |
| TypeScript, Python, Rust clients | V5 authors typed TypeScript/Python/Rust request parameters, enum choices, builders and result decoders from runtime-selected catalogues. Earlier exact-subject local evidence executes Python, offline compiled Rust and provisioned strict TypeScript 5.8.3/Node request/response consumers with hostile depth/work cases. [Phase 1 Product Workflow Evidence v1](GRAPH-OPERATIONAL-PHASE1-PRODUCT-WORKFLOW-EXECUTION-EVIDENCE-V1.md) additionally executes the complete bounded review/publish composition through all three generated clients. All-language generation remains deterministic; other heterogeneous compiler reports, exhaustive generated-method admission, complete typed payloads, packaged SDKs and ergonomics remain open. |
| Optional MCP/editor adapters | Partial. Exact-subject local evidence passes the pinned 2025-11-25 in-process MCP contract and actual stdio child. Separate exact subject `2888f84f123b7caa44aa6807388d98f851d4beaf` executes the saved-source adapter inside a selected local Visual Studio Code 1.135.0 Extension Host against a freshly built compiler: all 26 commands register; a typed `rename_declaration` produces a verified read-only virtual diff; dirty-buffer invalidation preserves saved source. The client frames remain authored Rust JSON rather than an independent MCP SDK/conformance client. HTTP, manual UI/accessibility, VSIX/Marketplace, typed-hole/repair host paths, asynchronous/cancelable scheduling, minimum-version, hosted and cross-platform evidence remain missing. |
| Machine-readable operation catalogue | Read query and target-specific constructor catalogues authored/unrun; general change catalogue remains partial. |
| `candidate/open`, `candidate/apply-intent` | Candidate library and candidate-only v2 methods authored/unrun; bounded immutable registry with no publication authority. |
| `candidate/query`, `candidate/validate`, `candidate/impact` | Authored/unrun v2 methods: bounded report chunks, complete independent replay and six-family impact. Additive v5 candidate dependency navigation exposes compact sites/callers/calls/members; [Impact Navigation](PROJECT-CANDIDATE-IMPACT-NAVIGATION-V1.md) pages the final candidate artifact's affected/edge/frontier arrays with exact query and truncation bindings. Neither retains a derived image or expands omitted impact. General incomplete-candidate validation and generalized impact remain open. |
| `candidate/test` | The library and explicitly host-selected v3 route require exact candidate replay before fixed-policy execution of the full declared interpreter test closure. The bounded product-workflow evidence executes and independently repeats that exact fixture report; static relevance is not coverage, and native/Wasm, broader candidate tests and policy-selected full gates remain separate. |
| `candidate/compare`, `candidate/discard` | Descriptive library and v2 lifecycle methods authored/unrun. Semantic compatibility proof remains missing. |
| `candidate/commit` | Partial separate host bridges to managed `ACTIVE` and canonical Git branch publication, each with exact candidate approval/replay. The Git adapter admits Unix bare SHA1/SHA256 repositories and never rewrites raw checkouts. V5 adds `candidate/commit` only with separately attached fixed authority and exact startup approval, consumed on invocation; success/uncertainty is terminal for that commit host. The bounded product-workflow evidence executes three generated-client publications through isolated local Unix bare SHA-256 repositories, including real post-ref result loss. Ordinary checkout integration, SHA-1 workflow composition, broader Git interoperability, hosted/cross-platform providers and physical durability remain missing. |

## Phase 5: multi-agent operation

| Requirement | Status and remaining evidence |
| --- | --- |
| Semantic rebase/merge | Partial, authored/unrun Rebase. Stable-ID target/dependency conflict classification, introduced-identity collision checks, display-normalized call facts, canonical source replay and same-root history merge cover supported intentions. Static interface additions now use an exact compiler-owned dependency fingerprint at every replay boundary and reject changed conformance facts, occupied receiver/protocol pairs and implementation-ID collisions before full replay. Draft Rebase carries checked history and pending body/expression/contract holes to an admitted destination after region conflicts and source-origin remapping. Draft Merge combines common-base checked histories and independently rebinds both pending inventories, checking opposing writes and overlaps before a bounded union. Both preserve explicit completion and regenerate contexts. General ownership-sensitive reconciliation, behavioral interface equivalence, filled-hole lineage and cross-manifest merging remain open. |
| Conflict cases | Focused Rebase cases are authored/unrun: unrelated body/display rename, body/postcondition revalidation, competing signature conflicts, deleted call dependencies, and exact static-interface receiver/protocol/member/function/pair/identity drift. Execute the matrix, expand to all intended operations and preserve expected-expression remapping before claiming full coverage. |
| Candidate branching | Partial in-memory immutable siblings and bounded candidate-only v2 registry. Complete histories and unfinished drafts have authored/unrun self-contained source archives and typed entry points to a host-selected immutable content-addressed store. Startup policy can restore explicitly selected candidates and drafts. Draft archives reconstruct pending selectors and valid history after the original checkout changes or disappears, while preserving explicit completion and historical startup fences. Sibling drafts can explicitly merge compatible histories and pending selections without inferring completion from another branch. Automatic durable branch registries, complete branch lifecycle recovery and executed evidence remain missing. |
| Parallel read-only requests | Partial, authored/unrun. The embedding-host batch API uses at most four scoped workers for sixteen selected immutable image/discovery/candidate/draft/diagnostic reads, restores input order and authenticates held source before/after the entire joined batch. Explicit startup-selected workspace/read-batch exposes the same engine through NDJSON and MCP with generated schemas/clients, unchanged 64 KiB request and 1 MiB aggregate response caps, and source authentication even for all-error inner batches. It detaches only request-selected subjects and shares pure sequential report handlers; no registry mutation, refresh, test execution, build or commit authority enters workers. Validation and source-only repair/archive replay keep their ordinary bounds, with no total CPU/RSS guarantee. Outer stdio requests remain sequential; general concurrent transport scheduling, cancellation and executed isolation/throughput evidence remain missing. |
| Session recovery and content-addressed candidate persistence | Partial, authored/unrun. Self-contained candidate archives retain canonical original sources plus complete-history capsules and persist through an explicit private immutable store. CLI host-policy v3 restores historical candidates before frames; separate host-policy v5 can select the authenticated semantic cache store. Self-contained draft archives add original-source reconstruction and ordinary pending-hole replay without importing contexts or candidate/approval registry entries. Explicit typed store APIs, persist/load commands and host-policy v6 now retain and select these drafts. Host historical draft restore remains startup-only and same-manifest; frame restore requires the current original base. No automatic registry checkpoints, cursors, pending validation or authority recovery. `.semaprax-candidates/` is Git-excluded. |
| Manual edits and stale recovery | Partial held-input absorbing invalidation, exact base rejection and library rebase onto a separately admitted revision. V5 reloads through cold, source-exact frontend, or explicitly selected checked-module reuse and preserves historical candidates; explicit startup archives recover complete candidates and drafts after original source removal/edits without making them current. Draft Rebase can move pending work onto a selected current candidate before filling. Drafts/attempts still clear on refresh, so historical draft recovery uses explicit archive startup. Authenticated cross-process checked-HIR reuse is authored/unrun. Complete session recovery, executed cross-process reuse evidence, broader incremental rechecking and measured recovery benchmarks remain open. |

## Required twelve-step signature demonstration

| Step | Current evidence or outstanding gate |
| --- | --- |
| 1. Open immutable Project snapshot | Passed in the exact local Git-workflow subject; broader current-head preservation gates remain open. |
| 2. Select explicit stable-ID function | Passed for the fixture's explicit `calculator.add` selection; general selection evidence remains broader. |
| 3. Change signature | Passed for the bounded reorder/rename/append scalar mapping; general evolution remains open. |
| 4. Migrate every authenticated caller | Passed for the fixture's three local/application/test callers with left-to-right staging; no external/dynamic migration claim. |
| 5. Preserve stable ID and exported identity | Passed for the fixture's declaration and manifest checks; external ABI compatibility is not implied. |
| 6. Prove no new effects/capabilities | Passed for the scalar, effect-free fixture; this is not a general proof. Conflict and publication hostile controls are recorded separately. |
| 7. Revalidate contracts/ownership/cleanup | Passed for rebuilt predicates, exact modes and empty scalar cleanup inventory; resource ownership remains open. |
| 8. Run affected tests | The explicit candidate interpreter-test request passed inside the exact local workflow; broader target/test gates remain open. |
| 9. Verify native/Wasm admission | C11 emission and structural Core-Wasm admission passed inside the workflow; target runtime execution remains open. |
| 10. Return semantic impact and human source diff | Impact, source differences and semantic-delta replay passed for the exact fixture; generalized facets remain open. |
| 11. Reject or semantically rebase concurrent source change | Sibling merge, competing signature rejection and real stale-ref preflight passed; manual refresh/rebase, mid-CAS race and general reconciliation remain open. |
| 12. Commit only through separate authority | Separate startup-approved SHA-1/SHA-256 Git publication, committed-object inspection, wrong approval and terminal reuse controls passed locally. Separate local managed-generation evidence also published and authenticated `ACTIVE` while preserving raw source; a single integrated general publication path, broader Git and platform evidence remain open. |

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
The scenario has focused exact-commit runners and machine-readable evidence
contracts. [Execution Evidence v1](GRAPH-OPERATIONAL-EXECUTION-EVIDENCE-V1.md)
selects the two real-provider scenarios plus the nonignored real stale-ref
preflight and requires both format-specific economics artifacts. The
[reviewed local bundle](evidence/graph-operational/474c481bf3c3561c144e077f0000460f61af55f2/5269b6acba08a197e6a8411ba95ccdec6e6a4ff724d35681344b5260087cb2e8/evidence.json)
records all three selected tests passing for exact subject
`474c481bf3c3561c144e077f0000460f61af55f2`. [Execution Evidence
v2](GRAPH-OPERATIONAL-EXECUTION-EVIDENCE-V2.md) additionally selects a real
post-CAS result-loss case, four managed-publication boundary regressions, and
the integrated managed workflow. Its reviewed local exact-subject bundle passes
all nine selected rows; the Phase 0 v2 aggregate passes all 86 selected rows;
generated clients, MCP, hosted CI and native/Wasm runtime remain independent
dimensions. Therefore neither tranche completes the general demonstration.

An integrated managed-generation precursor is executed by the v2 runner in
`tests/project_graph_operational_workflow_v1.rs`: it combines signature migration,
unrelated merge, competing-signature rejection, deltas, explicit test policy and
separate managed publication with stale rejection. It passed locally at the
recorded exact subject and deliberately
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

An additive [task-economics observation](AGENT-TASK-ECONOMICS-V1.md) is
authored inside the integrated twelve-step Git workflow, with deterministic
per-format export and a separate exact-commit envelope contract. It records exact
semantic protocol traffic, review-material sizes, scripted control rejections,
validation/replay/test operation counts, target-admission row counts and the
twelve asserted criteria. It explicitly records stale recoveries as zero and
leaves model
tokens, external agent tool calls, elapsed validation cost and human review time
unobserved. It is infrastructure for the missing comparison, not benchmark
results or a productivity claim.
