//! Canonical bounded Source Trace v1 rendering and replay.

use std::collections::BTreeMap;

use sha2::{Digest as _, Sha256};

use crate::cleanup_plan::{ContractPhase, StatusCase};
use crate::conformance::{NormalizedStatus, Retryability, StatusClass};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::interpreter::{
    PreparedResolvedEvaluation, PreparedResolvedEvaluationOutcome, ResolvedTraceEvent,
    ResolvedTracePhase,
};
use crate::runtime_status;

use super::{
    FunctionOrigin, PreparedProjectExecutionOptions, ProjectExecutionRole, ProjectRevision,
};

pub const PROJECT_SOURCE_TRACE_SCHEMA: &str = "semaprax.project-source-trace.v1";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.project-source-trace.payload.v1\0";
const NONCLAIMS: [&str; 8] = [
    "in_process_reference_interpreter_only",
    "no_target_execution_or_debugger_control",
    "no_wall_time_or_schedule_determinism",
    "no_filesystem_process_backend_or_publication_authority",
    "no_source_content",
    "expression_identities_are_revision_scoped",
    "trace_is_not_provenance_approval_or_compatibility_evidence",
    "cancellation_is_one_observed_cooperative_step_boundary",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectPreparedExecutionOutcome {
    Returned(i64),
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    Cancelled { before_step: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSourceTraceEvent {
    pub index: usize,
    pub step: usize,
    pub depth: usize,
    pub phase: &'static str,
    pub function_id: String,
    pub expression_id: String,
    pub path: String,
    pub source_revision: String,
    pub source_digest: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSourceTrace {
    envelope: String,
    digest: String,
    steps_used: usize,
    recorded_events: usize,
    dropped_events: usize,
}

impl ProjectSourceTrace {
    pub fn envelope(&self) -> &str {
        &self.envelope
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub const fn steps_used(&self) -> usize {
        self.steps_used
    }

    pub const fn recorded_events(&self) -> usize {
        self.recorded_events
    }

    pub const fn dropped_events(&self) -> usize {
        self.dropped_events
    }

    pub const fn truncated(&self) -> bool {
        self.dropped_events != 0
    }
}

pub(super) fn render(
    revision: &ProjectRevision,
    role: ProjectExecutionRole,
    options: PreparedProjectExecutionOptions,
    evaluated: PreparedResolvedEvaluation,
    origins: &BTreeMap<String, FunctionOrigin>,
) -> Result<(ProjectPreparedExecutionOutcome, ProjectSourceTrace), Vec<Diagnostic>> {
    if evaluated.max_steps != options.max_steps {
        return Err(vec![trace_error(
            "prepared evaluation fuel ceiling disagrees with its request",
        )]);
    }
    let outcome = map_outcome(evaluated.outcome)?;
    for event in &evaluated.events {
        validate_evaluated_origin(event, origins)?;
    }
    let module = match role {
        ProjectExecutionRole::Entry => revision.manifest().entry(),
        ProjectExecutionRole::Test => revision.manifest().test_module(),
    };
    let stable_id = match role {
        ProjectExecutionRole::Entry => revision.entry_program().entrypoint.as_str(),
        ProjectExecutionRole::Test => revision.test_program().entrypoint.as_str(),
    };
    let total_events = evaluated.events.len();
    let mut events = Vec::new();
    let mut rendered_event_bytes = 0usize;
    for event in &evaluated.events {
        let rendered = render_evaluated_event(events.len(), event, origins)?;
        let candidate_event_bytes = rendered_event_bytes
            .checked_add(rendered.len())
            .and_then(|value| value.checked_add(usize::from(!events.is_empty())))
            .ok_or_else(|| vec![trace_error("trace event byte accounting overflowed")])?;
        let candidate_count = events.len() + 1;
        let candidate_dropped = evaluated
            .dropped_events
            .checked_add(total_events - candidate_count)
            .ok_or_else(|| vec![trace_error("trace truncation accounting overflowed")])?;
        let subject = RenderSubject {
            project_schema: revision.manifest().schema(),
            project: revision.manifest().name(),
            project_revision: revision.project_revision(),
            workspace_revision: revision.workspace_revision(),
            project_graph_digest: revision.semantic_graph_digest(),
            role,
            module,
            stable_id,
            options,
            steps_used: evaluated.steps_used,
            outcome: &outcome,
            events: &[],
            dropped_events: candidate_dropped,
        };
        if !payload_fits(
            &subject,
            candidate_count,
            candidate_event_bytes,
            options.max_trace_bytes,
        )? {
            break;
        }
        rendered_event_bytes = candidate_event_bytes;
        events.push(owned_event(events.len(), event, origins)?);
    }
    let dropped = evaluated
        .dropped_events
        .checked_add(total_events - events.len())
        .ok_or_else(|| vec![trace_error("trace truncation accounting overflowed")])?;
    let subject = RenderSubject {
        project_schema: revision.manifest().schema(),
        project: revision.manifest().name(),
        project_revision: revision.project_revision(),
        workspace_revision: revision.workspace_revision(),
        project_graph_digest: revision.semantic_graph_digest(),
        role,
        module,
        stable_id,
        options,
        steps_used: evaluated.steps_used,
        outcome: &outcome,
        events: &events,
        dropped_events: dropped,
    };
    let payload = render_payload(&subject);
    let digest = domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes());
    let envelope = render_envelope(&payload, &digest);
    if envelope.len() > options.max_trace_bytes {
        return Err(vec![trace_error(
            "trace subject header cannot fit the declared max_trace_bytes",
        )]);
    }
    Ok((
        outcome,
        ProjectSourceTrace {
            envelope,
            digest,
            steps_used: evaluated.steps_used,
            recorded_events: events.len(),
            dropped_events: dropped,
        },
    ))
}

fn validate_evaluated_origin(
    event: &ResolvedTraceEvent,
    origins: &BTreeMap<String, FunctionOrigin>,
) -> Result<(), Vec<Diagnostic>> {
    let origin = origins.get(event.function_id.as_ref()).ok_or_else(|| {
        vec![trace_error(
            "evaluated function has no authenticated source-origin fact",
        )]
    })?;
    if event.span.start > event.span.end || event.span.end > origin.source_bytes {
        return Err(vec![trace_error(
            "evaluated expression span is outside its authenticated source",
        )]);
    }
    Ok(())
}

fn owned_event(
    index: usize,
    event: &ResolvedTraceEvent,
    origins: &BTreeMap<String, FunctionOrigin>,
) -> Result<ProjectSourceTraceEvent, Vec<Diagnostic>> {
    let origin = origins
        .get(event.function_id.as_ref())
        .ok_or_else(|| vec![trace_error("evaluated source origin disappeared")])?;
    Ok(ProjectSourceTraceEvent {
        index,
        step: event.step,
        depth: event.depth,
        phase: event.phase.text(),
        function_id: event.function_id.to_string(),
        expression_id: event.expression_id.to_string(),
        path: origin.path.clone(),
        source_revision: origin.source_revision.clone(),
        source_digest: origin.source_digest.clone(),
        start: event.span.start,
        end: event.span.end,
        line: event.span.line,
        column: event.span.column,
    })
}

fn render_evaluated_event(
    index: usize,
    event: &ResolvedTraceEvent,
    origins: &BTreeMap<String, FunctionOrigin>,
) -> Result<String, Vec<Diagnostic>> {
    let origin = origins
        .get(event.function_id.as_ref())
        .ok_or_else(|| vec![trace_error("evaluated source origin disappeared")])?;
    Ok(format!(
        "{{\"index\":{},\"step\":{},\"depth\":{},\"phase\":{},\"function_id\":{},\"expression_id\":{},\"source\":{{\"path\":{},\"revision\":{},\"digest\":{}}},\"span\":{{\"start\":{},\"end\":{},\"line\":{},\"column\":{}}}}}",
        index,
        event.step,
        event.depth,
        quote_json(event.phase.text()),
        quote_json(event.function_id.as_ref()),
        quote_json(event.expression_id.as_ref()),
        quote_json(&origin.path),
        quote_json(&origin.source_revision),
        quote_json(&origin.source_digest),
        event.span.start,
        event.span.end,
        event.span.line,
        event.span.column,
    ))
}

struct RenderSubject<'a> {
    project_schema: &'a str,
    project: &'a str,
    project_revision: &'a str,
    workspace_revision: &'a str,
    project_graph_digest: &'a str,
    role: ProjectExecutionRole,
    module: &'a str,
    stable_id: &'a str,
    options: PreparedProjectExecutionOptions,
    steps_used: usize,
    outcome: &'a ProjectPreparedExecutionOutcome,
    events: &'a [ProjectSourceTraceEvent],
    dropped_events: usize,
}

fn render_payload(subject: &RenderSubject<'_>) -> String {
    let prefix = render_payload_prefix(subject);
    let suffix = render_payload_suffix(subject.events.len(), subject.dropped_events);
    let event_bytes = subject
        .events
        .iter()
        .map(render_event)
        .collect::<Vec<_>>()
        .join(",");
    let mut payload = String::with_capacity(prefix.len() + event_bytes.len() + suffix.len());
    payload.push_str(&prefix);
    payload.push_str(&event_bytes);
    payload.push_str(&suffix);
    payload
}

fn render_payload_prefix(subject: &RenderSubject<'_>) -> String {
    format!(
        "{{\"schema\":{},\"project_schema\":{},\"project\":{},\"project_revision\":{},\"workspace_revision\":{},\"project_graph_digest\":{},\"role\":{},\"module\":{},\"stable_id\":{},\"limits\":{{\"max_bytes\":{},\"max_steps\":{},\"max_events\":{}}},\"fuel\":{{\"steps_used\":{},\"max_steps\":{}}},\"outcome\":{},\"events\":[",
        quote_json(PROJECT_SOURCE_TRACE_SCHEMA),
        quote_json(subject.project_schema),
        quote_json(subject.project),
        quote_json(subject.project_revision),
        quote_json(subject.workspace_revision),
        quote_json(subject.project_graph_digest),
        quote_json(role_text(subject.role)),
        quote_json(subject.module),
        quote_json(subject.stable_id),
        subject.options.max_trace_bytes,
        subject.options.max_steps,
        subject.options.max_trace_events,
        subject.steps_used,
        subject.options.max_steps,
        render_outcome(subject.outcome),
    )
}

fn render_payload_suffix(recorded_events: usize, dropped_events: usize) -> String {
    let nonclaims = NONCLAIMS
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "],\"truncation\":{{\"recorded_events\":{recorded_events},\"dropped_events\":{dropped_events},\"truncated\":{}}},\"nonclaims\":[{nonclaims}]}}",
        dropped_events != 0,
    )
}

fn payload_fits(
    subject: &RenderSubject<'_>,
    recorded_events: usize,
    rendered_event_bytes: usize,
    max_trace_bytes: usize,
) -> Result<bool, Vec<Diagnostic>> {
    let prefix = render_payload_prefix(subject);
    let suffix = render_payload_suffix(recorded_events, subject.dropped_events);
    let payload_bytes = prefix
        .len()
        .checked_add(rendered_event_bytes)
        .and_then(|value| value.checked_add(suffix.len()))
        .ok_or_else(|| vec![trace_error("trace payload byte accounting overflowed")])?;
    rendered_envelope_len(payload_bytes)
        .map(|bytes| bytes <= max_trace_bytes)
        .ok_or_else(|| vec![trace_error("trace envelope byte accounting overflowed")])
}

fn rendered_envelope_len(payload_bytes: usize) -> Option<usize> {
    const DIGEST_PLACEHOLDER: &str =
        "0000000000000000000000000000000000000000000000000000000000000000";
    let wrapper = format!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":}}",
        quote_json(PROJECT_SOURCE_TRACE_SCHEMA),
        quote_json(DIGEST_PLACEHOLDER),
        payload_bytes,
    );
    wrapper.len().checked_add(payload_bytes)
}

fn render_envelope(payload: &str, digest: &str) -> String {
    format!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        quote_json(PROJECT_SOURCE_TRACE_SCHEMA),
        quote_json(digest),
        payload.len(),
        payload
    )
}

