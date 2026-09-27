//! Compiler-private backend parity evidence for the minimal source-level
//! resumable slice.
//!
//! This is not a runtime scheduler and does not reuse [`super::core`]: start
//! and resume are separate, synchronous target executions of the yield-free
//! HIR projections produced by [`super::lowering`]. A suspension is bound to
//! the exact checked plan and invocation argument bits. Resume replays the
//! request projection and compares exact scalar bits before it executes the
//! continuation projection.
//!
//! The facade deliberately makes no step/fuel accounting or cleanup-event
//! claim. The admitted lowering is Copy-scalar and cleanup-free. Core Wasm
//! additionally refuses `usize` with the scalar-profile diagnostic because
//! that target's public scalar ABI intentionally has no host-width carrier.
//! Signed zero is transported and compared by bits. Arbitrary NaN payload
//! preservation is not claimed by the Core Wasm runner because its existing
//! scalar package crosses JavaScript `Number`; a future raw-bit adapter would
//! be required to make that stronger claim.

use crate::cleanup_plan::{ContractPhase, StatusCase};
use crate::conformance::{
    NormalizedStatus, ARITHMETIC_STATUS_DOMAIN_V1, CONTRACT_STATUS_DOMAIN_V1,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedProgram, ResolvedType};
use crate::resumable_effects::lowering::{
    ResumableScalar, ResumableStateId, ResumableSuspensionBinding, SequentialResumablePlan,
};

mod native;
mod wasm;

#[cfg(test)]
mod tests;

