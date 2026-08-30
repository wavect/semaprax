//! Candidate-only protocol: prepare in memory, authenticate, then retain.
//! No candidate or draft ever acquires canonical-source publication authority.

use std::collections::BTreeMap;

use super::*;
use crate::project::{
    CandidateTestPolicy, ProjectCandidate, ProjectCandidateDraft, SemanticChange,
};
use crate::project_transport::codec::RequestId;

pub const CANDIDATE_PROTOCOL_SCHEMA: &str = "semaprax.image-agent-protocol.v2";
pub const CANDIDATE_RESULT_SCHEMA: &str = "semaprax.image-agent-result.v2";
pub const TEST_PROTOCOL_SCHEMA: &str = "semaprax.image-agent-protocol.v3";
pub const TEST_RESULT_SCHEMA: &str = "semaprax.image-agent-result.v3";
pub(super) mod diagnostics;
pub use diagnostics::{DIAGNOSTIC_PROTOCOL_SCHEMA, DIAGNOSTIC_RESULT_SCHEMA};
const MAX_ATTEMPTS: usize = 16;
const MAX_CANDIDATES: usize = 16;
const MAX_DRAFTS: usize = 16;
const MAX_RETAINED_REPORT_BYTES: usize = 256 * 1024 * 1024;

const CANDIDATE: Parameter = Parameter {
    name: "candidate_revision",
    kind: ParameterKind::Digest,
    required: true,
};
const DRAFT: Parameter = Parameter {
    name: "draft_revision",
    kind: ParameterKind::Digest,
    required: true,
};
const HOLE: Parameter = Parameter {
    name: "hole_id",
    kind: ParameterKind::Text(128),
    required: true,
};
const OFFSET: Parameter = Parameter {
    name: "offset",
    kind: ParameterKind::Integer(0, crate::project::MAX_PROJECT_CANDIDATE_BYTES),
    required: false,
};
const CHUNK: Parameter = Parameter {
    name: "chunk_bytes",
    kind: ParameterKind::Integer(1024, 65_536),
    required: false,
};

#[derive(Clone, Copy)]
pub(super) enum Action {
    Open,
    Apply,
    Query,
    RecoveryExport,
    RecoveryRestore,
    Validate,
    Impact,
    Compare,
    Merge,
    Rebase,
    Discard,
    ChangeCatalog,
    ExpressionCatalog,
    ConstructorSchemas,
    ValidationCatalog,
    HoleOpen,
    HoleQuery,
    HoleFill,
    HoleComplete,
    HoleDiscard,
    TestPlan,
    Test,
    Diagnostic(diagnostics::Action),
}

macro_rules! method {
    ($name:literal, $action:ident, $params:expr, $query:expr, $schema:literal) => {
        Method {
            name: $name,
            operation: Operation::Candidate(Action::$action),
            parameters: $params,
            query: $query,
            payload_schema: $schema,
        }
    };
}

const TEST_METHODS: &[Method] = &[
    method!(
        "candidate/test-plan",
        TestPlan,
        &[REVISION, CANDIDATE],
        true,
        "semaprax.project-candidate-test-plan.v1"
    ),
    method!(
        "candidate/test",
        Test,
        &[REVISION, CANDIDATE],
        false,
        "semaprax.project-candidate-test-report.v1"
    ),
];

