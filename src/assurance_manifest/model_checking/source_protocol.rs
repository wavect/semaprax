//! LAW-10: finite safety over a source-executed pure scalar protocol dispatcher.
//!
//! The protocol declares identities and expected transitions. The table is
//! derived by executing checked HIR for every finite state/event input, not
//! accepted as a caller-authored edge list. This does not prove that any
//! external payment provider executes a command exactly once.

use std::collections::BTreeSet;

use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

use crate::ast::{SessionProtocolDeclaration, SessionProtocolNext};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExprKind, ResolvedProgram, ResolvedType};
use crate::interpreter::{PublicApiEvaluationOutcome, PublicApiValue};
use crate::project::{ProjectRevision, PublicApiArgument};

use super::engine::{check_safety, Bounds, SafetyOutcome, TransitionSystem};

pub const SOURCE_PROTOCOL_SCHEMA: &str = "semaprax.source-protocol-safety.v1";
const MAX_STATES: usize = 16;
const MAX_EVENTS: usize = 32;
const MAX_PAIR_EVALUATIONS: usize = MAX_STATES * MAX_EVENTS;
const MAX_EVALUATION_STEPS: usize = 4096;

fn refusal(code: &'static str, message: &'static str) -> Diagnostic {
    Diagnostic::io(code, message)
}

