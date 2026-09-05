//! Additive cached-closure and tracing seam for the prepared Project worker.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::conformance::NormalizedStatus;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedExpr, ResolvedType};

use super::{
    admitted_resolved_functions, child_expressions, guard_error, option_error, scan_closure,
    selection_error, Evaluator, Flow, FunctionLookup, Value, MAX_STEPS_LIMIT,
    REASON_AUTOMATIC_IDENTITY, REASON_UNSUPPORTED_CALLEE, REASON_UNSUPPORTED_RESULT_TYPE,
};

/// Fixed preparation bounds for the retained Project interpreter index.
pub(crate) const MAX_PREPARED_ORIGIN_NODES: usize = 262_144;
pub(crate) const MAX_PREPARED_INDEX_BYTES: usize = 16 * 1024 * 1024;

/// One expression origin observed by the retained Project evaluator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedTraceEvent {
    pub(crate) step: usize,
    pub(crate) depth: usize,
    pub(crate) phase: ResolvedTracePhase,
    pub(crate) function_id: Arc<str>,
    pub(crate) expression_id: Arc<str>,
    pub(crate) span: crate::ast::Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedTracePhase {
    Requires,
    Body,
    Ensures,
}

impl ResolvedTracePhase {
    pub(crate) const fn text(self) -> &'static str {
        match self {
            Self::Requires => "requires",
            Self::Body => "body",
            Self::Ensures => "ensures",
        }
    }
}

/// Closed outcomes for cancellation-aware prepared Project evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PreparedResolvedEvaluationOutcome {
    ReturnedI64(i64),
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    Cancelled { before_step: usize },
    GuardError(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedResolvedEvaluation {
    pub(crate) outcome: PreparedResolvedEvaluationOutcome,
    pub(crate) steps_used: usize,
    pub(crate) max_steps: usize,
    pub(crate) events: Vec<ResolvedTraceEvent>,
    pub(crate) dropped_events: usize,
    /// Retained contract-failure frame detail; `None` unless `outcome` is a
    /// contract failure.
    pub(crate) failure: Option<super::ContractFailureDetail>,
}

/// Authority-free cached closure index. It contains only owned identities and
/// vector positions, so it cannot outlive or alias a Project's HIR unsafely.
pub(crate) struct PreparedResolvedI64 {
    entry_id: String,
    entry_index: usize,
    function_indices: BTreeMap<String, usize>,
    origin_nodes: usize,
    index_bytes: usize,
}

impl PreparedResolvedI64 {
    pub(crate) fn function_ids(&self) -> impl Iterator<Item = &str> {
        self.function_indices.keys().map(String::as_str)
    }

    pub(crate) const fn origin_nodes(&self) -> usize {
        self.origin_nodes
    }

    pub(crate) const fn index_bytes(&self) -> usize {
        self.index_bytes
    }
}

/// Validate and retain the exact transitive zero-argument i64 closure once.
pub(crate) fn prepare_resolved_zero_arg_i64(
    program: &hir::ResolvedProgram,
    entry_id: &str,
) -> Result<PreparedResolvedI64, Vec<Diagnostic>> {
    hir::validate(program).map_err(|diagnostic| vec![diagnostic])?;
    if program.entrypoint.as_str() != entry_id {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_CALLEE,
            format!(
                "selection `{entry_id}` is not the resolved entry point `{}`",
                program.entrypoint
            ),
        )]);
    }
    let entry_index = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == entry_id)
        .ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("resolved entry `{entry_id}` is absent from the function index"),
            )]
        })?;
    let entry = &program.functions[entry_index];
    let explicit_entry = program
        .declarations
        .declaration(&entry.id)
        .is_some_and(|declaration| declaration.identity_origin == hir::IdentityOrigin::Explicit);
    if !explicit_entry {
        return Err(vec![selection_error(
            REASON_AUTOMATIC_IDENTITY,
            format!("resolved entry `{entry_id}` does not have an explicit stable identity"),
        )]);
    }
    if !entry.params.is_empty() || entry.return_type != ResolvedType::I64 {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!("resolved entry `{entry_id}` must have type `fn main() -> i64`"),
        )]);
    }
    let admitted = admitted_resolved_functions(program);
    let closure = scan_closure(entry_id, &admitted, &program.declarations)?;
    let mut function_indices = BTreeMap::new();
    let mut index_bytes = entry_id.len();
    let mut origin_nodes = 0usize;
    let mut expression_ids = BTreeSet::new();
    for id in &closure {
        let index = program
            .functions
            .iter()
            .position(|function| function.id.as_str() == id)
            .ok_or_else(|| vec![guard_error("prepared closure lost an admitted function")])?;
        index_bytes = index_bytes.checked_add(id.len()).ok_or_else(|| {
            vec![option_error(
                "prepared interpreter index byte accounting overflowed".to_owned(),
            )]
        })?;
        function_indices.insert(id.clone(), index);
        let function = &program.functions[index];
        let mut expressions = function
            .requires
            .iter()
            .chain(&function.ensures)
            .chain(std::iter::once(&function.body))
            .collect::<Vec<_>>();
        while let Some(expression) = expressions.pop() {
            origin_nodes = origin_nodes.checked_add(1).ok_or_else(|| {
                vec![option_error(
                    "prepared origin-node accounting overflowed".to_owned(),
                )]
            })?;
            index_bytes = index_bytes
                .checked_add(expression.id.as_str().len())
                .ok_or_else(|| {
                    vec![option_error(
                        "prepared index byte accounting overflowed".to_owned(),
                    )]
                })?;
            if !expression_ids.insert(expression.id.as_str().to_owned()) {
                return Err(vec![guard_error(
                    "prepared closure contains a duplicate expression identity",
                )]);
            }
            if origin_nodes > MAX_PREPARED_ORIGIN_NODES || index_bytes > MAX_PREPARED_INDEX_BYTES {
                return Err(vec![option_error(format!(
                    "prepared interpreter index exceeds {MAX_PREPARED_ORIGIN_NODES} nodes or {MAX_PREPARED_INDEX_BYTES} bytes"
                ))]);
            }
            expressions.extend(child_expressions(expression));
        }
    }
    Ok(PreparedResolvedI64 {
        entry_id: entry_id.to_owned(),
        entry_index,
        function_indices,
        origin_nodes,
        index_bytes,
    })
}

