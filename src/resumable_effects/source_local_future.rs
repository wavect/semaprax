//! Ephemeral Rust Future adapter for one checked source `yield`.
//!
//! The selected source prefix is pure and is evaluated before a host future is
//! created. The caller owns the executor and the handler's authority. Dropping
//! this future drops the pending host future; it does not undo host effects.
//! No waker or Rust future enters a source checkpoint or durable journal.

use crate::conformance::NormalizedStatus;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedType};
use crate::interpreter::resumable::{resume_resumable_effect, run_resumable_effect, ResumableStep};
use crate::interpreter::{ArgumentValue, MAX_STEPS_LIMIT};
use crate::project::{ProjectProfile, ProjectRevision};
use crate::resumable_effects::source_signature::{
    derive_source_effect_signature, SourceEffectSignature,
};
use std::future::Future;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};

const INVALID: &str = "SPX-H006";

fn invalid(message: &str) -> Vec<Diagnostic> {
    vec![profile_error(message)]
}

fn profile_error(message: &str) -> Diagnostic {
    Diagnostic::io(INVALID, message)
}

/// The exact compiler-owned one-site shape shared by Project Phase-A and the
/// interpreter adapter. This check grants no host or publication authority.
pub(crate) fn admitted_source_future_signature(
    program: &hir::ResolvedProgram,
    function_id: &str,
) -> Result<SourceEffectSignature, Diagnostic> {
    hir::validate(program)?;
    let signature = derive_source_effect_signature(program, function_id)?;
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == function_id)
        .ok_or_else(|| profile_error("local Future selected function is absent"))?;
    if signature.yield_count() != 1
        || signature.is_control_dependent()
        || signature.is_aggregate_channel()
        || function.params.len() != 1
        || function.params[0].ty != ResolvedType::I64
        || function.return_type != ResolvedType::I64
        || !matches!(function.yields.as_ref(), Some(y)
            if y.request_type == ResolvedType::I64 && y.response_type == ResolvedType::I64)
    {
        return Err(profile_error(
            "local Future requires one direct i64 yield, one i64 argument, and an i64 result",
        ));
    }
    Ok(signature)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceLocalFutureFailure {
    HandlerFailed,
    Panicked,
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    EvaluationRejected,
}

/// A one-shot, thread-local host future serving exactly one source request.
/// The opaque suspension binding is held in memory and never serialized.
pub struct SourceLocalFuture<F, H> {
    program: hir::ResolvedProgram,
    function_id: String,
    seed: i64,
    request: i64,
    state: crate::resumable_effects::lowering::ResumableStateId,
    binding: crate::resumable_effects::lowering::ResumableSuspensionBinding,
    max_steps: usize,
    handler: Option<Box<H>>,
    pending: Option<Pin<Box<F>>>,
    settled: bool,
    local: Rc<()>,
    revision: Option<Arc<ProjectRevision>>,
}

