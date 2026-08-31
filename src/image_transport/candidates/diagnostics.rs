//! V4 host-selected diagnostics profile; legacy catalogues are untouched.
use super::*;
use crate::project::{ProjectCandidateAttempt, ProjectCandidateAttemptOutcome};

#[path = "diagnostics_reads.rs"]
mod reads;

pub(in crate::image_transport) fn read_payload(
    action: Action,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    subjects: &super::reads::ReadSubjects,
) -> Result<Value, Vec<Diagnostic>> {
    reads::payload(action, params, image, subjects)
}

pub const DIAGNOSTIC_PROTOCOL_SCHEMA: &str = "semaprax.image-agent-protocol.v4";
pub const DIAGNOSTIC_RESULT_SCHEMA: &str = "semaprax.image-agent-result.v4";
const ATTEMPT: Parameter = Parameter {
    name: "attempt_revision",
    kind: ParameterKind::Digest,
    required: true,
};
#[derive(Clone, Copy)]
pub(in crate::image_transport) enum Action {
    Attempt,
    Summary,
    Query,
    RepairCatalog,
    RepairApply,
    Discard,
    Delta,
    DeltaCatalog,
    ExpressionHoleOpen,
    ProtocolConformance,
    InterfaceCatalog,
}
macro_rules! method {
    ($name:literal,$action:ident,$params:expr,$query:expr,$schema:literal) => {
        Method {
            name: $name,
            operation: Operation::Candidate(super::Action::Diagnostic(Action::$action)),
            parameters: $params,
            query: $query,
            payload_schema: $schema,
        }
    };
}
const METHODS_V4: &[Method] = &[
    method!(
        "protocol/conformance",
        ProtocolConformance,
        &[
            REVISION,
            Parameter {
                name: "candidate_revision",
                kind: ParameterKind::Digest,
                required: false
            },
            OFFSET,
            CHUNK
        ],
        true,
        "semaprax.image-protocol-conformance-chunk.v1"
    ),
    method!(
        "candidate/interface-catalog",
        InterfaceCatalog,
        &[REVISION, CANDIDATE, TARGET, OFFSET, CHUNK],
        true,
        "semaprax.image-interface-catalog-chunk.v1"
    ),
    method!(
        "hole/open-expression",
        ExpressionHoleOpen,
        &[
            REVISION,
            CANDIDATE,
            TARGET,
            HOLE,
            Parameter {
                name: "expression_id",
                kind: ParameterKind::Text(4096),
                required: true
            },
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
        "candidate/attempt",
        Attempt,
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
        "semaprax.image-candidate-attempt-outcome.v1"
    ),
    method!(
        "attempt/summary",
        Summary,
        &[REVISION, ATTEMPT],
        true,
        "semaprax.project-candidate-attempt-summary.v1"
    ),
    method!(
        "attempt/query",
        Query,
        &[REVISION, ATTEMPT, OFFSET, CHUNK],
        true,
        "semaprax.image-attempt-report-chunk.v1"
    ),
    method!(
        "attempt/repair-catalog",
        RepairCatalog,
        &[REVISION, ATTEMPT],
        true,
        "semaprax.project-candidate-repair-catalog.v1"
    ),
    method!(
        "attempt/repair-apply",
        RepairApply,
        &[
            REVISION,
            ATTEMPT,
            Parameter {
                name: "repair_id",
                kind: ParameterKind::Digest,
                required: true
            }
        ],
        false,
        "semaprax.image-candidate-handle.v1"
    ),
    method!(
        "attempt/discard",
        Discard,
        &[REVISION, ATTEMPT],
        false,
        "semaprax.image-attempt-discard.v1"
    ),
    method!(
        "candidate/semantic-delta",
        Delta,
        &[REVISION, CANDIDATE, TARGET, OFFSET, CHUNK],
        true,
        "semaprax.image-semantic-delta-chunk.v1"
    ),
    method!(
        "candidate/semantic-delta-catalog",
        DeltaCatalog,
        &[REVISION, CANDIDATE, OFFSET, CHUNK],
        true,
        "semaprax.image-semantic-delta-chunk.v1"
    ),
];
pub(in crate::image_transport) fn methods(test_enabled: bool) -> Vec<&'static Method> {
    let mut methods = super::methods(test_enabled);
    methods.extend(METHODS_V4);
    methods.sort_by_key(|method| method.name);
    methods
}
fn payload_schema(method: &Method, test_enabled: bool) -> String {
    if matches!(
        method.operation,
        Operation::Capabilities
            | Operation::Schemas
            | Operation::Catalog
            | Operation::Instructions
            | Operation::Client
    ) {
        format!(
            "{}.v4",
            method
                .payload_schema
                .strip_suffix(".v1")
                .expect("compiler discovery schema")
        )
    } else {
        super::profile_payload_schema(method, test_enabled)
    }
}
fn descriptor(method: &Method, test_enabled: bool) -> Value {
    let mut result = super::descriptor(method, test_enabled);
    result["success_response_schema"]["properties"]["result"]["properties"]["protocol"]["const"] =
        json!(DIAGNOSTIC_PROTOCOL_SCHEMA);
    result["success_response_schema"]["properties"]["result"]["properties"]["schema"]["const"] =
        json!(DIAGNOSTIC_RESULT_SCHEMA);
    result["success_response_schema"]["properties"]["result"]["properties"]["payload"]["$ref"] =
        json!(format!("urn:{}", payload_schema(method, test_enabled)));
    if matches!(
        method.operation,
        Operation::Candidate(super::Action::Diagnostic(_))
    ) {
        result["capability"] = json!("candidate_diagnostics");
    }
    if matches!(
        method.operation,
        Operation::Candidate(super::Action::Diagnostic(Action::ExpressionHoleOpen))
    ) {
        result["capability"] = json!("candidate_prepare");
    }
    if matches!(
        method.operation,
        Operation::Candidate(super::Action::Diagnostic(
            Action::ProtocolConformance | Action::InterfaceCatalog
        ))
    ) {
        result["capability"] = json!("semantic_read");
    }
    if matches!(
        method.operation,
        Operation::Candidate(super::Action::HoleQuery)
    ) {
        result["success_response_schema"]["properties"]["result"]["properties"]["payload"] = json!({"oneOf":[
            {"$ref":"urn:semaprax.project-candidate-hole-context.v1"},
            {"$ref":"urn:semaprax.project-candidate-expression-hole-context.v1"}
        ]});
    }
    result
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
    let Some(method) = methods(test_policy.is_some())
        .into_iter()
        .find(|method| method.name == name)
    else {
        return codec::bounded_error_response(
            Some(id),
            -32601,
            "method is not available in the host-selected diagnostics profile",
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
    let prepared=snapshot.with_authenticated_request(|_| {
        let (payload,mutation)=prepare(method,&params,image,registry,test_policy)?;
        registry.admit(&mutation)?;
        let mut result=json!({"schema":DIAGNOSTIC_RESULT_SCHEMA,"protocol":DIAGNOSTIC_PROTOCOL_SCHEMA,"image_revision":image.image_digest(),"project_revision":image.revision().project_revision(),"payload":payload});
        result.sort_all_objects();
        let response=codec::bounded_success_response(id,&result.to_string(),MAX_RESPONSE_BYTES);
        let mutation=if codec::is_overflow_response(&response) {Mutation::None} else {mutation};
        Ok((response,mutation))
    });
    match prepared {
        Ok((response, mutation)) => {
            registry.commit(mutation);
            response
        }
        Err(errors) => codec::bounded_error_response(
            Some(id),
            -32000,
            &super::super::diagnostics(&errors),
            MAX_RESPONSE_BYTES,
        ),
    }
}
pub(in crate::image_transport) fn prepare(
    method: &Method,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &Registry,
    policy: Option<&CandidateTestPolicy>,
) -> Result<(Value, Mutation), Vec<Diagnostic>> {
    let test_enabled = policy.is_some();
    let schema = payload_schema(method, test_enabled);
    let value = match method.operation {
        Operation::Candidate(super::Action::Diagnostic(action)) => {
            if text(params, "image_revision") != image.image_digest() {
                return Err(failure(
                    "SPX-G221",
                    "diagnostics protocol image revision is stale",
                ));
            }
            return action_payload(action, params, image, registry);
        }
        Operation::Capabilities => {
            let (mut value, _) = super::prepare(method, params, image, registry, policy)?;
            value["schema"] = json!(schema);
            value["protocol"] = json!(DIAGNOSTIC_PROTOCOL_SCHEMA);
            value["methods"] = json!(methods(test_enabled)
                .iter()
                .map(|method| method.name)
                .collect::<Vec<_>>());
            value["capabilities"]
                .as_array_mut()
                .expect("compiler capability list")
                .push(json!("candidate_diagnostics"));
            value["max_attempts"] = json!(MAX_ATTEMPTS);
            value["diagnostic_execution"] = json!("compiler_admission_only");
            value["protocol_conformance"] = json!({
                "method": "protocol/conformance", "candidate_selection": "optional",
                "evidence": "source_backed_static_signature_conformance",
                "implementation_discovery": "candidate/interface-catalog",
                "dynamic_dispatch": false,
            });
            value
        }
        Operation::Schemas => {
            json!({"schema":schema,"protocol":DIAGNOSTIC_PROTOCOL_SCHEMA,"methods":methods(test_enabled).iter().map(|method|descriptor(method,test_enabled)).collect::<Vec<_>>()})
        }
        Operation::Catalog => {
            json!({"schema":schema,"queries":methods(test_enabled).iter().filter(|method|method.query).map(|method|descriptor(method,test_enabled)).collect::<Vec<_>>()})
        }
        Operation::Instructions => {
            json!({"schema":schema,"protocol":DIAGNOSTIC_PROTOCOL_SCHEMA,"instructions":"Use workspace/open for image_revision and candidate/open for candidate_revision. candidate/apply-intent remains the ordinary fail-fast operation. candidate/attempt additionally retains bounded rejected intentions as attempt_revision handles, never invalid checked images. Inspect attempt/summary and attempt/query chunks, discover only compiler-admitted proposals with attempt/repair-catalog, then explicitly select attempt/repair-apply. Repair returns a new valid candidate; previous candidates and attempts remain unchanged. candidate/semantic-delta-catalog and candidate/semantic-delta return exact report chunks comparing selected declarations against their original base. Use expression/catalog for actual HIR selections, hole/open-expression to create a disjoint expression hole, then hole/query for current lexical context and hole/fill for typed construction. Surviving selections are reauthenticated after fills; no unresolved draft can complete. Runtime tests are available only if the host capability catalogue includes candidate_test; request parameters cannot enable tests or alter policy. Failures do not mutate registries. Source drift permanently invalidates the session. No source-write, filesystem-store, build, process, network or approval authority is granted."})
        }
        Operation::Client => {
            let old_names = serde_json::to_string(&method_names()).expect("compiler method list");
            let names = serde_json::to_string(
                &methods(test_enabled)
                    .iter()
                    .map(|method| method.name)
                    .collect::<Vec<_>>(),
            )
            .expect("compiler method list");
            let source = client_source(text(params, "language"))
                .replace(&old_names, &names)
                .replace(PROTOCOL_SCHEMA, DIAGNOSTIC_PROTOCOL_SCHEMA);
            json!({"schema":schema,"protocol":DIAGNOSTIC_PROTOCOL_SCHEMA,"language":text(params,"language"),"source":source})
        }
        _ => return super::prepare(method, params, image, registry, policy),
    };
    Ok((value, Mutation::None))
}
fn attempt<'a>(
    registry: &'a Registry,
    id: &str,
) -> Result<&'a Arc<ProjectCandidateAttempt>, Vec<Diagnostic>> {
    registry
        .attempts
        .get(id)
        .ok_or_else(|| failure("SPX-G243", "attempt handle is stale, discarded, or unknown"))
}
fn action_payload(
    action: Action,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &Registry,
) -> Result<(Value, Mutation), Vec<Diagnostic>> {
    match action {
        Action::ProtocolConformance
        | Action::InterfaceCatalog
        | Action::Summary
        | Action::Query
        | Action::RepairCatalog
        | Action::Delta
        | Action::DeltaCatalog => {
            let subjects = registry.detach_read(
                Operation::Candidate(super::Action::Diagnostic(action)),
                params,
            )?;
            Ok((
                read_payload(action, params, image, &subjects)?,
                Mutation::None,
            ))
        }
        Action::ExpressionHoleOpen => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            let draft = if let Some(id) = params.get("draft_revision").and_then(Value::as_str) {
                let entry = registry.draft(id)?;
                if entry.source_candidate != candidate.candidate_digest() {
                    return Err(failure(
                        "SPX-G232",
                        "draft belongs to a different candidate",
                    ));
                }
                entry.draft.with_expression_hole(
                    id,
                    text(params, "target"),
                    text(params, "expression_id"),
                    text(params, "hole_id"),
                )?
            } else {
                let draft = ProjectCandidateDraft::open(Arc::clone(candidate))?;
                draft.with_expression_hole(
                    draft.draft_digest(),
                    text(params, "target"),
                    text(params, "expression_id"),
                    text(params, "hole_id"),
                )?
            };
            retain_draft(draft, candidate.candidate_digest())
        }
        Action::Attempt => {
            let base = registry.candidate(text(params, "candidate_revision"))?;
            match ProjectCandidateAttempt::apply(
                Arc::clone(base),
                base.candidate_digest(),
                &params["intent"],
            )? {
                ProjectCandidateAttemptOutcome::Accepted(candidate) => {
                    let (handle, mutation) = retain_candidate(candidate)?;
                    Ok((
                        json!({"schema":"semaprax.image-candidate-attempt-outcome.v1","status":"accepted","candidate":handle,"attempt":null}),
                        mutation,
                    ))
                }
                ProjectCandidateAttemptOutcome::Rejected(attempt) => {
                    let summary = parse_payload(attempt.summary(attempt.attempt_digest())?)?;
                    Ok((
                        json!({"schema":"semaprax.image-candidate-attempt-outcome.v1","status":"rejected","candidate":null,"attempt":summary}),
                        Mutation::Attempt(attempt),
                    ))
                }
            }
        }
        Action::RepairApply => {
            let attempt = attempt(registry, text(params, "attempt_revision"))?;
            retain_candidate(
                attempt.repair_diagnostic(attempt.attempt_digest(), text(params, "repair_id"))?,
            )
        }
        Action::Discard => {
            let attempt = attempt(registry, text(params, "attempt_revision"))?;
            Ok((
                json!({"schema":"semaprax.image-attempt-discard.v1","attempt_revision":attempt.attempt_digest(),"discarded":true,"source_unchanged":true}),
                Mutation::DropAttempt(attempt.attempt_digest().to_owned()),
            ))
        }
    }
}
