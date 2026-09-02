//! Test-only conversion from the target-independent native trace wire format
//! into the compiler's typed conformance protocol.
//!
//! This boundary never manufactures semantic identities from target text. It
//! only clones identities already present in validated HIR or its attached,
//! replay-validated cleanup plan. Compiler-owned statuses are rebuilt from
//! their cleanup-plan producer and then compared field-for-field with the wire
//! value.

use std::error::Error;
use std::fmt;

use crate::cleanup::LivenessFlagId;
use crate::cleanup_plan::{
    CleanupPlace, CleanupResultSource, CleanupTransition, ContractPhase, ExitContinuation,
    StatusLane, StatusProducer, StatusSourceId, StorageId,
};
use crate::conformance::{
    ConformanceTrace, InvocationPath, NormalizedStatus, Retryability, StatusClass, TraceEvent,
    TraceEventKind, TraceOutcome, TraceResult,
};
use crate::hir::{
    DeclarationId, ResolvedFunction, ResolvedProgram, ResolvedResourceDropKind, ResolvedType,
};

use super::native_conformance_wire::{
    WireEvent, WireEventKind, WireOutcome, WirePlace, WireResult, WireResultSource,
    WireRetryability, WireStatus, WireStatusClass, WireStatusLane, WireStatusSource, WireStorage,
    WireTrace,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MaterializeError {
    SelectedFunctionNotInProgram,
    RootFunctionMismatch,
    EventFunctionMismatch { event_index: usize },
    NonRootInvocation { event_index: usize },
    UnsupportedCallArgumentStorage,
    UnsupportedProjection,
    UnknownTransfer,
    UnknownStatusSource,
    UnsupportedStatusProducer,
    StatusMismatch,
    UnknownFinalizer,
    UnknownResultSource,
    ResultTypeMismatch,
    FailureSourceIsNotTerminal,
}

impl fmt::Display for MaterializeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SelectedFunctionNotInProgram => {
                formatter.write_str("selected function is not the validated program function")
            }
            Self::RootFunctionMismatch => {
                formatter.write_str("wire root function does not match the selected function")
            }
            Self::EventFunctionMismatch { event_index } => write!(
                formatter,
                "wire event {event_index} does not belong to the selected function"
            ),
            Self::NonRootInvocation { event_index } => write!(
                formatter,
                "wire event {event_index} uses a non-root invocation path"
            ),
            Self::UnsupportedCallArgumentStorage => formatter
                .write_str("call-argument trace storage is outside the native root-frame slice"),
            Self::UnsupportedProjection => formatter
                .write_str("projected trace places are outside the native root-frame slice"),
            Self::UnknownTransfer => {
                formatter.write_str("wire transfer is absent from the selected cleanup plan")
            }
            Self::UnknownStatusSource => {
                formatter.write_str("wire status source is absent from the selected cleanup plan")
            }
            Self::UnsupportedStatusProducer => formatter.write_str(
                "propagated-call statuses are outside the native root-frame materializer slice",
            ),
            Self::StatusMismatch => formatter.write_str(
                "wire status does not exactly match its compiler-owned cleanup-plan status",
            ),
            Self::UnknownFinalizer => formatter
                .write_str("wire finalizer is absent from the selected cleanup plan or lifecycle"),
            Self::UnknownResultSource => formatter
                .write_str("wire result source is absent from a selected cleanup-plan exit"),
            Self::ResultTypeMismatch => {
                formatter.write_str("wire result does not match the selected function result type")
            }
            Self::FailureSourceIsNotTerminal => formatter.write_str(
                "wire failure source is not a failure-return source in the selected cleanup plan",
            ),
        }
    }
}

impl Error for MaterializeError {}

/// Bind a decoded native trace to validated semantic identities.
///
/// `function` must be the exact validated function value held by `program`.
/// The current native trace emitter is a single-frame slice, so invocation
/// paths, call-argument epochs, and field projections are rejected here even
/// when a future cleanup plan could describe them.
pub(super) fn materialize(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    wire: WireTrace,
) -> Result<ConformanceTrace, MaterializeError> {
    let selected = program
        .functions
        .iter()
        .find(|candidate| candidate.id == function.id)
        .filter(|candidate| *candidate == function)
        .ok_or(MaterializeError::SelectedFunctionNotInProgram)?;
    if wire.root_function_id != selected.id.as_str() {
        return Err(MaterializeError::RootFunctionMismatch);
    }

    let mut events = Vec::new();
    events
        .try_reserve_exact(wire.events.len())
        .map_err(|_| MaterializeError::UnknownTransfer)?;
    for (event_index, event) in wire.events.into_iter().enumerate() {
        events.push(materialize_event(program, selected, event, event_index)?);
    }
    let outcome = materialize_outcome(program, selected, wire.outcome)?;

    Ok(ConformanceTrace::new(
        wire.scenario_id,
        selected.id.clone(),
        events,
        outcome,
    ))
}