fn render_event(event: &ProjectSourceTraceEvent) -> String {
    format!(
        "{{\"index\":{},\"step\":{},\"depth\":{},\"phase\":{},\"function_id\":{},\"expression_id\":{},\"source\":{{\"path\":{},\"revision\":{},\"digest\":{}}},\"span\":{{\"start\":{},\"end\":{},\"line\":{},\"column\":{}}}}}",
        event.index,
        event.step,
        event.depth,
        quote_json(event.phase),
        quote_json(&event.function_id),
        quote_json(&event.expression_id),
        quote_json(&event.path),
        quote_json(&event.source_revision),
        quote_json(&event.source_digest),
        event.start,
        event.end,
        event.line,
        event.column,
    )
}

fn render_outcome(outcome: &ProjectPreparedExecutionOutcome) -> String {
    match outcome {
        ProjectPreparedExecutionOutcome::Returned(value) => format!(
            "{{\"kind\":\"returned\",\"type\":\"i64\",\"value\":{}}}",
            quote_json(&value.to_string())
        ),
        ProjectPreparedExecutionOutcome::LanguageFailure(status) => format!(
            "{{\"kind\":\"language_failure\",\"status\":{}}}",
            status.to_json()
        ),
        ProjectPreparedExecutionOutcome::FuelExhausted => {
            "{\"kind\":\"fuel_exhausted\"}".to_owned()
        }
        ProjectPreparedExecutionOutcome::CallDepthExceeded => {
            "{\"kind\":\"call_depth_exceeded\"}".to_owned()
        }
        ProjectPreparedExecutionOutcome::Cancelled { before_step } => {
            format!("{{\"kind\":\"cancelled\",\"before_step\":{before_step}}}")
        }
    }
}

