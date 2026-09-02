//! Deterministic reference execution for an attached cleanup plan.
//!
//! This is deliberately a cleanup/control-flow oracle, not a HIR value
//! interpreter. A [`CleanupScenario`] supplies the boolean values, operation
//! outcomes, and final result that ordinary expression evaluation would have
//! produced. Calls to SEMAPRAX functions are represented by supplied outcomes;
//! this first acyclic slice does not recursively execute callees.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::conformance::{
    ConformanceTrace, ImportSite, InvocationPath, NormalizedStatus, OperationOutcome, Retryability,
    StatusClass, TraceEvent, TraceEventKind, TraceOutcome, TraceResult,
};
use crate::hir::{
    self, DeclarationId, ExpressionId, IdentityOrigin, ResolvedFunction, ResolvedProgram,
    ResolvedResourceDropKind, ResolvedType, ResolvedTypeDeclarationKind,
};
use crate::prelude;
use crate::runtime_status::{ScopedStatusToken, StatusArena, StatusArenaError, StatusContextId};

use super::{
    BlockId, CleanupBlock, CleanupEdge, CleanupPlace, CleanupResultSource, CleanupTerminator,
    CleanupTransition, EdgeCondition, EdgeId, ExitContinuation, ExitTarget, StagedCopyResultSource,
    StatusLane, StatusProducer, StatusSourceId, StorageId,
};

/// All target-dependent expression observations needed by the cleanup oracle.
///
/// Maps are keyed by semantic identities and are consumed only when their
/// decision point is reached. Supplying an unused decision is an error, which
/// keeps scenarios precise when a lazy or conditional path changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupScenario {
    pub scenario_id: String,
    pub booleans: BTreeMap<ExpressionId, bool>,
    /// Selected semantic case for each reached variant-match scrutinee.
    ///
    /// Case identities, rather than target tags, keep this oracle independent
    /// of Native64/Wasm32 representation. The executor still checks that the
    /// supplied case belongs to the scrutinee's resolved variant before
    /// following any plan edge.
    pub variant_cases: BTreeMap<ExpressionId, DeclarationId>,
    /// Refutable Match v1: the selected arm index (0-based) for each reached
    /// scalar-match decision chain.
    pub arm_selections: BTreeMap<ExpressionId, u32>,
    pub operations: BTreeMap<StatusSourceId, OperationOutcome>,
    /// A value for `CommitResult`; failure scenarios use `None`. `ReturnUnit`
    /// is reserved for a future source-level unit return type and is rejected
    /// for every function the current source language can declare.
    pub result: Option<TraceResult>,
    /// Adapter bindings available to this invocation.
    ///
    /// Unlike boolean and operation outcomes, bindings are configuration:
    /// known bindings may remain unused on the selected execution path.
    pub available_finalizer_imports: BTreeSet<DeclarationId>,
    pub context_nonce: u64,
    pub status_capacity: u32,
}

impl CleanupScenario {
    pub fn new(scenario_id: impl Into<String>, result: Option<TraceResult>) -> Self {
        Self {
            scenario_id: scenario_id.into(),
            booleans: BTreeMap::new(),
            variant_cases: BTreeMap::new(),
            arm_selections: BTreeMap::new(),
            operations: BTreeMap::new(),
            result,
            available_finalizer_imports: BTreeSet::new(),
            context_nonce: 0,
            // One frame has write-once failure selection, so one record is
            // sufficient unless a hostile plan is being exercised.
            status_capacity: 1,
        }
    }
}

/// Harness failures are distinct from language-level status failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CleanupExecutionError {
    InvalidProgram(String),
    FunctionNotFound(DeclarationId),
    UnsupportedResultType(String),
    MissingBooleanDecision(ExpressionId),
    MissingVariantDecision(ExpressionId),
    MissingArmSelection(ExpressionId),
    InvalidVariantDecision {
        scrutinee: ExpressionId,
        case: DeclarationId,
    },
    MissingOperationOutcome(StatusSourceId),
    UnusedBooleanDecisions(Vec<ExpressionId>),
    UnusedVariantDecisions(Vec<ExpressionId>),
    UnusedOperationOutcomes(Vec<StatusSourceId>),
    CycleDetected(BlockId),
    UnknownFinalizerBinding(DeclarationId),
    MissingFinalizerBinding(DeclarationId),
    UnsupportedCallableImport(DeclarationId),
    StatusArena(StatusArenaError),
    HarnessInvariant(String),
}

impl fmt::Display for CleanupExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProgram(detail) => write!(formatter, "invalid resolved program: {detail}"),
            Self::FunctionNotFound(function) => {
                write!(formatter, "cleanup function `{function}` does not exist")
            }
            Self::UnsupportedResultType(result) => write!(
                formatter,
                "cleanup conformance executor does not support result type `{result}`"
            ),
            Self::MissingBooleanDecision(expression) => write!(
                formatter,
                "cleanup scenario has no boolean decision for `{expression}`"
            ),
            Self::MissingArmSelection(scrutinee) => write!(
                formatter,
                "cleanup scenario supplies no arm selection for scalar match `{scrutinee}`"
            ),
            Self::MissingVariantDecision(scrutinee) => write!(
                formatter,
                "cleanup scenario has no variant decision for `{scrutinee}`"
            ),
            Self::InvalidVariantDecision { scrutinee, case } => write!(
                formatter,
                "cleanup scenario selects foreign variant case `{case}` for `{scrutinee}`"
            ),
            Self::MissingOperationOutcome(source) => write!(
                formatter,
                "cleanup scenario has no operation outcome for `{}`",
                source.expression
            ),
            Self::UnusedBooleanDecisions(expressions) => write!(
                formatter,
                "cleanup scenario has unused boolean decisions: {expressions:?}"
            ),
            Self::UnusedVariantDecisions(expressions) => write!(
                formatter,
                "cleanup scenario has unused variant decisions: {expressions:?}"
            ),
            Self::UnusedOperationOutcomes(sources) => write!(
                formatter,
                "cleanup scenario has unused operation outcomes: {sources:?}"
            ),
            Self::CycleDetected(block) => write!(
                formatter,
                "cleanup plan revisited block {}; cyclic execution is not supported",
                block.0
            ),
            Self::UnknownFinalizerBinding(import) => {
                write!(
                    formatter,
                    "finalizer binding `{import}` is not a resolved import"
                )
            }
            Self::MissingFinalizerBinding(import) => {
                write!(
                    formatter,
                    "finalizer import `{import}` has no scenario binding"
                )
            }
            Self::UnsupportedCallableImport(import) => write!(
                formatter,
                "callable import `{import}` is outside the attached-plan oracle slice"
            ),
            Self::StatusArena(error) => write!(formatter, "status arena error: {error}"),
            Self::HarnessInvariant(detail) => {
                write!(formatter, "cleanup execution invariant failed: {detail}")
            }
        }
    }
}

impl Error for CleanupExecutionError {}

impl From<StatusArenaError> for CleanupExecutionError {
    fn from(error: StatusArenaError) -> Self {
        Self::StatusArena(error)
    }
}

/// Execute one function's validated, attached cleanup plan.
///
/// The executor seeds owned parameters from `entry_state`, observes supplied
/// expression decisions, applies transitions in plan order, clears finalizer
/// guards before invocation, and emits only target-neutral conformance events.
pub fn execute_for_conformance(
    program: &ResolvedProgram,
    function: &DeclarationId,
    scenario: CleanupScenario,
) -> Result<ConformanceTrace, CleanupExecutionError> {
    hir::validate_core(program)
        .map_err(|diagnostic| CleanupExecutionError::InvalidProgram(diagnostic.to_string()))?;
    hir::validate_attached_identity_references(program)
        .map_err(|diagnostic| CleanupExecutionError::InvalidProgram(diagnostic.to_string()))?;
    crate::cleanup::validate_program(program)
        .map_err(|diagnostic| CleanupExecutionError::InvalidProgram(diagnostic.to_string()))?;
    super::replay::validate_program(program)
        .map_err(|diagnostic| CleanupExecutionError::InvalidProgram(diagnostic.to_string()))?;
    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.id == *function)
        .ok_or_else(|| CleanupExecutionError::FunctionNotFound(function.clone()))?;
    validate_public_result_type(program, function)?;
    Executor::new(program, function, scenario)?.run()
}