const CANDIDATE_METHODS: &[Method] = &[
    method!(
        "candidate/apply-intent",
        Apply,
        &[
            REVISION,
            CANDIDATE,
            Parameter {
                name: "intent",
                kind: ParameterKind::Object("semaprax.semantic-change-intent.v1"),
                required: true
            }
        ],
        false,
        "semaprax.image-candidate-handle.v1"
    ),
    method!(
        "candidate/compare",
        Compare,
        &[
            REVISION,
            CANDIDATE,
            Parameter {
                name: "other_candidate_revision",
                kind: ParameterKind::Digest,
                required: true
            }
        ],
        true,
        "semaprax.project-candidate-comparison.v1"
    ),
    method!(
        "candidate/discard",
        Discard,
        &[REVISION, CANDIDATE],
        false,
        "semaprax.image-candidate-discard.v1"
    ),
    method!(
        "candidate/impact",
        Impact,
        &[REVISION, CANDIDATE, TARGET, DEPTH, BYTES, NODES],
        true,
        "semaprax.image-candidate-impact.v1"
    ),
    method!(
        "candidate/merge",
        Merge,
        &[
            REVISION,
            CANDIDATE,
            Parameter {
                name: "other_candidate_revision",
                kind: ParameterKind::Digest,
                required: true
            }
        ],
        false,
        "semaprax.image-candidate-reconciliation.v1"
    ),
    method!(
        "candidate/rebase",
        Rebase,
        &[
            REVISION,
            CANDIDATE,
            Parameter {
                name: "new_base_candidate_revision",
                kind: ParameterKind::Digest,
                required: true
            }
        ],
        false,
        "semaprax.image-candidate-reconciliation.v1"
    ),
    method!(
        "candidate/open",
        Open,
        &[REVISION],
        false,
        "semaprax.image-candidate-handle.v1"
    ),
    method!(
        "candidate/query",
        Query,
        &[REVISION, CANDIDATE, OFFSET, CHUNK],
        true,
        "semaprax.image-candidate-report-chunk.v1"
    ),
    method!(
        "candidate/recovery-export",
        RecoveryExport,
        &[REVISION, CANDIDATE, OFFSET, CHUNK],
        true,
        "semaprax.image-candidate-recovery-chunk.v1"
    ),
    method!(
        "candidate/recovery-restore",
        RecoveryRestore,
        &[
            REVISION,
            Parameter {
                name: "capsule",
                kind: ParameterKind::Object("semaprax.project-candidate-recovery.v1"),
                required: true
            }
        ],
        false,
        "semaprax.image-candidate-handle.v1"
    ),
    method!(
        "candidate/validate",
        Validate,
        &[REVISION, CANDIDATE],
        true,
        "semaprax.image-candidate-validation.v1"
    ),
    method!(
        "change/catalog",
        ChangeCatalog,
        &[REVISION, CANDIDATE, TARGET],
        true,
        "semaprax.project-change-catalog.v1"
    ),
    method!(
        "expression/catalog",
        ExpressionCatalog,
        &[REVISION, CANDIDATE, TARGET],
        true,
        "semaprax.project-expression-catalog.v1"
    ),
    method!(
        "protocol/constructor-schemas",
        ConstructorSchemas,
        &[REVISION],
        true,
        "semaprax.candidate-constructor-schemas.v1"
    ),
    method!(
        "hole/complete",
        HoleComplete,
        &[REVISION, DRAFT],
        false,
        "semaprax.image-candidate-handle.v1"
    ),
    method!(
        "hole/discard",
        HoleDiscard,
        &[REVISION, DRAFT],
        false,
        "semaprax.image-draft-discard.v1"
    ),
    method!(
        "hole/fill",
        HoleFill,
        &[
            REVISION,
            DRAFT,
            HOLE,
            Parameter {
                name: "expression",
                kind: ParameterKind::Object("semaprax.typed-expression.v1"),
                required: true
            }
        ],
        false,
        "semaprax.image-draft-handle.v1"
    ),
    method!(
        "hole/open",
        HoleOpen,
        &[
            REVISION,
            CANDIDATE,
            TARGET,
            HOLE,
            Parameter {
                name: "draft_revision",
                kind: ParameterKind::Digest,
                required: false
            }
        ],
        false,
        "semaprax.image-draft-handle.v1"
    ),
    method!(
        "hole/query",
        HoleQuery,
        &[REVISION, DRAFT, HOLE],
        true,
        "semaprax.project-candidate-hole-context.v1"
    ),
    method!(
        "validation/catalog",
        ValidationCatalog,
        &[REVISION],
        true,
        "semaprax.image-validation-catalog.v1"
    ),
];

pub(super) struct DraftEntry {
    draft: Arc<ProjectCandidateDraft>,
    source_candidate: String,
}

#[derive(Default)]
pub(super) struct Registry {
    candidates: BTreeMap<String, Arc<ProjectCandidate>>,
    drafts: BTreeMap<String, DraftEntry>,
    attempts: BTreeMap<String, Arc<crate::project::ProjectCandidateAttempt>>,
}