fn map_outcome(
    outcome: PreparedResolvedEvaluationOutcome,
) -> Result<ProjectPreparedExecutionOutcome, Vec<Diagnostic>> {
    match outcome {
        PreparedResolvedEvaluationOutcome::ReturnedI64(value) => {
            Ok(ProjectPreparedExecutionOutcome::Returned(value))
        }
        PreparedResolvedEvaluationOutcome::LanguageFailure(status) => {
            Ok(ProjectPreparedExecutionOutcome::LanguageFailure(status))
        }
        PreparedResolvedEvaluationOutcome::FuelExhausted => {
            Ok(ProjectPreparedExecutionOutcome::FuelExhausted)
        }
        PreparedResolvedEvaluationOutcome::CallDepthExceeded => {
            Ok(ProjectPreparedExecutionOutcome::CallDepthExceeded)
        }
        PreparedResolvedEvaluationOutcome::Cancelled { before_step } => {
            Ok(ProjectPreparedExecutionOutcome::Cancelled { before_step })
        }
        PreparedResolvedEvaluationOutcome::GuardError(detail) => Err(vec![trace_error(&format!(
            "prepared evaluation reached an impossible post-validation state: {detail}"
        ))]),
    }
}

/// Validate the canonical closed wire and return authority-neutral facts used
/// by the stronger revision-bound verifier.
pub fn verify_project_source_trace(envelope: &str) -> Result<(), Diagnostic> {
    parse(envelope).map(|_| ())
}

