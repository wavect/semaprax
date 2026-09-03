//! V5 descriptions and clients derive from the exact host-selected registry.
use super::super::{
    method_description, text, Method, Operation, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES,
};
use super::{
    VNextPolicy, VNEXT_APPLICATION_ERROR_DATA_SCHEMA, VNEXT_PROTOCOL_SCHEMA, VNEXT_RESULT_SCHEMA,
};
use crate::diagnostic::Diagnostic;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[path = "candidate_impact_schemas.rs"]
mod candidate_impact_schemas;
#[path = "clients.rs"]
mod clients;
#[path = "hole_suggestions_schemas.rs"]
mod hole_suggestions_schemas;
#[path = "payload_schemas.rs"]
mod payload_schemas;
#[path = "repair_schemas.rs"]
mod repair_schemas;
#[path = "workflow_metadata.rs"]
mod workflow_metadata;

const MAX_DISCOVERY_BYTES: usize = 900 * 1024;
pub(super) use workflow_metadata::{
    EVENTS as WORKFLOW_EVENTS, OUTCOMES as WORKFLOW_OUTCOMES,
    REPAIR_ACTIONS as WORKFLOW_REPAIR_ACTIONS,
};
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub(super) fn payload(
    method: &Method,
    params: &Map<String, Value>,
    methods: &[&'static Method],
    policy: &VNextPolicy,
    commit_enabled: bool,
) -> Result<Value> {
    let capabilities = capabilities(methods, policy, commit_enabled);
    let descriptors = methods
        .iter()
        .map(|method| descriptor(method, policy))
        .collect::<Vec<_>>();
    let mut result = match method.name {
        "protocol/capabilities" => capabilities,
        "protocol/instructions" => {
            json!({"schema":"semaprax.image-agent-instructions.v5","protocol":VNEXT_PROTOCOL_SCHEMA,
            "instructions":"Read protocol/capabilities, protocol/schemas, then workspace/open. Only the host-selected methods exist. Every semantic request must bind the current exact image revision and any required candidate/draft/attempt digest. Optional means omitted, never null unless its schema explicitly permits null. Use workspace/refresh-preview with the currently bound image revision to observe a fresh Project revision without replacing session state; pass that observed_project_revision as expected_new_project_revision to workspace/refresh. Old semantic request tokens remain stale after replacement. Refresh is explicit and source drift never refreshes implicitly. Candidate preparation, diagnostics, interpreted tests, artifact projection, and source commit are distinct host-selected capabilities. Use only the selected catalogue and respect its limits. Schema bundle unbundled_payload_schemas explicitly identifies opaque payloads requiring their owning specification. Generated clients construct and decode messages only: they perform no I/O and cannot enable any capability. A source commit receipt is not permission to repeat publication."})
        }
        "query/catalog" => {
            json!({"schema":"semaprax.image-agent-query-catalog.v5","protocol":VNEXT_PROTOCOL_SCHEMA,
            "queries":methods.iter().zip(&descriptors).filter(|(method,_)|method.query).map(|(_,descriptor)|descriptor).collect::<Vec<_>>()})
        }
        "protocol/schemas" => bundle(&descriptors, &capabilities)?,
        "protocol/client" => {
            let language = text(params, "language");
            let bundle = bundle(&descriptors, &capabilities)?;
            let source = clients::generate(language, &bundle)?;
            json!({"schema":"semaprax.image-agent-client.v5","protocol":VNEXT_PROTOCOL_SCHEMA,"language":language,
                "source":source,"io":false,"request_validation":"selected_descriptor_outer_parameters; nested_constructor_objects_require_compiler_admission",
                "result_validation":"envelope_and_bundled_payload_shapes; explicitly_unbundled_payloads_remain_opaque",
                "unbundled_payload_schemas":bundle["unbundled_payload_schemas"],
                "typescript_integer_policy":"reject_integers_outside_safe_integer_range; use_string_request_ids",
                "dependencies":match language { "rust"=>json!(["serde with derive","serde_json"]),"python"=>json!(["Python 3.11 standard library"]),_=>json!(["TypeScript ES2022","TextEncoder","structuredClone"])}})
        }
        _ => return Err(invalid("v5 discovery received a non-discovery method")),
    };
    if method.name == "protocol/instructions" {
        let mut instructions = result["instructions"].as_str().unwrap_or("").to_owned();
        if methods
            .iter()
            .any(|method| method.name == "workspace/read-batch")
        {
            let eligible = methods
                .iter()
                .filter(|method| super::read_batch::parallel_read(method.operation))
                .map(|method| method.name)
                .collect::<Vec<_>>()
                .join(", ");
            instructions.push_str(" The host selected parallel_read. Use workspace/read-batch with image_revision and batch containing only frames, an array of one to sixteen ordinary JSON-RPC request strings. Existing generated request builders may supply those exact strings, including their trailing LF. Every inner request keeps its ordinary selectors and grants. Empty frames and notifications produce null response positions; malformed or unavailable requests keep ordinary per-row errors. Responses are exact JSON-RPC strings in input order: decode each non-null string with the existing method decoder, rather than treating the outer shape as proof of its contents. The host fixes one to four workers before startup; requests cannot choose concurrency. The complete outer request remains at most 64 KiB and the complete encoded response at most 1 MiB, so use smaller report chunks or fewer reads when needed. Combined response overflow, worker failure or live source drift releases no partial batch. All workers join before results return. No mutation, workspace refresh or refresh-preview, build, artifact delta, interpreter execution, commit, approval, storage operation or nested workspace/read-batch is admitted, even if another grant selected it. Expensive validation and source replay remain possible; concurrency does not promise CPU, memory, stack or latency bounds. The exact selected inner read methods are: ");
            instructions.push_str(&eligible);
            instructions.push('.');
        }
        if methods
            .iter()
            .any(|method| method.name == "workspace/retained-subjects")
        {
            instructions.push_str(" With candidate_prepare, use workspace/retained-subjects to recover the deterministic bounded inventory of candidate, draft and rejected-attempt references currently retained by this exact session. Candidate associations on drafts and attempts can outlive removal of the associated candidate handle; the explicit retained flag reports only current registry membership. Every later operation still authenticates its own exact selector. Inventory entries grant no source, execution, materialization or publication authority and do not make drafts or rejected attempts into checked candidates. Refresh preserves candidates and clears drafts and attempts. This live registry query is intentionally excluded from workspace/read-batch and parallel immutable reads.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/test-task-start")
        {
            instructions.push_str(" With candidate_test, candidate/test-task-start schedules at most one session-scoped cooperative reference-interpreter test task for an exact retained candidate. Poll candidate/test-task-status; candidate/test-task-cancel requests monotonic cancellation and reports whether completion or cancellation is currently terminal; fetch a completed canonical report through bounded candidate/test-task-result chunks. The digest handle binds the starting image, project and candidate and grants no authority. Every poll, cancel and result release reauthenticates held source. Source drift, successful refresh, stream finish or session drop cancels and joins the worker; a late outcome cannot escape an invalidated session. Build and source publication are never task-wrapped or cancellable. This explicit Semaprax tool lifecycle is not MCP notifications/cancelled or a claim of wall-time preemption, scheduling fairness, native/Wasm execution, deployment inspection or external API behavior.");
        }
        if methods
            .iter()
            .any(|method| method.name == "image/function-instances")
        {
            instructions.push_str(" Use image/function-instances with image_revision and a source generic-function template target to list only its retained concrete instances, including a valid empty inventory. The closed report binds the template's source provenance, exact ordered type arguments and concrete signature counts; each instance carries nine facet handles. Follow next_cursor with the same image, target and page_size. Use image/function-instance-facet with that template target, exact instance_id, facet and handle, keeping those selectors and page_size fixed across pages. page_size is 1 through 128 (default 32), max_bytes is 1024 through 1048576 (default 65536); the ordinary response-envelope bound still applies. Both queries are read-only and eligible for authenticated parallel batches. They do not instantiate new type arguments, create candidates or infer that retained instances are all possible instantiations. Caller facts match concrete call instance identities, and template source spans are provenance rather than executed-site locations. Entry/test instance inventory and template export membership do not prove execution, test coverage or instance export. Facet envelopes are closed and typed, but their heterogeneous item interiors remain explicitly unbundled as semaprax.image-instance-facet-item.v1; full contract/loan/cleanup facts retain their owning compiler format and canonical vector order. Existing image/function-summary and image/facet keep their declared-function selectors and unchanged payload schemas. No execution, dynamic/external caller completeness or source/publication authority is granted.");
        }
        if methods
            .iter()
            .any(|method| method.name == "image/function-reference-export")
        {
            instructions.push_str(" Use image/function-reference-export with the exact current image_revision, a declared-function target and optional facet to obtain a bounded canonical exact-revision reference object. Serialize that complete object as the reference argument to image/function-reference-resolve with the same exact current image_revision. Resolution reauthenticates the image, project graph and source provenance, then freshly derives the closed function summary and optional facet handle; no HIR, graph, source or handle fact is trusted from the reference. Both queries are pure immutable reads eligible for authenticated parallel batches. References are integrity and staleness bindings, not capabilities, secrets, persistent server state, migration tokens or general session recovery. They grant no source, execution, candidate-retention or publication authority, and stale references fail instead of automatically migrating.");
        }
        if methods
            .iter()
            .any(|method| method.name == "package/summary")
        {
            instructions.push_str(" The host attached an independently verified package graph before startup. Call package/summary with image_revision to discover its graph_revision and source-capsule bindings. Then call package/consumers with image_revision, package_revision equal to that graph_revision, provider_package, provider_version and target. The package subject is independent: project_association is none, and the session image authenticates only the live request boundary, not package linkage to this Project. Imports are declared dependencies; calls contain only authenticated cross-package direct sites, so an import can have no calls. Both closed reports are bounded to 1 MiB before the ordinary response-envelope bound and support parallel reads. No request can attach, replace, load or fetch a package graph, resolve a registry, build or execute packages, create candidates, or publish source.");
        }
        instructions.push_str(" Use image/analysis-coverage with the current image_revision to read the closed retained-source analysis boundary inventory. It names known, partial and not_inspected areas; these statuses are descriptive, not percentages or completeness proofs. Its blind_spots ledger binds absent deployment-configuration, generated-file-provenance and external-API/deployed-runtime evidence to the exact retained Project and manifest source inventory. Evidence absence never asserts that the corresponding runtime contract is absent. The report binds manifest-listed sources and declared interface imports, including their native_rust flag, but does not inspect deployment configuration, generated-file provenance, generated artifacts, external API behavior, runtime environments or external consumers. Missing imports or graph edges never prove absence of external dependencies. This direct report is bounded to 1 MiB before the ordinary response-envelope bound, is eligible for authenticated parallel reads, and performs no external I/O, execution or source publication. Source drift checks belong to the ordinary live session boundary.");
        instructions.push_str(" Use image/dependencies with the current image_revision and a stable declaration target to inspect bounded compiler-derived reverse dependency facts. It is read-only and grants no candidate or execution authority. Read UTF8 chunks from offset zero using next_offset; chunk_bytes is 1024 through 65536 (default 16384), and the complete report is bounded to 8 MiB. The heterogeneous dependency report remains explicitly unbundled; its closed chunk envelope does not prove transitive runtime effects.");
        instructions.push_str(" Use image/cleanup-dependencies with the current image_revision and a stable declaration target to inspect compiler-derived relationships to ordered cleanup facts. Read UTF-8 chunks from offset zero using next_offset; chunk_bytes is 1024 through 65536 (default 16384), and the report is bounded to 8 MiB. The heterogeneous report is explicitly unbundled. This immutable read grants no candidate, execution, physical cleanup or source authority and does not prove runtime liveness or destruction order.");
        if methods
            .iter()
            .any(|method| method.name == "candidate/cleanup-dependencies")
        {
            instructions.push_str(" With candidate_prepare, use candidate/cleanup-dependencies with candidate_revision and target for before/after cleanup dependency facts against the original source base. Keep image_revision, candidate_revision and target fixed while reassembling offset/next_offset chunks; chunk_bytes is 1024 through 65536 (default 16384), report bound 8 MiB. The heterogeneous report remains explicitly unbundled. This pure read is eligible for authenticated parallel read batches and grants no execution or publication authority.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/source-review")
        {
            instructions.push_str(" With candidate_prepare, candidate/source-review independently replays the exact retained candidate and returns a closed source-review report in UTF-8 chunks. Keep image_revision and candidate_revision fixed, start at offset zero, and follow next_offset until null; chunk_bytes is 1024 through 65536 (default 16384), and the full report is bounded to 16 MiB. Its bundled report schema describes changed canonical paths, exact base/candidate source text, source digests and ordinary source diffs, with report_revision binding the complete report. No filesystem edit, candidate installation, execution or publication authority is granted. Embedding hosts may include this pure read in authenticated parallel read batches. JSON shape checks do not establish digest authenticity or source replay.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/merge-preview")
        {
            instructions.push_str(" With candidate_prepare, call candidate/merge-preview using exact image_revision, candidate_revision and other_candidate_revision for two retained candidates sharing an original base. This read performs the ordinary merge in both history orders, including full candidate admission for successful directions; it can be expensive and is not the lightweight candidate/compare report. left_then_right replays left history before the right suffix, and right_then_left reverses that order. Each direction is a closed accepted result-digest summary or a bounded diagnostic rejection; rejection can reflect conservative checks or capacity limits and is not proof of semantic incompatibility. same_source is null unless both directions succeed, then compares exact resulting canonical sources, not behavior, history identity or external compatibility. The report is bounded to 256 KiB, with at most 64 diagnostics and 16 KiB of combined diagnostic code/message UTF-8 per rejected direction. Result candidate digests do not register handles; the query retains no candidate or attempt, grants no execution or publication, and does not require candidate_diagnostics. Authenticated parallel read batches may run this pure query over detached candidates. candidate/compare and earlier protocol profiles remain unchanged.");
        }
        instructions.push_str(" For compact navigation, first call image/dependency-summary, then pass the selected sites, callers, calls or members facet handle to image/dependency-page. Keep image_revision, target, view, page_size and max_bytes fixed while following next_cursor; omit cursor on the first page. Page size is 1 through 128 (default 32), and max_bytes is 1024 through 1048576 (default 65536). Handles and cursors are bound compiler references, not authority; do not synthesize or reuse them with another target or image. Page wrappers are closed, but heterogeneous item facts remain explicitly unbundled.");
        if methods
            .iter()
            .any(|method| method.name == "candidate/analysis-coverage")
        {
            instructions.push_str(" With candidate_prepare, use candidate/analysis-coverage with exact image_revision and candidate_revision to inspect the retained-source analysis boundary inventory of one fully admitted candidate. The payload image_revision is the ephemeral image digest derived from that candidate; the outer response image_revision remains the live session binding. Project, workspace, graph, source and inventory facts describe the candidate revision, while base_project_revision identifies its original base. Known, partial and not_inspected areas remain descriptive rather than completeness percentages or external evidence. This pure query is eligible for authenticated parallel read batches over a detached immutable candidate and retains no candidate, source, execution or publication authority.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/analysis-boundary-bundle")
        {
            instructions.push_str(" With candidate_prepare, use candidate/analysis-boundary-bundle with exact image_revision, candidate_revision, canonical bundle string and bundle_digest. The closed bundle is bounded to 24576 UTF-8 bytes and must carry all three canonical child declarations and their domain digests: deployment contract, generated-file provenance and external API contract. Every child is independently regenerated through its owning candidate attachment method. Reassemble the report using offset and next_offset with chunk_bytes 1024 through 65536 (default 16384), keeping the complete bundle and selectors identical; the report is bounded to 2 MiB and report_sha256 must match across chunks. The composition advances only deployment_configuration, generated_file_provenance and external_api_behavior to partial while preserving every other coverage fact and nonclaim. It is not filesystem, environment, generator, provider, network, runtime, freshness, reproducibility, conformance, consumer or deployment evidence. The request grants no source, artifact, filesystem, process, network, ambient, publication or deployment authority and is not in the parallel-read subset.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/environment-aware-review")
        {
            instructions.push_str(" With candidate_prepare, use candidate/environment-aware-review with exact image_revision, candidate_revision, canonical bundle string and bundle_digest. The bundle retains the 24576-byte bound and all three child declaration digests. The compiler independently regenerates the complete candidate source review and analysis-boundary bundle, validates their candidate, base, project, workspace, graph and retained-source joins, and returns both complete nested canonical reports. Reassemble offset/next_offset chunks with chunk_bytes 1024 through 65536 (default 16384), keeping the bundle and selectors identical; the report is bounded to 18939904 bytes and report_sha256 must match across chunks. Nested report hashes and revisions bind exact evidence bytes, not approval or compatibility. Semantic compatibility is not_assessed. No filesystem or environment state, generator, provider, network, runtime, consumer, conformance or deployment is observed, and no source, approval, publication, ambient or deployment authority is granted. Caller-supplied bundle bytes keep this query outside the parallel-read subset.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/environment-consumer-review")
        {
            instructions.push_str(" With candidate_prepare and a host-attached package graph, use candidate/environment-consumer-review with exact image_revision, candidate_revision, package_revision, canonical bundle string and bundle_digest, provider_package, provider_version, provider_source_path and target. The compiler independently regenerates the complete environment review and the attached graph's complete package summary and selected consumer report, then requires exact candidate, base, project, workspace, bundle, package graph, provider source and target joins. Reassemble offset/next_offset chunks with chunk_bytes 1024 through 65536 (default 16384), keeping every selector and all bundle bytes identical; the report is bounded to 23265280 bytes and report_sha256 must match across chunks. Its closed operational projection advances only external_consumers to partial; the original environment review remains complete and unchanged. Compatibility is not_assessed. The attached graph is not installed, ambient or deployed consumer discovery; imports and static calls are not runtime use. No filesystem, environment, registry, provider, network, generator, runtime, conformance or deployment observation or authority is granted. Caller bundle bytes keep this query outside the parallel-read subset.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/analysis-deployment-contract-evidence")
        {
            instructions.push_str(" With candidate_prepare, use candidate/analysis-deployment-contract-evidence with exact image_revision, candidate_revision, canonical declaration string and declaration_digest. The declaration is bounded to 65536 UTF-8 bytes and must bind the exact candidate and complete ordered manifest export inventory; its configuration rows describe only sorted key names, string/integer/boolean shapes and required flags. Reassemble the independently regenerated report using offset and next_offset with chunk_bytes 1024 through 65536 (default 16384), keeping all declaration bytes and selectors identical; the report is bounded to 2 MiB and report_sha256 must match across chunks. This caller declaration changes only deployment_configuration coverage from not_inspected to partial. It is not environment observation, deployed state, artifact/runtime/API/consumer compatibility, freshness, drift or conformance evidence. Requests grant no filesystem, environment, secret, network, provider, deployment, source or publication authority, and this potentially large caller-data query is not in the parallel-read subset.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/analysis-generated-file-provenance-evidence")
        {
            instructions.push_str(" With candidate_prepare, use candidate/analysis-generated-file-provenance-evidence with exact image_revision, candidate_revision, canonical declaration string and declaration_digest. The library declaration bound is 65536 UTF-8 bytes, while the ordinary 64 KiB JSON-RPC frame bound remains tighter after envelope encoding. Each of at most 64 canonically ordered rows must join artifact path, byte count and digest to one exact retained candidate source path, revision and digest; its generator identity is an opaque declared token and canonical digest, never a path, URL, command or capability. Reassemble the independently regenerated report using offset and next_offset with chunk_bytes 1024 through 65536 (default 16384), keeping all declaration bytes and selectors identical; the report is bounded to 2 MiB and report_sha256 must match across chunks. Only generated_file_provenance advances to partial. No generator input, execution, reproducibility, filesystem scan, materialization, freshness, runtime, deployment or consumer claim is made. The request grants no source, artifact, generator, filesystem, process, network, publication or deployment authority and is not in the parallel-read subset.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/analysis-external-api-contract-evidence")
        {
            instructions.push_str(" With candidate_prepare, use candidate/analysis-external-api-contract-evidence with exact image_revision, candidate_revision, canonical declaration string and declaration_digest. The library declaration bound is 131072 UTF-8 bytes, while the ordinary 64 KiB JSON-RPC frame bound remains tighter after envelope encoding. The declaration binds digest-only operation and schema facts to the complete manifest export inventory or a canonical nonempty subset of explicit manifest exports; it contains no endpoint, URL, provider, credential or locator. Reassemble the independently regenerated report using offset and next_offset with chunk_bytes 1024 through 65536 (default 16384), keeping all declaration bytes and selectors identical; the report is bounded to 2 MiB and report_sha256 must match across chunks. Only external_api_behavior advances to partial. Declared digests are not network, provider, runtime, availability, authentication, side-effect or conformance observations. The request grants no source, filesystem, process, network, provider, ambient, publication or deployment authority and is not in the parallel-read subset.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/external-api-contract-delta")
        {
            instructions.push_str(" With candidate_prepare, use candidate/external-api-contract-delta with exact image_revision and candidate_revision plus canonical base_declaration/base_declaration_digest and candidate_declaration/candidate_declaration_digest pairs. Each declaration has a 131072-byte library bound, while the ordinary 64 KiB JSON-RPC frame bound remains tighter for the complete request. The base declaration binds base_project_revision and complete manifest exports or a canonical nonempty explicit subset; the candidate declaration uses the existing exact candidate contract schema and binding. Reassemble offset/next_offset chunks with chunk_bytes 1024 through 65536 (default 16384), keeping both complete declarations, digests and selectors identical; the report is bounded to 2 MiB and report_sha256 must match across chunks. Added and removed mean only changes in the caller-declared contract inventory. compatibility is not_assessed. The delta is not provider, network, runtime, version, conformance or consumer evidence and grants no source, filesystem, process, network, ambient, publication or deployment authority. Caller-supplied declaration bytes keep this query outside the parallel-read subset.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/dependency-summary")
        {
            instructions.push_str(" With candidate_prepare, use candidate/dependency-summary and candidate/dependency-page to navigate the exact fully admitted revision of one retained candidate, including changed or introduced declarations. Keep image_revision, candidate_revision, target, view, page_size and max_bytes fixed while following next_cursor. Candidate handles and cursors are isolated from base-image and sibling-candidate references and grant no retention, execution, source or publication authority. One signature change can alter several dependency views; these final-candidate pages are not a before/after delta, test coverage, runtime liveness or evidence of external callers. Both queries are eligible for authenticated parallel read batches over detached immutable candidates.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/function-summary")
        {
            instructions.push_str(" With candidate_prepare, call candidate/function-summary with exact image_revision, candidate_revision and function target to obtain the final candidate's compact signature and nine candidate-bound facet handles. Expand one handle through candidate/function-facet with the same candidate and target, facet, optional cursor, page_size 1 through 128 (default 32), and max_bytes 1024 through 1048576 (default 65536). Keep page_size fixed while following next_cursor; max_bytes may vary under the existing facet cursor contract. Handles and cursors are isolated from the base image and sibling candidates and confer no retention or authority. Closed summary and page envelopes bind final-candidate source provenance, while heterogeneous item values remain explicitly unbundled as semaprax.project-candidate-function-facet-item.v1. These descriptive retained-HIR facts do not prove runtime liveness, dynamic or external callers, test coverage, behavioral equivalence or publication. Both pure queries are eligible for authenticated parallel read batches over a detached candidate.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/impact-summary")
        {
            instructions.push_str(" With candidate_prepare, use candidate/impact-summary and candidate/impact-page to navigate the existing reverse semantic-impact artifact for one exact retained candidate and declaration target. The impact query options are depth 0 through 1024 (default 16), impact_max_bytes 4096 through 16777216 (default 1048576), and max_nodes 1 through 8208 (default 1024); keep them fixed when expanding affected, dependency_edges or frontier. Page size is 1 through 128 (default 32), and max_bytes is 1024 through 1048576 (default 65536). Follow next_cursor with the same candidate, target, query, view, handle and page options. Handles bind the exact recomputed artifact digest and grant no retention or authority. Truncation, frontier and budget remain part of the evidence: a bounded inventory is not complete impact. These pages are not a candidate delta, behavioral change, runtime liveness, test coverage, repair ranking or external-consumer compatibility. The pure reads can join authenticated parallel batches over a detached candidate and grant no source, execution or publication authority. Existing candidate/impact remains unchanged.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/interface-delta")
        {
            instructions.push_str(" Use candidate/interface-delta to compare whole-candidate static interface bindings, every affected member and retained direct-call dependencies; this does not prove runtime dispatch or behavior.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/contract-delta")
        {
            instructions.push_str(" Use candidate/contract-delta with candidate_revision to review whole-candidate contract changes and changed functions used by predicates against the candidate's original base. There is no target parameter. Reassemble the exact UTF-8 report using offset and next_offset; chunk_bytes is 1024 through 65536 (default 16384), and the report is bounded to 8 MiB. Its heterogeneous compiler report remains explicitly unbundled. Descriptive changes do not prove predicate implication, behavioral equivalence, or execution and grant no source authority.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/ownership-delta")
        {
            instructions.push_str(" Use candidate/ownership-delta with candidate_revision to compare whole-candidate ownership, loan, and cleanup compiler facts against the candidate's original base. There is no target parameter. Reassemble UTF-8 chunks using offset and next_offset; chunk_bytes is 1024 through 65536 (default 16384), and the report is bounded to 8 MiB. Full compiler plan payloads remain explicitly unbundled. These descriptive facts do not establish runtime liveness, destruction traces, backend execution, or publication authority.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/artifact-delta")
        {
            instructions.push_str(" Use candidate/build, candidate/artifact-delta, or candidate/analysis-artifact-evidence only when candidate_build is granted. Bind candidate_revision and select kind web, npm, openapi or c for an independently replayed pathless artifact projection, its before/after comparison, or a candidate analysis boundary report that changes only generated_artifacts to partial for that selected carrier. OpenAPI projects supported manifest-selected export signatures as schema documents; it does not host or execute an HTTP service or establish external compatibility. C projects supported manifest-selected scalar exports into native-emitter-derived C declarations; it does not compile or link a C consumer or establish a general foreign ABI. Candidate replay precedes builds; each side has a fixed 16 MiB build limit that requests cannot override. Reassemble reports using offset and next_offset, with chunk_bytes 1024 through 65536 (default 16384); projection reports are bounded to 1 MiB, delta reports to 8 MiB, and composed analysis-artifact reports to 10 MiB. Match report_sha256 across every composed-report chunk. Heterogeneous reports remain explicitly unbundled, and each chunk request freshly replays the complete report rather than reading a retained cache. No artifact paths are written, package manager, compiler executable, native compilation, or target executable is run, and no deployment, compatibility, or publication authority is established.");
        }
        if methods
            .iter()
            .any(|method| method.name == "hole/recovery-export")
        {
            if methods
                .iter()
                .any(|method| method.name == "hole/open-contract-expression")
            {
                instructions.push_str(" Use candidate/contract-expression-catalog for authenticated requires/ensures selections, then hole/open-contract-expression with candidate_revision, target, expression_id, hole_id and optional draft_revision. Phase and predicate ordinal are compiler-derived, never source spans or request paths. Existing hole/open-expression remains body-only. Use hole/query for the selected contract scope; result is available only in ensures. Fill through hole/fill, then complete only after every body and contract hole is resolved. Failed fills preserve the draft; selections are reauthenticated after successful fills. Context and catalogue reports remain explicitly unbundled and confer no validity, execution or publication authority.");
            }
            instructions.push_str(" Use hole/recovery-export to save prior valid history and pending selectors. hole/recovery-restore requires the same exact original source base and returns only a draft; every remaining hole must still be filled before completion. Recovery does not restore approvals or implicitly rebase after source edits.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/symbol-diagnostics")
        {
            instructions.push_str(" Use candidate/symbol-diagnostics for retained rejected intentions matching an exact candidate and target. Start at offset zero, then pass expected_report_revision from that first chunk for every nonzero offset. If that report revision becomes stale, restart at zero. Empty results do not mean the candidate has no diagnostics; rejected spans are not verified candidate locations.");
        }
        if methods
            .iter()
            .any(|method| method.name == "attempt/repair-catalog")
        {
            instructions.push_str(" The selected repair catalogue has a complete closed response schema, including empty catalogues and separate literal and field-borrow proposal shapes. Its recursive expression bodies reuse compiler constructor grammar; generated response decoders validate normalized local definitions through exact registry references. Structural validation does not replay source, establish lexical or ownership admission, authorize repair selection, or run tests. Other explicitly unbundled reports remain opaque.");
            instructions.push_str(" Discover repairs only from attempt/repair-catalog after a retained rejection. change/catalog may describe multiple repair_diagnostic classes for one target; those descriptors do not promise an available repair. retag_integer_literal_to_retained_return_type preserves the existing bounded literal repair. borrow_owned_byte_field_without_staging requires a recorded SPX-T266 rejection and an exact compiler builtin byte-view pattern over a direct lexical field projection; its proposal replaces staging with authenticated field_place and is emitted only after ordinary full candidate admission succeeds. The rejected_intent remains a replace_function_body for the same target with a closed typed expression body, never a nested repair. Select the exact predecessor-bound repair_id explicitly; stale histories require rediscovery. No arbitrary borrow repair, ownership transfer, contract weakening, test execution or source authority is implied.");
        }
        if methods
            .iter()
            .any(|method| method.name == "hole/archive-export")
        {
            instructions.push_str(" Use hole/archive-export to obtain a self-contained source-backed draft archive in UTF-8 chunks. Keep draft_revision and image_revision fixed while following next_offset; chunk_bytes is 1024 through 65536 (default 16384). To restore, send the structured archive plus exact archive_revision and draft_revision to hole/archive-restore. RPC restoration requires the same exact original source base as the current session; a saved historical base cannot be selected through a request. Only an explicit startup host import may restore a historical source archive before the first frame. The unchanged 64 KiB request-frame limit applies even though the library archive limit is 128 MiB; larger archives require an explicit library host, not larger RPC frames. Restore retains only a draft, not a registered candidate, approval, trusted HIR or source authority. Its source_candidate_revision is a reconstructed association and need not name a registered candidate. Fill and complete through ordinary hole APIs; unresolved holes still block completion. The pure archive export is eligible for authenticated parallel read batches; archive restoration remains excluded because it retains a draft.");
        }
        if methods.iter().any(|method| method.name == "hole/rebase") {
            instructions.push_str(" Use hole/rebase with exact draft_revision and new_base_candidate_revision to replay a retained draft onto that selected candidate's checked Project revision. Only the resulting draft is retained, with its pending selectors revalidated; its source_candidate_revision need not name a registered candidate. The bounded inline report is limited to 64 KiB, and failed replay, capacity or response preparation retains no new draft. This is conservative typed selector rebinding, not a general semantic merge or behavioral equivalence proof. No implicit completion, source publication, approval, build or test authority is granted. Workspace refresh still clears drafts: recover historical work through explicit startup archive restoration before rebasing. This mutation is excluded from immutable image batches.");
        }
        if methods.iter().any(|method| method.name == "hole/merge") {
            instructions.push_str(" Use hole/merge with exact draft_revision and other_draft_revision to merge checked histories sharing an original base and readmit both parents' pending selectors. The result retains only a draft, not its underlying valid candidate. Its report preserves each parent's selector mappings and the final union, bounded to 16 holes; conflicting IDs, regions or checked intentions fail closed. Read the bounded inline report (64 KiB limit) and continue through ordinary hole APIs; there is no implicit completion, placeholder source, arbitrary subtree merge, execution or publication authority. The immutable image batch excludes this mutation, and workspace refresh still clears drafts.");
        }
        if methods
            .iter()
            .any(|method| method.name == "hole/expression-catalog")
        {
            instructions.push_str(" Use hole/expression-catalog with exact image_revision, draft_revision, target and region body or contract to select expressions from a retained draft's current last-valid source revision, including after successful fills. The closed typed report is bounded to 1 MiB and separates last_valid_revision from last_valid_candidate_digest. Neither field registers or grants a candidate handle. Body inventories contain body expressions only; contract inventories contain requires/ensures expressions. Lexical scope is not owned-value liveness, and replaceable is not hole-open admission or draft validity. Read current selectors again after each draft change; ordinary hole opening still rejects overlap and stale selections. This pure query is available to authenticated parallel read batches under candidate_prepare and grants no execution, completion or source authority.");
        }
        if methods
            .iter()
            .any(|method| method.name == "hole/fill-suggestions")
        {
            instructions.push_str(" With candidate_prepare, call hole/fill-suggestions using exact image_revision, draft_revision and hole_id to search a bounded set of possible fills. The compiler tries scope places of the expected type and direct calls with exact result type and permitted effects, excluding the hole's own target function and using only same-type scope places as arguments. It lazily enumerates at most 32 fill attempts globally; it invents no defaults or nested calls. Each returned expression passed ordinary full fill source replay against the same original draft, including ownership, loans, cleanup, contracts and selected target admission. The ephemeral result is discarded: preview_draft_revision is a digest, not a registered draft handle. To choose a suggestion, explicitly submit its expression to ordinary hole/fill with the original exact draft and hole selectors; that operation validates again. considered and rejected describe attempted proposals, and search_exhausted describes only this restricted enumeration, not all possible expressions. Rejections can include conservative or capacity failures, and no suggestions is not proof that the hole cannot be filled. The 64 KiB report binds context_revision and last_valid_revision and can join authenticated parallel read batches. No draft is retained or completed, no tests or runtime contracts are executed, and source admission does not establish intent correctness, behavioral equivalence, or liveness inferred from old proof data. Existing hole/query, summary and page payloads remain unchanged.");
        }
        if methods.iter().any(|method| method.name == "hole/summary") {
            instructions.push_str(" With candidate_prepare, use hole/summary with the exact image_revision, draft_revision and hole_id for a compact typed summary. Select the compiler-issued scope, calls, obligations or constructors reference and pass it to hole/page with the same selectors. Offset is 0 through 16384 (default 0); limit is 1 through 64 (default 16). Follow next_offset until null, retaining the same context-bound reference. Each closed summary or typed facet page is bounded to 64 KiB. These descriptive facts do not establish owned-value liveness, callable admission, successful fill or candidate validity. hole/query remains the unchanged full proof/context report and explicitly unbundled. Embedding hosts may use the compact pure reads in authenticated parallel batches; no candidate installation, execution or source authority is granted.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/apply-intent")
        {
            instructions.push_str(" When change/catalog offers ordered signature parameters, from and optional name retain the selected existing parameter's exact type and ownership while permitting reordering. Additional checked owning metadata may describe bare string (surface mode value, checked ownership own) or explicitly own named records and variants with sized, resource-free, non-Copy drop facts. Every such owning parameter must be retained exactly once; omission or duplication is rejected. Exact borrow str and borrow Slice<u8> views may also be retained, reordered and renamed, and must each remain present exactly once; no borrowed nominal/storage type, borrow Bytes, shared parameter, resource or class is admitted. A mapping with only name and borrow_from may add one fresh borrowed parameter whose exact view type comes from the named original borrowed parameter. The source view must also be retained exactly once. Each caller reuses that original argument's already staged view, so this creates no new root, projection, copy, storage or lifetime; every alias remains subject to ordinary loan and provenance checks. Existing caller arguments are evaluated once in their original left-to-right order before mapped arguments reach the ordinary call commit boundary; full loan, provenance, ownership, cleanup, contract and target validation still decides admission. String reads retain ordinary compiler cloning semantics; staging can change allocations and does not promise allocation equivalence. This grants no owning default, copying operation or permission to execute, and the existing computed/new parameter forms remain limited by their separate Copy admission.");
            instructions.push_str(" Typed expressions may use builtin_call with an exact compiler byte or string operation target and arguments in left-to-right order. Constructor schemas derive seven byte and seven string identities and exact argument counts from their compiler owners. Read optional builtin_calls metadata in change/catalog or full hole/query for source-compatible names, checked parameter ownership, return types and type families. Existing compiler_byte_operations rows retain their fields; compiler_string_operations rows append the StringOp inventory. array_u8_any_length remains a family rather than an exact zero-length type. String length, emptiness, prefix, substring and Unicode scalar-count reads have borrowed String parameters; concat consumes both owning String arguments. FromChar copies one checked char value and returns String; its argument may be an authenticated char place, ordinary call result or the separate exact char literal constructor, never an integer/string coercion. Resolved ownership metadata is not a request to spell borrow string in source. No operation here returns char or introduces core.str borrowed-view operations. Ordinary call and accessible_calls still select existing local or imported functions only. Empty declared effects do not imply allocation-free or infallible behavior; String results and bytes_copy may allocate, and all views, loans, ownership, cleanup, capacity and target-profile checks remain mandatory. These catalogues describe potential constructors, never contract validity, successful fill, or permission to execute. Every fill or candidate change requires full compiler admission; no ambient capability or source authority is granted.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/apply-intent")
        {
            instructions.push_str(" Use field_place with only kind, target (an exact record field ID) and root (an existing lexical name) to select original field storage without the typed temporary introduced by project. Optional field_places metadata in change/catalog and full hole/query describes visible field membership and owner identity; it does not establish a root's current type, availability, loan state or legal use. Root nominal identity and any existing generic arguments must match the selected owner exactly; a same-named field in a different owner is not interchangeable. There is no recursive base, caller-provided type assertion or implicit staging. Using a field place as a value may move it; borrowing still requires the ordinary bytes_as_slice direct owned-byte-field profile. Whole candidate source, ownership, cleanup and target admission remain mandatory, and no general nested or mutable borrow is granted.");
            instructions.push_str(" When change/catalog advertises add_record_field for an explicit monomorphic checked sized resource-free record, supply only field id, name, type and default. Existing source-admitted String, Bytes, array and nested record/variant storage can qualify through bounded compiler TypeFacts; the target need not be Copy or match the flat owned-byte profile. Generic targets, classes, variant targets, resources and borrowed storage remain excluded. The new field must be i64, bool, i32, u8 or usize with the matching bounded literal constructor and an exactly representable value. Defaults are inert and appended after existing field evaluations; no arbitrary expression, Bytes default, new owned field or ownership transfer is introduced. Existing constructors and exact patterns are migrated through authenticated source, preserving old field identities, types, order and checked Copy/drop/resource/sized flags. All ownership, cleanup, layout and selected-target checks still run before any candidate is admitted; nested owning patterns, projected loans and owning imports gain no new admission. Migrating String-bearing or variant constructor bodies outside the selected executable closure does not establish active native or Wasm support; aggregate String layouts and nested or mixed owned-Bytes storage retain their existing target and SPX-T268 source restrictions. This operation can change layout and artifact contracts and is not a binary compatibility guarantee.");
            instructions.push_str(" For add_declaration record fields and variant payload fields, type_declaration_forms advertises i64, bool, i32, u8, usize, string and Bytes, plus existing nominal_types selectors. This is request vocabulary, not admission of every choice in every aggregate: non-Bytes variant payloads with string, i32, u8 or usize still fail the existing SPX-T215 gate, and nested generic record fields still fail SPX-T223. A named field type is exactly kind nominal, target stable owner ID and type_arguments containing only direct i64 or bool with exact declared arity. The owner must already be visible in the anchor module; no new import, self reference, arbitrary source spelling, direct array type, borrowed/shared field, class or resource is introduced. Existing nominal dependencies retain their ordinary source-admitted field vocabulary. The rebuilt record or variant must have compiler TypeFacts proving sized and resource-free; fields need not be Copy. The existing nominal_types copy_admission metadata describes the Copy function-signature path and is separate from field admission. Source formatting, identity replay, layout, ownership, cleanup and existing Project/target admission still decide whether the new declaration is accepted. No default value, runtime execution or source authority is granted.");
            instructions.push_str(" The add_declaration function form also admits parameter type string with surface mode value and checked owning String semantics, plus existing stable-ID nominal selectors with explicit mode own. Do not spell the string token as String or pair it with mode own. Owning nominal parameters require monomorphic source declarations and exactly empty type_arguments; generic owning parameters remain excluded even though the shared structural nominal selector can represent generic Copy selections. nominal_owning_admission marks a distinct checked signature path: nominal value parameters still require Copy with no drop; nominal own parameters require non-Copy with drop; both require sized, resource-free records or variants. Nominal results may follow either checked classification, and string results are admitted through ordinary source checks. Existing Bytes own and str/Slice<u8> borrow forms remain unchanged; borrowed/shared nominal parameters, classes and resources remain excluded. Read nominal_types as provisional source-visible selectors, not proof that every argument combination is Copy or owning. No new import or profile permission is created, and existing SPX-G172 owning/nominal import restrictions still apply. Extraction and computed/new signature parameter rules keep their separate Copy limits. Bodies, contracts, ordinary String cloning, ownership transfer, cleanup and target support are decided by full candidate source admission; no automatic default, execution or source authority is granted.");
            instructions.push_str(" For extract_function, the Copy limits apply to immutable captures and the result, not every internal value. Select an exact expression/catalog identity and supply only target, expression_id, globally fresh new_id and new_name. A non-root authored block may create internal String, Bytes or monomorphic checked owning record/variant values with sized, non-Copy, drop and resource-free facts. Such an extraction preserves the original block inside an additional helper-root block so cleanup stays at the nested lexical boundary before helper result publication. No owner may enter through a capture or escape as the result; borrowed/shared values, resources, owning pattern bindings, owning assignments, propagation and unsafe audit relocation remain excluded. Existing internal mutable Copy assignments retain their separate admission. An original function-root block containing owners is not admitted by this extension. Source replay, ownership, cleanup, contracts and existing native/Wasm target admission remain mandatory; metadata does not prove that a selected block is extractable or grant execution or source authority.");
            instructions.push_str(" move_declaration retains its kind, target and destination-anchor request. Its structural destination list can include functions using checked resource-free String, Bytes, monomorphic owning records or variants and existing internal views; it does not promise a successful move. Borrowed signature parameters, resources, classes and unsupported generic types remain excluded. Existing workspace import restrictions still apply: introducing an owning parameter import or an unsupported nominal type import can reject the complete move with SPX-G172. Compiler builtin identities must remain exact across source and destination bindings. Relocation rewrites authenticated bindings without staging, duplicating or reordering argument/body evaluation; canonical source replay and ordinary ownership, loan, cleanup, import and target admission remain mandatory. No new allocation, execution, publication or source authority is granted.");
        }
        if methods
            .iter()
            .any(|method| method.name == "candidate/apply-intent")
        {
            instructions.push_str(" Typed expressions additionally accept string with only kind and value, array_u8 with only kind and values, char with kind and an eight-lowercase-hex scalar, and f32/f64 with kind and exact eight-/sixteen-lowercase-hex bits. String value is decoded UTF-8 data, including empty strings and escaped controls, bounded to 16384 UTF-8 bytes per literal; it is never raw source. Byte-array values are zero through 255 integers in source order, including an empty array, with at most 4095 elements. Each element consumes the shared expression-node budget in addition to the literal root; enclosing constructors, aggregate JSON and wire limits still apply. Character scalar values must be valid Unicode scalars. Floating bits must denote finite IEEE-754 values; hexadecimal transport preserves negative zero and finite subnormal values without JSON-number conversion. A negative float is lowered through the ordinary unary-negation node and consumes that additional shared expression-node/depth capacity. No repeated-array form, non-finite float or arbitrary element expressions are introduced. All forms compose with existing typed lets and ordinary compiler admission; String ownership, fixed-array type/length, loans, cleanup and selected targets remain checked. Legacy append_parameters and ordered literal signature defaults admit the eight Copy scalar kinds i64, i32, u8, usize, bool, char, f32 and f64. New computed signature parameter types remain separately restricted; record-field defaults retain their five inert scalar kinds and diagnostic retagging remains integer-only. Hole constructor discovery lists these forms, but fill suggestions retain their existing place/direct-call search and do not invent literal values.");
        }
        result["instructions"] = json!(instructions);
    }
    bounded(result)
}

fn capabilities(methods: &[&Method], policy: &VNextPolicy, commit: bool) -> Value {
    let mut grants = vec!["semantic_read", "workspace_refresh"];
    if methods
        .iter()
        .any(|method| method.name == "workspace/read-batch")
    {
        grants.push("parallel_read");
    }
    if policy.candidate_prepare {
        grants.push("candidate_prepare");
    }
    if policy.diagnostics {
        grants.push("candidate_diagnostics");
    }
    if policy.test_policy.is_some() {
        grants.push("candidate_test");
    }
    if policy.build_enabled {
        grants.push("candidate_build");
    }
    if commit {
        grants.push("source_commit");
    }
    grants.sort();
    json!({"schema":"semaprax.image-agent-capabilities.v5","protocol":VNEXT_PROTOCOL_SCHEMA,
        "capabilities":&grants,"methods":methods.iter().map(|method|method.name).collect::<Vec<_>>(),
        "workflows":supported_workflows(methods, &grants, policy),
        "max_request_bytes":MAX_REQUEST_BYTES,"max_response_bytes":MAX_RESPONSE_BYTES,
        "source_authority":commit,"test_execution":policy.test_policy.is_some(),"target_execution":false,
        "artifact_projection":policy.build_enabled,"request_capability_changes":false,
        "test_policy":policy.test_policy.as_ref().map(|policy|json!({"max_steps":policy.max_steps(),"max_execution_bytes":policy.max_execution_bytes(),"max_report_bytes":policy.max_report_bytes(),"engine":"project_interpreter","request_overrides":false}))})
}

fn supported_workflows(methods: &[&Method], grants: &[&str], policy: &VNextPolicy) -> Vec<Value> {
    workflow_metadata::supported(methods, grants, policy)
}

fn descriptor(method: &Method, policy: &VNextPolicy) -> Value {
    let mut value = method_description(method);
    let correlated_id = value["success_response_schema"]["properties"]["id"].clone();
    let error = &mut value["error_response_schema"]["properties"]["error"];
    error["properties"]["data"] =
        json!({"$ref":format!("urn:{VNEXT_APPLICATION_ERROR_DATA_SCHEMA}")});
    error["allOf"] = json!([{
        "if":{"required":["data"]},
        "then":{"properties":{"code":{"const":-32000}}}
    }]);
    value["error_response_schema"]["allOf"] = json!([{
        "if":{"properties":{"error":{"required":["data"]}}},
        "then":{"properties":{"id":correlated_id}}
    }]);
    let properties = &mut value["success_response_schema"]["properties"]["result"]["properties"];
    properties["schema"] = json!({"const":VNEXT_RESULT_SCHEMA});
    properties["protocol"] = json!({"const":VNEXT_PROTOCOL_SCHEMA});
    let payload = match method.name {
        "protocol/capabilities" => "semaprax.image-agent-capabilities.v5",
        "protocol/schemas" => "semaprax.image-agent-schemas.v5",
        "protocol/instructions" => "semaprax.image-agent-instructions.v5",
        "protocol/client" => "semaprax.image-agent-client.v5",
        "query/catalog" => "semaprax.image-agent-query-catalog.v5",
        "validation/catalog" if policy.test_policy.is_some() => {
            "semaprax.image-validation-catalog.v2"
        }
        _ => method.payload_schema,
    };
    properties["payload"] = if method.name == "hole/query" {
        json!({"oneOf":[{"$ref":"urn:semaprax.project-candidate-hole-context.v1"},{"$ref":"urn:semaprax.project-candidate-expression-hole-context.v1"},{"$ref":"urn:semaprax.project-candidate-contract-expression-hole-context.v1"}]})
    } else {
        json!({"$ref":format!("urn:{payload}")})
    };
    value["query"] = json!(method.query);
    value["capability"] = json!(method_capability(method));
    value
}

fn method_capability(method: &Method) -> &'static str {
    match method.name {
        "workspace/read-batch" => "parallel_read",
        "workspace/retained-subjects" => "candidate_prepare",
        "workspace/refresh" | "workspace/refresh-preview" => "workspace_refresh",
        "candidate/test"
        | "candidate/test-task-start"
        | "candidate/test-task-status"
        | "candidate/test-task-cancel"
        | "candidate/test-task-result" => "candidate_test",
        "candidate/build" | "candidate/artifact-delta" | "candidate/analysis-artifact-evidence" => {
            "candidate_build"
        }
        "candidate/interface-delta"
        | "candidate/contract-delta"
        | "candidate/ownership-delta"
        | "candidate/source-review"
        | "candidate/merge-preview"
        | "candidate/analysis-coverage"
        | "candidate/analysis-boundary-bundle"
        | "candidate/environment-aware-review"
        | "candidate/environment-consumer-review"
        | "candidate/external-api-contract-delta"
        | "candidate/analysis-deployment-contract-evidence"
        | "candidate/analysis-external-api-contract-evidence"
        | "candidate/analysis-generated-file-provenance-evidence"
        | "candidate/dependency-summary"
        | "candidate/dependency-page"
        | "candidate/function-summary"
        | "candidate/function-facet"
        | "candidate/impact-summary"
        | "candidate/impact-page"
        | "candidate/cleanup-dependencies"
        | "candidate/contract-expression-catalog"
        | "hole/open-contract-expression"
        | "hole/recovery-export"
        | "hole/recovery-restore"
        | "hole/archive-export"
        | "hole/archive-restore"
        | "hole/rebase"
        | "hole/merge" => "candidate_prepare",
        "hole/summary" | "hole/page" | "hole/expression-catalog" | "hole/fill-suggestions" => {
            "candidate_prepare"
        }
        "candidate/commit" | "candidate/commit-report" | "source-commit/status" => "source_commit",
        name if name == "candidate/attempt"
            || name == "candidate/symbol-diagnostics"
            || name.starts_with("attempt/") =>
        {
            "candidate_diagnostics"
        }
        _ if matches!(method.operation, Operation::Candidate(_)) && !method.query => {
            "candidate_prepare"
        }
        _ => "semantic_read",
    }
}

fn bundle(descriptors: &[Value], capabilities: &Value) -> Result<Value> {
    let constructors: Value =
        serde_json::from_str(&crate::project::SemanticChange::constructor_schemas()?)
            .map_err(|_| invalid("compiler constructor schema bundle is invalid"))?;
    let mut documents = BTreeMap::new();
    for document in constructors["documents"]
        .as_array()
        .ok_or_else(|| invalid("constructor schema documents are absent"))?
    {
        let id = document["$id"]
            .as_str()
            .ok_or_else(|| invalid("constructor schema identity is absent"))?;
        documents.insert(id.to_owned(), document.clone());
    }
    for (id, document) in payload_schemas::documents(capabilities) {
        documents.insert(id, document);
    }
    if descriptors
        .iter()
        .any(|descriptor| descriptor["method"] == "candidate/impact-summary")
    {
        documents.extend(candidate_impact_schemas::documents());
    }
    if descriptors
        .iter()
        .any(|descriptor| descriptor["method"] == "hole/fill-suggestions")
    {
        let schema = hole_suggestions_schemas::schema(&documents)?;
        documents.insert(hole_suggestions_schemas::ID.to_owned(), schema);
    }
    if descriptors
        .iter()
        .any(|descriptor| descriptor["method"] == "attempt/repair-catalog")
    {
        let repair = repair_schemas::schema(&documents)?;
        documents.insert(repair_schemas::ID.to_owned(), repair);
    }
    if descriptors.iter().any(|descriptor| {
        matches!(
            descriptor["method"].as_str(),
            Some("hole/recovery-export" | "hole/recovery-restore")
        )
    }) {
        for schema in draft_recovery_schemas() {
            let id = schema["$id"]
                .as_str()
                .expect("draft recovery document has a static identity")
                .to_owned();
            documents.insert(id, schema);
        }
    }
    if descriptors.iter().any(|descriptor| {
        matches!(
            descriptor["method"].as_str(),
            Some("hole/archive-export" | "hole/archive-restore")
        )
    }) {
        let schema = draft_archive_schema();
        documents.insert(
            format!(
                "urn:{}",
                crate::project::PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA
            ),
            schema,
        );
    }
    for descriptor in descriptors {
        let method = descriptor["method"]
            .as_str()
            .ok_or_else(|| invalid("method descriptor lacks a name"))?;
        for field in [
            "request_schema",
            "success_response_schema",
            "error_response_schema",
        ] {
            let mut schema = descriptor[field].clone();
            let id = format!(
                "urn:semaprax.image-v5.{}:{}",
                field,
                method.replace('/', ".")
            );
            schema["$id"] = json!(id);
            documents.insert(id, schema);
        }
    }
    let mut references = BTreeSet::new();
    for document in documents.values() {
        references_in(document, &mut references);
    }
    // Chunk data is a string, so JSON Schema $ref traversal cannot discover its
    // owning report schema. List those report schemas explicitly as well.
    for descriptor in descriptors {
        if matches!(
            descriptor["method"].as_str(),
            Some("hole/archive-export" | "hole/archive-restore")
        ) {
            // Canonical nested archives remain strings rather than imported
            // source/HIR objects. Their compiler-owned interiors are not
            // described by the closed transport archive wrapper.
            references.insert(format!(
                "urn:{}",
                crate::project::PROJECT_CANDIDATE_ARCHIVE_SCHEMA
            ));
            references.insert(format!(
                "urn:{}",
                crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA
            ));
            references.insert(format!(
                "urn:{}",
                crate::project::PROJECT_CANDIDATE_DRAFT_LINEAGE_RECOVERY_SCHEMA
            ));
        }
        if matches!(
            descriptor["method"].as_str(),
            Some("hole/recovery-export" | "hole/recovery-restore")
        ) {
            references.insert(format!(
                "urn:{}",
                crate::project::PROJECT_CANDIDATE_DRAFT_LINEAGE_RECOVERY_SCHEMA
            ));
        }
        let report = match descriptor["method"].as_str().unwrap_or("") {
            "candidate/query" => Some("semaprax.project-candidate.v1"),
            "candidate/recovery-export" => Some("semaprax.project-candidate-recovery.v1"),
            "image/dependencies" => Some(crate::project::IMAGE_DECLARATION_DEPENDENCIES_SCHEMA),
            "image/cleanup-dependencies" => Some(crate::project::IMAGE_CLEANUP_DEPENDENCIES_SCHEMA),
            "candidate/cleanup-dependencies" => {
                Some(crate::project::PROJECT_CANDIDATE_CLEANUP_DEPENDENCIES_SCHEMA)
            }
            "hole/recovery-export" => Some(crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA),
            "hole/archive-export" => Some(crate::project::PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA),
            "attempt/query" => Some("semaprax.project-candidate-attempt.v1"),
            "candidate/semantic-delta" => Some("semaprax.project-candidate-semantic-delta.v1"),
            "candidate/semantic-delta-catalog" => {
                Some("semaprax.project-candidate-semantic-delta-catalog.v1")
            }
            "protocol/conformance" => Some(crate::project::IMAGE_PROTOCOL_CONFORMANCE_SCHEMA),
            "candidate/interface-catalog" => Some("semaprax.project-interface-change-catalog.v1"),
            "candidate/interface-delta" => Some("semaprax.project-candidate-interface-delta.v1"),
            "candidate/contract-delta" => Some("semaprax.project-candidate-contract-delta.v1"),
            "candidate/ownership-delta" => Some("semaprax.project-candidate-ownership-delta.v1"),
            "candidate/source-review" => {
                Some(crate::project::PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA)
            }
            "candidate/symbol-diagnostics" => {
                Some("semaprax.project-candidate-symbol-diagnostics.v1")
            }
            "image/target-admission" => Some(crate::project::IMAGE_TARGET_ADMISSION_SCHEMA),
            "candidate/build" => Some(crate::project::IMAGE_ARTIFACT_PROJECTION_SCHEMA),
            "candidate/artifact-delta" => Some("semaprax.project-candidate-artifact-delta.v1"),
            "candidate/analysis-artifact-evidence" => {
                Some(crate::project::PROJECT_CANDIDATE_ANALYSIS_ARTIFACT_EVIDENCE_SCHEMA)
            }
            "candidate/analysis-boundary-bundle" => {
                Some(crate::project::PROJECT_CANDIDATE_ANALYSIS_BOUNDARY_BUNDLE_REPORT_SCHEMA)
            }
            "candidate/environment-aware-review" => {
                Some(crate::project::PROJECT_CANDIDATE_ENVIRONMENT_REVIEW_SCHEMA)
            }
            "candidate/environment-consumer-review" => {
                Some(crate::project::PROJECT_CANDIDATE_ENVIRONMENT_CONSUMER_REVIEW_SCHEMA)
            }
            "candidate/external-api-contract-delta" => {
                Some(crate::project::PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_DELTA_SCHEMA)
            }
            "candidate/analysis-deployment-contract-evidence" => {
                Some(crate::project::PROJECT_CANDIDATE_DEPLOYMENT_CONTRACT_EVIDENCE_SCHEMA)
            }
            "candidate/analysis-external-api-contract-evidence" => {
                Some(crate::project::PROJECT_CANDIDATE_EXTERNAL_API_CONTRACT_EVIDENCE_SCHEMA)
            }
            "candidate/analysis-generated-file-provenance-evidence" => {
                Some(crate::project::PROJECT_CANDIDATE_GENERATED_FILE_PROVENANCE_EVIDENCE_SCHEMA)
            }
            "candidate/commit-report" => {
                Some(crate::project::PROJECT_CANDIDATE_GIT_PUBLICATION_SCHEMA)
            }
            _ => None,
        };
        if let Some(report) = report {
            references.insert(format!("urn:{report}"));
        }
    }
    let unresolved = references
        .into_iter()
        .filter(|id| {
            !id.starts_with('#') && !documents.contains_key(id.split('#').next().unwrap_or(id))
        })
        .collect::<Vec<_>>();
    let mut request_refs = BTreeSet::new();
    for descriptor in descriptors {
        references_in(&descriptor["request_schema"], &mut request_refs);
    }
    if request_refs
        .iter()
        .any(|reference| unresolved.contains(reference))
    {
        return Err(invalid(
            "selected request descriptor has an unbundled constructor reference",
        ));
    }
    let string_rules = if descriptors
        .iter()
        .any(|descriptor| descriptor["method"] == "workspace/read-batch")
    {
        "UTF-8-byte_limits; control_character_rejection_where_required_by_descriptor; batch_frame_strings_allow_empty_and_LF"
    } else {
        "UTF-8-byte_limits_and_control_character_rejection"
    };
    bounded(
        json!({"schema":"semaprax.image-agent-schemas.v5","protocol":VNEXT_PROTOCOL_SCHEMA,
        "methods":descriptors,"documents":documents.into_values().collect::<Vec<_>>(),
        "unbundled_payload_schemas":unresolved,"request_schemas_complete":true,
        "payload_completeness":"only_bundled_documents_are_structurally_described; unbundled_payloads_require_owning_specification",
        "wire_rules":{"unknown_parameters":"reject","optional_parameters":"omit; null_rejected_unless_explicitly_nullable","strings":string_rules,"request_ids":"unsigned_u64_or_nonempty_bounded_string; notifications_do_not_execute","integer_bounds":"exact_descriptor_minimum_and_maximum","max_request_bytes":MAX_REQUEST_BYTES,"max_response_bytes":MAX_RESPONSE_BYTES}}),
    )
}

fn draft_recovery_schemas() -> [Value; 2] {
    use payload_schemas::{digest, document, object};
    let id = json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.-]+$"});
    let target = json!({"type":"string","minLength":1,"maxLength":4096,"x-max-utf8-bytes":4096});
    let body = object(vec![
        ("kind", json!({"const":"function_body"})),
        ("hole_id", id.clone()),
        ("target", target.clone()),
    ]);
    let expression = object(vec![
        ("kind", json!({"const":"expression"})),
        ("hole_id", id.clone()),
        ("target", target.clone()),
        ("expression_id", target.clone()),
    ]);
    let contract = object(vec![
        ("kind", json!({"const":"contract_expression"})),
        ("hole_id", id.clone()),
        ("target", target.clone()),
        ("expression_id", target.clone()),
    ]);
    let holes = json!({"type":"array","maxItems":crate::project::MAX_PROJECT_CANDIDATE_HOLES,
        "items":{"oneOf":[body,expression,contract]},"x-sorted-by":"hole_id","x-unique-hole-id":true});
    let v1 = document(
        crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA,
        vec![
            (
                "compiler",
                json!({"const":{
                    "package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),
                    "compatibility":crate::project::PROJECT_CANDIDATE_DRAFT_RECOVERY_COMPATIBILITY,
                }}),
            ),
            ("base_revision", digest()),
            (
                "draft_schema",
                json!({"const":crate::project::PROJECT_CANDIDATE_DRAFT_SCHEMA}),
            ),
            (
                "candidate_recovery",
                json!({"$ref":"urn:semaprax.project-candidate-recovery.v1"}),
            ),
            ("holes", holes.clone()),
            ("draft_digest", digest()),
            ("capsule_digest", digest()),
        ],
    );
    let filled = json!({"type":"array","maxItems":crate::project::MAX_PROJECT_CANDIDATE_DRAFT_LINEAGE,
    "x-sorted-by":"event_id","items":object(vec![
        ("event_id",digest()),("hole_id",id),("kind",json!({"enum":["replace_function_body","replace_expression","replace_contract_expression"]})),
        ("target",target.clone()),("expression_id",payload_schemas::nullable(target.clone())),
        ("intent_digest",digest()),("history_ordinal",payload_schemas::uint()),
        ("origin_draft_digest",digest()),
    ])});
    let ancestry = json!({"type":"array","maxItems":crate::project::MAX_PROJECT_CANDIDATE_DRAFT_LINEAGE,
    "items":object(vec![
        ("operation",json!({"enum":["rebase","merge"]})),
        ("parents",json!({"type":"array","minItems":1,"maxItems":2,"items":digest()})),
        ("onto_revision",payload_schemas::nullable(digest())),
    ])});
    let v2 = document(
        crate::project::PROJECT_CANDIDATE_DRAFT_LINEAGE_RECOVERY_SCHEMA,
        vec![
            (
                "compiler",
                json!({"const":{
                    "package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),
                    "compatibility":crate::project::PROJECT_CANDIDATE_DRAFT_LINEAGE_RECOVERY_COMPATIBILITY,
                }}),
            ),
            ("base_revision", digest()),
            (
                "draft_schema",
                json!({"const":crate::project::PROJECT_CANDIDATE_DRAFT_LINEAGE_SCHEMA}),
            ),
            (
                "candidate_recovery",
                json!({"$ref":"urn:semaprax.project-candidate-recovery.v1"}),
            ),
            ("holes", holes),
            ("filled_hole_lineage", filled),
            ("branch_ancestry", ancestry),
            ("draft_digest", digest()),
            ("capsule_digest", digest()),
        ],
    );
    [v1, v2]
}

fn draft_archive_schema() -> Value {
    use payload_schemas::{digest, document};
    document(
        crate::project::PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA,
        vec![
            (
                "compiler",
                json!({"const":{
                    "package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),
                    "compatibility":crate::project::PROJECT_CANDIDATE_DRAFT_ARCHIVE_COMPATIBILITY,
                    "binary_identity_claimed":false,
                }}),
            ),
            ("base_revision", digest()),
            ("candidate_digest", digest()),
            ("candidate_archive_digest", digest()),
            ("draft_digest", digest()),
            (
                "candidate_archive",
                json!({"type":"string","minLength":1,"maxLength":crate::project::MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES,"x-max-utf8-bytes":crate::project::MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES,"description":"Exact canonical source-backed candidate archive JSON string; independently rebuilt and replayed by the compiler."}),
            ),
            (
                "draft_recovery_capsule",
                json!({"type":"string","minLength":1,"maxLength":crate::project::MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES,"x-max-utf8-bytes":crate::project::MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES,"description":"Exact canonical draft recovery capsule JSON string; pending selectors are reconstructed through ordinary hole APIs."}),
            ),
            ("source_authority", json!({"const":false})),
            ("approval_authority", json!({"const":false})),
            ("trusted_hir", json!({"const":false})),
            ("archive_digest", digest()),
        ],
    )
}

fn references_in(value: &Value, output: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                output.insert(reference.to_owned());
            }
            for child in object.values() {
                references_in(child, output);
            }
        }
        Value::Array(values) => {
            for child in values {
                references_in(child, output);
            }
        }
        _ => {}
    }
}
fn bounded(mut value: Value) -> Result<Value> {
    value.sort_all_objects();
    if serde_json::to_vec(&value)
        .map_err(|_| invalid("v5 discovery serialization failed"))?
        .len()
        > MAX_DISCOVERY_BYTES
    {
        return Err(vec![Diagnostic::io(
            "SPX-G289",
            "v5 discovery exceeds its bounded response payload",
        )]);
    }
    Ok(value)
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G288", message)]
}

#[cfg(test)]
#[path = "discovery/tests.rs"]
mod tests;