fn materialize_event(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    wire: WireEvent,
    event_index: usize,
) -> Result<TraceEvent, MaterializeError> {
    if wire.function_id != function.id.as_str() {
        return Err(MaterializeError::EventFunctionMismatch { event_index });
    }
    if !wire.invocation.is_empty() {
        return Err(MaterializeError::NonRootInvocation { event_index });
    }

    let event = match wire.kind {
        WireEventKind::Transfer {
            at,
            source,
            destination,
        } => {
            reject_unsupported_place(&source)?;
            reject_unsupported_place(&destination)?;
            let (at, source, destination) = function
                .cleanup_plan
                .blocks
                .iter()
                .flat_map(|block| &block.transitions)
                .find_map(|transition| match transition {
                    CleanupTransition::Transfer {
                        at: candidate_at,
                        source: candidate_source,
                        destination: candidate_destination,
                    } if candidate_at.as_str() == at
                        && wire_place_matches(&source, candidate_source)
                        && wire_place_matches(&destination, candidate_destination) =>
                    {
                        Some((
                            candidate_at.clone(),
                            candidate_source.clone(),
                            candidate_destination.clone(),
                        ))
                    }
                    _ => None,
                })
                .ok_or(MaterializeError::UnknownTransfer)?;
            TraceEventKind::Transfer {
                at,
                source,
                destination,
            }
        }
        WireEventKind::SelectFailure { source, status } => {
            let source = intern_status_source(function, &source)?;
            let status = materialize_status(function, &source, &status)?;
            TraceEventKind::SelectFailure { source, status }
        }
        WireEventKind::FinalizeBegin {
            source,
            lifecycle_id,
            guard_flag,
            binding_import_id,
        } => {
            let (source, lifecycle_id, guard_flag, binding_import) = intern_finalizer(
                program,
                function,
                &source,
                &lifecycle_id,
                guard_flag,
                binding_import_id.as_deref(),
            )?;
            TraceEventKind::FinalizeBegin {
                source,
                lifecycle_id,
                guard_flag,
                binding_import,
            }
        }
        WireEventKind::FinalizeEnd {
            source,
            lifecycle_id,
            guard_flag,
            binding_import_id,
        } => {
            let (source, lifecycle_id, guard_flag, binding_import) = intern_finalizer(
                program,
                function,
                &source,
                &lifecycle_id,
                guard_flag,
                binding_import_id.as_deref(),
            )?;
            TraceEventKind::FinalizeEnd {
                source,
                lifecycle_id,
                guard_flag,
                binding_import,
            }
        }
        WireEventKind::ResultCommit { source } => TraceEventKind::ResultCommit {
            source: intern_result_source(function, &source)?,
        },
    };

    Ok(TraceEvent {
        function: function.id.clone(),
        invocation: InvocationPath::default(),
        event,
    })
}

fn materialize_outcome(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    wire: WireOutcome,
) -> Result<TraceOutcome, MaterializeError> {
    match wire {
        WireOutcome::Success(result) => Ok(TraceOutcome::Success {
            result: materialize_result(program, function, result)?,
        }),
        WireOutcome::Failure {
            selected_source,
            status,
        } => {
            let source = intern_status_source(function, &selected_source)?;
            if !function.cleanup_plan.exits.iter().any(|exit| {
                matches!(
                    &exit.continuation,
                    ExitContinuation::ReturnFailure { source: candidate } if *candidate == source
                )
            }) {
                return Err(MaterializeError::FailureSourceIsNotTerminal);
            }
            let status = materialize_status(function, &source, &status)?;
            Ok(TraceOutcome::Failure {
                selected_source: source,
                status,
            })
        }
    }
}