/// Bind every source origin to one immutable revision. This verifier does not
/// grant filesystem, process, backend, publication, or mutation authority.
pub fn verify_project_source_trace_against_revision(
    revision: &ProjectRevision,
    envelope: &str,
) -> Result<(), Diagnostic> {
    let parsed = parse(envelope)?;
    if parsed.project_schema != revision.manifest().schema()
        || parsed.project != revision.manifest().name()
        || parsed.project_revision != revision.project_revision()
        || parsed.workspace_revision != revision.workspace_revision()
        || parsed.project_graph_digest != revision.semantic_graph_digest()
    {
        return Err(verification_error(
            "trace subject does not equal the supplied Project revision",
        ));
    }
    let program = match parsed.role {
        ProjectExecutionRole::Entry => revision.entry_program(),
        ProjectExecutionRole::Test => revision.test_program(),
    };
    if parsed.module != program.module || parsed.stable_id != program.entrypoint.as_str() {
        return Err(verification_error(
            "trace role/module/entry does not equal the retained closure",
        ));
    }
    let prepared =
        crate::interpreter::prepare_resolved_zero_arg_i64(program, program.entrypoint.as_str())
            .map_err(|_| verification_error("retained trace closure no longer passes admission"))?;
    let mut expression_facts = BTreeMap::new();
    for function_id in prepared.function_ids() {
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == function_id)
            .ok_or_else(|| verification_error("retained trace closure lost a function"))?;
        let source_path = revision
            .semantic
            .rename_function(function.id.as_str())
            .map(|fact| fact.path.as_str());
        let Some(source_path) = source_path else {
            continue;
        };
        let Some(source) = revision
            .sources()
            .iter()
            .find(|source| source.path() == source_path)
        else {
            return Err(verification_error(
                "retained function source is absent from the revision",
            ));
        };
        for (phase, roots) in [
            (
                ResolvedTracePhase::Requires.text(),
                function.requires.as_slice(),
            ),
            (
                ResolvedTracePhase::Body.text(),
                std::slice::from_ref(&function.body),
            ),
            (
                ResolvedTracePhase::Ensures.text(),
                function.ensures.as_slice(),
            ),
        ] {
            let mut expressions = roots.iter().collect::<Vec<_>>();
            while let Some(expression) = expressions.pop() {
                if expression_facts
                    .insert(
                        (function.id.as_str(), expression.id.as_str()),
                        (
                            source.path(),
                            source.source_revision(),
                            source.source_digest(),
                            expression.span,
                            phase,
                        ),
                    )
                    .is_some()
                {
                    return Err(verification_error(
                        "retained trace closure contains duplicate expression identity",
                    ));
                }
                expressions.extend(crate::interpreter::trace_child_expressions(expression));
            }
        }
    }
    for event in &parsed.events {
        let Some((path, revision_fact, digest, span, phase)) =
            expression_facts.get(&(event.function_id.as_str(), event.expression_id.as_str()))
        else {
            return Err(verification_error(
                "trace event does not name an expression in the exact retained closure",
            ));
        };
        if *path != event.path
            || *revision_fact != event.source_revision
            || *digest != event.source_digest
            || span.start != event.start
            || span.end != event.end
            || span.line != event.line
            || span.column != event.column
            || *phase != event.phase
        {
            return Err(verification_error(
                "trace event source origin or structural phase disagrees with retained HIR",
            ));
        }
    }
    Ok(())
}