pub(crate) enum PreparedCancellation<'a> {
    Never,
    Atomic(&'a AtomicBool),
}

impl PreparedCancellation<'_> {
    pub(super) fn cancelled(&self, _completed_steps: usize) -> bool {
        match self {
            Self::Never => false,
            Self::Atomic(flag) => flag.load(Ordering::Acquire),
        }
    }
}

/// Execute one previously admitted closure without rebuilding its function
/// map or rescanning HIR. Callers choose the worker/thread boundary.
pub(crate) fn evaluate_prepared_resolved_zero_arg_i64(
    program: &hir::ResolvedProgram,
    prepared: &PreparedResolvedI64,
    max_steps: usize,
    max_events: usize,
    cancellation: PreparedCancellation<'_>,
) -> Result<PreparedResolvedEvaluation, Vec<Diagnostic>> {
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "prepared evaluation requires max_steps 1..={MAX_STEPS_LIMIT}"
        ))]);
    }
    if program.entrypoint.as_str() != prepared.entry_id
        || program
            .functions
            .get(prepared.entry_index)
            .is_none_or(|entry| entry.id.as_str() != prepared.entry_id)
    {
        return Err(vec![guard_error(
            "prepared closure no longer matches its resolved program",
        )]);
    }
    let lookup = FunctionLookup::Prepared {
        functions: &program.functions,
        indices: &prepared.function_indices,
    };
    let entry = &program.functions[prepared.entry_index];
    let mut evaluator = Evaluator::new_prepared(
        lookup,
        &program.declarations,
        max_steps,
        max_events,
        cancellation,
    );
    let evaluated = evaluator.call_frame(entry, Vec::new(), 0);
    let outcome = match evaluated {
        Ok(Value::Int(value)) => PreparedResolvedEvaluationOutcome::ReturnedI64(value),
        Ok(_) => PreparedResolvedEvaluationOutcome::GuardError(
            "zero-argument i64 entry returned a non-i64 value".to_owned(),
        ),
        Err(Flow::Failure(status)) => PreparedResolvedEvaluationOutcome::LanguageFailure(status),
        Err(Flow::Exhausted) => PreparedResolvedEvaluationOutcome::FuelExhausted,
        Err(Flow::DepthExceeded) => PreparedResolvedEvaluationOutcome::CallDepthExceeded,
        Err(Flow::Cancelled { before_step }) => {
            PreparedResolvedEvaluationOutcome::Cancelled { before_step }
        }
        Err(Flow::Utf8MaterializationLimitExceeded { .. }) => {
            PreparedResolvedEvaluationOutcome::GuardError(
                "unexpected UTF-8 materialization limit in legacy prepared evaluation".to_owned(),
            )
        }
        Err(Flow::Guard(detail)) => {
            PreparedResolvedEvaluationOutcome::GuardError(detail.to_owned())
        }
    };
    Ok(PreparedResolvedEvaluation {
        outcome,
        steps_used: evaluator.steps,
        max_steps,
        events: evaluator.trace_events,
        dropped_events: evaluator.dropped_trace_events,
        failure: evaluator.failure_detail.take(),
    })
}

/// Read-only structural traversal seam used by Project Source Trace replay.
pub(crate) fn trace_child_expressions(expression: &ResolvedExpr) -> Vec<&ResolvedExpr> {
    child_expressions(expression)
}