fn validate_public_result_type(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), CleanupExecutionError> {
    let ResolvedType::Nominal { declaration, .. } = &function.return_type else {
        return if matches!(function.return_type, ResolvedType::I64 | ResolvedType::Bool) {
            Ok(())
        } else {
            Err(CleanupExecutionError::UnsupportedResultType(
                function.return_type.identity_key(),
            ))
        };
    };
    match program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .map(|item| &item.kind)
    {
        Some(ResolvedTypeDeclarationKind::Resource { .. }) => Ok(()),
        Some(ResolvedTypeDeclarationKind::Variant { .. })
            if expression_has_try(&function.body)
                && program
                    .declarations
                    .type_facts(&function.return_type)
                    .is_some_and(|facts| {
                        facts.copy && facts.sized && !facts.contains_resource && !facts.needs_drop
                    }) =>
        {
            // The executor authenticates staged Copy-result control/state, but
            // the public conformance protocol intentionally has no aggregate
            // value representation. Terminal materialization remains closed.
            Ok(())
        }
        Some(
            ResolvedTypeDeclarationKind::Record { .. }
            | ResolvedTypeDeclarationKind::Class { .. }
            | ResolvedTypeDeclarationKind::Variant { .. },
        )
        | None => Err(CleanupExecutionError::UnsupportedResultType(
            function.return_type.identity_key(),
        )),
    }
}

fn collect_variant_domains(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<BTreeMap<ExpressionId, BTreeSet<DeclarationId>>, CleanupExecutionError> {
    fn visit(
        program: &ResolvedProgram,
        expression: &hir::ResolvedExpr,
        domains: &mut BTreeMap<ExpressionId, BTreeSet<DeclarationId>>,
    ) -> Result<(), CleanupExecutionError> {
        match &expression.kind {
            hir::ResolvedExprKind::Int(_)
            | hir::ResolvedExprKind::Int32(_)
            | hir::ResolvedExprKind::Char(_)
            | hir::ResolvedExprKind::Uint8(_)
            | hir::ResolvedExprKind::Usize(_)
            | hir::ResolvedExprKind::ArrayU8(_)
            | hir::ResolvedExprKind::RepeatArrayU8 { .. }
            | hir::ResolvedExprKind::Float32(_)
            | hir::ResolvedExprKind::Float64(_)
            | hir::ResolvedExprKind::Bool(_)
            | hir::ResolvedExprKind::String(_)
            | hir::ResolvedExprKind::Place(_)
            | hir::ResolvedExprKind::BorrowPlace { .. } => {}
            hir::ResolvedExprKind::Call { args, .. } => {
                for argument in args {
                    visit(program, argument, domains)?;
                }
            }
            hir::ResolvedExprKind::NativeRustImportCall(call) => {
                for argument in &call.args {
                    visit(program, argument, domains)?;
                }
            }
            hir::ResolvedExprKind::HostCommandCall(call) => {
                for argument in &call.args {
                    visit(program, argument, domains)?;
                }
            }
            hir::ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => {
                visit(program, source, domains)?;
                visit(program, start, domains)?;
                visit(program, end, domains)?;
            }
            hir::ResolvedExprKind::Unary { value, .. }
            | hir::ResolvedExprKind::Project { base: value, .. }
            | hir::ResolvedExprKind::Upcast { source: value } => {
                visit(program, value, domains)?;
            }
            hir::ResolvedExprKind::Binary { left, right, .. } => {
                visit(program, left, domains)?;
                visit(program, right, domains)?;
            }
            hir::ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            visit(program, child, domains)?;
                        }
                    }
                }
                visit(program, tail, domains)?;
            }
            hir::ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                visit(program, condition, domains)?;
                visit(program, then_branch, domains)?;
                visit(program, else_branch, domains)?;
            }
            hir::ResolvedExprKind::ConstructRecord { fields, .. }
            | hir::ResolvedExprKind::ConstructVariant { fields, .. } => {
                for field in fields {
                    visit(program, &field.value, domains)?;
                }
            }
            hir::ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                visit(program, base, domains)?;
                for field in fields {
                    visit(program, &field.value, domains)?;
                }
            }
            hir::ResolvedExprKind::Try { operand, .. }
            | hir::ResolvedExprKind::TryOption { operand, .. } => {
                visit(program, operand, domains)?;
                let ResolvedType::Nominal { declaration, .. } = &operand.ty else {
                    return Err(invariant(format!(
                        "postfix `?` operand `{}` is not a nominal Result",
                        operand.id
                    )));
                };
                let cases = program
                    .types
                    .iter()
                    .find(|item| item.id == *declaration)
                    .and_then(|item| match &item.kind {
                        ResolvedTypeDeclarationKind::Variant { cases } => Some(cases),
                        ResolvedTypeDeclarationKind::Resource { .. }
                        | ResolvedTypeDeclarationKind::Record { .. }
                        | ResolvedTypeDeclarationKind::Class { .. } => None,
                    })
                    .ok_or_else(|| {
                        invariant(format!(
                            "postfix `?` operand `{}` references a non-variant declaration",
                            operand.id
                        ))
                    })?;
                let domain = cases
                    .iter()
                    .map(|case| case.id.clone())
                    .collect::<BTreeSet<_>>();
                if domain.is_empty() || domains.insert(operand.id.clone(), domain).is_some() {
                    return Err(invariant(format!(
                        "postfix `?` operand `{}` has an invalid or duplicate case domain",
                        operand.id
                    )));
                }
            }
            hir::ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                visit(program, scrutinee, domains)?;
                let ResolvedType::Nominal { declaration, .. } = &scrutinee.ty else {
                    return Err(invariant(format!(
                        "match scrutinee `{}` is not a nominal variant",
                        scrutinee.id
                    )));
                };
                let facts = program
                    .declarations
                    .type_facts(&scrutinee.ty)
                    .ok_or_else(|| {
                        invariant(format!(
                            "match scrutinee `{}` has no concrete type facts",
                            scrutinee.id
                        ))
                    })?;
                if !facts.copy || facts.contains_resource || !facts.sized || facts.needs_drop {
                    return Err(invariant(format!(
                        "match scrutinee `{}` is outside the copy-only variant executor",
                        scrutinee.id
                    )));
                }
                let cases = program
                    .types
                    .iter()
                    .find(|item| item.id == *declaration)
                    .and_then(|item| match &item.kind {
                        ResolvedTypeDeclarationKind::Variant { cases } => Some(cases),
                        ResolvedTypeDeclarationKind::Resource { .. }
                        | ResolvedTypeDeclarationKind::Record { .. }
                        | ResolvedTypeDeclarationKind::Class { .. } => None,
                    })
                    .ok_or_else(|| {
                        invariant(format!(
                            "match scrutinee `{}` references a non-variant declaration",
                            scrutinee.id
                        ))
                    })?;
                let domain = cases
                    .iter()
                    .map(|case| case.id.clone())
                    .collect::<BTreeSet<_>>();
                if domain.is_empty() || domains.insert(scrutinee.id.clone(), domain).is_some() {
                    return Err(invariant(format!(
                        "match scrutinee `{}` has an invalid or duplicate case domain",
                        scrutinee.id
                    )));
                }
                for arm in arms {
                    visit(program, &arm.value, domains)?;
                }
            }
        }
        Ok(())
    }

    let mut domains = BTreeMap::new();
    for expression in &function.requires {
        visit(program, expression, &mut domains)?;
    }
    for expression in &function.ensures {
        visit(program, expression, &mut domains)?;
    }
    visit(program, &function.body, &mut domains)?;
    Ok(domains)
}