struct ParsedTrace {
    project_schema: String,
    project: String,
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    role: ProjectExecutionRole,
    module: String,
    stable_id: String,
    events: Vec<ProjectSourceTraceEvent>,
}

fn parse(envelope: &str) -> Result<ParsedTrace, Diagnostic> {
    if envelope.len() > super::MAX_PROJECT_SOURCE_TRACE_BYTES {
        return Err(verification_error("trace exceeds the absolute byte bound"));
    }
    let root: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|error| verification_error(&format!("trace is not valid JSON: {error}")))?;
    let root = object(&root, "trace wrapper")?;
    keys(
        root,
        &["bytes", "digest", "payload", "schema"],
        "trace wrapper",
    )?;
    text_eq(root, "schema", PROJECT_SOURCE_TRACE_SCHEMA)?;
    let payload = object(&root["payload"], "payload")?;
    keys(
        payload,
        &[
            "events",
            "fuel",
            "limits",
            "module",
            "nonclaims",
            "outcome",
            "project",
            "project_graph_digest",
            "project_revision",
            "project_schema",
            "role",
            "schema",
            "stable_id",
            "truncation",
            "workspace_revision",
        ],
        "payload",
    )?;
    text_eq(payload, "schema", PROJECT_SOURCE_TRACE_SCHEMA)?;
    let project_schema = text(payload, "project_schema")?.to_owned();
    let project = text(payload, "project")?.to_owned();
    let project_revision = digest(payload, "project_revision")?.to_owned();
    let workspace_revision = digest(payload, "workspace_revision")?.to_owned();
    let project_graph_digest = digest(payload, "project_graph_digest")?.to_owned();
    let role = match text(payload, "role")? {
        "entry" => ProjectExecutionRole::Entry,
        "test" => ProjectExecutionRole::Test,
        _ => {
            return Err(verification_error(
                "trace role is outside the closed vocabulary",
            ))
        }
    };
    let module = text(payload, "module")?.to_owned();
    let stable_id = text(payload, "stable_id")?.to_owned();
    if !matches!(
        project_schema.as_str(),
        super::super::PROJECT_SCHEMA
            | super::super::PROJECT_SCHEMA_V2
            | super::super::PROJECT_SCHEMA_V3
            | super::super::PROJECT_SCHEMA_V4
            | super::super::PROJECT_SCHEMA_V5
            | super::super::PROJECT_SCHEMA_V6
            | super::super::PROJECT_SCHEMA_V7
            | super::super::PROJECT_SCHEMA_V8
            | super::super::PROJECT_SCHEMA_V9
            | super::super::PROJECT_SCHEMA_V10
    ) || project.len() > super::super::MAX_NAME_BYTES
        || module.len() > super::super::MAX_MODULE_BYTES
        || stable_id.len() > super::super::MAX_STABLE_ID_BYTES
    {
        return Err(verification_error(
            "trace Project schema or subject identity is outside the closed bounds",
        ));
    }
    let limits = object(&payload["limits"], "limits")?;
    keys(limits, &["max_bytes", "max_events", "max_steps"], "limits")?;
    let max_bytes = usize_value(limits, "max_bytes")?;
    let max_events = usize_value(limits, "max_events")?;
    let max_steps = usize_value(limits, "max_steps")?;
    super::PreparedProjectExecutionOptions::new(max_steps, max_bytes, max_events)
        .map_err(|_| verification_error("trace limits are outside the public bounds"))?;
    if envelope.len() > max_bytes {
        return Err(verification_error("trace exceeds its declared max_bytes"));
    }
    let fuel = object(&payload["fuel"], "fuel")?;
    keys(fuel, &["max_steps", "steps_used"], "fuel")?;
    let steps_used = usize_value(fuel, "steps_used")?;
    if usize_value(fuel, "max_steps")? != max_steps || steps_used > max_steps {
        return Err(verification_error(
            "trace fuel facts violate their declared bounds",
        ));
    }
    let outcome = parse_outcome(&payload["outcome"], steps_used, max_steps)?;
    let array = payload["events"]
        .as_array()
        .ok_or_else(|| verification_error("events must be an array"))?;
    if array.len() > max_events {
        return Err(verification_error("events exceed max_events"));
    }
    let mut events = Vec::with_capacity(array.len());
    let mut previous_step = 0usize;
    for (index, value) in array.iter().enumerate() {
        let event = parse_event(value, index)?;
        if event.step == 0 || event.step > steps_used || event.step <= previous_step {
            return Err(verification_error(
                "event steps must be strictly increasing within used fuel",
            ));
        }
        previous_step = event.step;
        events.push(event);
    }
    let truncation = object(&payload["truncation"], "truncation")?;
    keys(
        truncation,
        &["dropped_events", "recorded_events", "truncated"],
        "truncation",
    )?;
    let dropped_events = usize_value(truncation, "dropped_events")?;
    if usize_value(truncation, "recorded_events")? != events.len()
        || truncation["truncated"].as_bool() != Some(dropped_events != 0)
    {
        return Err(verification_error(
            "trace truncation facts are inconsistent",
        ));
    }
    if events
        .len()
        .checked_add(dropped_events)
        .is_none_or(|observed| observed > steps_used)
    {
        return Err(verification_error(
            "trace event accounting exceeds charged evaluator steps",
        ));
    }
    let nonclaims = payload["nonclaims"]
        .as_array()
        .ok_or_else(|| verification_error("nonclaims must be an array"))?;
    if nonclaims.len() != NONCLAIMS.len()
        || nonclaims
            .iter()
            .zip(NONCLAIMS)
            .any(|(value, expected)| value.as_str() != Some(expected))
    {
        return Err(verification_error(
            "trace nonclaims are not the fixed ordered list",
        ));
    }
    let subject = RenderSubject {
        project_schema: &project_schema,
        project: &project,
        project_revision: &project_revision,
        workspace_revision: &workspace_revision,
        project_graph_digest: &project_graph_digest,
        role,
        module: &module,
        stable_id: &stable_id,
        options: PreparedProjectExecutionOptions {
            max_steps,
            max_trace_bytes: max_bytes,
            max_trace_events: max_events,
        },
        steps_used,
        outcome: &outcome,
        events: &events,
        dropped_events,
    };
    let canonical_payload = render_payload(&subject);
    if root["bytes"].as_u64() != Some(canonical_payload.len() as u64) {
        return Err(verification_error("trace payload byte count is incorrect"));
    }
    let expected_digest = domain_digest(PAYLOAD_DIGEST_DOMAIN, canonical_payload.as_bytes());
    if root["digest"].as_str() != Some(expected_digest.as_str()) {
        return Err(verification_error("trace payload digest is incorrect"));
    }
    let canonical = render_envelope(&canonical_payload, &expected_digest);
    if canonical != envelope {
        return Err(verification_error(
            "trace is not the exact canonical reconstruction",
        ));
    }
    Ok(ParsedTrace {
        project_schema,
        project,
        project_revision,
        workspace_revision,
        project_graph_digest,
        role,
        module,
        stable_id,
        events,
    })
}

