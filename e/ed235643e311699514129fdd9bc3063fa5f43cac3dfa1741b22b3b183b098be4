//! Actual existing core child attempt, with an owned final-refusal sidecar.
use super::{
    receiving::{CoreFailure, Setup},
    report::{Owner, Published, Unpublished},
};
use crate::bounded_output::active_refused;
use crate::diagnostic::Diagnostic;
use std::mem::size_of;
pub(in crate::workspace_graph) struct Attempt<T> {
    result: Result<T, CoreFailure>,
    publication: Option<Result<Published, Unpublished>>,
}
// This named private helper observes the concrete closure type without changing
// run::<T>'s original public-in-module generic call shape. Its borrowed argument
// and returned bank have their own explicit actual/formal and return/receiver
// roles; no dyn closure or guessed conservative reference is substituted.
fn forwarding_controls<T, F>(_: &F) -> usize {
    // Const evaluation creates no runtime array, iterator, checked-add receiver,
    // or callback. Overflow still propagates into the existing arithmetic refusal.
    const {
        let rows = [
            size_of::<F>(),          // moved actual operation into the new callee
            size_of::<F>(),          // run_with_report_controls formal operation
            size_of::<Attempt<T>>(), // new callee's returned Attempt
            size_of::<Attempt<T>>(), // run's forwarding return
            size_of::<&F>(),         // this census helper's actual borrowed argument
            size_of::<&F>(),         // this census helper's formal borrowed argument
            size_of::<usize>(),      // this census helper's returned bank
            size_of::<usize>(),      // run's receiving bank local
            size_of::<usize>(),      // forwarded maximum actual argument
            size_of::<usize>(),      // new callee's maximum formal argument
            size_of::<usize>(),      // forwarded extra-controls actual argument
            size_of::<usize>(),      // new callee's extra-controls formal argument
        ];
        let mut total = 0usize;
        let mut index = 0;
        while index < rows.len() {
            total = match total.checked_add(rows[index]) {
                Some(total) => total,
                None => {
                    total = usize::MAX;
                    break;
                }
            };
            index += 1;
        }
        total
    }
}
pub(in crate::workspace_graph) fn run<T>(
    maximum: usize,
    operation: impl FnOnce() -> Result<T, CoreFailure>,
) -> Attempt<T> {
    let forwarding = forwarding_controls::<T, _>(&operation);
    run_with_report_controls(maximum, forwarding, operation)
}
pub(in crate::workspace_graph) fn run_with_report_controls<T>(
    maximum: usize,
    report_controls: usize,
    operation: impl FnOnce() -> Result<T, CoreFailure>,
) -> Attempt<T> {
    let controls = size_of::<Attempt<T>>()
        .checked_add(report_controls)
        .and_then(|bytes| bytes.checked_add(size_of::<Result<T, CoreFailure>>()))
        .and_then(|bytes| {
            bytes.checked_add(size_of::<(Result<T, CoreFailure>, Option<Vec<Diagnostic>>)>())
        })
        // Separate outer match receivers are real controls, in addition to
        // the inner &mut CoreFailure argument paid by the report owner.
        .and_then(|bytes| bytes.checked_add(size_of::<&Result<T, CoreFailure>>()))
        .and_then(|bytes| bytes.checked_add(size_of::<&mut Result<T, CoreFailure>>()))
        .and_then(|bytes| bytes.checked_add(size_of::<&Setup>()))
        // The copied *setup initializer and seal_setup's by-value receiver.
        .and_then(|bytes| bytes.checked_add(2 * size_of::<Setup>()))
        .ok_or(Setup::Arithmetic);
    let owner = match controls.and_then(|bytes| Owner::acquire(maximum, bytes)) {
        Ok(owner) => owner,
        Err(failure) => {
            return Attempt {
                result: Err(CoreFailure::Setup(failure)),
                publication: None,
            };
        }
    };
    let mut result = operation();
    // The first receipt is sealed BEFORE with_limit restores the child. A
    // successful/no-sticky attempt drops real unused storage with no refund.
    let publication = if active_refused() {
        Some(owner.seal())
    } else if let Err(CoreFailure::Setup(setup)) = &result {
        // A non-sticky setup result must not discard an already paid slot and
        // allocate another Vec after the child restores. On secondary failure
        // the complete original primary remains; no authority is borrowed.
        match owner.seal_setup(*setup) {
            Ok(published) => match &mut result {
                Err(error) => error.receive_published(published),
                Ok(_) => unreachable!("selected setup result remains Err"),
            },
            Err(unpublished) => {
                drop(unpublished.owner);
                let _reason = unpublished.refusal;
            }
        }
        None
    } else {
        drop(owner);
        None
    };
    Attempt {
        result,
        publication,
    }
}
impl<T> Attempt<T> {
    #[cfg(feature = "unstable-workflow-profiling")]
    pub(in crate::workspace_graph) fn observation_succeeded(&self) -> bool {
        self.result.is_ok()
    }
    pub(in crate::workspace_graph) fn into_parts(
        self,
    ) -> (Result<T, CoreFailure>, Option<Vec<Diagnostic>>) {
        let publication = self.publication.and_then(|publication| match publication {
            Ok(published) => Some(published.into_report()),
            // Secondary publication failure never substitutes for the primary
            // result/receipt. The explicit old public fallback remains real.
            Err(unpublished) => {
                drop(unpublished.owner);
                let _reason = unpublished.refusal;
                None
            }
        });
        (self.result, publication)
    }
}

/// Typed refusal at the actual attempt boundary: no diagnostic construction
/// follows a failed ledger reservation. Original helpers keep their legacy API.
pub(in crate::workspace_graph) fn reserve(bytes: usize) -> Result<(), CoreFailure> {
    if crate::bounded_output::active_remaining().is_none() {
        return Err(CoreFailure::Setup(Setup::NoOwner));
    }
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(CoreFailure::Setup(Setup::Budget {
            maximum: super::super::active_builder_limit(),
        }))
    }
}