fn materialize_result(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    wire: WireResult,
) -> Result<TraceResult, MaterializeError> {
    match (&function.return_type, wire) {
        (ResolvedType::I64, WireResult::I64(value)) => Ok(TraceResult::I64(value)),
        (ResolvedType::I32, WireResult::I64(value)) => i32::try_from(value)
            .map(TraceResult::Int32)
            .map_err(|_| MaterializeError::ResultTypeMismatch),
        (ResolvedType::Char, WireResult::I64(value)) => u32::try_from(value)
            .map(TraceResult::Char)
            .map_err(|_| MaterializeError::ResultTypeMismatch),
        (ResolvedType::U8, WireResult::I64(value)) => u8::try_from(value)
            .map(TraceResult::Uint8)
            .map_err(|_| MaterializeError::ResultTypeMismatch),
        (ResolvedType::Bool, WireResult::Bool(value)) => Ok(TraceResult::Bool(value)),
        (
            ResolvedType::Nominal {
                declaration,
                arguments,
            },
            WireResult::Owned { type_id },
        ) if arguments.is_empty()
            && declaration.as_str() == type_id
            && program.types.iter().any(|item| {
                item.id == *declaration
                    && matches!(
                        item.kind,
                        crate::hir::ResolvedTypeDeclarationKind::Resource { .. }
                    )
            }) =>
        {
            Ok(TraceResult::Owned {
                type_id: declaration.clone(),
            })
        }
        (_, WireResult::Unit)
        | (ResolvedType::Unit, _)
        | (ResolvedType::I64, _)
        | (ResolvedType::Char, WireResult::Bool(_))
        | (ResolvedType::Char, WireResult::Owned { .. })
        | (ResolvedType::I32, WireResult::Bool(_))
        | (ResolvedType::I32, WireResult::Owned { .. })
        | (ResolvedType::U8, WireResult::Bool(_))
        | (ResolvedType::U8, WireResult::Owned { .. })
        | (ResolvedType::Usize, _)
        | (ResolvedType::F32, _)
        | (ResolvedType::F64, _)
        | (ResolvedType::Bool, _)
        | (ResolvedType::String, _)
        | (ResolvedType::Str, _)
        | (ResolvedType::SliceU8, _)
        | (ResolvedType::ArrayU8(_), _)
        | (ResolvedType::Bytes, _)
        | (ResolvedType::TypeParameter { .. }, _)
        | (ResolvedType::Nominal { .. }, _) => Err(MaterializeError::ResultTypeMismatch),
    }
}

fn intern_status_source(
    function: &ResolvedFunction,
    wire: &WireStatusSource,
) -> Result<StatusSourceId, MaterializeError> {
    function
        .cleanup_plan
        .status_sources
        .iter()
        .find(|candidate| {
            candidate.id.expression.as_str() == wire.expression_id
                && status_lane_matches(candidate.id.lane, wire.lane)
        })
        .map(|candidate| candidate.id.clone())
        .ok_or(MaterializeError::UnknownStatusSource)
}

fn materialize_status(
    function: &ResolvedFunction,
    source: &StatusSourceId,
    wire: &WireStatus,
) -> Result<NormalizedStatus, MaterializeError> {
    let producer = &function
        .cleanup_plan
        .status_sources
        .iter()
        .find(|candidate| candidate.id == *source)
        .ok_or(MaterializeError::UnknownStatusSource)?
        .producer;
    match producer {
        StatusProducer::ContractFalse { phase, .. } => {
            require_exact_status(wire, NormalizedStatus::contract(*phase))
        }
        StatusProducer::CheckedArithmetic {
            normalized_cases, ..
        } => normalized_cases
            .iter()
            .map(|case| NormalizedStatus::arithmetic(*case))
            .find(|candidate| wire_status_matches(wire, candidate))
            .ok_or(MaterializeError::StatusMismatch),
        StatusProducer::PropagatedCall { .. } => Err(MaterializeError::UnsupportedStatusProducer),
    }
}

fn require_exact_status(
    wire: &WireStatus,
    expected: NormalizedStatus,
) -> Result<NormalizedStatus, MaterializeError> {
    wire_status_matches(wire, &expected)
        .then_some(expected)
        .ok_or(MaterializeError::StatusMismatch)
}

fn wire_status_matches(wire: &WireStatus, expected: &NormalizedStatus) -> bool {
    wire.schema == expected.schema()
        && wire.domain_id == expected.domain_id()
        && wire.code == expected.code()
        && status_class_matches(expected.class(), wire.class)
        && retryability_matches(expected.retryability(), wire.retryability)
}

fn intern_finalizer(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    wire_source: &WirePlace,
    wire_lifecycle: &str,
    wire_guard: u32,
    wire_binding: Option<&str>,
) -> Result<
    (
        CleanupPlace,
        DeclarationId,
        LivenessFlagId,
        Option<DeclarationId>,
    ),
    MaterializeError,
