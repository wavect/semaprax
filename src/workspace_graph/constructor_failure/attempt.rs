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
pub(in crate::workspace_graph) fn run<T>(
    maximum: usize,
    operation: impl FnOnce() -> Result<T, CoreFailure>,
) -> Attempt<T> {
    let controls = size_of::<Attempt<T>>()
        .checked_add(size_of::<Result<T, CoreFailure>>())
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