fn validate_staged_copy_source(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    source: &StagedCopyResultSource,
) -> Result<(), CleanupExecutionError> {
    match source {
        StagedCopyResultSource::Body {
            expression,
            instance,
        } => {
            if expression != &function.body.id || instance != &function.return_type {
                return Err(invariant(
                    "body Copy-result stage changes expression or concrete instance",
                ));
            }
        }
        StagedCopyResultSource::TryResidual {
            expression,
            operand,
            source_instance,
            target_instance,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
        } => {
            if result.as_str() != prelude::RESULT_ID
                || ok_case.as_str() != prelude::RESULT_OK_ID
                || ok_field.as_str() != prelude::RESULT_OK_VALUE_ID
                || err_case.as_str() != prelude::RESULT_ERR_ID
                || err_field.as_str() != prelude::RESULT_ERR_ERROR_ID
                || target_instance != &function.return_type
            {
                return Err(invariant(
                    "Try residual stage changes Result identities or target instance",
                ));
            }
            for id in [result, ok_case, ok_field, err_case, err_field] {
                if program
                    .declarations
                    .declaration(id)
                    .is_none_or(|declaration| {
                        declaration.identity_origin != IdentityOrigin::CompilerOwned
                    })
                {
                    return Err(invariant(format!(
                        "Try residual identity `{id}` is not compiler-owned"
                    )));
                }
            }
            let actual = find_expression(&function.body, expression)
                .ok_or_else(|| invariant("Try residual stage names an unknown expression"))?;
            let hir::ResolvedExprKind::Try {
                operand: actual_operand,
                result: actual_result,
                ok_case: actual_ok_case,
                ok_field: actual_ok_field,
                err_case: actual_err_case,
                err_field: actual_err_field,
                residual_type,
            } = &actual.kind
            else {
                return Err(invariant(
                    "Try residual stage does not name a Try expression",
                ));
            };
            if &actual_operand.id != operand
                || &actual_operand.ty != source_instance
                || residual_type != target_instance
                || actual_result != result
                || actual_ok_case != ok_case
                || actual_ok_field != ok_field
                || actual_err_case != err_case
                || actual_err_field != err_field
            {
                return Err(invariant(
                    "Try residual stage disagrees with exact typed HIR",
                ));
            }
            let selected = program
                .declarations
                .type_facts(source_instance)
                .zip(program.declarations.type_facts(target_instance));
            if selected.is_none_or(|(source, target)| {
                [source, target].into_iter().any(|facts| {
                    !facts.copy || !facts.sized || facts.contains_resource || facts.needs_drop
                })
            }) {
                return Err(invariant(
                    "Try residual stage is outside the Copy Result slice",
                ));
            }
        }
        StagedCopyResultSource::TryOptionNone {
            expression,
            operand,
            source_instance,
            target_instance,
            option,
            some_case,
            some_field,
            none_case,
        } => {
            if option.as_str() != prelude::OPTION_ID
                || some_case.as_str() != prelude::OPTION_SOME_ID
                || some_field.as_str() != prelude::OPTION_SOME_VALUE_ID
                || none_case.as_str() != prelude::OPTION_NONE_ID
                || target_instance != &function.return_type
            {
                return Err(invariant(
                    "Option Try residual stage changes Option identities or target instance",
                ));
            }
            for id in [option, some_case, some_field, none_case] {
                if program
                    .declarations
                    .declaration(id)
                    .is_none_or(|declaration| {
                        declaration.identity_origin != IdentityOrigin::CompilerOwned
                    })
                {
                    return Err(invariant(format!(
                        "Option Try residual identity `{id}` is not compiler-owned"
                    )));
                }
            }
            let actual = find_expression(&function.body, expression).ok_or_else(|| {
                invariant("Option Try residual stage names an unknown expression")
            })?;
            let hir::ResolvedExprKind::TryOption {
                operand: actual_operand,
                option: actual_option,
                some_case: actual_some_case,
                some_field: actual_some_field,
                none_case: actual_none_case,
                residual_type,
            } = &actual.kind
            else {
                return Err(invariant(
                    "Option Try residual stage does not name a TryOption expression",
                ));
            };
            if &actual_operand.id != operand
                || &actual_operand.ty != source_instance
                || residual_type != target_instance
                || actual_option != option
                || actual_some_case != some_case
                || actual_some_field != some_field
                || actual_none_case != none_case
            {
                return Err(invariant(
                    "Option Try residual stage disagrees with exact typed HIR",
                ));
            }
            let selected = program
                .declarations
                .type_facts(source_instance)
                .zip(program.declarations.type_facts(target_instance));
            if selected.is_none_or(|(source, target)| {
                [source, target].into_iter().any(|facts| {
                    !facts.copy || !facts.sized || facts.contains_resource || facts.needs_drop
                })
            }) {
                return Err(invariant(
                    "Option Try residual stage is outside the Copy Option slice",
                ));
            }
        }
    }
    Ok(())
}

fn expression_has_try(expression: &hir::ResolvedExpr) -> bool {
    find_expression_by(expression, &|candidate| {
        matches!(
            candidate.kind,
            hir::ResolvedExprKind::Try { .. } | hir::ResolvedExprKind::TryOption { .. }
        )
    })
    .is_some()
}

fn find_expression<'a>(
    expression: &'a hir::ResolvedExpr,
    id: &ExpressionId,
) -> Option<&'a hir::ResolvedExpr> {
    find_expression_by(expression, &|candidate| &candidate.id == id)
}

fn find_expression_by<'a>(
    expression: &'a hir::ResolvedExpr,
    predicate: &impl Fn(&hir::ResolvedExpr) -> bool,
) -> Option<&'a hir::ResolvedExpr> {
    if predicate(expression) {
        return Some(expression);
    }
    match &expression.kind {
        hir::ResolvedExprKind::Call { args, .. } => args
            .iter()
            .find_map(|argument| find_expression_by(argument, predicate)),
        hir::ResolvedExprKind::NativeRustImportCall(call) => call
            .args
            .iter()
            .find_map(|argument| find_expression_by(argument, predicate)),
        hir::ResolvedExprKind::HostCommandCall(call) => call
            .args
            .iter()
            .find_map(|argument| find_expression_by(argument, predicate)),
        hir::ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => find_expression_by(source, predicate)
            .or_else(|| find_expression_by(start, predicate))
            .or_else(|| find_expression_by(end, predicate)),
        hir::ResolvedExprKind::Unary { value, .. }
        | hir::ResolvedExprKind::Project { base: value, .. }
        | hir::ResolvedExprKind::Try { operand: value, .. }
        | hir::ResolvedExprKind::TryOption { operand: value, .. }
        | hir::ResolvedExprKind::Upcast { source: value } => find_expression_by(value, predicate),
        hir::ResolvedExprKind::Binary { left, right, .. } => {
            find_expression_by(left, predicate).or_else(|| find_expression_by(right, predicate))
        }
        hir::ResolvedExprKind::Block { statements, tail } => statements
            .iter()
            .find_map(|statement| {
                (0..statement.child_count()).find_map(|index| {
                    statement
                        .child(index)
                        .and_then(|child| find_expression_by(child, predicate))
                })
            })
            .or_else(|| find_expression_by(tail, predicate)),
        hir::ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => find_expression_by(condition, predicate)
            .or_else(|| find_expression_by(then_branch, predicate))
            .or_else(|| find_expression_by(else_branch, predicate)),
        hir::ResolvedExprKind::ConstructRecord { fields, .. }
        | hir::ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .find_map(|field| find_expression_by(&field.value, predicate)),
        hir::ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => find_expression_by(scrutinee, predicate).or_else(|| {
            arms.iter()
                .find_map(|arm| find_expression_by(&arm.value, predicate))
        }),
        hir::ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            find_expression_by(base, predicate).or_else(|| {
                fields
                    .iter()
                    .find_map(|field| find_expression_by(&field.value, predicate))
            })
        }
        hir::ResolvedExprKind::Int(_)
        | hir::ResolvedExprKind::Int32(_)
        | hir::ResolvedExprKind::Char(_)
        | hir::ResolvedExprKind::Uint8(_)
        | hir::ResolvedExprKind::Usize(_)
        | hir::ResolvedExprKind::ArrayU8(_)
        | hir::ResolvedExprKind::RepeatArrayU8 { .. }
        | hir::ResolvedExprKind::Float32(_)
        | hir::ResolvedExprKind::Float64(_)
        | hir::ResolvedExprKind::Bool(_)
        | hir::ResolvedExprKind::String(_)
        | hir::ResolvedExprKind::Place(_)
        | hir::ResolvedExprKind::BorrowPlace { .. } => None,
    }
}

