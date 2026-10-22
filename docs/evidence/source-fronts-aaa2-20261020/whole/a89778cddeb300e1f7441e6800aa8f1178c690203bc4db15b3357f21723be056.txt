//! Sealed semantic error data. These atoms grant no replay/lookup authority.
//! Detailed H006 text uses the diagnostic bridge; ordinary formatting retains
//! the legacy receiver and test-only invocation authority selects funded text.
use super::*;
#[derive(Debug)]
pub(super) enum Atom {
    SelectedCall(structural_control::call_receiving::Kind),
    UnknownCallCallee(DeclarationId),
    StatusUnknownCallee(DeclarationId),
    SelectedSession(&'static str),
    SelectedFamily(selected_context::family::Reason),
    SelectedControl(structural_control::selected::reason::Reason),
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
    pub(super) fn selected_child(
        function: &ResolvedFunction,
        reason: crate::cleanup_plan::renewal_profile::child_refusal::Refusal,
    ) -> Self {
        Self {
            source: function.body.id.clone(),
            atom: Atom::SelectedChild(reason),
        }
    }
    pub(super) fn family(
        function: &ResolvedFunction,
        reason: selected_context::family::Reason,
    ) -> Self {
        Self {
            source: function.body.id.clone(),
            atom: Atom::SelectedFamily(reason),
        }
    }
    pub(super) fn control(
        function: &ResolvedFunction,
        reason: structural_control::selected::reason::Reason,
    ) -> Self {
        Self {
            source: function.body.id.clone(),
            atom: Atom::SelectedControl(reason),
        }
    }
    pub(super) fn stage(&self) -> &'static str {
        match &self.atom {
            Atom::SelectedCall(_) | Atom::UnknownCallCallee(_) => "selected call",
            Atom::SelectedSession(_) => "selected session",
            Atom::SelectedChild(_) => "selected child",
            Atom::SelectedFamily(_) => "selected family",
            Atom::SelectedControl(_) => "selected control",
            _ => "structural replay",
        }
    }

    pub(super) fn legacy(self, function: &ResolvedFunction) -> Diagnostic {
        // Branch-local calls: no `fmt::Arguments` escapes its own statement.
        match self.atom {
            Atom::SelectedCall(kind) => replay_error(function, kind.detail()),
            Atom::StatusUnknownCallee(callee) => replay_error(
                function,
                format_args!("status source call has unknown callee `{callee}`"),
            ),
            Atom::UnknownCallCallee(callee) => replay_error(
                function,
                format_args!("call commit has unknown callee `{callee}`"),
            ),
            Atom::SelectedSession(detail) => replay_error(function, detail),
            Atom::SelectedFamily(reason) => reason.legacy(function),
            Atom::SelectedControl(reason) => reason.legacy(function),
            Atom::SelectedChild(reason) => {
                if reason.prefixed() {
                    replay_error(function, reason.detail())
                } else {
                    // Ordinary legacy text keeps its complete original renderer.
                    // An explicit matching oracle invocation must fund raw text
                    // before allocation, just as the prefixed replay bridge does.
                    #[cfg(test)]
                    match allocation::authority_for_function(function) {
                        Ok(Some(lease)) => {
                            let result = if matches!(allocation::opted(), Ok(Some(current)) if current == lease)
                            {
                                allocation::raw_error_metered(
                                    lease,
                                    function,
                                    format_args!("{}", reason.detail()),
                                )
                            } else {
                                allocation::raw_error_funded(
                                    lease,
                                    format_args!("{}", reason.detail()),
                                )
                            };
                            return match result {
                                Ok(diagnostic) => diagnostic,
                                Err(failure) => failure.into_diagnostic(),
                            };
                        }
                        Err(_) => {
                            return Diagnostic::io_static(
                                "SPX-H006",
                                "oracle invocation authority refused a raw diagnostic",
                            );
                        }
                        Ok(None) => {}
                    }
                    reason.legacy()
                }
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
    pub(super) fn call_callee(&self) -> Option<&DeclarationId> {
        match &self.atom {
            Atom::UnknownCallCallee(id) => Some(id),
            _ => None,
        }
    }
    /// Borrowed test observation only; this grants no replay or payer authority.
    /// The full Copy reason is retained, rather than reducing it to a code/string.
    #[cfg(test)]
    pub(crate) fn borrowed_child_receipt(
        &self,
    ) -> Option<(
        &ExpressionId,
        crate::cleanup_plan::renewal_profile::child_refusal::Refusal,
    )> {
        match &self.atom {
            Atom::SelectedChild(reason) => Some((&self.source, *reason)),
            _ => None,
        }
    }
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay) fn borrowed_control_receipt(
        &self,
    ) -> Option<(&ExpressionId, &structural_control::selected::reason::Reason)> {
        match &self.atom {
            Atom::SelectedControl(reason) => Some((&self.source, reason)),
            _ => None,
        }
    }
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay) fn borrowed_family_receipt(
        &self,
    ) -> Option<(&ExpressionId, selected_context::family::Reason)> {
        match self.atom {
            Atom::SelectedFamily(reason) => Some((&self.source, reason)),
            _ => None,
        }
    }
    /// Explicit allocating legacy observation OUTSIDE the paid/no-allocation cut.
    /// Both the new renderer and complete independent old renderer see the exact
    /// saved atom/source, and return all six fields for the actual-caller oracle.
    #[cfg(test)]
    pub(crate) fn child_legacy_pair(
        &self,
        function: &ResolvedFunction,
    ) -> (Diagnostic, Diagnostic) {
        let make = || {
            let Atom::SelectedChild(reason) = &self.atom else {
                panic!("exact selected-child atom");
            };
            Self {
                source: self.source.clone(),
                atom: Atom::SelectedChild(*reason),
            }
        };
        (
            make().legacy(function),
            tests::complete_original_legacy(make(), function),
        )
    }
    #[cfg(test)]
    pub(super) fn source(&self) -> &ExpressionId {
        &self.source
    }
}
pub(super) trait Error: From<Diagnostic> + lexical_boundary::Policy {
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
