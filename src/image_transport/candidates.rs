//! Candidate-only protocol: prepare in memory, authenticate, then retain.
//! No candidate or draft ever acquires canonical-source publication authority.

use std::collections::BTreeMap;

use super::*;
use crate::project::{ProjectCandidate, ProjectCandidateDraft, SemanticChange};
use crate::project_transport::codec::RequestId;

pub const CANDIDATE_PROTOCOL_SCHEMA: &str = "semaprax.image-agent-protocol.v2";
pub const CANDIDATE_RESULT_SCHEMA: &str = "semaprax.image-agent-result.v2";
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
    Validate,
    Impact,
    Compare,
    Discard,
    ChangeCatalog,
    ValidationCatalog,
    HoleOpen,
    HoleQuery,
    HoleFill,
    HoleComplete,
    HoleDiscard,
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

struct DraftEntry {
    draft: Arc<ProjectCandidateDraft>,
    source_candidate: String,
}

#[derive(Default)]
pub(super) struct Registry {
    candidates: BTreeMap<String, Arc<ProjectCandidate>>,
    drafts: BTreeMap<String, DraftEntry>,
}

enum Mutation {
    None,
    Candidate(Arc<ProjectCandidate>),
    Draft(DraftEntry),
    DropCandidate(String),
    DropDraft(String),
}

impl Registry {
    fn candidate(&self, id: &str) -> Result<&Arc<ProjectCandidate>, Vec<Diagnostic>> {
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
    fn report_bytes(&self) -> usize {
        self.candidates
            .values()
            .map(|value| value.to_json().len())
            .chain(
                self.drafts
                    .values()
                    .map(|value| value.draft.retained_report_bytes()),
            )
            .sum()
    }
    fn admit(&self, mutation: &Mutation) -> Result<(), Vec<Diagnostic>> {
        let added = match mutation {
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
    fn commit(&mut self, mutation: Mutation) {
        match mutation {
            Mutation::None => (),
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
}

pub(super) fn handle(
    snapshot: &mut ProjectSnapshot,
    image: &ProjectSemanticImage,
    registry: &mut Registry,
    id: &RequestId,
    name: &str,
    params: Map<String, Value>,
) -> Vec<u8> {
    let Some(method) = methods().into_iter().find(|method| method.name == name) else {
        return codec::bounded_error_response(
            Some(id),
            -32601,
            "method is not available in the candidate-only image profile",
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
        let (payload, mutation) = prepare(method, &params, image, registry)?;
        registry.admit(&mutation)?;
        let mut result = json!({"schema":CANDIDATE_RESULT_SCHEMA, "protocol":CANDIDATE_PROTOCOL_SCHEMA, "image_revision":image.image_digest(), "project_revision":image.revision().project_revision(), "payload":payload});
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

fn methods() -> Vec<&'static Method> {
    let mut methods = METHODS.iter().chain(CANDIDATE_METHODS).collect::<Vec<_>>();
    methods.sort_by_key(|method| method.name);
    methods
}

fn descriptor(method: &Method) -> Value {
    let mut descriptor = method_description(method);
    descriptor["success_response_schema"]["properties"]["result"]["properties"]["protocol"]
        ["const"] = json!(CANDIDATE_PROTOCOL_SCHEMA);
    descriptor["success_response_schema"]["properties"]["result"]["properties"]["schema"]
        ["const"] = json!(CANDIDATE_RESULT_SCHEMA);
    descriptor["success_response_schema"]["properties"]["result"]["properties"]["payload"]
        ["$ref"] = json!(format!("urn:{}", profile_payload_schema(method)));
    if matches!(method.operation, Operation::Candidate(_)) {
        descriptor["capability"] = json!("candidate_prepare");
    }
    descriptor
}

fn profile_payload_schema(method: &Method) -> String {
    if matches!(
        method.operation,
        Operation::Capabilities
            | Operation::Schemas
            | Operation::Catalog
            | Operation::Instructions
            | Operation::Client
    ) {
        format!(
            "{}.v2",
            method
                .payload_schema
                .strip_suffix(".v1")
                .expect("v1 discovery payload")
        )
    } else {
        method.payload_schema.to_owned()
    }
}

fn prepare(
    method: &Method,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &Registry,
) -> Result<(Value, Mutation), Vec<Diagnostic>> {
    let payload_schema = profile_payload_schema(method);
    let payload = match method.operation {
        Operation::Candidate(action) => {
            if text(params, "image_revision") != image.image_digest() {
                return Err(failure(
                    "SPX-G221",
                    "candidate protocol image revision is stale",
                ));
            }
            return prepare_candidate(action, params, image, registry);
        }
        Operation::Capabilities => {
            json!({"schema":payload_schema, "protocol":CANDIDATE_PROTOCOL_SCHEMA, "capabilities":["semantic_read","candidate_prepare"], "methods":methods().iter().map(|method| method.name).collect::<Vec<_>>(), "max_request_bytes":MAX_REQUEST_BYTES, "max_response_bytes":MAX_RESPONSE_BYTES, "max_candidates":MAX_CANDIDATES,"max_drafts":MAX_DRAFTS,"max_retained_report_bytes":MAX_RETAINED_REPORT_BYTES,"source_authority":false,"test_execution":false,"target_execution":false})
        }
        Operation::Schemas => {
            json!({"schema":payload_schema,"protocol":CANDIDATE_PROTOCOL_SCHEMA,"methods":methods().into_iter().map(descriptor).collect::<Vec<_>>()})
        }
        Operation::Catalog => {
            json!({"schema":payload_schema,"queries":methods().into_iter().filter(|method| method.query).map(descriptor).collect::<Vec<_>>()})
        }
        Operation::Instructions => {
            json!({"schema":payload_schema,"protocol":CANDIDATE_PROTOCOL_SCHEMA,"instructions":"Use workspace/open for the exact image_revision. candidate/open returns a candidate_revision. Send both on candidate operations. apply-intent returns a new immutable sibling; previous candidates remain unchanged. Query report chunks, run independent candidate/validate, inspect impact and compare. hole/open and hole/fill return draft_revision handles; unresolved drafts can only be queried, filled, completed or discarded, never built or committed. A failed request leaves registries unchanged. Source drift permanently invalidates this session. Only the host can choose this candidate-only profile; no operation elevates it to source-write, runtime test or build authority."})
        }
        Operation::Client => {
            let old_names = serde_json::to_string(&method_names()).expect("method names serialize");
            let names = serde_json::to_string(
                &methods()
                    .iter()
                    .map(|method| method.name)
                    .collect::<Vec<_>>(),
            )
            .expect("method names serialize");
            let source = client_source(text(params, "language"))
                .replace(&old_names, &names)
                .replace(PROTOCOL_SCHEMA, CANDIDATE_PROTOCOL_SCHEMA);
            json!({"schema":payload_schema,"protocol":CANDIDATE_PROTOCOL_SCHEMA,"language":text(params,"language"),"source":source})
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
) -> Result<(Value, Mutation), Vec<Diagnostic>> {
    match action {
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
        Action::ValidationCatalog => Ok((
            json!({"schema":"semaprax.image-validation-catalog.v1","available":[{"method":"candidate/validate","kind":"independent_source_and_target_projection_replay","runtime_execution":false}],"required_external_gates":["affected_project_tests","native_and_wasm_runtime_conformance","full_quality_profile"],"tests":"not_run","source_authority":false}),
            Mutation::None,
        )),
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

fn retain_candidate(
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