#[derive(Clone)]
struct Leaf {
    place: CleanupPlace,
    lifecycle: DeclarationId,
}

#[derive(Clone)]
struct SelectedFailure {
    source: StatusSourceId,
    token: ScopedStatusToken,
}

/// Semantic caller out-slot state. No payload, address, alignment, or target
/// representation is stored in the reference executor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResultSlotState {
    Uninitialized,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalKind {
    CommitResult,
    ReturnFailure,
    /// Reserved for a future source-level unit return type.
    ReturnUnit,
}

struct Executor<'a> {
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    scenario: CleanupScenario,
    leaves: BTreeMap<LivenessFlagId, Leaf>,
    lifecycle_bindings: BTreeMap<DeclarationId, Option<DeclarationId>>,
    live: BTreeSet<LivenessFlagId>,
    used_booleans: BTreeSet<ExpressionId>,
    used_variant_cases: BTreeSet<ExpressionId>,
    used_arm_selections: BTreeSet<ExpressionId>,
    variant_domains: BTreeMap<ExpressionId, BTreeSet<DeclarationId>>,
    used_operations: BTreeSet<StatusSourceId>,
    visited: BTreeSet<BlockId>,
    selected: Option<SelectedFailure>,
    staged_copy_result: Option<StagedCopyResultSource>,
    result_slot: ResultSlotState,
    status_arena: StatusArena,
    events: Vec<TraceEvent>,
}

impl<'a> Executor<'a> {
    fn new(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
        scenario: CleanupScenario,
    ) -> Result<Self, CleanupExecutionError> {
        let mut leaves = BTreeMap::new();
        for slot in &function.cleanup_plan.slots {
            collect_leaves(
                &slot.storage,
                &mut Vec::new(),
                &slot.field_liveness_shape,
                &mut leaves,
            )?;
        }
        // Adapter configuration is validated before entry guards are seeded
        // and before execution can emit any event. A binding is required for
        // every imported lifecycle present anywhere in this function's plan,
        // even when the selected path would leave its guard false.
        let lifecycle_bindings = preflight_finalizer_bindings(program, function, &scenario)?;
        let variant_domains = collect_variant_domains(program, function)?;
        let status_arena = StatusArena::new(
            StatusContextId::new(scenario.context_nonce),
            scenario.status_capacity,
        )?;
        let mut executor = Self {
            program,
            function,
            scenario,
            leaves,
            lifecycle_bindings,
            live: BTreeSet::new(),
            used_booleans: BTreeSet::new(),
            used_variant_cases: BTreeSet::new(),
            used_arm_selections: BTreeSet::new(),
            variant_domains,
            used_operations: BTreeSet::new(),
            visited: BTreeSet::new(),
            selected: None,
            staged_copy_result: None,
            result_slot: ResultSlotState::Uninitialized,
            status_arena,
            events: Vec::new(),
        };
        for place in executor
            .function
            .cleanup_plan
            .entry_state
            .live_owned_parameters
            .clone()
        {
            executor.initialize_flags(&place, "owned entry parameter")?;
        }
        for entry in executor
            .function
            .cleanup_plan
            .entry_state
            .conditional_owned_parameters
            .clone()
        {
            let decision = match &entry.storage {
                StorageId::Value(value) => executor
                    .scenario
                    .variant_cases
                    .keys()
                    .find(|expression| expression.as_str() == value.as_str())
                    .cloned()
                    .ok_or_else(|| {
                        invariant("conditional entry has no value-identity case decision")
                    })?,
                StorageId::Temporary(expression) => expression.clone(),
                StorageId::CallArgument {
                    value_expression, ..
                } => value_expression.clone(),
                StorageId::ProvisionalResult => {
                    return Err(invariant(
                        "provisional result cannot be a conditional entry parameter",
                    ));
                }
            };
            executor.used_variant_cases.insert(decision.clone());
            let active = executor
                .scenario
                .variant_cases
                .get(&decision)
                .ok_or_else(|| CleanupExecutionError::MissingVariantDecision(decision.clone()))?;
            let selected = entry
                .cases
                .iter()
                .find(|case| case.case == *active)
                .ok_or_else(|| CleanupExecutionError::InvalidVariantDecision {
                    scrutinee: decision,
                    case: active.clone(),
                })?;
            for place in &selected.live_places {
                executor.initialize_flags(place, "conditional owned entry parameter")?;
            }
        }
        Ok(executor)
    }

    fn run(mut self) -> Result<ConformanceTrace, CleanupExecutionError> {
        let mut current = self.function.cleanup_plan.entry;
        let (outcome, terminal) = loop {
            if !self.visited.insert(current) {
                return Err(CleanupExecutionError::CycleDetected(current));
            }
            let block = self.block(current)?.clone();
            for transition in block.transitions {
                self.execute_transition(transition)?;
            }
            match block.terminator {
                CleanupTerminator::Goto(edge) => current = self.follow_goto(current, edge)?,
                CleanupTerminator::Branch(edges) => {
                    current = self.follow_branch(current, &edges)?;
                }
                CleanupTerminator::Exit(exit) => {
                    let exit = self.exit(exit, current)?.clone();
                    self.execute_finalizers(&exit)?;
                    match exit.continuation {
                        ExitContinuation::Continue(edge) => {
                            current = self.follow_goto(current, edge)?;
                        }
                        ExitContinuation::CommitResult { source } => {
                            break (self.commit_result(source)?, TerminalKind::CommitResult);
                        }
                        ExitContinuation::ReturnFailure { source } => {
                            break (self.return_failure(source)?, TerminalKind::ReturnFailure);
                        }
                        ExitContinuation::ReturnUnit => {
                            break (self.return_unit()?, TerminalKind::ReturnUnit);
                        }
                    }
                }
            }
        };
        self.finish(outcome, terminal)
    }

    fn block(&self, id: BlockId) -> Result<&CleanupBlock, CleanupExecutionError> {
        self.function
            .cleanup_plan
            .blocks
            .iter()
            .find(|block| block.id == id)
            .ok_or_else(|| invariant(format!("missing cleanup block {}", id.0)))
    }

    fn edge(&self, id: EdgeId, from: BlockId) -> Result<&CleanupEdge, CleanupExecutionError> {
        let edge = self
            .function
            .cleanup_plan
            .edges
            .iter()
            .find(|edge| edge.id == id)
            .ok_or_else(|| invariant(format!("missing cleanup edge {}", id.0)))?;
        if edge.from != from {
            return Err(invariant(format!(
                "edge {} starts at block {}, not block {}",
                id.0, edge.from.0, from.0
            )));
        }
        Ok(edge)
    }