pub(super) enum Mutation {
    None,
    Candidate(Arc<ProjectCandidate>),
    Draft(DraftEntry),
    DropCandidate(String),
    DropDraft(String),
    Attempt(Arc<crate::project::ProjectCandidateAttempt>),
    DropAttempt(String),
}

impl Registry {
    pub(in crate::image_transport) fn retained_attempts(
        &self,
    ) -> impl Iterator<Item = &Arc<crate::project::ProjectCandidateAttempt>> {
        self.attempts.values()
    }

    pub(super) fn candidate(&self, id: &str) -> Result<&Arc<ProjectCandidate>, Vec<Diagnostic>> {
        self.candidates.get(id).ok_or_else(|| {
            failure(
                "SPX-G224",
                "candidate handle is stale, discarded, or unknown",
            )
        })
    }
    fn draft(&self, id: &str) -> Result<&DraftEntry, Vec<Diagnostic>> {
        self.drafts
            .get(id)
            .ok_or_else(|| failure("SPX-G232", "draft handle is stale, discarded, or unknown"))
    }
    pub(super) fn draft_value(&self, id: &str) -> Result<&ProjectCandidateDraft, Vec<Diagnostic>> {
        Ok(self.draft(id)?.draft.as_ref())
    }
    pub(super) fn retain_recovered_draft(
        &self,
        draft: ProjectCandidateDraft,
        source_candidate: &str,
    ) -> Result<(Value, Mutation), Vec<Diagnostic>> {
        let source_candidate = self
            .drafts
            .get(draft.draft_digest())
            .map(|entry| entry.source_candidate.as_str())
            .unwrap_or(source_candidate);
        retain_draft(draft, source_candidate)
    }
    fn report_bytes(&self) -> usize {
        self.candidates
            .values()
            .map(|value| value.to_json().len())
            .chain(
                self.drafts
                    .values()
                    .map(|value| value.draft.retained_report_bytes()),
            )
            .chain(
                self.attempts
                    .values()
                    .map(|attempt| attempt.retained_report_bytes()),
            )
            .sum()
    }
    pub(super) fn admit(&self, mutation: &Mutation) -> Result<(), Vec<Diagnostic>> {
        let added = match mutation {
            Mutation::Attempt(attempt) if !self.attempts.contains_key(attempt.attempt_digest()) => {
                if self.attempts.len() >= MAX_ATTEMPTS {
                    return Err(failure("SPX-G242", "rejected attempt registry is full"));
                }
                attempt.retained_report_bytes()
            }
            Mutation::Candidate(value)
                if !self.candidates.contains_key(value.candidate_digest()) =>
            {
                if self.candidates.len() >= MAX_CANDIDATES {
                    return Err(failure("SPX-G223", "candidate registry is full"));
                }
                value.to_json().len()
            }
            Mutation::Draft(value) if !self.drafts.contains_key(value.draft.draft_digest()) => {
                if self.drafts.len() >= MAX_DRAFTS {
                    return Err(failure("SPX-G231", "draft registry is full"));
                }
                value.draft.retained_report_bytes()
            }
            _ => 0,
        };
        if self.report_bytes().saturating_add(added) > MAX_RETAINED_REPORT_BYTES {
            return Err(failure(
                "SPX-G223",
                "retained candidate and draft report bytes exceed registry bound",
            ));
        }
        Ok(())
    }
    pub(super) fn commit(&mut self, mutation: Mutation) {
        match mutation {
            Mutation::None => (),
            Mutation::Attempt(attempt) => {
                self.attempts
                    .entry(attempt.attempt_digest().to_owned())
                    .or_insert(attempt);
            }
            Mutation::DropAttempt(id) => {
                self.attempts.remove(&id);
            }
            Mutation::Candidate(candidate) => {
                self.candidates
                    .entry(candidate.candidate_digest().to_owned())
                    .or_insert(candidate);
            }
            Mutation::Draft(value) => {
                self.drafts
                    .entry(value.draft.draft_digest().to_owned())
                    .or_insert(value);
            }
            Mutation::DropCandidate(id) => {
                self.candidates.remove(&id);
            }
            Mutation::DropDraft(id) => {
                self.drafts.remove(&id);
            }
        }
    }

