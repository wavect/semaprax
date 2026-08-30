use std::collections::BTreeMap;

use crate::diagnostic::{quote_json, Diagnostic};
use crate::interpreter::{
    PreparedResolvedEvaluation, PreparedResolvedEvaluationOutcome, ResolvedTraceEvent,
};

use super::super::{
    FunctionOrigin, PreparedProjectExecutionOptions, ProjectExecutionRole, ProjectRevision,
};
use super::model::{
    domain_digest, ProjectPreparedExecutionOutcome, ProjectSourceTrace, ProjectSourceTraceEvent,
    NONCLAIMS, PAYLOAD_DIGEST_DOMAIN, PROJECT_SOURCE_TRACE_SCHEMA,
};

pub(crate) fn render(
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

pub(super) struct RenderSubject<'a> {
    pub(super) project_schema: &'a str,
    pub(super) project: &'a str,
    pub(super) project_revision: &'a str,
    pub(super) workspace_revision: &'a str,
    pub(super) project_graph_digest: &'a str,
    pub(super) role: ProjectExecutionRole,
    pub(super) module: &'a str,
    pub(super) stable_id: &'a str,
    pub(super) options: PreparedProjectExecutionOptions,
    pub(super) steps_used: usize,
    pub(super) outcome: &'a ProjectPreparedExecutionOutcome,
    pub(super) events: &'a [ProjectSourceTraceEvent],
    pub(super) dropped_events: usize,
}

pub(super) fn render_payload(subject: &RenderSubject<'_>) -> String {
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

pub(super) fn render_envelope(payload: &str, digest: &str) -> String {
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

fn role_text(role: ProjectExecutionRole) -> &'static str {
    match role {
        ProjectExecutionRole::Entry => "entry",
        ProjectExecutionRole::Test => "test",
    }
}

fn trace_error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-F110", message)
}