    fn exit(
        &self,
        id: super::ExitTargetId,
        from: BlockId,
    ) -> Result<&ExitTarget, CleanupExecutionError> {
        let exit = self
            .function
            .cleanup_plan
            .exits
            .iter()
            .find(|exit| exit.id == id)
            .ok_or_else(|| invariant(format!("missing cleanup exit {}", id.0)))?;
        if exit.from != from {
            return Err(invariant(format!(
                "exit {} starts at block {}, not block {}",
                id.0, exit.from.0, from.0
            )));
        }
        Ok(exit)
    }

    fn execute_transition(
        &mut self,
        transition: CleanupTransition,
    ) -> Result<(), CleanupExecutionError> {
        match transition {
            CleanupTransition::Initialize { at, destination } => {
                self.initialize_flags(&destination, "initialize transition")?;
                self.emit(TraceEventKind::Initialize { at, destination });
            }
            CleanupTransition::Transfer {
                at,
                source,
                destination,
            } => {
                self.transfer_flags(&source, &destination)?;
                self.emit(TraceEventKind::Transfer {
                    at,
                    source,
                    destination,
                });
            }
            CleanupTransition::AuthenticateVariantCase {
                at,
                source,
                variant: _,
                case,
            } => {
                let selected_prefix = source
                    .projections
                    .iter()
                    .chain(std::iter::once(&case))
                    .cloned()
                    .collect::<Vec<_>>();
                for flag in self.flags_under(&source)? {
                    if self.live.contains(&flag)
                        && !self.leaves[&flag]
                            .place
                            .projections
                            .starts_with(&selected_prefix)
                    {
                        return Err(invariant(
                            "variant authentication observed live inactive-case payload",
                        ));
                    }
                }
                let actual = self
                    .scenario
                    .variant_cases
                    .get(&at)
                    .ok_or_else(|| CleanupExecutionError::MissingVariantDecision(at.clone()))?;
                self.used_variant_cases.insert(at.clone());
                if actual != &case {
                    return Err(CleanupExecutionError::InvalidVariantDecision {
                        scrutinee: at,
                        case: actual.clone(),
                    });
                }
            }
            CleanupTransition::TransferVariant {
                at,
                source,
                destination,
                variant: _,
            } => {
                self.transfer_variant_flags(&source, &destination)?;
                self.emit(TraceEventKind::Transfer {
                    at,
                    source,
                    destination,
                });
            }
            CleanupTransition::InitializeVariant {
                at,
                destination,
                variant,
            } => {
                let actual = self
                    .scenario
                    .variant_cases
                    .get(&at)
                    .ok_or_else(|| CleanupExecutionError::MissingVariantDecision(at.clone()))?;
                let cases = self
                    .program
                    .declarations
                    .variant_cases(&variant)
                    .ok_or_else(|| invariant("variant initialization names a non-variant"))?;
                if !cases.iter().any(|case| case.id == *actual) {
                    return Err(CleanupExecutionError::InvalidVariantDecision {
                        scrutinee: at,
                        case: actual.clone(),
                    });
                }
                self.used_variant_cases.insert(at.clone());
                let selected_prefix = destination
                    .projections
                    .iter()
                    .chain(std::iter::once(actual))
                    .cloned()
                    .collect::<Vec<_>>();
                let all_flags = self.flags_under(&destination)?;
                if all_flags.iter().any(|flag| self.live.contains(flag)) {
                    return Err(invariant(
                        "variant initialization targets a live cleanup place",
                    ));
                }
                self.live.extend(all_flags.into_iter().filter(|flag| {
                    self.leaves[flag]
                        .place
                        .projections
                        .starts_with(&selected_prefix)
                }));
                self.emit(TraceEventKind::Initialize { at, destination });
            }
            CleanupTransition::CallCommit { call, arguments } => {
                let callee = self.callee_for_call(&call)?;
                let mut consumed = BTreeSet::new();
                for argument in &arguments {
                    let all_flags = self.flags_under(&argument.source)?;
                    let conditional = self
                        .function
                        .cleanup_plan
                        .slots
                        .iter()
                        .find(|slot| slot.storage == argument.source.storage)
                        .is_some_and(|slot| {
                            matches!(
                                slot.field_liveness_shape,
                                FieldLivenessShape::Variant { .. }
                            )
                        });
                    for flag in all_flags {
                        if conditional && !self.live.contains(&flag) {
                            continue;
                        }
                        if !self.live.contains(&flag) {
                            return Err(invariant(format!(
                                "call `{call}` consumes dead argument flag {}",
                                flag.0
                            )));
                        }
                        if !consumed.insert(flag) {
                            return Err(invariant(format!(
                                "call `{call}` consumes flag {} more than once",
                                flag.0
                            )));
                        }
                    }
                }
                // Clear the complete group only after every argument epoch has
                // been checked, preserving atomic call commit.
                self.live.retain(|flag| !consumed.contains(flag));
                self.emit(TraceEventKind::CallCommit {
                    call,
                    callee,
                    arguments,
                });
            }
            CleanupTransition::SelectFailure { source } => self.select_failure(source)?,
            CleanupTransition::StageCopyResult { source } => {
                self.stage_copy_result(source)?;
            }
        }
        Ok(())
    }

    fn stage_copy_result(
        &mut self,
        source: StagedCopyResultSource,
    ) -> Result<(), CleanupExecutionError> {
        if self.result_slot != ResultSlotState::Uninitialized {
            return Err(invariant("Copy result staging follows publication"));
        }
        if self.selected.is_some() {
            return Err(invariant("Copy result staging follows failure selection"));
        }
        if self.staged_copy_result.is_some() {
            return Err(invariant("Copy result is staged more than once"));
        }
        if !self.live.is_empty() {
            return Err(invariant("Copy result staging carries resource liveness"));
        }
        validate_staged_copy_source(self.program, self.function, &source)?;
        self.staged_copy_result = Some(source);
        Ok(())
    }

    fn callee_for_call(&self, call: &ExpressionId) -> Result<DeclarationId, CleanupExecutionError> {
        if let Some(expression) = find_expression(&self.function.body, call) {
            if let hir::ResolvedExprKind::ByteRange { operation, .. } = &expression.kind {
                if operation.as_str() != crate::byte_ops::RANGE_ID {
                    return Err(invariant(
                        "byte range carries an unknown operation identity",
                    ));
                }
                return Ok(operation.clone());
            }
            if let hir::ResolvedExprKind::HostCommandCall(command) = &expression.kind {
                return Ok(DeclarationId::new(crate::command_io_ops::id(
                    command.operation,
                )));
            }
            if let hir::ResolvedExprKind::Call {
                callee,
                instance: None,
                ..
            } = &expression.kind
            {
                if crate::byte_ops::by_id(callee.as_str()).is_some()
                    || crate::host_io_ops::by_id(callee.as_str()).is_some()
                {
                    return Ok(callee.clone());
                }
            }
        }
        let source = self
            .function
            .cleanup_plan
            .status_sources
            .iter()
            .find(|source| {
                source.id.expression == *call && source.id.lane == StatusLane::OperationFailure
            })
            .ok_or_else(|| invariant(format!("call `{call}` has no propagated status source")))?;
        let StatusProducer::PropagatedCall { callee } = &source.producer else {
            return Err(invariant(format!(
                "call `{call}` status source is not a propagated call"
            )));
        };
        if self.program.functions.iter().any(|item| item.id == *callee) {
            return Ok(callee.clone());
        }
        if self
            .program
            .interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
            .any(|import| import.id == *callee)
        {
            return Err(CleanupExecutionError::UnsupportedCallableImport(
                callee.clone(),
            ));
        }
        Err(invariant(format!("call target `{callee}` does not exist")))
    }