    pub(super) fn refresh_inventory(&self) -> Value {
        json!({"retained_candidates":self.candidates.keys().collect::<Vec<_>>(),"cleared_drafts":self.drafts.len(),"cleared_attempts":self.attempts.len()})
    }

    pub(super) fn clear_transients(&mut self) {
        self.drafts.clear();
        self.attempts.clear();
    }
}

pub(super) fn handle_diagnostics(
    snapshot: &mut ProjectSnapshot,
    image: &ProjectSemanticImage,
    registry: &mut Registry,
    test_policy: Option<&CandidateTestPolicy>,
    id: &RequestId,
    name: &str,
    params: Map<String, Value>,
) -> Vec<u8> {
    diagnostics::handle(snapshot, image, registry, test_policy, id, name, params)
}

pub(super) fn handle(
    snapshot: &mut ProjectSnapshot,
    image: &ProjectSemanticImage,
    registry: &mut Registry,
    test_policy: Option<&CandidateTestPolicy>,
    id: &RequestId,
    name: &str,
    params: Map<String, Value>,
) -> Vec<u8> {
    let test_enabled = test_policy.is_some();
    let Some(method) = methods(test_enabled)
        .into_iter()
        .find(|method| method.name == name)
    else {
        return codec::bounded_error_response(
            Some(id),
            -32601,
            if test_enabled {
                "method is not available in the test-enabled image profile"
            } else {
                "method is not available in the candidate-only image profile"
            },
            MAX_RESPONSE_BYTES,
        );
    };
    if let Err(message) = validate_parameters(method, &params) {
        return codec::bounded_error_response(
            Some(id),
            codec::INVALID_PARAMS,
            &message,
            MAX_RESPONSE_BYTES,
        );
    }
    let prepared = snapshot.with_authenticated_request(|_| {
        let (payload, mutation) = prepare(method, &params, image, registry, test_policy)?;
        registry.admit(&mutation)?;
        let mut result = json!({"schema":result_schema(test_enabled), "protocol":protocol_schema(test_enabled), "image_revision":image.image_digest(), "project_revision":image.revision().project_revision(), "payload":payload});
        result.sort_all_objects();
        let response = codec::bounded_success_response(id, &result.to_string(), MAX_RESPONSE_BYTES);
        // Response overflow is a failure. Discard the prepared mutation even
        // though the typed object itself was valid and capacity was available.
        let mutation = if codec::is_overflow_response(&response) { Mutation::None } else { mutation };
        Ok((response, mutation))
    });
    match prepared {
        Ok((response, mutation)) => {
            registry.commit(mutation);
            response
        }
        Err(errors) => codec::bounded_error_response(
            Some(id),
            -32000,
            &diagnostics(&errors),
            MAX_RESPONSE_BYTES,
        ),
    }
}

pub(super) fn methods(test_enabled: bool) -> Vec<&'static Method> {
    let mut methods = METHODS.iter().chain(CANDIDATE_METHODS).collect::<Vec<_>>();
    if test_enabled {
        methods.extend(TEST_METHODS);
    }
    methods.sort_by_key(|method| method.name);
    methods
}

fn protocol_schema(test_enabled: bool) -> &'static str {
    if test_enabled {
        TEST_PROTOCOL_SCHEMA
    } else {
        CANDIDATE_PROTOCOL_SCHEMA
    }
}
fn result_schema(test_enabled: bool) -> &'static str {
    if test_enabled {
        TEST_RESULT_SCHEMA
    } else {
        CANDIDATE_RESULT_SCHEMA
    }
}

fn descriptor(method: &Method, test_enabled: bool) -> Value {
    let mut descriptor = method_description(method);
    descriptor["success_response_schema"]["properties"]["result"]["properties"]["protocol"]
        ["const"] = json!(protocol_schema(test_enabled));
    descriptor["success_response_schema"]["properties"]["result"]["properties"]["schema"]
        ["const"] = json!(result_schema(test_enabled));
    descriptor["success_response_schema"]["properties"]["result"]["properties"]["payload"]
        ["$ref"] = json!(format!(
        "urn:{}",
        profile_payload_schema(method, test_enabled)
    ));
    if matches!(method.operation, Operation::Candidate(_)) {
        descriptor["capability"] = json!("candidate_prepare");
    }
    if matches!(method.operation, Operation::Candidate(Action::Test)) {
        descriptor["capability"] = json!("candidate_test");
    }
    descriptor
}

