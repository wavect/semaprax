use std::collections::BTreeMap;

use crate::diagnostic::Diagnostic;
use crate::interpreter::ResolvedTracePhase;

use super::super::super::{
    ProjectExecutionRole, ProjectRevision, MAX_MODULE_BYTES, MAX_NAME_BYTES, MAX_PATH_BYTES,
    MAX_STABLE_ID_BYTES, PROJECT_SCHEMA, PROJECT_SCHEMA_V10, PROJECT_SCHEMA_V11,
    PROJECT_SCHEMA_V12, PROJECT_SCHEMA_V2, PROJECT_SCHEMA_V3, PROJECT_SCHEMA_V4, PROJECT_SCHEMA_V5,
    PROJECT_SCHEMA_V6, PROJECT_SCHEMA_V7, PROJECT_SCHEMA_V8, PROJECT_SCHEMA_V9,
};
use super::super::{PreparedProjectExecutionOptions, MAX_PROJECT_SOURCE_TRACE_BYTES};
use super::model::{
    digest, domain_digest, keys, object, parse_status, text, text_eq, usize_value,
    verification_error, ProjectPreparedExecutionOutcome, ProjectSourceTraceEvent, NONCLAIMS,
    PAYLOAD_DIGEST_DOMAIN, PROJECT_SOURCE_TRACE_SCHEMA,
};
use super::render::{render_envelope, render_payload, RenderSubject};

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
    if envelope.len() > MAX_PROJECT_SOURCE_TRACE_BYTES {
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
        PROJECT_SCHEMA
            | PROJECT_SCHEMA_V2
            | PROJECT_SCHEMA_V3
            | PROJECT_SCHEMA_V4
            | PROJECT_SCHEMA_V5
            | PROJECT_SCHEMA_V6
            | PROJECT_SCHEMA_V7
            | PROJECT_SCHEMA_V8
            | PROJECT_SCHEMA_V9
            | PROJECT_SCHEMA_V10
            | PROJECT_SCHEMA_V11
            | PROJECT_SCHEMA_V12
    ) || project.len() > MAX_NAME_BYTES
        || module.len() > MAX_MODULE_BYTES
        || stable_id.len() > MAX_STABLE_ID_BYTES
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
    PreparedProjectExecutionOptions::new(max_steps, max_bytes, max_events)
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
    if text(event, "function_id")?.len() > MAX_STABLE_ID_BYTES
        || text(event, "expression_id")?.len() > crate::interpreter::MAX_PREPARED_INDEX_BYTES
        || text(source, "path")?.len() > MAX_PATH_BYTES
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