    fn select_failure(&mut self, source: StatusSourceId) -> Result<(), CleanupExecutionError> {
        if self.selected.is_some() {
            return Err(invariant("failure selection is write-once"));
        }
        let producer = self
            .function
            .cleanup_plan
            .status_sources
            .iter()
            .find(|candidate| candidate.id == source)
            .map(|candidate| candidate.producer.clone())
            .ok_or_else(|| invariant(format!("unknown failure source `{}`", source.expression)))?;
        let status = match producer {
            StatusProducer::ContractFalse { phase, .. } => NormalizedStatus::contract(phase),
            StatusProducer::CheckedArithmetic {
                normalized_cases, ..
            } => {
                let status = self.failure_outcome(&source)?;
                if !normalized_cases
                    .iter()
                    .any(|case| status == NormalizedStatus::arithmetic(*case))
                {
                    return Err(invariant(format!(
                        "checked operation `{}` supplied a status outside its normalized cases",
                        source.expression
                    )));
                }
                status
            }
            StatusProducer::PropagatedCall { callee } => {
                let status = self.failure_outcome(&source)?;
                validate_propagated_status(&callee, &status)?;
                status
            }
        };
        let token = self.status_arena.record(status.clone())?;
        self.selected = Some(SelectedFailure {
            source: source.clone(),
            token,
        });
        self.emit(TraceEventKind::SelectFailure { source, status });
        Ok(())
    }

    fn failure_outcome(
        &mut self,
        source: &StatusSourceId,
    ) -> Result<NormalizedStatus, CleanupExecutionError> {
        self.used_operations.insert(source.clone());
        match self
            .scenario
            .operations
            .get(source)
            .ok_or_else(|| CleanupExecutionError::MissingOperationOutcome(source.clone()))?
        {
            OperationOutcome::Failure(status) => Ok(status.clone()),
            OperationOutcome::Success => Err(invariant(format!(
                "failure source `{}` selected a successful operation",
                source.expression
            ))),
        }
    }

    fn follow_goto(&mut self, from: BlockId, id: EdgeId) -> Result<BlockId, CleanupExecutionError> {
        let edge = self.edge(id, from)?.clone();
        if !self.condition_matches(&edge.condition)? {
            return Err(invariant(format!("goto edge {} condition is false", id.0)));
        }
        Ok(edge.to)
    }

    fn follow_branch(
        &mut self,
        from: BlockId,
        ids: &[EdgeId],
    ) -> Result<BlockId, CleanupExecutionError> {
        let mut selected = None;
        for id in ids {
            let edge = self.edge(*id, from)?.clone();
            if self.condition_matches(&edge.condition)? && selected.replace(edge.to).is_some() {
                return Err(invariant(format!(
                    "branch from block {} selects multiple edges",
                    from.0
                )));
            }
        }
        selected.ok_or_else(|| invariant(format!("branch from block {} selects no edge", from.0)))
    }

