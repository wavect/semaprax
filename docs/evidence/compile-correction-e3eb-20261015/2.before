//! Selected call-local typed refusals. No handle grants source or payer authority.
use super::*;
use std::marker::PhantomData;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) enum Kind {
    StructuralCallCursorAllocationRefused,
    StructuralCallCursorCapacityDiffersFromReservation,
    StructuralCallLookupDepthExceedsBound,
    CallCommitExpressionIsAbsent,
    SelectedCallCommitDoesNotSupportGenericCalls,
    SelectedStringTransferCallIdentityDiffers,
    SelectedCallCommitDoesNotSupportEffectfulCallees,
    SelectedCallLacksItsLookupCursor,
    CallCommitDoesNotNameASelectedOrdinaryCall,
    CallCommitFactsDisagreeWithCurrentTypedCall,
    CallCommitCalleeSignatureHasInconsistentArity,
    CallCommitArgumentDisagreesWithExactParameterTypeOrOwnership,
    TooManyCallParameters,
    CallCommitDoesNotContainEveryAndOnlyOwnedDroppableParameter,
    CallCommitParametersAreNotInStrictSignatureOrder,
    CallCommitParameterIndexDoesNotFitUsize,
    CallCommitParameterIndexExceedsCallArity,
    CallCommitDoesNotConsumeItsMatchingWholeArgumentEpoch,
    AtomicCallCommitConsumesOverlappingArgumentEpochs,
}
impl Kind {
    pub(in crate::cleanup_plan::replay) fn detail(self) -> &'static str {
        match self {
            Self::StructuralCallCursorAllocationRefused => {
                "structural call cursor allocation refused"
            }
            Self::StructuralCallCursorCapacityDiffersFromReservation => {
                "structural call cursor capacity differs from reservation"
            }
            Self::StructuralCallLookupDepthExceedsBound => {
                "structural call lookup depth exceeds bound"
            }
            Self::CallCommitExpressionIsAbsent => "call commit expression is absent",
            Self::SelectedCallCommitDoesNotSupportGenericCalls => {
                "selected call commit does not support generic calls"
            }
            Self::SelectedStringTransferCallIdentityDiffers => {
                "selected String transfer call identity differs"
            }
            Self::SelectedCallCommitDoesNotSupportEffectfulCallees => {
                "selected call commit does not support effectful callees"
            }
            Self::SelectedCallLacksItsLookupCursor => "selected call lacks its lookup cursor",
            Self::CallCommitDoesNotNameASelectedOrdinaryCall => {
                "call commit does not name a selected ordinary call"
            }
            Self::CallCommitFactsDisagreeWithCurrentTypedCall => {
                "call commit facts disagree with current typed call"
            }
            Self::CallCommitCalleeSignatureHasInconsistentArity => {
                "call commit callee signature has inconsistent arity"
            }
            Self::CallCommitArgumentDisagreesWithExactParameterTypeOrOwnership => {
                "call commit argument disagrees with exact parameter type or ownership"
            }
            Self::TooManyCallParameters => "too many call parameters",
            Self::CallCommitDoesNotContainEveryAndOnlyOwnedDroppableParameter => {
                "call commit does not contain every and only owned droppable parameter"
            }
            Self::CallCommitParametersAreNotInStrictSignatureOrder => {
                "call-commit parameters are not in strict signature order"
            }
            Self::CallCommitParameterIndexDoesNotFitUsize => {
                "call-commit parameter index does not fit usize"
            }
            Self::CallCommitParameterIndexExceedsCallArity => {
                "call-commit parameter index exceeds call arity"
            }
            Self::CallCommitDoesNotConsumeItsMatchingWholeArgumentEpoch => {
                "call commit does not consume its matching whole argument epoch"
            }
            Self::AtomicCallCommitConsumesOverlappingArgumentEpochs => {
                "atomic call commit consumes overlapping argument epochs"
            }
        }
    }
}
// The boolean describes this private acquisition only. Legacy compatibility
// never becomes paid because an enclosing invocation happens to exist.
pub(in crate::cleanup_plan::replay) struct Permit<'a, E> {
    function: &'a ResolvedFunction,
    paid: bool,
    error: PhantomData<E>,
}
pub(in crate::cleanup_plan::replay) trait Policy:
    semantic_refusal::Error
{
    fn enter<'a, T>(
        function: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Permit<'a, Self>, Self>;
    #[cfg(test)]
    fn borrowed_controls(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Self>;
    fn static_error(permit: &Permit<'_, Self>, kind: Kind) -> Self;
    fn missing_callee(permit: &Permit<'_, Self>, callee: &DeclarationId) -> Self;
    fn field(
        program: &dyn ReplayContext,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<(), Self>;
    fn call_facts(
        program: &dyn ReplayContext,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<Option<bool>, Self>;
    fn string_operand<'a>(
        function: &ResolvedFunction,
        expression: &'a ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<Option<&'a ResolvedExpr>, Self>;
}
impl Policy for Diagnostic {
    fn enter<'a, T>(
        function: &'a ResolvedFunction,
        _budget: &mut ReplayBudget,
    ) -> Result<Permit<'a, Self>, Self> {
        Ok(Permit {
            function,
            paid: false,
            error: PhantomData,
        })
    }
    #[cfg(test)]
    fn borrowed_controls(
        _function: &ResolvedFunction,
        _budget: &mut ReplayBudget,
    ) -> Result<(), Self> {
        Ok(())
    }
    fn static_error(permit: &Permit<'_, Self>, kind: Kind) -> Self {
        replay_error(permit.function, kind.detail())
    }
    fn missing_callee(permit: &Permit<'_, Self>, callee: &DeclarationId) -> Self {
        replay_error(
            permit.function,
            format_args!("call commit has unknown callee `{callee}`"),
        )
    }
    fn field(
        program: &dyn ReplayContext,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<(), Self> {
        selected_profile::collections::field(program, function, expression, budget)
    }
    fn call_facts(
        program: &dyn ReplayContext,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<Option<bool>, Self> {
        selected_profile::collections::call_facts(program, function, expression, budget)
    }
    fn string_operand<'a>(
        function: &ResolvedFunction,
        expression: &'a ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<Option<&'a ResolvedExpr>, Self> {
        selected_profile::string_transfer::operand(function, expression, budget)
    }
}
type Engine = expression_facts::errors::EngineError;
impl Policy for Engine {
    fn enter<'a, T>(
        function: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Permit<'a, Self>, Self> {
        if !budget.selected() || budget.physical.is_none() {
            return Ok(Permit {
                function,
                paid: false,
                error: PhantomData,
            });
        }
        type Controls<'a, T> = (
            Permit<'a, Engine>,
            Kind,
            DeclarationId,
            semantic_refusal::Atom,
            semantic_refusal::Receipt,
            Engine,
            Option<Engine>,
            Result<T, Engine>,
            Result<(), Engine>,
            Result<Permit<'a, Engine>, Engine>,
            &'a Permit<'a, Engine>,
        );
        budget.reserve_array::<Controls<'_, T>>(function, 1, "selected call refusal controls")?;
        budget.charge(function, 11, "selected call refusal controls")?;
        Ok(Permit {
            function,
            paid: true,
            error: PhantomData,
        })
    }
    #[cfg(test)]
    fn borrowed_controls(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Self> {
        if !budget.selected() || budget.physical.is_none() {
            return Ok(());
        }
        // These are the actual lease, lookup result, held result and borrowed
        // receiver at validate's child boundary. Child Diagnostics stay Owned.
        type Controls<'a> = (
            Option<crate::cleanup_plan::ReplayBorrowedLease<'a>>,
            Result<(&'a ResolvedExpr, schema::borrowed::Call<'a>), Diagnostic>,
            Option<(&'a ResolvedExpr, schema::borrowed::Call<'a>)>,
            &'a Option<crate::cleanup_plan::ReplayBorrowedLease<'a>>,
        );
        budget.reserve_array::<Controls<'_>>(function, 1, "selected borrowed call controls")?;
        budget.charge(function, 4, "selected borrowed call controls")?;
        Ok(())
    }
    fn static_error(permit: &Permit<'_, Self>, kind: Kind) -> Self {
        if !permit.paid {
            return Self::Owned(replay_error(permit.function, kind.detail()));
        }
        <Self as semantic_refusal::Error>::structural(
            permit.function,
            semantic_refusal::Atom::SelectedCall(kind),
        )
    }
    fn missing_callee(permit: &Permit<'_, Self>, callee: &DeclarationId) -> Self {
        if !permit.paid {
            return Self::Owned(replay_error(
                permit.function,
                format_args!("call commit has unknown callee `{callee}`"),
            ));
        }
        // This copies an immutable handle only. It neither allocates text nor
        // retroactively certifies how that handle's original backing was paid.
        <Self as semantic_refusal::Error>::structural(
            permit.function,
            semantic_refusal::Atom::UnknownCallCallee(callee.clone()),
        )
    }
    fn field(
        program: &dyn ReplayContext,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<(), Self> {
        selected_profile::paid_children::collections::field(program, function, expression, budget)
    }
    fn call_facts(
        program: &dyn ReplayContext,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<Option<bool>, Self> {
        selected_profile::paid_children::collections::call_facts(
            program, function, expression, budget,
        )
    }
    fn string_operand<'a>(
        function: &ResolvedFunction,
        expression: &'a ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<Option<&'a ResolvedExpr>, Self> {
        selected_profile::paid_children::string_transfer::operand(function, expression, budget)
    }
}
