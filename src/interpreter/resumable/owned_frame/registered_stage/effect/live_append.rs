//! Production authorization ACK derived only from a held actual live envelope.
use super::*;
use crate::agent_lifecycle::authorization::CheckedOwnedWaitReadyCommitmentsV8;
use crate::live_invocation::source_journal::{LiveEffectAuthorizationPermitV8, SourceJournalError};
use std::cell::Cell;

fn closed_references(staged: u32, ready: u32, consumed: u32) -> bool {
    staged.checked_add(1) == Some(ready) && ready.checked_add(1) == Some(consumed)
}

pub(crate) enum LiveEffectPreparationRejectionV8<'j> {
    Before {
        rejected: OwnedEffectPreparationRejectionV8<'j>,
        error: SourceJournalError,
    },
    After {
        prepared: PreparedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
}

pub(crate) enum EffectPreparationGuardV8<'g, 'p, 'j> {
    Initial(&'g LiveEffectAuthorizationPermitV8<'p, 'j>),
    Continued(
        &'g crate::live_invocation::source_journal::LiveContinuedEffectAuthorizationPermitV8<
            'p,
            'j,
        >,
    ),
}
impl EffectPreparationGuardV8<'_, '_, '_> {
    fn validate_guard(&self, inputs: &OwnedEffectInputsV8<'_>) -> Result<(), SourceJournalError> {
        match self {
            Self::Initial(p) => p.validate_guard(inputs),
            Self::Continued(p) => p.validate_guard(inputs),
        }
    }
    fn validate_current(&self) -> Result<(), SourceJournalError> {
        match self {
            Self::Initial(p) => p.validate_current(),
            Self::Continued(p) => p.validate_current(),
        }
    }
    fn references(&self) -> (u32, u32, u32) {
        match self {
            Self::Initial(p) => p.references(),
            Self::Continued(p) => p.references(),
        }
    }
    fn matches_commitments(&self, actual: &CheckedOwnedWaitReadyCommitmentsV8) -> bool {
        match self {
            Self::Initial(p) => p.matches_commitments(actual),
            Self::Continued(p) => p.matches_commitments(actual),
        }
    }
}

pub(crate) fn prepare_live_owned_effect_v8<'j>(
    inputs: OwnedEffectInputsV8<'j>,
    ready: ReadyOwnedAuthorizeV2,
    permit: &LiveEffectAuthorizationPermitV8<'_, 'j>,
) -> Result<PreparedOwnedEffectV8<'j>, LiveEffectPreparationRejectionV8<'j>> {
    prepare_with_guard_v8(inputs, ready, EffectPreparationGuardV8::Initial(permit))
}
pub(crate) fn prepare_with_guard_v8<'j>(
    inputs: OwnedEffectInputsV8<'j>,
    ready: ReadyOwnedAuthorizeV2,
    permit: EffectPreparationGuardV8<'_, '_, 'j>,
) -> Result<PreparedOwnedEffectV8<'j>, LiveEffectPreparationRejectionV8<'j>> {
    let fail = |ready, inputs, error| LiveEffectPreparationRejectionV8::Before {
        rejected: OwnedEffectPreparationRejectionV8 {
            ready,
            inputs,
            diagnostic: rejected("live effect authorization lineage differs"),
        },
        error,
    };
    if let Err(error) = permit.validate_guard(&inputs) {
        return Err(fail(ready, inputs, error));
    }
    let (staged, ready_ref, consumed) = permit.references();
    if !closed_references(staged, ready_ref, consumed) {
        return Err(fail(ready, inputs, SourceJournalError::Binding));
    }
    let Some((state, decision)) = ready.live_checked_facts(inputs.execution.wait()) else {
        return Err(fail(ready, inputs, SourceJournalError::Binding));
    };
    let commitments = match checked_owned_wait_ready_commitments_v8(
        inputs.runtime,
        inputs.execution,
        &inputs.store.registration().expected_facts().scope,
        inputs.turn,
        inputs.attempt,
        &state,
        &decision,
        &inputs.proposal,
    ) {
        Ok(commitments) => commitments,
        Err(error) => return Err(fail(ready, inputs, error)),
    };
    if !permit.matches_commitments(&commitments) {
        return Err(fail(ready, inputs, SourceJournalError::Binding));
    }
    let Some((basis, _)) = checked_basis_facts(&inputs, (state, decision, commitments.budget()))
    else {
        return Err(fail(ready, inputs, SourceJournalError::Binding));
    };
    // No raw-facts constructor: this private ACK exists only inside the actual
    // consuming held-envelope entry and cannot be detached from its live owner.
    let ack = OwnedEffectAuthorizationAckV8 {
        basis,
        staged,
        ready: ready_ref,
        consumed,
    };
    let selected = Cell::new(None);
    let result = prepare_owned_effect_v8(inputs, ready, ack, |_| match permit.validate_current() {
        Ok(()) => true,
        Err(error) => {
            if selected.get().is_none() {
                selected.set(Some(error));
            }
            false
        }
    });
    #[cfg(test)]
    if result.is_ok() && matches!(&permit, EffectPreparationGuardV8::Continued(_)) {
        crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_note_preparation();
    }
    match result {
        Ok(prepared) => {
            if let Err(error) = prepared.validate_preparation_guard(&permit) {
                prepared.quarantine_live_authorization();
                return Err(LiveEffectPreparationRejectionV8::After { prepared, error });
            }
            Ok(prepared)
        }
        Err(rejected) => Err(LiveEffectPreparationRejectionV8::Before {
            rejected,
            error: selected.get().unwrap_or(SourceJournalError::Binding),
        }),
    }
}
impl PreparedOwnedEffectV8<'_> {
    pub(crate) fn validate_live_authorization(
        &self,
        permit: &LiveEffectAuthorizationPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_preparation_guard(&EffectPreparationGuardV8::Initial(permit))
    }
    pub(crate) fn validate_preparation_guard(
        &self,
        permit: &EffectPreparationGuardV8<'_, '_, '_>,
    ) -> Result<(), SourceJournalError> {
        let result = self.validate_authorization_inner(permit);
        if result.is_err() {
            self.inputs.store.quarantine();
        }
        result
    }
    fn validate_authorization_inner(
        &self,
        permit: &EffectPreparationGuardV8<'_, '_, '_>,
    ) -> Result<(), SourceJournalError> {
        permit.validate_guard(&self.inputs)?;
        let Some((basis, budget)) = checked_basis(&self.inputs, &self.owner) else {
            return Err(SourceJournalError::Binding);
        };
        if basis != self.basis
            || budget != self.budget
            || permit.references() != (self.staged, self.ready, self.authorization_tail)
            || self.request.grant != self.basis.target_grant
            || self.request.authorization != self.basis.authorization
        {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_guard(&self.inputs)
    }
    pub(crate) fn quarantine_live_authorization(&self) {
        self.inputs.store.quarantine();
    }
    #[cfg(test)]
    pub(crate) fn live_test_metadata(&self) -> (u32, u32, u32, &str, &str, &str) {
        (
            self.staged,
            self.ready,
            self.authorization_tail,
            &self.basis.grant,
            &self.basis.target_grant,
            &self.basis.proposal,
        )
    }
}

#[cfg(test)]
mod tests;

pub(super) mod intent;

pub(in crate::interpreter::resumable::owned_frame::registered_stage::effect) mod settlement;
pub(crate) use settlement::CheckedLiveOwnedEffectSettlementV8;