    fn condition_matches(
        &mut self,
        condition: &EdgeCondition,
    ) -> Result<bool, CleanupExecutionError> {
        match condition {
            EdgeCondition::Always => Ok(true),
            EdgeCondition::BooleanResult(expression, expected) => {
                self.used_booleans.insert(expression.clone());
                let actual = self.scenario.booleans.get(expression).ok_or_else(|| {
                    CleanupExecutionError::MissingBooleanDecision(expression.clone())
                })?;
                Ok(actual == expected)
            }
            EdgeCondition::VariantCase {
                scrutinee,
                case,
                matches,
            } => {
                self.used_variant_cases.insert(scrutinee.clone());
                let actual = self.scenario.variant_cases.get(scrutinee).ok_or_else(|| {
                    CleanupExecutionError::MissingVariantDecision(scrutinee.clone())
                })?;
                let domain = self.variant_domains.get(scrutinee).ok_or_else(|| {
                    invariant(format!(
                        "variant edge references non-match scrutinee `{scrutinee}`"
                    ))
                })?;
                if !domain.contains(actual) {
                    return Err(CleanupExecutionError::InvalidVariantDecision {
                        scrutinee: scrutinee.clone(),
                        case: actual.clone(),
                    });
                }
                Ok((actual == case) == *matches)
            }
            EdgeCondition::ArmSelected {
                scrutinee,
                arm,
                selected,
            } => {
                self.used_arm_selections.insert(scrutinee.clone());
                let actual =
                    self.scenario.arm_selections.get(scrutinee).ok_or_else(|| {
                        CleanupExecutionError::MissingArmSelection(scrutinee.clone())
                    })?;
                Ok((*actual == *arm) == *selected)
            }
            EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                self.used_operations.insert(source.clone());
                let outcome = self.scenario.operations.get(source).ok_or_else(|| {
                    CleanupExecutionError::MissingOperationOutcome(source.clone())
                })?;
                let success = matches!(outcome, OperationOutcome::Success);
                Ok(match condition {
                    EdgeCondition::StatusZero(_) => success,
                    EdgeCondition::StatusNonzero(_) => !success,
                    EdgeCondition::Always
                    | EdgeCondition::BooleanResult(_, _)
                    | EdgeCondition::VariantCase { .. }
                    | EdgeCondition::ArmSelected { .. } => unreachable!(),
                })
            }
        }
    }

    fn execute_finalizers(&mut self, exit: &ExitTarget) -> Result<(), CleanupExecutionError> {
        for action in &exit.finalize_in_order {
            let leaf = self
                .leaves
                .get(&action.guard_flag)
                .cloned()
                .ok_or_else(|| {
                    invariant(format!(
                        "finalizer references unknown guard {}",
                        action.guard_flag.0
                    ))
                })?;
            if leaf.place != action.source || leaf.lifecycle != action.lifecycle_id {
                return Err(invariant(format!(
                    "finalizer guard {} disagrees with its cleanup leaf",
                    action.guard_flag.0
                )));
            }
            let binding_import = self
                .lifecycle_bindings
                .get(&action.lifecycle_id)
                .cloned()
                .ok_or_else(|| {
                    invariant(format!(
                        "lifecycle `{}` was not preflighted",
                        action.lifecycle_id
                    ))
                })?;
            if !self.live.remove(&action.guard_flag) {
                continue;
            }
            self.emit(TraceEventKind::FinalizeBegin {
                source: action.source.clone(),
                lifecycle_id: action.lifecycle_id.clone(),
                guard_flag: action.guard_flag,
                binding_import: binding_import.clone(),
            });
            if let Some(import_id) = binding_import.clone() {
                self.emit(TraceEventKind::ImportBegin {
                    site: ImportSite::Finalizer {
                        source: action.source.clone(),
                        lifecycle_id: action.lifecycle_id.clone(),
                    },
                    import_id: import_id.clone(),
                });
                self.emit(TraceEventKind::FinalizerImportEnd {
                    source: action.source.clone(),
                    lifecycle_id: action.lifecycle_id.clone(),
                    import_id,
                });
            }
            self.emit(TraceEventKind::FinalizeEnd {
                source: action.source.clone(),
                lifecycle_id: action.lifecycle_id.clone(),
                guard_flag: action.guard_flag,
                binding_import,
            });
        }
        Ok(())
    }

    fn commit_result(
        &mut self,
        source: CleanupResultSource,
    ) -> Result<TraceOutcome, CleanupExecutionError> {
        if self.result_slot != ResultSlotState::Uninitialized {
            return Err(invariant("caller result slot is already published"));
        }
        if self.selected.is_some() {
            return Err(invariant("result publication follows failure selection"));
        }
        if expression_has_try(&self.function.body) {
            let staged = self
                .staged_copy_result
                .take()
                .ok_or_else(|| invariant("Copy Result commit has no staged producer"))?;
            validate_staged_copy_source(self.program, self.function, &staged)?;
            return Err(CleanupExecutionError::UnsupportedResultType(
                self.function.return_type.identity_key(),
            ));
        }
        if self.staged_copy_result.is_some() {
            return Err(invariant(
                "non-try result publication carries a staged Copy result",
            ));
        }
        let result = self
            .scenario
            .result
            .as_ref()
            .ok_or_else(|| invariant("result commit has no supplied result"))?;
        self.validate_result(&source, result)?;
        let published_flags = match &source {
            CleanupResultSource::Scalar { .. } => {
                if !self.live.is_empty() {
                    return Err(invariant(
                        "result publication occurs before non-result cleanup",
                    ));
                }
                BTreeSet::new()
            }
            CleanupResultSource::Owned { storage } => {
                let flags = self.flags_under(storage)?;
                if flags.iter().any(|flag| !self.live.contains(flag)) {
                    return Err(invariant("owned result is incomplete at publication"));
                }
                if self.live != flags {
                    return Err(invariant(
                        "result publication occurs before non-result cleanup",
                    ));
                }
                flags
            }
        };

        let result = self
            .scenario
            .result
            .take()
            .expect("validated result observation remains present");
        self.live.retain(|flag| !published_flags.contains(flag));
        self.result_slot = ResultSlotState::Published;
        self.emit(TraceEventKind::ResultCommit { source });
        Ok(TraceOutcome::Success { result })
    }

    fn validate_result(
        &self,
        source: &CleanupResultSource,
        result: &TraceResult,
    ) -> Result<(), CleanupExecutionError> {
        let matches_type = match (&self.function.return_type, result) {
            (ResolvedType::Unit, _) => false,
            (ResolvedType::I64, TraceResult::I64(_))
            | (ResolvedType::I32, TraceResult::Int32(_))
            | (ResolvedType::Bool, TraceResult::Bool(_))
            | (ResolvedType::Char, TraceResult::Char(_))
            | (ResolvedType::U8, TraceResult::Uint8(_))
            | (ResolvedType::Usize, TraceResult::Usize(_))
            | (ResolvedType::F32, TraceResult::F32(_))
            | (ResolvedType::F64, TraceResult::F64(_)) => true,
            (ResolvedType::Bytes, TraceResult::Bytes) => true,
            (ResolvedType::Nominal { declaration, .. }, TraceResult::Owned { type_id }) => {
                declaration == type_id
            }
            (ResolvedType::TypeParameter { .. }, _) | (_, _) => false,
        };
        let source_matches = match (source, &self.function.return_type) {
            (
                CleanupResultSource::Scalar { .. },
                ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::ArrayU8(_)
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool,
            ) => true,
            (CleanupResultSource::Owned { storage }, ResolvedType::Nominal { .. }) => {
                storage.storage == StorageId::ProvisionalResult && storage.projections.is_empty()
            }
            (CleanupResultSource::Owned { storage }, ResolvedType::Bytes) => {
                storage.storage == StorageId::ProvisionalResult && storage.projections.is_empty()
            }
            (CleanupResultSource::Scalar { .. }, ResolvedType::Nominal { .. })
            | (CleanupResultSource::Scalar { .. }, ResolvedType::Unit)
            | (CleanupResultSource::Scalar { .. }, ResolvedType::String)
            | (CleanupResultSource::Scalar { .. }, ResolvedType::Bytes)
            | (CleanupResultSource::Scalar { .. }, ResolvedType::Str)
            | (CleanupResultSource::Scalar { .. }, ResolvedType::SliceU8)
            | (CleanupResultSource::Owned { .. }, ResolvedType::Unit)
            | (
                CleanupResultSource::Owned { .. },
                ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::ArrayU8(_)
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool
                | ResolvedType::String
                | ResolvedType::Str
                | ResolvedType::SliceU8,
            )
            | (_, ResolvedType::TypeParameter { .. }) => false,
        };
        if !matches_type || !source_matches {
            return Err(invariant(
                "supplied trace result disagrees with function result",
            ));
        }
        Ok(())
    }

    fn return_failure(
        &mut self,
        source: StatusSourceId,
    ) -> Result<TraceOutcome, CleanupExecutionError> {
        if self.result_slot != ResultSlotState::Uninitialized {
            return Err(invariant("failure return follows result publication"));
        }
        let selected = self
            .selected
            .as_ref()
            .ok_or_else(|| invariant("failure return has no selected status"))?;
        if selected.source != source {
            return Err(invariant("failure return changes the selected source"));
        }
        let status = self.status_arena.resolve(selected.token)?.clone();
        Ok(TraceOutcome::Failure {
            selected_source: source,
            status,
        })
    }

    fn return_unit(&mut self) -> Result<TraceOutcome, CleanupExecutionError> {
        Err(invariant(
            "ReturnUnit is invalid for source functions without a unit return type",
        ))
    }

    fn initialize_flags(
        &mut self,
        place: &CleanupPlace,
        operation: &str,
    ) -> Result<(), CleanupExecutionError> {
        let flags = self.flags_under(place)?;
        if flags.iter().any(|flag| self.live.contains(flag)) {
            return Err(invariant(format!(
                "{operation} targets a live cleanup place"
            )));
        }
        self.live.extend(flags);
        Ok(())
    }

    fn transfer_flags(
        &mut self,
        source: &CleanupPlace,
        destination: &CleanupPlace,
    ) -> Result<(), CleanupExecutionError> {
        let source_flags = self.flags_under(source)?;
        let destination_flags = self.flags_under(destination)?;
        if source_flags.len() != destination_flags.len() {
            return Err(invariant("cleanup transfer has unequal leaf counts"));
        }
        if source_flags.iter().any(|flag| !self.live.contains(flag)) {
            return Err(invariant("cleanup transfer reads a dead source"));
        }
        if destination_flags
            .iter()
            .any(|flag| self.live.contains(flag))
        {
            return Err(invariant("cleanup transfer initializes a live destination"));
        }
        self.live.retain(|flag| !source_flags.contains(flag));
        self.live.extend(destination_flags);
        Ok(())
    }

    fn transfer_variant_flags(
        &mut self,
        source: &CleanupPlace,
        destination: &CleanupPlace,
    ) -> Result<(), CleanupExecutionError> {
        let source_flags = self.flags_under(source)?;
        let destination_flags = self.flags_under(destination)?;
        let active = source_flags
            .iter()
            .filter(|flag| self.live.contains(flag))
            .copied()
            .collect::<Vec<_>>();
        // A selected case may contain only Copy fields (for example
        // `Option<Bytes>::None`); its authenticated tag carries no cleanup
        // flag to move.
        let mut mapped = Vec::with_capacity(active.len());
        for flag in &active {
            let source_leaf = &self.leaves[flag].place;
            let relative = source_leaf
                .projections
                .strip_prefix(source.projections.as_slice())
                .ok_or_else(|| invariant("variant transfer source prefix is invalid"))?;
            let expected = destination
                .projections
                .iter()
                .chain(relative)
                .cloned()
                .collect::<Vec<_>>();
            let destination_flag = destination_flags
                .iter()
                .find(|candidate| self.leaves[candidate].place.projections == expected)
                .copied()
                .ok_or_else(|| invariant("variant transfer destination shape differs"))?;
            if self.live.contains(&destination_flag) {
                return Err(invariant("variant transfer initializes a live destination"));
            }
            mapped.push(destination_flag);
        }
        self.live.retain(|flag| !active.contains(flag));
        self.live.extend(mapped);
        Ok(())
    }

    fn flags_under(
        &self,
        place: &CleanupPlace,
    ) -> Result<BTreeSet<LivenessFlagId>, CleanupExecutionError> {
        let flags = self
            .leaves
            .iter()
            .filter_map(|(flag, leaf)| {
                (leaf.place.storage == place.storage
                    && leaf.place.projections.starts_with(&place.projections))
                .then_some(*flag)
            })
            .collect::<BTreeSet<_>>();
        if flags.is_empty() {
            return Err(invariant(format!(
                "cleanup place `{place:?}` has no liveness flags"
            )));
        }
        Ok(flags)
    }

    fn emit(&mut self, event: TraceEventKind) {
        self.events.push(TraceEvent {
            function: self.function.id.clone(),
            invocation: InvocationPath::default(),
            event,
        });
    }

    fn finish(
        self,
        outcome: TraceOutcome,
        terminal: TerminalKind,
    ) -> Result<ConformanceTrace, CleanupExecutionError> {
        let unused_booleans = self
            .scenario
            .booleans
            .keys()
            .filter(|expression| !self.used_booleans.contains(*expression))
            .cloned()
            .collect::<Vec<_>>();
        if !unused_booleans.is_empty() {
            return Err(CleanupExecutionError::UnusedBooleanDecisions(
                unused_booleans,
            ));
        }
        let unused_variant_cases = self
            .scenario
            .variant_cases
            .keys()
            .filter(|expression| !self.used_variant_cases.contains(*expression))
            .cloned()
            .collect::<Vec<_>>();
        if !unused_variant_cases.is_empty() {
            return Err(CleanupExecutionError::UnusedVariantDecisions(
                unused_variant_cases,
            ));
        }
        let unused_operations = self
            .scenario
            .operations
            .keys()
            .filter(|source| !self.used_operations.contains(*source))
            .cloned()
            .collect::<Vec<_>>();
        if !unused_operations.is_empty() {
            return Err(CleanupExecutionError::UnusedOperationOutcomes(
                unused_operations,
            ));
        }
        if self.scenario.result.is_some() {
            return Err(invariant("cleanup scenario supplied an unused result"));
        }
        if !self.live.is_empty() {
            return Err(invariant(format!(
                "terminal cleanup state retains live flags {:?}",
                self.live
            )));
        }
        let terminal_state_is_valid = matches!(
            (&outcome, terminal, self.result_slot),
            (
                TraceOutcome::Success { .. },
                TerminalKind::CommitResult,
                ResultSlotState::Published,
            ) | (
                TraceOutcome::Success {
                    result: TraceResult::Unit,
                },
                TerminalKind::ReturnUnit,
                ResultSlotState::Uninitialized,
            ) | (
                TraceOutcome::Failure { .. },
                TerminalKind::ReturnFailure,
                ResultSlotState::Uninitialized,
            )
        );
        if !terminal_state_is_valid {
            return Err(invariant(
                "terminal outcome disagrees with caller result-slot publication",
            ));
        }
        Ok(ConformanceTrace::new(
            self.scenario.scenario_id,
            self.function.id.clone(),
            self.events,
            outcome,
        ))
    }
}