fn parse_event(
    value: &serde_json::Value,
    index: usize,
) -> Result<ProjectSourceTraceEvent, Diagnostic> {
    let event = object(value, "event")?;
    keys(
        event,
        &[
            "depth",
            "expression_id",
            "function_id",
            "index",
            "phase",
            "source",
            "span",
            "step",
        ],
        "event",
    )?;
    if usize_value(event, "index")? != index {
        return Err(verification_error("event index is not canonical"));
    }
    let phase = match text(event, "phase")? {
        "requires" => ResolvedTracePhase::Requires.text(),
        "body" => ResolvedTracePhase::Body.text(),
        "ensures" => ResolvedTracePhase::Ensures.text(),
        _ => {
            return Err(verification_error(
                "event phase is outside the closed vocabulary",
            ))
        }
    };
    let depth = usize_value(event, "depth")?;
    if depth >= crate::interpreter::MAX_CALL_DEPTH {
        return Err(verification_error(
            "event depth exceeds the interpreter ceiling",
        ));
    }
    let source = object(&event["source"], "event source")?;
    keys(source, &["digest", "path", "revision"], "event source")?;
    let span = object(&event["span"], "event span")?;
    keys(span, &["column", "end", "line", "start"], "event span")?;
    let start = usize_value(span, "start")?;
    let end = usize_value(span, "end")?;
    if start > end {
        return Err(verification_error("event span start exceeds end"));
    }
    if text(event, "function_id")?.len() > super::super::MAX_STABLE_ID_BYTES
        || text(event, "expression_id")?.len() > crate::interpreter::MAX_PREPARED_INDEX_BYTES
        || text(source, "path")?.len() > super::super::MAX_PATH_BYTES
    {
        return Err(verification_error(
            "event identity or source path exceeds its closed bound",
        ));
    }
    Ok(ProjectSourceTraceEvent {
        index,
        step: usize_value(event, "step")?,
        depth,
        phase,
        function_id: text(event, "function_id")?.to_owned(),
        expression_id: text(event, "expression_id")?.to_owned(),
        path: text(source, "path")?.to_owned(),
        source_revision: digest(source, "revision")?.to_owned(),
        source_digest: digest(source, "digest")?.to_owned(),
        start,
        end,
        line: usize_value(span, "line")?,
        column: usize_value(span, "column")?,
    })
}