impl<F, H, E> SourceLocalFuture<F, H>
where
    F: Future<Output = Result<i64, E>> + 'static,
    H: FnOnce(i64) -> F + 'static,
{
    /// Select an exact canonical source function with one i64 request and
    /// answer. No host operation occurs during preparation.
    pub fn prepare(
        source: &str,
        path: &Path,
        function_id: &str,
        seed: i64,
        max_steps: usize,
        handler: H,
    ) -> Result<Self, Vec<Diagnostic>> {
        if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
            return Err(invalid(
                "local Future interpreter fuel is outside its bounds",
            ));
        }
        let (parsed, canonical) = crate::parse_canonical(source, path).map_err(|e| vec![e])?;
        if canonical != source {
            return Err(invalid("local Future source is not canonical"));
        }
        let checked = crate::check(source, path)?;
        if checked != parsed {
            return Err(invalid("local Future source changed during checking"));
        }
        let program = hir::resolve(&checked)?;
        admitted_source_future_signature(&program, function_id).map_err(|e| vec![e])?;
        Self::prepare_program(program, function_id, seed, max_steps, handler, None)
    }

    /// Consume only a retained, Phase-A admitted Project revision. A caller
    /// needing filesystem provenance obtains it through an authenticated
    /// Project snapshot; this constructor cannot recheck the original files.
    /// No caller-supplied digest or source text selects the exported function.
    pub fn prepare_revision(
        revision: Arc<ProjectRevision>,
        seed: i64,
        max_steps: usize,
        handler: H,
    ) -> Result<Self, Vec<Diagnostic>> {
        if revision.manifest().project_profile() != ProjectProfile::SourceLocalFutureV1 {
            return Err(invalid("local Future Project profile is not selected"));
        }
        let function_id = revision
            .source_local_future_signature()?
            .function_id()
            .to_owned();
        let program = revision.public_api_program().clone();
        Self::prepare_program(
            program,
            &function_id,
            seed,
            max_steps,
            handler,
            Some(revision),
        )
    }

    fn prepare_program(
        program: hir::ResolvedProgram,
        function_id: &str,
        seed: i64,
        max_steps: usize,
        handler: H,
        revision: Option<Arc<ProjectRevision>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
            return Err(invalid(
                "local Future interpreter fuel is outside its bounds",
            ));
        }
        let step = run_resumable_effect(
            &program,
            function_id,
            &[ArgumentValue::Int(seed)],
            max_steps,
        )?;
        let ResumableStep::Suspended {
            state,
            binding,
            request: ArgumentValue::Int(request),
        } = step.step
        else {
            return Err(invalid(
                "local Future source did not suspend at its selected yield",
            ));
        };
        Ok(Self {
            program,
            function_id: function_id.to_owned(),
            seed,
            request,
            state,
            binding,
            max_steps,
            handler: Some(Box::new(handler)),
            pending: None,
            settled: false,
            local: Rc::new(()),
            revision,
        })
    }
}

impl<F, H, E> Future for SourceLocalFuture<F, H>
where
    F: Future<Output = Result<i64, E>> + 'static,
    H: FnOnce(i64) -> F + 'static,
{
    type Output = Result<i64, SourceLocalFutureFailure>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        assert!(!this.settled, "local source Future polled after settlement");
        let _ = &this.local;
        let _ = &this.revision;
        if this.pending.is_none() {
            let handler = this.handler.take().expect("one handler before settlement");
            match catch_unwind(AssertUnwindSafe(|| handler(this.request))) {
                Ok(future) => this.pending = Some(Box::pin(future)),
                Err(_) => {
                    this.settled = true;
                    return Poll::Ready(Err(SourceLocalFutureFailure::Panicked));
                }
            }
        }
        let answer = match catch_unwind(AssertUnwindSafe(|| {
            this.pending
                .as_mut()
                .expect("pending future")
                .as_mut()
                .poll(cx)
        })) {
            Ok(Poll::Pending) => return Poll::Pending,
            Ok(Poll::Ready(result)) => result,
            Err(_) => {
                this.settled = true;
                this.pending = None;
                return Poll::Ready(Err(SourceLocalFutureFailure::Panicked));
            }
        };
        this.settled = true;
        this.pending = None;
        let answer = answer.map_err(|_| SourceLocalFutureFailure::HandlerFailed)?;
        let step = resume_resumable_effect(
            &this.program,
            &this.function_id,
            &[ArgumentValue::Int(this.seed)],
            &this.state,
            &this.binding,
            &ArgumentValue::Int(this.request),
            &ArgumentValue::Int(answer),
            this.max_steps,
        )
        .map_err(|_| SourceLocalFutureFailure::EvaluationRejected)?;
        Poll::Ready(match step.step {
            ResumableStep::Completed {
                result: ArgumentValue::Int(value),
                ..
            } => Ok(value),
            ResumableStep::LanguageFailure(status) => {
                Err(SourceLocalFutureFailure::LanguageFailure(status))
            }
            ResumableStep::FuelExhausted => Err(SourceLocalFutureFailure::FuelExhausted),
            ResumableStep::CallDepthExceeded => Err(SourceLocalFutureFailure::CallDepthExceeded),
            _ => Err(SourceLocalFutureFailure::EvaluationRejected),
        })
    }
}

#[cfg(test)]
mod tests;
