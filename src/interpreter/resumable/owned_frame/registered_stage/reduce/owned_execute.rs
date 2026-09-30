//! Consuming physical effect-to-reducer handoff. Every holder keeps the same
//! sealed store borrower. Reducer settlement/publication needs successor ACKs
//! and is deliberately absent from this private foundation.
use super::super::effect::{
    reducer_guard, ExecutedOwnedAgentTurnV2, OwnedEffectInputsV8, OwnedEffectPhaseV8,
};
use super::*;

pub(crate) struct PreparedExecutedOwnedReduceV2<'a> {
    input: PreparedOwnedReduceV2,
    inputs: OwnedEffectInputsV8<'a>,
    effect_settled: u32,
}
pub(crate) struct StagedExecutedOwnedReduceV2<'a> {
    pub(super) staged: StagedOwnedReduceV2,
    pub(super) inputs: OwnedEffectInputsV8<'a>,
    pub(super) effect_settled: u32,
    pub(super) allowance: usize,
    pub(super) consumed: usize,
}
pub(crate) struct ExecutedOwnedReducePreparationRejectionV2<'a> {
    pub(crate) executed: ExecutedOwnedAgentTurnV2<'a>,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) struct ExecutedOwnedReduceRejectionV2<'a> {
    pub(crate) prepared: PreparedExecutedOwnedReduceV2<'a>,
    pub(crate) diagnostic: Diagnostic,
}
impl StagedExecutedOwnedReduceV2<'_> {
    pub(super) fn observed_fuel(&self) -> (usize, usize) {
        (self.allowance, self.consumed)
    }
    pub(crate) fn failure(&self) -> Option<&OwnedFrameFailure> {
        self.staged.failure()
    }
    pub(crate) fn effect_settled(&self) -> u32 {
        self.effect_settled
    }
    pub(crate) fn validate_store(&self) -> bool {
        self.staged.creator == std::process::id() && self.inputs.store.validate_guard().is_ok()
    }
}
pub(crate) fn prepare_executed_owned_reduce_v2<'a>(
    executed: ExecutedOwnedAgentTurnV2<'a>,
    plan: &CheckedOwnedReduceV2,
    check: impl FnMut(OwnedEffectPhaseV8) -> bool,
) -> Result<PreparedExecutedOwnedReduceV2<'a>, ExecutedOwnedReducePreparationRejectionV2<'a>> {
    let (roots, inputs, effect_settled) =
        executed
            .into_reduce_parts(plan, check)
            .map_err(|executed| ExecutedOwnedReducePreparationRejectionV2 {
                executed,
                diagnostic: rejected("physical reducer handoff authority/proof differs"),
            })?;
    Ok(PreparedExecutedOwnedReduceV2 {
        input: PreparedOwnedReduceV2 {
            plan: plan.clone(),
            state: roots.state,
            outcome: roots.outcome,
            proposal: roots.proposal,
            allocations: roots.allocations,
            creator: roots.creator,
        },
        inputs,
        effect_settled,
    })
}
pub(crate) fn stage_executed_owned_reduce_v2<'a>(
    prepared: PreparedExecutedOwnedReduceV2<'a>,
    budget: &mut OwnedFrameBudget,
    mut check: impl FnMut(OwnedEffectPhaseV8) -> bool,
) -> Result<StagedExecutedOwnedReduceV2<'a>, ExecutedOwnedReduceRejectionV2<'a>> {
    let valid = reducer_guard(
        &prepared.inputs,
        prepared.input.creator,
        prepared.effect_settled,
        &mut check,
    );
    if !valid {
        return Err(ExecutedOwnedReduceRejectionV2 {
            prepared,
            diagnostic: rejected("physical reducer stage authority differs"),
        });
    }
    let allowance = budget.remaining;
    match stage_owned_reduce_v2(prepared.input, budget) {
        Ok(staged) => Ok(StagedExecutedOwnedReduceV2 {
            staged,
            inputs: prepared.inputs,
            effect_settled: prepared.effect_settled,
            allowance,
            consumed: allowance
                .checked_sub(budget.remaining)
                .expect("bounded evaluator charge"),
        }),
        Err(error) => Err(ExecutedOwnedReduceRejectionV2 {
            prepared: PreparedExecutedOwnedReduceV2 {
                input: error.input,
                ..prepared
            },
            diagnostic: error.diagnostic,
        }),
    }
}