fn parse_outcome(
    value: &serde_json::Value,
    steps_used: usize,
    max_steps: usize,
) -> Result<ProjectPreparedExecutionOutcome, Diagnostic> {
    let outcome = object(value, "outcome")?;
    match text(outcome, "kind")? {
        "returned" => {
            keys(outcome, &["kind", "type", "value"], "returned outcome")?;
            text_eq(outcome, "type", "i64")?;
            let text = text(outcome, "value")?;
            let value = text
                .parse::<i64>()
                .map_err(|_| verification_error("returned i64 is not canonical"))?;
            if value.to_string() != text {
                return Err(verification_error("returned i64 is not canonical"));
            }
            Ok(ProjectPreparedExecutionOutcome::Returned(value))
        }
        "language_failure" => {
            keys(outcome, &["kind", "status"], "language failure outcome")?;
            Ok(ProjectPreparedExecutionOutcome::LanguageFailure(
                parse_status(&outcome["status"])?,
            ))
        }
        "fuel_exhausted" => {
            keys(outcome, &["kind"], "fuel outcome")?;
            if steps_used != max_steps {
                return Err(verification_error(
                    "fuel exhaustion does not consume max_steps",
                ));
            }
            Ok(ProjectPreparedExecutionOutcome::FuelExhausted)
        }
        "call_depth_exceeded" => {
            keys(outcome, &["kind"], "depth outcome")?;
            Ok(ProjectPreparedExecutionOutcome::CallDepthExceeded)
        }
        "cancelled" => {
            keys(outcome, &["before_step", "kind"], "cancelled outcome")?;
            let before_step = usize_value(outcome, "before_step")?;
            if steps_used >= max_steps || before_step != steps_used.saturating_add(1) {
                return Err(verification_error(
                    "cancelled boundary must immediately follow used fuel before exhaustion",
                ));
            }
            Ok(ProjectPreparedExecutionOutcome::Cancelled { before_step })
        }
        _ => Err(verification_error(
            "outcome is outside the closed vocabulary",
        )),
    }
}