fn validate_propagated_status(
    callee: &DeclarationId,
    status: &NormalizedStatus,
) -> Result<(), CleanupExecutionError> {
    if callee.as_str() == crate::byte_ops::RANGE_ID {
        if status.domain_id() != crate::byte_ops::RANGE_STATUS_DOMAIN
            || ![
                crate::byte_ops::RANGE_START_AFTER_END_CODE,
                crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
            ]
            .contains(&status.code())
            || status.class() != StatusClass::Adapter
            || status.retryability() != Retryability::Known(false)
        {
            return Err(invariant(
                "byte range supplied a status outside its exact normalized failure domain",
            ));
        }
        return Ok(());
    }
    let Some(operation) = crate::command_io_ops::by_id(callee.as_str()) else {
        // Authored and other target-neutral calls retain their existing
        // normalized-status contract.
        return Ok(());
    };
    let metadata = crate::command_io_ops::status_metadata(operation).ok_or_else(|| {
        invariant(format!(
            "infallible command operation `{callee}` supplied a propagated status"
        ))
    })?;
    if status.domain_id() != metadata.domain
        || !metadata.codes.contains(&status.code())
        || status.class() != StatusClass::Adapter
        || status.retryability() != Retryability::Known(false)
    {
        return Err(invariant(format!(
            "command operation `{callee}` supplied a status outside its exact normalized failure domain"
        )));
    }
    Ok(())
}

fn preflight_finalizer_bindings(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    scenario: &CleanupScenario,
) -> Result<BTreeMap<DeclarationId, Option<DeclarationId>>, CleanupExecutionError> {
    let known_imports = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .map(|import| import.id.clone())
        .collect::<BTreeSet<_>>();
    if let Some(unknown) = scenario
        .available_finalizer_imports
        .iter()
        .find(|import| !known_imports.contains(*import))
    {
        return Err(CleanupExecutionError::UnknownFinalizerBinding(
            unknown.clone(),
        ));
    }

    let mut bindings = BTreeMap::new();
    for action in function
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
    {
        if bindings.contains_key(&action.lifecycle_id) {
            continue;
        }
        let binding = resolve_lifecycle_binding(program, &action.lifecycle_id)?;
        if let Some(import) = &binding {
            if !scenario.available_finalizer_imports.contains(import) {
                return Err(CleanupExecutionError::MissingFinalizerBinding(
                    import.clone(),
                ));
            }
        }
        bindings.insert(action.lifecycle_id.clone(), binding);
    }
    Ok(bindings)
}

fn resolve_lifecycle_binding(
    program: &ResolvedProgram,
    lifecycle: &DeclarationId,
) -> Result<Option<DeclarationId>, CleanupExecutionError> {
    if lifecycle.as_str() == crate::cleanup::BYTES_DROP_LIFECYCLE_ID {
        return Ok(None);
    }
    let mut binding = None;
    for declaration in &program.types {
        let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
            continue;
        };
        if drop.id != *lifecycle {
            continue;
        }
        if binding.is_some() {
            return Err(invariant(format!(
                "lifecycle `{lifecycle}` resolves more than once"
            )));
        }
        binding = Some(match &drop.kind {
            ResolvedResourceDropKind::Trivial => None,
            ResolvedResourceDropKind::Imported { import, .. } => Some(import.clone()),
        });
    }
    binding.ok_or_else(|| invariant(format!("unknown lifecycle `{lifecycle}`")))
}

fn collect_leaves(
    storage: &StorageId,
    projections: &mut Vec<DeclarationId>,
    shape: &FieldLivenessShape,
    leaves: &mut BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), CleanupExecutionError> {
    match shape {
        FieldLivenessShape::NoDrop => {}
        FieldLivenessShape::Leaf { flag, lifecycle } => {
            let leaf = Leaf {
                place: CleanupPlace {
                    storage: storage.clone(),
                    projections: projections.clone(),
                },
                lifecycle: lifecycle.clone(),
            };
            if leaves.insert(*flag, leaf).is_some() {
                return Err(invariant(format!(
                    "cleanup flag {} is declared more than once",
                    flag.0
                )));
            }
        }
        FieldLivenessShape::Record { fields, .. } => {
            for field in fields {
                projections.push(field.field.clone());
                collect_leaves(storage, projections, &field.shape, leaves)?;
                projections.pop();
            }
        }
        FieldLivenessShape::Variant { cases, .. } => {
            for case in cases {
                projections.push(case.case.clone());
                for field in &case.fields {
                    projections.push(field.field.clone());
                    collect_leaves(storage, projections, &field.shape, leaves)?;
                    projections.pop();
                }
                projections.pop();
            }
        }
    }
    Ok(())
}

fn invariant(detail: impl Into<String>) -> CleanupExecutionError {
    CleanupExecutionError::HarnessInvariant(detail.into())
}

#[cfg(test)]
#[path = "execute/tests.rs"]
mod tests;