fn profile_payload_schema(method: &Method, test_enabled: bool) -> String {
    if test_enabled
        && matches!(
            method.operation,
            Operation::Candidate(Action::ValidationCatalog)
        )
    {
        return "semaprax.image-validation-catalog.v2".to_owned();
    }
    if matches!(
        method.operation,
        Operation::Capabilities
            | Operation::Schemas
            | Operation::Catalog
            | Operation::Instructions
            | Operation::Client
    ) {
        format!(
            "{}.v{}",
            method
                .payload_schema
                .strip_suffix(".v1")
                .expect("v1 discovery payload"),
            if test_enabled { 3 } else { 2 }
        )
    } else {
        method.payload_schema.to_owned()
    }
}

pub(super) fn prepare(
    method: &Method,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &Registry,
    test_policy: Option<&CandidateTestPolicy>,
) -> Result<(Value, Mutation), Vec<Diagnostic>> {
    let test_enabled = test_policy.is_some();
    let protocol = protocol_schema(test_enabled);
    let payload_schema = profile_payload_schema(method, test_enabled);
    let payload = match method.operation {
        Operation::Candidate(action) => {
            if text(params, "image_revision") != image.image_digest() {
                return Err(failure(
                    "SPX-G221",
                    "candidate protocol image revision is stale",
                ));
            }
            return prepare_candidate(action, params, image, registry, test_policy);
        }
        Operation::Capabilities => {
            let mut payload = json!({"schema":payload_schema, "protocol":protocol, "capabilities":["semantic_read","candidate_prepare"], "methods":methods(test_enabled).iter().map(|method| method.name).collect::<Vec<_>>(), "max_request_bytes":MAX_REQUEST_BYTES, "max_response_bytes":MAX_RESPONSE_BYTES, "max_candidates":MAX_CANDIDATES,"max_drafts":MAX_DRAFTS,"max_retained_report_bytes":MAX_RETAINED_REPORT_BYTES,"source_authority":false,"test_execution":test_enabled,"target_execution":false});
            if let Some(policy) = test_policy {
                payload["capabilities"] =
                    json!(["semantic_read", "candidate_prepare", "candidate_test"]);
                payload["test_policy"] = json!({"max_steps":policy.max_steps(),"max_execution_bytes":policy.max_execution_bytes(),"max_report_bytes":policy.max_report_bytes(),"engine":"project_interpreter","scope":"complete_declared_test_closure","request_overrides":false});
            }
            payload
        }
        Operation::Schemas => {
            json!({"schema":payload_schema,"protocol":protocol,"methods":methods(test_enabled).into_iter().map(|method| descriptor(method,test_enabled)).collect::<Vec<_>>()})
        }
        Operation::Catalog => {
            json!({"schema":payload_schema,"queries":methods(test_enabled).into_iter().filter(|method| method.query).map(|method| descriptor(method,test_enabled)).collect::<Vec<_>>()})
        }
        Operation::Instructions => {
            let instructions = if test_enabled {
                "Use workspace/open then candidate/open for exact revision handles. Candidate preparation, validation and holes remain source-authority-free. candidate/test-plan reports static relevance, not coverage. candidate/test independently replays the complete candidate and executes its complete declared test closure under fixed host limits. No request can increase limits or grant source, native/Wasm runtime, process, build or artifact authority. Unresolved drafts cannot be tested. No test result commits a candidate or satisfies external target/full-quality gates. Source drift invalidates the session."
            } else {
                "Use workspace/open for the exact image_revision. candidate/open returns a candidate_revision. Send both on candidate operations. apply-intent returns a new immutable sibling; previous candidates remain unchanged. Query report chunks, run independent candidate/validate, inspect impact and compare. hole/open and hole/fill return draft_revision handles; unresolved drafts can only be queried, filled, completed or discarded, never built or committed. A failed request leaves registries unchanged. Source drift permanently invalidates this session. Only the host can choose this candidate-only profile; no operation elevates it to source-write, runtime test or build authority."
            };
            json!({"schema":payload_schema,"protocol":protocol,"instructions":instructions})
        }
        Operation::Client => {
            let old_names = serde_json::to_string(&method_names()).expect("method names serialize");
            let names = serde_json::to_string(
                &methods(test_enabled)
                    .iter()
                    .map(|method| method.name)
                    .collect::<Vec<_>>(),
            )
            .expect("method names serialize");
            let source = client_source(text(params, "language"))
                .replace(&old_names, &names)
                .replace(PROTOCOL_SCHEMA, protocol);
            json!({"schema":payload_schema,"protocol":protocol,"language":text(params,"language"),"source":source})
        }
        _ => dispatch(method, params, image)?,
    };
    Ok((payload, Mutation::None))
}

