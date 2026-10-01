//! Sealed inert profile/coordinate join; no live target or append authority.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::{
    accounting::CheckedTargetAccountingV8, OwnedEffectSettlementInputsV8,
};

pub(crate) struct CheckedCumulativeEffectPrefixV8<'a> {
    inputs: OwnedEffectSettlementInputsV8<'a>,
    previous: Option<&'a CheckedTargetAccountingV8>,
}
impl CheckedCumulativeEffectPrefixV8<'_> {
    pub(crate) fn previous(&self) -> Option<&CheckedTargetAccountingV8> {
        self.previous
    }
    pub(crate) fn validate(&self, inputs: &OwnedEffectSettlementInputsV8<'_>) -> Result<(), Error> {
        require(
            std::ptr::eq(self.inputs.runtime, inputs.runtime)
                && std::ptr::eq(self.inputs.execution, inputs.execution)
                && std::ptr::eq(self.inputs.scope, inputs.scope)
                && std::ptr::eq(self.inputs.state, inputs.state)
                && std::ptr::eq(self.inputs.decision, inputs.decision)
                && std::ptr::eq(self.inputs.proposal, inputs.proposal)
                && self.inputs.turn == inputs.turn
                && self.inputs.attempt == inputs.attempt,
        )
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn checked_prefix<'a>(
    context: &super::super::FoldContextV8,
    rows: &[ValidatedEntryV8],
    inputs: &OwnedEffectSettlementInputsV8<'a>,
    previous: Option<&'a CheckedTargetAccountingV8>,
) -> Result<CheckedCumulativeEffectPrefixV8<'a>, Error> {
    let folded = fold::fold(context, rows)?;
    let expected_scope = scope(context)?;
    let Body::OwnedRunCreated { execution, .. } = &context.created else {
        return Err(Error::Binding);
    };
    require(
        inputs.scope.program_root() == expected_scope.program_root()
            && inputs.scope.invocation_id() == expected_scope.invocation_id()
            && inputs.scope.policy_epoch() == expected_scope.policy_epoch(),
    )?;
    require(
        fold::cumulative::effect_prefix_matches(
            context,
            &folded,
            inputs.turn,
            inputs.attempt,
            inputs.state,
        ) && inputs.execution.wait().binding() == context.checked_binding.binding()
            && inputs.execution.ordinary().invocation() == execution,
    )?;
    if inputs.turn == 0 {
        require(previous.is_none())?;
    } else {
        let previous = previous.ok_or(Error::Binding)?;
        require(
            previous.total().calls() == u64::from(inputs.turn)
                && previous.total().fuel() == u64::from(inputs.turn),
        )?;
    }
    Ok(CheckedCumulativeEffectPrefixV8 {
        inputs: OwnedEffectSettlementInputsV8 {
            runtime: inputs.runtime,
            execution: inputs.execution,
            scope: inputs.scope,
            turn: inputs.turn,
            attempt: inputs.attempt,
            state: inputs.state,
            decision: inputs.decision,
            proposal: inputs.proposal,
        },
        previous,
    })
}

#[cfg(all(test, unix))]
mod tests;