> {
    reject_unsupported_place(wire_source)?;
    function
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
        .find_map(|candidate| {
            let binding = lifecycle_binding(program, &candidate.lifecycle_id)?;
            (candidate.lifecycle_id.as_str() == wire_lifecycle
                && candidate.guard_flag.0 == wire_guard
                && wire_place_matches(wire_source, &candidate.source)
                && binding.as_ref().map(DeclarationId::as_str) == wire_binding)
                .then(|| {
                    (
                        candidate.source.clone(),
                        candidate.lifecycle_id.clone(),
                        candidate.guard_flag,
                        binding,
                    )
                })
        })
        .ok_or(MaterializeError::UnknownFinalizer)
}

fn lifecycle_binding(
    program: &ResolvedProgram,
    lifecycle: &DeclarationId,
) -> Option<Option<DeclarationId>> {
    program.types.iter().find_map(|declaration| {
        let crate::hir::ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
            return None;
        };
        if drop.id != *lifecycle {
            return None;
        }
        Some(match &drop.kind {
            ResolvedResourceDropKind::Trivial => None,
            ResolvedResourceDropKind::Imported { import, .. } => Some(import.clone()),
        })
    })
}

fn intern_result_source(
    function: &ResolvedFunction,
    wire: &WireResultSource,
) -> Result<CleanupResultSource, MaterializeError> {
    if let WireResultSource::Owned { storage } = wire {
        reject_unsupported_place(storage)?;
    }
    function
        .cleanup_plan
        .exits
        .iter()
        .find_map(|exit| match &exit.continuation {
            ExitContinuation::CommitResult { source }
                if wire_result_source_matches(wire, source) =>
            {
                Some(source.clone())
            }
            _ => None,
        })
        .ok_or(MaterializeError::UnknownResultSource)
}

fn reject_unsupported_place(wire: &WirePlace) -> Result<(), MaterializeError> {
    if matches!(wire.storage, WireStorage::CallArgument { .. }) {
        return Err(MaterializeError::UnsupportedCallArgumentStorage);
    }
    if !wire.projections.is_empty() {
        return Err(MaterializeError::UnsupportedProjection);
    }
    Ok(())
}

fn wire_result_source_matches(wire: &WireResultSource, candidate: &CleanupResultSource) -> bool {
    match (wire, candidate) {
        (
            WireResultSource::Scalar { expression_id },
            CleanupResultSource::Scalar { expression },
        ) => expression_id == expression.as_str(),
        (
            WireResultSource::Owned { storage },
            CleanupResultSource::Owned { storage: candidate },
        ) => wire_place_matches(storage, candidate),
        _ => false,
    }
}

fn wire_place_matches(wire: &WirePlace, candidate: &CleanupPlace) -> bool {
    wire.projections
        .iter()
        .map(String::as_str)
        .eq(candidate.projections.iter().map(DeclarationId::as_str))
        && match (&wire.storage, &candidate.storage) {
            (WireStorage::Value { value_id }, StorageId::Value(candidate)) => {
                value_id == candidate.as_str()
            }
            (WireStorage::Temporary { expression_id }, StorageId::Temporary(candidate)) => {
                expression_id == candidate.as_str()
            }
            (
                WireStorage::CallArgument {
                    call_id,
                    parameter_index,
                    value_expression_id,
                },
                StorageId::CallArgument {
                    call,
                    parameter_index: candidate_parameter,
                    value_expression,
                },
            ) => {
                call_id == call.as_str()
                    && parameter_index == candidate_parameter
                    && value_expression_id == value_expression.as_str()
            }
            (WireStorage::ProvisionalResult, StorageId::ProvisionalResult) => true,
            _ => false,
        }
}

const fn status_lane_matches(candidate: StatusLane, wire: WireStatusLane) -> bool {
    matches!(
        (candidate, wire),
        (
            StatusLane::OperationFailure,
            WireStatusLane::OperationFailure
        ) | (StatusLane::ContractFalse, WireStatusLane::ContractFalse)
    )
}

const fn status_class_matches(candidate: StatusClass, wire: WireStatusClass) -> bool {
    matches!(
        (candidate, wire),
        (StatusClass::Contract, WireStatusClass::Contract)
            | (StatusClass::Arithmetic, WireStatusClass::Arithmetic)
            | (StatusClass::Import, WireStatusClass::Import)
            | (StatusClass::ExplicitClose, WireStatusClass::ExplicitClose)
            | (StatusClass::Adapter, WireStatusClass::Adapter)
    )
}

const fn retryability_matches(candidate: Retryability, wire: WireRetryability) -> bool {
    matches!(
        (candidate, wire),
        (Retryability::Unknown, WireRetryability::Unknown)
            | (Retryability::Known(false), WireRetryability::False)
            | (Retryability::Known(true), WireRetryability::True)
    )
}

#[cfg(test)]
#[path = "native_conformance_materialize/tests.rs"]
mod tests;
