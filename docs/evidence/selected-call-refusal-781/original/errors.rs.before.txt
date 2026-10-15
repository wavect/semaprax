//! Private expression-fact refusal protocol. A receipt is error data only:
//! shared identity handles retain exact source spelling, never grant lookup or
//! replay authority. Legacy policies synchronously render the old full text.
use super::*;

pub(super) trait Policy {
    type Error: From<Diagnostic>;
    fn capacity(function: &ResolvedFunction, failure: capacity::Failure) -> Self::Error;
    fn duplicate(function: &ResolvedFunction, expression: &ExpressionId) -> Self::Error;
}

pub(super) struct Legacy;
impl Policy for Legacy {
    type Error = Diagnostic;
    fn capacity(function: &ResolvedFunction, failure: capacity::Failure) -> Diagnostic {
        replay_error(function, failure.detail)
    }
    fn duplicate(function: &ResolvedFunction, expression: &ExpressionId) -> Diagnostic {
        replay_error(
            function,
            format_args!("HIR expression identity `{expression}` is repeated"),
        )
    }
}

/// Partial engine migration: unmigrated children keep their original owned
/// Diagnostic. Fact/Structural receipts never format authored identity text.
#[derive(Debug)]
pub(crate) enum EngineError {
    Failed,
    Owned(Diagnostic),
    Fact(FactReceipt),
    Structural(semantic_refusal::Receipt),
}
impl From<Diagnostic> for EngineError {
    fn from(error: Diagnostic) -> Self {
        Self::Owned(error)
    }
}

#[derive(Debug)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "source identity operands are retained for owning receipt observation, never permission"
    )
)]
pub(crate) struct FactReceipt {
    pub(crate) source: ExpressionId,
    pub(crate) expression: Option<ExpressionId>,
    detail: &'static str,
    physical: bool,
}
impl FactReceipt {
    pub(super) fn legacy(self, function: &ResolvedFunction) -> Diagnostic {
        if let Some(expression) = self.expression {
            replay_error(
                function,
                format_args!("HIR expression identity `{expression}` is repeated"),
            )
        } else {
            replay_error(function, self.detail)
        }
    }

    pub(super) fn physical(&self) -> bool {
        self.physical
    }
    pub(crate) fn detail(&self) -> &'static str {
        self.detail
    }
}

pub(super) struct Selected;
impl Policy for Selected {
    type Error = EngineError;
    fn capacity(function: &ResolvedFunction, failure: capacity::Failure) -> EngineError {
        EngineError::Fact(FactReceipt {
            source: function.body.id.clone(),
            expression: None,
            detail: failure.detail,
            physical: failure.physical,
        })
    }
    fn duplicate(function: &ResolvedFunction, expression: &ExpressionId) -> EngineError {
        EngineError::Fact(FactReceipt {
            source: function.body.id.clone(),
            expression: Some(expression.clone()),
            detail: "HIR expression identity is repeated",
            physical: false,
        })
    }
}