const TYPE_MISMATCH: &str = "SPX-F113";
const SUSPENSION_DRIFT: &str = "SPX-F114";
const SUSPENSION_MISMATCH: &str = "SPX-F115";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Backend {
    NativeO0,
    NativeO2,
    CoreWasm,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendSuspension {
    state: ResumableStateId,
    request: ResumableScalar,
    binding: ResumableSuspensionBinding,
}

impl BackendSuspension {
    pub fn state(&self) -> &ResumableStateId {
        &self.state
    }

    pub fn request(&self) -> &ResumableScalar {
        &self.request
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackendYieldRecord {
    request: ResumableScalar,
    answer: ResumableScalar,
}

/// Opaque in-memory carrier for the private sequential parity lane. It has no
/// wire representation and grants no authority to answer the current request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendContinuation {
    state: ResumableStateId,
    request: ResumableScalar,
    binding: ResumableSuspensionBinding,
    history: Vec<BackendYieldRecord>,
}

impl BackendContinuation {
    pub fn state(&self) -> &ResumableStateId {
        &self.state
    }

    pub fn request(&self) -> &ResumableScalar {
        &self.request
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendStep {
    Suspended(BackendSuspension),
    SequentialSuspended(BackendContinuation),
    Complete {
        state: ResumableStateId,
        result: ResumableScalar,
    },
    LanguageFailure(NormalizedStatus),
}

enum TargetOutcome {
    Success(ResumableScalar),
    LanguageFailure(NormalizedStatus),
}

/// Execute the request projection on one real backend.
pub fn run(
    backend: Backend,
    program: &ResolvedProgram,
    plan: &SequentialResumablePlan,
    arguments: &[ResumableScalar],
) -> Result<BackendStep, Diagnostic> {
    validate_arguments(&plan.start.function, arguments)?;
    let first = plan
        .suspensions
        .first()
        .ok_or_else(|| suspension_mismatch("resumable plan has no suspension sites"))?;
    let projected = plan.start_program(program)?;
    let request = match execute(
        backend,
        &projected,
        plan.function_id.as_str(),
        arguments,
        &first.request_type,
    )? {
        TargetOutcome::Success(request) => request,
        TargetOutcome::LanguageFailure(status) => return Ok(BackendStep::LanguageFailure(status)),
    };
    let binding = plan.suspension_binding(arguments);
    if plan.suspensions.len() == 1 {
        Ok(BackendStep::Suspended(BackendSuspension {
            state: first.state.id.clone(),
            request,
            binding,
        }))
    } else {
        Ok(BackendStep::SequentialSuspended(BackendContinuation {
            state: first.state.id.clone(),
            request,
            binding,
            history: Vec::new(),
        }))
    }
}

/// Replay the request projection and, only after its exact bits agree with the
/// recorded suspension, execute the continuation projection.
pub fn resume(
    backend: Backend,
    program: &ResolvedProgram,
    plan: &SequentialResumablePlan,
    arguments: &[ResumableScalar],
    suspension: &BackendSuspension,
    answer: ResumableScalar,
) -> Result<BackendStep, Diagnostic> {
    validate_arguments(&plan.start.function, arguments)?;
    let first = plan
        .suspensions
        .first()
        .ok_or_else(|| suspension_mismatch("resumable plan has no suspension sites"))?;
    if plan.suspensions.len() != 1 {
        return Err(suspension_mismatch(
            "a multi-site plan must be resumed through its opaque sequential continuation",
        ));
    }
    if !scalar_matches_type(&answer, &first.response_type) {
        return Err(type_mismatch(format!(
            "resume value has type {}, but this suspension requires {}",
            scalar_type_text(&answer),
            resolved_type_text(&first.response_type),
        )));
    }
    if suspension.state != first.state.id
        || suspension.binding != plan.suspension_binding(arguments)
    {
        return Err(suspension_mismatch(
            "resumable suspension does not belong to this exact plan and invocation argument bits",
        ));
    }

    let start = plan.start_program(program)?;
    let replayed = match execute(
        backend,
        &start,
        plan.function_id.as_str(),
        arguments,
        &first.request_type,
    )? {
        TargetOutcome::Success(request) => request,
        TargetOutcome::LanguageFailure(status) => return Ok(BackendStep::LanguageFailure(status)),
    };
    if replayed != suspension.request {
        return Err(drift(format!(
            "resumable request changed during replay: recorded {}, replayed {}",
            scalar_debug(&suspension.request),
            scalar_debug(&replayed),
        )));
    }

    let resume = plan.resume_program_at(program, 0)?;
    let mut resumed_arguments = arguments.to_vec();
    resumed_arguments.push(answer);
    validate_arguments(&plan.resumes[0].function, &resumed_arguments)?;
    let result = match execute(
        backend,
        &resume,
        plan.function_id.as_str(),
        &resumed_arguments,
        &plan.resumes[0].function.return_type,
    )? {
        TargetOutcome::Success(result) => result,
        TargetOutcome::LanguageFailure(status) => return Ok(BackendStep::LanguageFailure(status)),
    };
    Ok(BackendStep::Complete {
        state: plan.complete.id.clone(),
        result,
    })
}

/// Replay every recorded request in order, consume one newly supplied answer,
/// and either park the next request or return the final result.
pub fn resume_sequential(
    backend: Backend,
    program: &ResolvedProgram,
    plan: &SequentialResumablePlan,
    arguments: &[ResumableScalar],
    continuation: &BackendContinuation,
    answer: ResumableScalar,
) -> Result<BackendStep, Diagnostic> {
    validate_arguments(&plan.start.function, arguments)?;
    let Some(index) = plan.suspension_index(&continuation.state) else {
        return Err(suspension_mismatch(
            "sequential continuation state does not belong to this exact checked plan",
        ));
    };
    if plan.suspensions.len() == 1 || continuation.history.len() != index {
        return Err(suspension_mismatch(
            "sequential continuation history length does not match its suspension state",
        ));
    }
    let prior_answers = continuation
        .history
        .iter()
        .map(|record| record.answer.clone())
        .collect::<Vec<_>>();
    let expected_binding = plan.suspension_binding_at(index, arguments, &prior_answers)?;
    if continuation.binding != expected_binding {
        return Err(suspension_mismatch(
            "sequential continuation does not match this exact plan, site, invocation, and answer history",
        ));
    }

    for (record_index, record) in continuation.history.iter().enumerate() {
        let site = &plan.suspensions[record_index];
        require_scalar_type(&record.request, &site.request_type, "historical request")?;
        require_scalar_type(&record.answer, &site.response_type, "historical answer")?;
    }
    let current = &plan.suspensions[index];
    require_scalar_type(&continuation.request, &current.request_type, "request")?;
    require_scalar_type(&answer, &current.response_type, "answer")?;

    let start = plan.start_program(program)?;
    let mut replayed = match execute(
        backend,
        &start,
        plan.function_id.as_str(),
        arguments,
        &plan.suspensions[0].request_type,
    )? {
        TargetOutcome::Success(request) => request,
        TargetOutcome::LanguageFailure(status) => return Ok(BackendStep::LanguageFailure(status)),
    };
    let expected_first = continuation
        .history
        .first()
        .map_or(&continuation.request, |record| &record.request);
    require_replayed_request(expected_first, &replayed)?;

    let mut resumed_arguments = arguments.to_vec();
    for (record_index, record) in continuation.history.iter().enumerate() {
        resumed_arguments.push(record.answer.clone());
        let projection = plan.resume_program_at(program, record_index)?;
        let next_site = &plan.suspensions[record_index + 1];
        replayed = match execute(
            backend,
            &projection,
            plan.function_id.as_str(),
            &resumed_arguments,
            &next_site.request_type,
        )? {
            TargetOutcome::Success(request) => request,
            TargetOutcome::LanguageFailure(status) => {
                return Ok(BackendStep::LanguageFailure(status));
            }
        };
        let expected = continuation
            .history
            .get(record_index + 1)
            .map_or(&continuation.request, |next| &next.request);
        require_replayed_request(expected, &replayed)?;
    }

    resumed_arguments.push(answer.clone());
    let projection = plan.resume_program_at(program, index)?;
    validate_arguments(&plan.resumes[index].function, &resumed_arguments)?;
    let value = match execute(
        backend,
        &projection,
        plan.function_id.as_str(),
        &resumed_arguments,
        &plan.resumes[index].function.return_type,
    )? {
        TargetOutcome::Success(value) => value,
        TargetOutcome::LanguageFailure(status) => {
            return Ok(BackendStep::LanguageFailure(status));
        }
    };

    let next_index = index + 1;
    if let Some(next) = plan.suspensions.get(next_index) {
        require_scalar_type(&value, &next.request_type, "next request")?;
        let mut history = continuation.history.clone();
        history.push(BackendYieldRecord {
            request: continuation.request.clone(),
            answer,
        });
        let prior_answers = history
            .iter()
            .map(|record| record.answer.clone())
            .collect::<Vec<_>>();
        Ok(BackendStep::SequentialSuspended(BackendContinuation {
            state: next.state.id.clone(),
            request: value,
            binding: plan.suspension_binding_at(next_index, arguments, &prior_answers)?,
            history,
        }))
    } else {
        Ok(BackendStep::Complete {
            state: plan.complete.id.clone(),
            result: value,
        })
    }
}

fn require_scalar_type(
    value: &ResumableScalar,
    expected: &ResolvedType,
    role: &str,
) -> Result<(), Diagnostic> {
    if scalar_matches_type(value, expected) {
        Ok(())
    } else {
        Err(type_mismatch(format!(
            "resumable {role} has type {}, but the declared type is {}",
            scalar_type_text(value),
            resolved_type_text(expected),
        )))
    }
}

fn require_replayed_request(
    recorded: &ResumableScalar,
    replayed: &ResumableScalar,
) -> Result<(), Diagnostic> {
    if recorded == replayed {
        Ok(())
    } else {
        Err(drift(format!(
            "resumable request changed during replay: recorded {}, replayed {}",
            scalar_debug(recorded),
            scalar_debug(replayed),
        )))
    }
}

fn execute(
    backend: Backend,
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ResumableScalar],
    result_type: &ResolvedType,
) -> Result<TargetOutcome, Diagnostic> {
    match backend {
        Backend::NativeO0 => native::execute(program, function_id, arguments, result_type, "-O0"),
        Backend::NativeO2 => native::execute(program, function_id, arguments, result_type, "-O2"),
        Backend::CoreWasm => wasm::execute(program, function_id, arguments, result_type),
    }
}

fn validate_arguments(
    function: &crate::hir::ResolvedFunction,
    arguments: &[ResumableScalar],
) -> Result<(), Diagnostic> {
    if function.params.len() != arguments.len() {
        return Err(type_mismatch(format!(
            "resumable invocation expected {} arguments but received {}",
            function.params.len(),
            arguments.len(),
        )));
    }
    for (index, (parameter, argument)) in function.params.iter().zip(arguments).enumerate() {
        if !scalar_matches_type(argument, &parameter.ty) {
            return Err(type_mismatch(format!(
                "resumable argument {index} has type {}, but parameter `{}` requires {}",
                scalar_type_text(argument),
                parameter.name,
                resolved_type_text(&parameter.ty),
            )));
        }
    }
    Ok(())
}

fn scalar_matches_type(value: &ResumableScalar, ty: &ResolvedType) -> bool {
    matches!(
        (value, ty),
        (ResumableScalar::I64(_), ResolvedType::I64)
            | (ResumableScalar::I32(_), ResolvedType::I32)
            | (ResumableScalar::U8(_), ResolvedType::U8)
            | (ResumableScalar::Usize(_), ResolvedType::Usize)
            | (ResumableScalar::Char(_), ResolvedType::Char)
            | (ResumableScalar::F32(_), ResolvedType::F32)
            | (ResumableScalar::F64(_), ResolvedType::F64)
            | (ResumableScalar::Bool(_), ResolvedType::Bool)
    )
}

fn scalar_type_text(value: &ResumableScalar) -> &'static str {
    match value {
        ResumableScalar::I64(_) => "i64",
        ResumableScalar::I32(_) => "i32",
        ResumableScalar::U8(_) => "u8",
        ResumableScalar::Usize(_) => "usize",
        ResumableScalar::Char(_) => "char",
        ResumableScalar::F32(_) => "f32",
        ResumableScalar::F64(_) => "f64",
        ResumableScalar::Bool(_) => "bool",
        // Issue #296 R20: this parity runner's own plan arguments stay
        // Copy-scalar always (an aggregate channel never widens a
        // function's parameters), so this is purely defensive labeling for
        // a diagnostic message, never a value this backend actually runs.
        ResumableScalar::Record(_) => "record",
        ResumableScalar::Variant { .. } => "variant",
    }
}

fn resolved_type_text(ty: &ResolvedType) -> &'static str {
    match ty {
        ResolvedType::I64 => "i64",
        ResolvedType::I32 => "i32",
        ResolvedType::U8 => "u8",
        ResolvedType::Usize => "usize",
        ResolvedType::Char => "char",
        ResolvedType::F32 => "f32",
        ResolvedType::F64 => "f64",
        ResolvedType::Bool => "bool",
        _ => "non-scalar",
    }
}

fn scalar_debug(value: &ResumableScalar) -> String {
    match value {
        ResumableScalar::I64(value) => format!("i64:{:016x}", *value as u64),
        ResumableScalar::I32(value) => format!("i32:{:08x}", *value as u32),
        ResumableScalar::U8(value) => format!("u8:{value:02x}"),
        ResumableScalar::Usize(value) => format!("usize:{value:016x}"),
        ResumableScalar::Char(value) => format!("char:{value:08x}"),
        ResumableScalar::F32(bits) => format!("f32:{bits:08x}"),
        ResumableScalar::F64(bits) => format!("f64:{bits:016x}"),
        ResumableScalar::Bool(value) => format!("bool:{}", u8::from(*value)),
        // Issue #296 R20: this parity runner never actually replays an
        // aggregate channel value (see `scalar_type_text`); this stays a
        // defensive, human-readable label rather than an assumed-total match.
        ResumableScalar::Record(fields) => format!("record:{}", fields.len()),
        ResumableScalar::Variant { case, fields } => format!("variant:{case}:{}", fields.len()),
    }
}

fn type_mismatch(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(TYPE_MISMATCH, message)
}

fn drift(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(SUSPENSION_DRIFT, message)
}

fn suspension_mismatch(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(SUSPENSION_MISMATCH, message)
}

fn backend_failure(code: &'static str, detail: impl Into<String>) -> Diagnostic {
    Diagnostic::io(code, detail)
}

fn normalized_status(domain: &str, code: u32) -> Result<NormalizedStatus, Diagnostic> {
    let status = match (domain, code) {
        (ARITHMETIC_STATUS_DOMAIN_V1, 1) => NormalizedStatus::arithmetic(StatusCase::AddOverflow),
        (ARITHMETIC_STATUS_DOMAIN_V1, 2) => NormalizedStatus::arithmetic(StatusCase::SubOverflow),
        (ARITHMETIC_STATUS_DOMAIN_V1, 3) => NormalizedStatus::arithmetic(StatusCase::MulOverflow),
        (ARITHMETIC_STATUS_DOMAIN_V1, 4) => {
            NormalizedStatus::arithmetic(StatusCase::DivisionByZero)
        }
        (ARITHMETIC_STATUS_DOMAIN_V1, 5) => {
            NormalizedStatus::arithmetic(StatusCase::DivisionOverflow)
        }
        (ARITHMETIC_STATUS_DOMAIN_V1, 6) => {
            NormalizedStatus::arithmetic(StatusCase::RemainderByZero)
        }
        (ARITHMETIC_STATUS_DOMAIN_V1, 7) => {
            NormalizedStatus::arithmetic(StatusCase::RemainderOverflow)
        }
        (ARITHMETIC_STATUS_DOMAIN_V1, 8) => {
            NormalizedStatus::arithmetic(StatusCase::NegationOverflow)
        }
        (CONTRACT_STATUS_DOMAIN_V1, 1) => NormalizedStatus::contract(ContractPhase::Requires),
        (CONTRACT_STATUS_DOMAIN_V1, 2) => NormalizedStatus::contract(ContractPhase::Ensures),
        _ => {
            return Err(backend_failure(
                "SPX-F105",
                format!("resumable target returned unknown normalized status {domain}:{code}"),
            ));
        }
    };
    Ok(status)
}