fn framed(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value.as_bytes());
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionCoverage {
    pub from: String,
    pub label: String,
    pub next: String,
    pub via: String,
    pub source_path: String,
    pub source_line: usize,
    pub state_code: i64,
    pub event_code: i64,
    pub return_code: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolTraceStep {
    pub from: String,
    pub label: String,
    pub next: String,
    pub via: String,
    pub source_path: String,
    pub source_line: usize,
    pub charge_command: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolSafetyOutcome {
    ModelChecked,
    ConcreteCounterexample { trace: Vec<ProtocolTraceStep> },
    AbstractCounterexample { trace: Vec<ProtocolTraceStep> },
    BoundsExhausted,
    DeadState,
    EmptyStateSpace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceProtocolReport {
    pub schema: &'static str,
    pub project_revision: String,
    pub protocol_id: String,
    pub protocol_source_path: String,
    pub protocol_source_digest: String,
    pub dispatcher_id: String,
    pub caller_id: String,
    pub state_domain: Vec<String>,
    pub event_domain: Vec<String>,
    pub initial_state: String,
    pub success_state: String,
    pub charge_label: String,
    pub fairness: &'static str,
    pub coverage: Vec<TransitionCoverage>,
    pub bounds: Bounds,
    pub explored_states: usize,
    pub explored_transitions: usize,
    pub evidence_digest: String,
    pub outcome: ProtocolSafetyOutcome,
}

impl SourceProtocolReport {
    pub fn model_checked(&self) -> bool {
        matches!(self.outcome, ProtocolSafetyOutcome::ModelChecked)
    }

    pub fn to_json(&self) -> String {
        let coverage = self
            .coverage
            .iter()
            .map(|row| {
                json!({
                    "from": row.from,
                    "label": row.label,
                    "next": row.next,
                    "via": row.via,
                    "source_path": row.source_path,
                    "source_line": row.source_line,
                    "state_code": row.state_code,
                    "event_code": row.event_code,
                    "return_code": row.return_code,
                })
            })
            .collect::<Vec<_>>();
        let (status, replay, trace) = match &self.outcome {
            ProtocolSafetyOutcome::ModelChecked => ("model_checked", "not_applicable", Vec::new()),
            ProtocolSafetyOutcome::ConcreteCounterexample { trace } => {
                ("violated", "concrete_source_replay", trace.clone())
            }
            ProtocolSafetyOutcome::AbstractCounterexample { trace } => {
                ("violated", "abstract_only", trace.clone())
            }
            ProtocolSafetyOutcome::BoundsExhausted => {
                ("bounds_exhausted", "not_applicable", Vec::new())
            }
            ProtocolSafetyOutcome::DeadState => ("dead_state", "not_applicable", Vec::new()),
            ProtocolSafetyOutcome::EmptyStateSpace => {
                ("empty_state_space", "not_applicable", Vec::new())
            }
        };
        let trace = trace
            .iter()
            .map(|row| {
                json!({
                    "from": row.from,
                    "label": row.label,
                    "next": row.next,
                    "via": row.via,
                    "source_path": row.source_path,
                    "source_line": row.source_line,
                    "charge_command": row.charge_command,
                })
            })
            .collect::<Vec<_>>();
        let value = json!({
            "schema": self.schema,
            "status": status,
            "trace_replay": replay,
            "authority": "none",
            "claim": "finite_pure_dispatcher_safety_only_no_external_exactly_once",
            "project_revision": self.project_revision,
            "protocol_id": self.protocol_id,
            "protocol_source_path": self.protocol_source_path,
            "protocol_source_digest": self.protocol_source_digest,
            "dispatcher_id": self.dispatcher_id,
            "caller_id": self.caller_id,
            "state_domain": self.state_domain,
            "event_domain": self.event_domain,
            "initial_state": self.initial_state,
            "success_state": self.success_state,
            "charge_label": self.charge_label,
            "fairness": self.fairness,
            "coverage": coverage,
            "bounds": {"max_states":self.bounds.max_states,"max_depth":self.bounds.max_depth,"max_transitions":self.bounds.max_transitions},
            "explored_states": self.explored_states,
            "explored_transitions": self.explored_transitions,
            "evidence_digest": self.evidence_digest,
            "trace": trace,
        });
        format!(
            "{}\n",
            serde_json::to_string(&value).expect("closed protocol report")
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct State {
    phase: usize,
    success_seen: bool,
    charge_after_success: bool,
}

struct SourceModel {
    table: Vec<Vec<Option<(usize, bool)>>>,
    success: usize,
    initial: usize,
    charge_events: BTreeSet<usize>,
    terminals: BTreeSet<usize>,
}

impl TransitionSystem for SourceModel {
    type State = State;
    type Event = usize;

    fn initial_states(&self) -> Vec<Self::State> {
        vec![State {
            phase: self.initial,
            success_seen: self.initial == self.success,
            charge_after_success: false,
        }]
    }
    fn enabled_events(&self, state: &Self::State) -> Vec<Self::Event> {
        self.table[state.phase]
            .iter()
            .enumerate()
            .filter_map(|(event, next)| next.map(|_| event))
            .collect()
    }
    fn apply(&self, state: &Self::State, event: &Self::Event) -> Option<Self::State> {
        let (phase, _charge_bit) = self.table[state.phase][*event]?;
        Some(State {
            phase,
            success_seen: state.success_seen || phase == self.success,
            charge_after_success: state.charge_after_success
                || (state.success_seen && self.charge_events.contains(event)),
        })
    }
    fn is_terminal(&self, state: &Self::State) -> bool {
        self.terminals.contains(&state.phase)
    }
    fn safety_invariant(&self, state: &Self::State) -> Result<(), String> {
        if state.charge_after_success {
            Err("no_charge_command_after_success".into())
        } else {
            Ok(())
        }
    }
}

fn selected_protocol(
    revision: &ProjectRevision,
    protocol_id: &str,
) -> Result<(SessionProtocolDeclaration, String, String), Diagnostic> {
    let mut selected = None;
    for source in revision.sources() {
        if !source.source().contains("session protocol") {
            continue;
        }
        let program = crate::parse(source.source(), source.path()).map_err(|_| {
            refusal(
                "SPX-LP400",
                "retained Project protocol source did not reparse",
            )
        })?;
        if crate::session_protocol::source::check(&program)
            .iter()
            .any(|error| error.severity.is_error())
        {
            return Err(refusal(
                "SPX-LP400",
                "retained Project protocol source failed checking",
            ));
        }
        for declaration in &program.session_protocols {
            if declaration.stable_id == protocol_id {
                if selected.is_some() {
                    return Err(refusal("SPX-LP400", "duplicate retained protocol identity"));
                }
                selected = Some((
                    declaration.clone(),
                    source.path().to_owned(),
                    source.source_digest().to_owned(),
                ));
            }
        }
    }
    selected.ok_or_else(|| refusal("SPX-LP400", "selected protocol is absent"))
}

fn selected_dispatcher<'a>(
    revision: &'a ProjectRevision,
    dispatcher_id: &str,
) -> Result<&'a ResolvedProgram, Diagnostic> {
    let program = [
        revision.entry_program(),
        revision.public_api_program(),
        revision.test_program(),
    ]
    .into_iter()
    .find(|program| {
        program
            .functions
            .iter()
            .any(|function| function.id.as_str() == dispatcher_id)
    })
    .ok_or_else(|| {
        refusal(
            "SPX-LP401",
            "protocol dispatcher is absent from checked Project HIR",
        )
    })?;
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == dispatcher_id)
        .unwrap();
    if function.params.len() != 2
        || function
            .params
            .iter()
            .any(|parameter| parameter.ty != ResolvedType::I64)
        || function.return_type != ResolvedType::I64
        || !function.effects.is_empty()
        || !function.requires.is_empty()
        || !function.ensures.is_empty()
        || function.yields.is_some()
    {
        return Err(refusal(
            "SPX-LP401",
            "protocol dispatcher must be a pure (i64,i64)->i64 checked function",
        ));
    }
    Ok(program)
}

/// The selected public route must pass its exact two inputs to the dispatcher.
/// No other checked function may call or take a reference to that dispatcher.
/// This is an explicit narrow caller boundary, not a whole-program assertion
/// that effectful payment code or external providers follow this protocol.
fn require_caller_route(
    revision: &ProjectRevision,
    dispatcher_id: &str,
    caller_id: &str,
) -> Result<(), Diagnostic> {
    if !revision
        .manifest()
        .web_exports()
        .iter()
        .any(|selected| selected == caller_id)
    {
        return Err(refusal(
            "SPX-LP408",
            "selected protocol caller is not an explicit Project web export",
        ));
    }
    let mut found = false;
    for program in [
        revision.entry_program(),
        revision.public_api_program(),
        revision.test_program(),
    ] {
        for template in &program.function_templates {
            let mut calls_dispatcher = false;
            for expression in template
                .requires
                .iter()
                .chain(std::iter::once(&template.body))
                .chain(&template.ensures)
            {
                crate::hir::visit_resolved_calls(expression, &mut |callee, _, _| {
                    calls_dispatcher |= callee.as_str() == dispatcher_id;
                });
            }
            if calls_dispatcher {
                return Err(refusal(
                    "SPX-LP408",
                    "checked generic template calls or references the selected dispatcher outside the public route",
                ));
            }
        }
        for instance in &program.function_instances {
            let mut calls_dispatcher = false;
            for expression in instance
                .function
                .requires
                .iter()
                .chain(std::iter::once(&instance.function.body))
                .chain(&instance.function.ensures)
            {
                crate::hir::visit_resolved_calls(expression, &mut |callee, _, _| {
                    calls_dispatcher |= callee.as_str() == dispatcher_id;
                });
            }
            if calls_dispatcher {
                return Err(refusal(
                    "SPX-LP408",
                    "checked generic instance calls or references the selected dispatcher outside the public route",
                ));
            }
        }
        for function in &program.functions {
            if function.id.as_str() == caller_id {
                found = true;
            }
            let mut calls_dispatcher = false;
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                crate::hir::visit_resolved_calls(expression, &mut |callee, _, _| {
                    calls_dispatcher |= callee.as_str() == dispatcher_id;
                });
            }
            if calls_dispatcher && function.id.as_str() != caller_id {
                return Err(refusal(
                    "SPX-LP408",
                    "checked dispatcher has a call or reference outside the selected public route",
                ));
            }
        }
    }
    let caller = revision
        .public_api_program()
        .functions
        .iter()
        .find(|function| function.id.as_str() == caller_id)
        .ok_or_else(|| {
            refusal(
                "SPX-LP408",
                "selected caller is not a checked public API route",
            )
        })?;
    if !found
        || caller.params.len() != 2
        || caller
            .params
            .iter()
            .any(|param| param.ty != ResolvedType::I64)
        || caller.return_type != ResolvedType::I64
        || !caller.effects.is_empty()
        || !caller.requires.is_empty()
        || !caller.ensures.is_empty()
        || caller.yields.is_some()
    {
        return Err(refusal(
            "SPX-LP408",
            "selected caller must be a pure public (i64,i64)->i64 route",
        ));
    }
    let ResolvedExprKind::Block { statements, tail } = &caller.body.kind else {
        return Err(refusal(
            "SPX-LP408",
            "selected caller body is not a direct dispatcher call",
        ));
    };
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        args,
    } = &tail.kind
    else {
        return Err(refusal(
            "SPX-LP408",
            "selected caller body is not a direct dispatcher call",
        ));
    };
    if !statements.is_empty()
        || callee.as_str() != dispatcher_id
        || !type_arguments.is_empty()
        || instance.is_some()
        || args.len() != 2
        || args.iter().zip(&caller.params).any(|(arg, param)| {
            !matches!(&arg.kind, ResolvedExprKind::Place(place) if place.root == param.id && place.projections.is_empty())
        })
    {
        return Err(refusal(
            "SPX-LP408",
            "selected caller must forward its exact parameters once to the dispatcher",
        ));
    }
    Ok(())
}

fn execute(
    program: &ResolvedProgram,
    dispatcher_id: &str,
    state: usize,
    event: usize,
) -> Result<i64, Diagnostic> {
    let state = i64::try_from(state).map_err(|_| refusal("SPX-LP402", "state code overflow"))?;
    let event = i64::try_from(event).map_err(|_| refusal("SPX-LP402", "event code overflow"))?;
    let evaluated = crate::interpreter::evaluate_resolved_public_api(
        program,
        dispatcher_id,
        &[PublicApiArgument::I64(state), PublicApiArgument::I64(event)],
        MAX_EVALUATION_STEPS,
    )
    .map_err(|_| {
        refusal(
            "SPX-LP402",
            "protocol dispatcher is outside pure interpreter profile",
        )
    })?;
    if !evaluated.cleanup_events.is_empty() {
        return Err(refusal(
            "SPX-LP402",
            "protocol dispatcher emitted cleanup effects",
        ));
    }
    match evaluated.outcome {
        PublicApiEvaluationOutcome::Returned(PublicApiValue::I64(value)) => Ok(value),
        _ => Err(refusal(
            "SPX-LP402",
            "protocol dispatcher did not return an i64 within fuel",
        )),
    }
}

fn digest(
    revision: &ProjectRevision,
    source_path: &str,
    source_digest: &str,
    protocol: &SessionProtocolDeclaration,
    dispatcher_id: &str,
    caller_id: &str,
    states: &[String],
    events: &[String],
    table: &[Vec<Option<(usize, bool)>>],
    success: &str,
    charge: &str,
    bounds: Bounds,
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.source-protocol-safety.v1\0");
    for text in [
        revision.project_revision(),
        source_path,
        source_digest,
        &protocol.stable_id,
        dispatcher_id,
        caller_id,
        success,
        charge,
        "fairness:none",
    ] {
        framed(&mut hash, text);
    }
    for text in states.iter().chain(events) {
        framed(&mut hash, text);
    }
    for row in table {
        for cell in row {
            framed(&mut hash, &format!("{cell:?}"));
        }
    }
    for number in [bounds.max_states, bounds.max_depth, bounds.max_transitions] {
        hash.update((number as u64).to_be_bytes());
    }
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

/// Build a finite source-derived table, demand exact protocol coverage, then
/// run the existing explicit-state checker. No caller-authored table is read.
pub fn check_project_source_protocol(
    revision: &ProjectRevision,
    protocol_id: &str,
    dispatcher_id: &str,
    caller_id: &str,
    success_state: &str,
    charge_label: &str,
    bounds: Bounds,
) -> Result<SourceProtocolReport, Diagnostic> {
    let (protocol, source_path, source_digest) = selected_protocol(revision, protocol_id)?;
    let states = protocol
        .states
        .iter()
        .map(|state| state.name.clone())
        .collect::<Vec<_>>();
    let events = protocol
        .transitions
        .iter()
        .map(|transition| transition.label.name.clone())
        .collect::<Vec<_>>();
    if states.is_empty()
        || states.len() > MAX_STATES
        || events.is_empty()
        || events.len() > MAX_EVENTS
        || states.len().saturating_mul(events.len()) > MAX_PAIR_EVALUATIONS
        || bounds.max_states == 0
        || bounds.max_depth == 0
        || bounds.max_transitions == 0
        || bounds.max_states > 4096
        || bounds.max_transitions > 8192
        || bounds.max_depth > 4096
    {
        return Err(refusal(
            "SPX-LP403",
            "protocol domain or exploration bounds are outside the finite profile",
        ));
    }
    let state_index = |name: &str| states.iter().position(|state| state == name);
    let initial = state_index(&protocol.initial.name)
        .ok_or_else(|| refusal("SPX-LP400", "protocol initial state is absent"))?;
    let success = state_index(success_state)
        .ok_or_else(|| refusal("SPX-LP403", "selected success state is absent"))?;
    let charge_events = events
        .iter()
        .enumerate()
        .filter_map(|(index, label)| (label == charge_label).then_some(index))
        .collect::<BTreeSet<_>>();
    if charge_events.is_empty() {
        return Err(refusal(
            "SPX-LP403",
            "selected charge command label is absent",
        ));
    }
    let program = selected_dispatcher(revision, dispatcher_id)?;
    require_caller_route(revision, dispatcher_id, caller_id)?;
    let terminals = protocol
        .terminals
        .iter()
        .filter_map(|terminal| state_index(&terminal.state.name))
        .collect::<BTreeSet<_>>();
    let mut expected = vec![vec![None; events.len()]; states.len()];
    let mut coverage = Vec::with_capacity(events.len());
    for (event, transition) in protocol.transitions.iter().enumerate() {
        if transition.via.as_ref().map(|via| via.name.as_str()) != Some(dispatcher_id) {
            return Err(refusal("SPX-LP404", "every finite protocol transition requires the selected checked dispatcher via identity"));
        }
        let SessionProtocolNext::Then(next) = &transition.next else {
            return Err(refusal(
                "SPX-LP404",
                "finite source protocol does not admit choice transitions",
            ));
        };
        let from = state_index(&transition.from.name)
            .ok_or_else(|| refusal("SPX-LP400", "protocol transition source state is absent"))?;
        let next = state_index(&next.name)
            .ok_or_else(|| refusal("SPX-LP400", "protocol transition target state is absent"))?;
        expected[from][event] = Some((next, charge_events.contains(&event)));
        coverage.push(TransitionCoverage {
            from: transition.from.name.clone(),
            label: transition.label.name.clone(),
            next: states[next].clone(),
            via: dispatcher_id.to_owned(),
            source_path: source_path.clone(),
            source_line: transition.span.line,
            state_code: from as i64,
            event_code: event as i64,
            return_code: (next as i64) * 2 + i64::from(charge_events.contains(&event)),
        });
    }
    let mut table = vec![vec![None; events.len()]; states.len()];
    for state in 0..states.len() {
        for event in 0..events.len() {
            let value = execute(program, dispatcher_id, state, event)?;
            let observed = if value == -1 {
                None
            } else {
                let max = (states.len() as i64) * 2;
                if value < 0 || value >= max {
                    return Err(refusal(
                        "SPX-LP405",
                        "dispatcher returned a state/command outside the declared domain",
                    ));
                }
                Some(((value / 2) as usize, value % 2 == 1))
            };
            if observed != expected[state][event] {
                return Err(refusal(
                    "SPX-LP406",
                    "source dispatcher transition differs from the declared protocol coverage",
                ));
            }
            table[state][event] = observed;
        }
    }
    let evidence_digest = digest(
        revision,
        &source_path,
        &source_digest,
        &protocol,
        dispatcher_id,
        caller_id,
        &states,
        &events,
        &table,
        success_state,
        charge_label,
        bounds,
    );
    let model = SourceModel {
        table,
        success,
        initial,
        charge_events,
        terminals,
    };
    let explored = check_safety(&model, bounds);
    let outcome = match explored.outcome {
        SafetyOutcome::Verified => ProtocolSafetyOutcome::ModelChecked,
        SafetyOutcome::Violated { trace, .. } => {
            let trace_rows = trace
                .iter()
                .map(|step| {
                    let row = &coverage[step.event];
                    ProtocolTraceStep {
                        from: states[step.from.phase].clone(),
                        label: row.label.clone(),
                        next: states[step.to.phase].clone(),
                        via: dispatcher_id.to_owned(),
                        source_path: row.source_path.clone(),
                        source_line: row.source_line,
                        charge_command: model.charge_events.contains(&step.event),
                    }
                })
                .collect::<Vec<_>>();
            let mut replay_state = model.initial_states()[0];
            let mut replayed = true;
            for step in &trace {
                let code = execute(program, dispatcher_id, replay_state.phase, step.event);
                let expected = model.table[replay_state.phase][step.event]
                    .map(|(next, charge)| (next as i64) * 2 + i64::from(charge));
                if code.ok() != expected || model.apply(&replay_state, &step.event) != Some(step.to)
                {
                    replayed = false;
                    break;
                }
                replay_state = step.to;
            }
            if replayed && model.safety_invariant(&replay_state).is_err() {
                ProtocolSafetyOutcome::ConcreteCounterexample { trace: trace_rows }
            } else {
                ProtocolSafetyOutcome::AbstractCounterexample { trace: trace_rows }
            }
        }
        SafetyOutcome::BoundExhausted { .. } => ProtocolSafetyOutcome::BoundsExhausted,
        SafetyOutcome::DeadState { .. } => ProtocolSafetyOutcome::DeadState,
        SafetyOutcome::EmptyStateSpace => ProtocolSafetyOutcome::EmptyStateSpace,
    };
    Ok(SourceProtocolReport {
        schema: SOURCE_PROTOCOL_SCHEMA,
        project_revision: revision.project_revision().to_owned(),
        protocol_id: protocol.stable_id,
        protocol_source_path: source_path,
        protocol_source_digest: source_digest,
        dispatcher_id: dispatcher_id.to_owned(),
        caller_id: caller_id.to_owned(),
        state_domain: states,
        event_domain: events,
        initial_state: protocol.initial.name,
        success_state: success_state.to_owned(),
        charge_label: charge_label.to_owned(),
        fairness: "none",
        coverage,
        bounds,
        explored_states: explored.counters.explored_states,
        explored_transitions: explored.counters.explored_transitions,
        evidence_digest,
        outcome,
    })
}

/// Recompute all source executions and the finite exploration on a new
/// retained Project. Any source, protocol, initial state, assumption, or bound
/// change refuses attachment to the earlier report.
pub fn replay(
    recorded: &SourceProtocolReport,
    revision: &ProjectRevision,
) -> Result<(), Diagnostic> {
    let expected = check_project_source_protocol(
        revision,
        &recorded.protocol_id,
        &recorded.dispatcher_id,
        &recorded.caller_id,
        &recorded.success_state,
        &recorded.charge_label,
        recorded.bounds,
    )?;
    if &expected != recorded {
        return Err(refusal(
            "SPX-LP407",
            "source-bound finite protocol evidence is stale",
        ));
    }
    Ok(())
}

/// Agent-facing Project diagnostic under the ordinary held-input recheck.
/// It returns a finite report only; no command is issued to a payment provider.
pub fn check_authenticated_snapshot(
    snapshot: &mut crate::project::ProjectSnapshot,
    protocol_id: &str,
    dispatcher_id: &str,
    caller_id: &str,
    success_state: &str,
    charge_label: &str,
    bounds: Bounds,
) -> Result<String, Vec<Diagnostic>> {
    snapshot.with_authenticated_request(|snapshot| {
        check_project_source_protocol(
            &snapshot.retain_revision(),
            protocol_id,
            dispatcher_id,
            caller_id,
            success_state,
            charge_label,
            bounds,
        )
        .map(|report| report.to_json())
        .map_err(|error| vec![error])
    })
}