pub(super) fn parse_status(value: &serde_json::Value) -> Result<NormalizedStatus, Diagnostic> {
    let status = object(value, "status")?;
    keys(
        status,
        &["class", "code", "domain_id", "retryable", "schema"],
        "status",
    )?;
    text_eq(
        status,
        "schema",
        crate::conformance::NORMALIZED_STATUS_SCHEMA_V1,
    )?;
    if status["retryable"].as_bool() != Some(false) {
        return Err(verification_error("language status must be non-retryable"));
    }
    let code = status["code"]
        .as_u64()
        .ok_or_else(|| verification_error("status code must be unsigned"))?;
    match (text(status, "domain_id")?, text(status, "class")?) {
        (crate::conformance::ARITHMETIC_STATUS_DOMAIN_V1, "arithmetic") => {
            let case = match code {
                1 => StatusCase::AddOverflow,
                2 => StatusCase::SubOverflow,
                3 => StatusCase::MulOverflow,
                4 => StatusCase::DivisionByZero,
                5 => StatusCase::DivisionOverflow,
                6 => StatusCase::RemainderByZero,
                7 => StatusCase::RemainderOverflow,
                8 => StatusCase::NegationOverflow,
                _ => return Err(verification_error("unknown arithmetic status code")),
            };
            Ok(runtime_status::normalize_arithmetic(case))
        }
        (crate::conformance::CONTRACT_STATUS_DOMAIN_V1, "contract") => match code {
            1 => Ok(runtime_status::normalize_contract(ContractPhase::Requires)),
            2 => Ok(runtime_status::normalize_contract(ContractPhase::Ensures)),
            _ => Err(verification_error("unknown contract status code")),
        },
        (crate::byte_ops::RANGE_STATUS_DOMAIN, "adapter") => {
            let code = match code {
                1 => crate::byte_ops::RANGE_START_AFTER_END_CODE,
                2 => crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
                _ => return Err(verification_error("unknown byte-range status code")),
            };
            NormalizedStatus::try_new(
                crate::byte_ops::RANGE_STATUS_DOMAIN,
                code,
                StatusClass::Adapter,
                Retryability::Known(false),
            )
            .map_err(|_| verification_error("byte-range status is not canonical"))
        }
        _ => Err(verification_error(
            "status domain/class is not compiler-owned",
        )),
    }
}

fn object<'a>(
    value: &'a serde_json::Value,
    section: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, Diagnostic> {
    value
        .as_object()
        .ok_or_else(|| verification_error(&format!("{section} must be an object")))
}

fn keys(
    object: &serde_json::Map<String, serde_json::Value>,
    expected: &[&str],
    section: &str,
) -> Result<(), Diagnostic> {
    if object.keys().map(String::as_str).collect::<Vec<_>>() != expected {
        return Err(verification_error(&format!(
            "{section} keys are not the exact closed set"
        )));
    }
    Ok(())
}

fn text<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, Diagnostic> {
    object[key]
        .as_str()
        .filter(|value| !value.is_empty() && !value.contains('\0'))
        .ok_or_else(|| verification_error(&format!("{key} must be nonempty NUL-free text")))
}

fn text_eq(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    expected: &str,
) -> Result<(), Diagnostic> {
    if text(object, key)? != expected {
        return Err(verification_error(&format!(
            "{key} must equal `{expected}`"
        )));
    }
    Ok(())
}

fn usize_value(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<usize, Diagnostic> {
    object[key]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| verification_error(&format!("{key} must fit usize")))
}

fn digest<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, Diagnostic> {
    let value = text(object, key)?;
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(verification_error(&format!(
            "{key} is not a canonical digest"
        )));
    }
    Ok(value)
}

fn role_text(role: ProjectExecutionRole) -> &'static str {
    match role {
        ProjectExecutionRole::Entry => "entry",
        ProjectExecutionRole::Test => "test",
    }
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn trace_error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-F110", message)
}

fn verification_error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-F110", message)
}