fn prepare_candidate(
    action: Action,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &Registry,
    test_policy: Option<&CandidateTestPolicy>,
) -> Result<(Value, Mutation), Vec<Diagnostic>> {
    match action {
        Action::Diagnostic(_) => Err(failure(
            "SPX-G241",
            "diagnostic actions require the explicitly selected v4 profile",
        )),
        Action::TestPlan => {
            if test_policy.is_none() {
                return Err(failure(
                    "SPX-G239",
                    "candidate test profile is not selected by the host",
                ));
            }
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            Ok((
                parse_payload(candidate.test_plan(candidate.candidate_digest())?)?,
                Mutation::None,
            ))
        }
        Action::Test => {
            let policy = test_policy.ok_or_else(|| {
                failure(
                    "SPX-G239",
                    "candidate test execution requires explicit host policy",
                )
            })?;
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            let report = candidate.execute_tests(candidate.candidate_digest(), policy)?;
            Ok((parse_payload(report.to_json().to_owned())?, Mutation::None))
        }
        Action::Open => retain_candidate(Arc::new(ProjectCandidate::open(
            Arc::clone(image.revision()),
            image.revision().project_revision(),
        )?)),
        Action::Apply => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            let change =
                SemanticChange::new(candidate.revision().project_revision(), &params["intent"])?;
            retain_candidate(Arc::new(
                candidate.apply(candidate.candidate_digest(), &change)?,
            ))
        }
        Action::Query => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            let report = candidate.to_json();
            let offset = number(params, "offset", 0);
            if offset > report.len() || !report.is_char_boundary(offset) {
                return Err(failure(
                    "SPX-G222",
                    "candidate query offset is outside canonical UTF-8 report",
                ));
            }
            let mut end = offset
                .saturating_add(number(params, "chunk_bytes", 16_384))
                .min(report.len());
            while !report.is_char_boundary(end) {
                end -= 1;
            }
            Ok((
                json!({"schema":"semaprax.image-candidate-report-chunk.v1","candidate_revision":candidate.candidate_digest(),"report_schema":crate::project::PROJECT_CANDIDATE_SCHEMA,"offset":offset,"total_bytes":report.len(),"chunk":&report[offset..end],"next_offset":(end<report.len()).then_some(end),"source_authority":false}),
                Mutation::None,
            ))
        }
        Action::RecoveryExport => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            let capsule = candidate.recovery_capsule()?;
            let offset = number(params, "offset", 0);
            if offset > capsule.len() || !capsule.is_char_boundary(offset) {
                return Err(failure(
                    "SPX-G236",
                    "recovery offset is outside canonical UTF-8 capsule",
                ));
            }
            let mut end = offset
                .saturating_add(number(params, "chunk_bytes", 16_384))
                .min(capsule.len());
            while !capsule.is_char_boundary(end) {
                end -= 1;
            }
            Ok((
                json!({"schema":"semaprax.image-candidate-recovery-chunk.v1","candidate_revision":candidate.candidate_digest(),"capsule_schema":crate::project::PROJECT_CANDIDATE_RECOVERY_SCHEMA,"offset":offset,"total_bytes":capsule.len(),"chunk":&capsule[offset..end],"next_offset":(end<capsule.len()).then_some(end),"source_authority":false}),
                Mutation::None,
            ))
        }
        Action::RecoveryRestore => {
            let mut capsule = params["capsule"].clone();
            capsule.sort_all_objects();
            let bytes = format!("{capsule}\n");
            retain_candidate(Arc::new(ProjectCandidate::restore(
                Arc::clone(image.revision()),
                image.revision().project_revision(),
                bytes.as_bytes(),
            )?))
        }
        Action::Validate => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            let report = parse_payload(candidate.to_json().to_owned())?;
            let changes = report["changes"]
                .as_array()
                .ok_or_else(|| failure("SPX-G222", "retained candidate lacks change inventory"))?
                .iter()
                .map(|change| {
                    SemanticChange::new(
                        change["base_revision"]
                            .as_str()
                            .ok_or_else(|| failure("SPX-G222", "retained change lacks revision"))?,
                        &change["intent"],
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let replay = ProjectCandidate::replay(
                Arc::clone(candidate.base_revision()),
                candidate.base_revision().project_revision(),
                &changes,
                candidate.to_json().as_bytes(),
            )?;
            Ok((
                json!({"schema":"semaprax.image-candidate-validation.v1","candidate_revision":replay.candidate_digest(),"independently_replayed":true,"source_reparsed":true,"project_profile_admitted":true,"tests":"not_run","target_execution":false,"commit_authority":false}),
                Mutation::None,
            ))
        }
        Action::Impact => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            let options = WorkspaceImpactOptions::new(
                number(params, "depth", 16),
                number(params, "max_bytes", MAX_QUERY_BYTES),
                number(params, "max_nodes", 1024),
            )
            .map_err(|error| vec![error])?;
            let mut impact = parse_payload(candidate.revision().semantic_impact(
                WorkspaceAnalysisTargetKind::Declaration,
                text(params, "target"),
                options,
            )?)?;
            // Keep the existing payload unchanged; candidate selection is
            // explicit in this additive wrapper, never an authority receipt.
            impact = json!({"schema":"semaprax.image-candidate-impact.v1","candidate_revision":candidate.candidate_digest(),"impact":impact});
            Ok((impact, Mutation::None))
        }
        Action::Compare => {
            let left = registry.candidate(text(params, "candidate_revision"))?;
            let right = registry.candidate(text(params, "other_candidate_revision"))?;
            Ok((parse_payload(left.compare(right)?)?, Mutation::None))
        }
        Action::Merge | Action::Rebase => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            let prepared = if matches!(action, Action::Merge) {
                let other = registry.candidate(text(params, "other_candidate_revision"))?;
                candidate.merge(
                    candidate.candidate_digest(),
                    other,
                    other.candidate_digest(),
                )?
            } else {
                let new_base = registry.candidate(text(params, "new_base_candidate_revision"))?;
                candidate.rebase(
                    candidate.candidate_digest(),
                    Arc::clone(new_base.revision()),
                    new_base.revision().project_revision(),
                )?
            };
            if prepared.to_json().len() > 65_536 {
                return Err(failure(
                    "SPX-G234",
                    "candidate reconciliation report exceeds transport report bound",
                ));
            }
            let report = parse_payload(prepared.to_json().to_owned())?;
            let (handle, mutation) = retain_candidate(Arc::new(prepared.into_candidate()))?;
            let selected_candidate = text(
                params,
                if matches!(action, Action::Merge) {
                    "other_candidate_revision"
                } else {
                    "new_base_candidate_revision"
                },
            );
            Ok((
                json!({"schema":"semaprax.image-candidate-reconciliation.v1","kind":if matches!(action,Action::Merge){"merge"}else{"rebase"},"selected_candidate_revision":selected_candidate,"candidate":handle,"report":report}),
                mutation,
            ))
        }
        Action::Discard => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            Ok((
                json!({"schema":"semaprax.image-candidate-discard.v1","candidate_revision":candidate.candidate_digest(),"discarded":true,"source_unchanged":true}),
                Mutation::DropCandidate(candidate.candidate_digest().to_owned()),
            ))
        }
        Action::ChangeCatalog => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            Ok((
                parse_payload(candidate.change_catalog(text(params, "target"))?)?,
                Mutation::None,
            ))
        }
        Action::ExpressionCatalog => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            Ok((
                parse_payload(candidate.expression_catalog(text(params, "target"))?)?,
                Mutation::None,
            ))
        }
        Action::ConstructorSchemas => Ok((
            parse_payload(SemanticChange::constructor_schemas()?)?,
            Mutation::None,
        )),
        Action::ValidationCatalog => {
            let mut payload = json!({"schema":"semaprax.image-validation-catalog.v1","available":[{"method":"candidate/validate","kind":"independent_source_and_target_projection_replay","runtime_execution":false}],"required_external_gates":["affected_project_tests","native_and_wasm_runtime_conformance","full_quality_profile"],"tests":"not_run","source_authority":false});
            if test_policy.is_some() {
                payload["schema"] = json!("semaprax.image-validation-catalog.v2");
                payload["available"].as_array_mut().expect("array").push(json!({"method":"candidate/test","kind":"bounded_project_interpreter_test_closure","runtime_execution":true}));
                payload["tests"] = json!("available_only_on_explicit_request");
                payload["required_external_gates"] = json!([
                    "native_and_wasm_runtime_conformance",
                    "full_quality_profile"
                ]);
            }
            Ok((payload, Mutation::None))
        }
        Action::HoleOpen => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            let draft = if let Some(id) = params.get("draft_revision").and_then(Value::as_str) {
                let entry = registry.draft(id)?;
                if entry.source_candidate != candidate.candidate_digest() {
                    return Err(failure(
                        "SPX-G232",
                        "draft belongs to a different candidate",
                    ));
                }
                entry
                    .draft
                    .with_body_hole(id, text(params, "target"), text(params, "hole_id"))?
            } else {
                let draft = ProjectCandidateDraft::open(Arc::clone(candidate))?;
                draft.with_body_hole(
                    draft.draft_digest(),
                    text(params, "target"),
                    text(params, "hole_id"),
                )?
            };
            retain_draft(draft, candidate.candidate_digest())
        }
        Action::HoleQuery => {
            let entry = registry.draft(text(params, "draft_revision"))?;
            Ok((
                parse_payload(
                    entry
                        .draft
                        .hole_context(text(params, "draft_revision"), text(params, "hole_id"))?,
                )?,
                Mutation::None,
            ))
        }
        Action::HoleFill => {
            let entry = registry.draft(text(params, "draft_revision"))?;
            retain_draft(
                entry.draft.fill_hole(
                    text(params, "draft_revision"),
                    text(params, "hole_id"),
                    &params["expression"],
                )?,
                &entry.source_candidate,
            )
        }
        Action::HoleComplete => {
            let entry = registry.draft(text(params, "draft_revision"))?;
            retain_candidate(entry.draft.complete(text(params, "draft_revision"))?)
        }
        Action::HoleDiscard => {
            let entry = registry.draft(text(params, "draft_revision"))?;
            Ok((
                json!({"schema":"semaprax.image-draft-discard.v1","draft_revision":entry.draft.draft_digest(),"discarded":true,"source_unchanged":true}),
                Mutation::DropDraft(entry.draft.draft_digest().to_owned()),
            ))
        }
    }
}

pub(super) fn retain_candidate(
    candidate: Arc<ProjectCandidate>,
) -> Result<(Value, Mutation), Vec<Diagnostic>> {
    Ok((
        json!({"schema":"semaprax.image-candidate-handle.v1","candidate_revision":candidate.candidate_digest(),"project_revision":candidate.revision().project_revision(),"base_revision":candidate.base_revision().project_revision(),"report_bytes":candidate.to_json().len(),"source_authority":false,"tests":"not_run"}),
        Mutation::Candidate(candidate),
    ))
}
fn retain_draft(
    draft: ProjectCandidateDraft,
    source_candidate: &str,
) -> Result<(Value, Mutation), Vec<Diagnostic>> {
    let draft = Arc::new(draft);
    Ok((
        json!({"schema":"semaprax.image-draft-handle.v1","draft_revision":draft.draft_digest(),"source_candidate_revision":source_candidate,"report_bytes":draft.to_json().len(),"source_authority":false,"buildable":false}),
        Mutation::Draft(DraftEntry {
            draft,
            source_candidate: source_candidate.to_owned(),
        }),
    ))
}
fn failure(code: &'static str, message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(code, message)]
}
