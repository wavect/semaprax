//! Sealed semantic error data. These atoms grant no replay/lookup authority.
//! Detailed H006 text uses the diagnostic bridge; ordinary formatting retains
//! the legacy receiver and test-only invocation authority selects funded text.
use super::*;
#[derive(Debug)]
pub(super) enum Atom {
    SelectedSession(&'static str),
    SelectedChild(crate::cleanup_plan::renewal_profile::child_refusal::Refusal),
    RepeatedStatus(ExpressionId),
    UnknownStatus(ExpressionId),
    NoncontiguousSlot(usize),
    RepeatedStorage(StorageId),
    DeadCallFlag(ExpressionId, LivenessFlagId),
    RepeatedCallFlag(ExpressionId, LivenessFlagId),
    RepeatedVariantEpoch(ExpressionId),
    MismatchedFailure(ExpressionId),
}
#[derive(Debug)]
pub(crate) struct Receipt {
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "shared source anchor retained with the semantic receipt"
        )
    )]
    source: ExpressionId,
    atom: Atom,
}
impl Receipt {
    pub(super) fn selected(function: &ResolvedFunction, detail: &'static str) -> Self {
        Self {
            source: function.body.id.clone(),
            atom: Atom::SelectedSession(detail),
        }
    }
    pub(super) fn selected_child(function:&ResolvedFunction,reason:crate::cleanup_plan::renewal_profile::child_refusal::Refusal)->Self {
        Self {source:function.body.id.clone(),atom:Atom::SelectedChild(reason)}
    }
    pub(super) fn stage(&self) -> &'static str {
        match &self.atom {
            Atom::SelectedSession(_) => "selected session",
            Atom::SelectedChild(_) => "selected child",
            _ => "structural replay",
        }
    }

    pub(super) fn legacy(self, function: &ResolvedFunction) -> Diagnostic {
        // Branch-local calls: no `fmt::Arguments` escapes its own statement.
        match self.atom {
            Atom::SelectedSession(detail) => replay_error(function, detail),
            Atom::SelectedChild(reason) => {
                if reason.prefixed(){replay_error(function,reason.detail())}else{reason.legacy()}
            }
            Atom::RepeatedStatus(id) => replay_error(
                function,
                format_args!("status source for `{id}` is repeated"),
            ),
            Atom::UnknownStatus(id) => replay_error(
                function,
                format_args!("status source references unknown expression `{id}`"),
            ),
            Atom::NoncontiguousSlot(index) => replay_error(
                function,
                format_args!("cleanup slot at index {index} has non-contiguous identity or index"),
            ),
            Atom::DeadCallFlag(call, flag) => replay_error(
                function,
                format_args!(
                    "call `{call}` atomically consumes dead argument flag {}",
                    flag.0
                ),
            ),
            Atom::RepeatedCallFlag(call, flag) => replay_error(
                function,
                format_args!(
                    "call `{call}` atomically consumes argument flag {} twice",
                    flag.0
                ),
            ),
            Atom::RepeatedVariantEpoch(call) => replay_error(
                function,
                format_args!("call `{call}` atomically consumes a variant epoch twice"),
            ),
            Atom::MismatchedFailure(expression) => replay_error(
                function,
                format_args!(
                    "selected failure `{expression}` does not match the pending failing edge"
                ),
            ),
            Atom::RepeatedStorage(storage) => replay_error(
                function,
                format_args!("cleanup storage `{storage:?}` is repeated"),
            ),
        }
    }
    #[cfg(test)]
    pub(super) fn source(&self) -> &ExpressionId {
        &self.source
    }
}
pub(super) trait Error: From<Diagnostic> {
    fn structural(function: &ResolvedFunction, atom: Atom) -> Self;
}
impl Error for Diagnostic {
    fn structural(function: &ResolvedFunction, atom: Atom) -> Self {
        Receipt {
            source: function.body.id.clone(),
            atom,
        }
        .legacy(function)
    }
}
impl Error for expression_facts::errors::EngineError {
    fn structural(function: &ResolvedFunction, atom: Atom) -> Self {
        Self::Structural(Receipt {
            source: function.body.id.clone(),
            atom,
        })
    }
}
#[cfg(test)]
mod tests;
