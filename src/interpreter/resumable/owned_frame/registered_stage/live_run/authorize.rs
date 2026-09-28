//! Same-root stage handoff, authorized only by the actual live ACK producer.
use super::super::authorize::{
    settle_owned_authorize_v2, OwnedAuthorizeSettledV2, ReadyOwnedAuthorizeV2,
};
use super::super::authorize::{stage_owned_authorize_v2, StagedOwnedAuthorizeV2};
use super::*;
use crate::live_invocation::source_journal::LiveReadyPromotionPermitV8;
use crate::live_invocation::source_journal::{LiveAuthorizePermitV8, LiveStateTransferPermitV8};
use crate::resumable_effects::owned_frame::v2::{
    CheckedOwnedAgentWaitBindingV8, CheckedOwnedWaitProposalV8,
};

pub(crate) struct LiveTransferredStateV8 {
    state: CompletedOwnedAgentStateV2,
}
impl LiveTransferredStateV8 {
    pub(crate) fn checked_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
        proposal: &CheckedOwnedWaitProposalV8,
    ) -> Option<serde_json::Value> {
        let state = &self.state;
        let root = state.root.as_ref()?;
        if state.creator != std::process::id()
            || !state.plan.same_helper(binding.helper())
            || state.proposal != *proposal.carrier()
            || !state.allocations.validate(&[root])
        {
            return None;
        }
        root_facts(&state.plan, root)
    }
    #[cfg(test)]
    pub(crate) fn test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        super::super::super::snapshot::weak_leaves(self.state.root.as_ref().unwrap())
    }
}
pub(crate) enum LiveStateTransferOutcomeV8 {
    Moved(LiveTransferredStateV8),
    Refused(LiveResumedStateV8),
    GuardLost(LiveTransferredStateV8),
}
pub(crate) fn transfer_live_owned_state_v8(
    permit: LiveStateTransferPermitV8<'_>,
    owner: LiveResumedStateV8,
    proposal: &CheckedOwnedWaitProposalV8,
) -> LiveStateTransferOutcomeV8 {
    transfer_with_guard_v8(TransferGuardV8::Initial(&permit), owner, proposal)
}
pub(in crate::interpreter::resumable::owned_frame::registered_stage) fn transfer_with_guard_v8(
    permit: TransferGuardV8<'_, '_, '_>,
    owner: LiveResumedStateV8,
    proposal: &CheckedOwnedWaitProposalV8,
) -> LiveStateTransferOutcomeV8 {
    let t = &owner.terminal;
    // Prove no failure-vector path or physical finalizer can run before consuming.
    if permit.validate_guard().is_err()
        || owner.checked_facts(permit.binding()).is_none()
        || !t.plan.liveness().completion_cleanup.is_empty()
        || t.proposal.as_ref() != Some(proposal.carrier())
    {
        return LiveStateTransferOutcomeV8::Refused(owner);
    }
    let consumed = owner.consumed;
    let moved = match settle_owned_copy_wait_v2(
        owner.terminal,
        || permit.validate_guard().is_ok(),
        |_| unreachable!("checked empty completion vector"),
    ) {
        Ok(OwnedCopyWaitSettledV2::Completed(state)) => LiveTransferredStateV8 { state },
        Ok(OwnedCopyWaitSettledV2::Failed { .. }) => unreachable!("preproved successful helper"),
        Err(rejected) => {
            return LiveStateTransferOutcomeV8::Refused(LiveResumedStateV8 {
                terminal: rejected.terminal,
                consumed,
            })
        }
    };
    if permit.validate_guard().is_err() {
        LiveStateTransferOutcomeV8::GuardLost(moved)
    } else {
        LiveStateTransferOutcomeV8::Moved(moved)
    }
}
pub(crate) struct LiveStagedAuthorizationV8 {
    staged: StagedOwnedAuthorizeV2,
    consumed: u64,
}
/// Unpublished success owner. Abandonment drops backing only; no semantic Drop.
pub(crate) struct LiveReadyAuthorizationV8 {
    ready: ReadyOwnedAuthorizeV2,
    consumed: u64,
}
impl LiveReadyAuthorizationV8 {
    pub(crate) fn consumed(&self) -> u64 {
        self.consumed
    }
    pub(crate) fn checked_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<(serde_json::Value, serde_json::Value)> {
        self.ready.live_checked_facts(binding)
    }
    #[cfg(test)]
    pub(crate) fn test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        self.ready.live_test_weak()
    }
}
/// Opaque rejection retains the exact pre- or post-handoff physical owner.
pub(crate) enum LiveReadyEffectPreparationRejectionV8<'j> {
    Before {
        _owner: LiveReadyAuthorizationV8,
        inputs: super::super::effect::OwnedEffectInputsV8<'j>,
        error: crate::live_invocation::source_journal::SourceJournalError,
    },
    After {
        _owner: super::super::effect::PreparedOwnedEffectV8<'j>,
        _consumed: u64,
        error: crate::live_invocation::source_journal::SourceJournalError,
    },
}
impl LiveReadyEffectPreparationRejectionV8<'_> {
    pub(crate) fn quarantine(&self) {
        match self {
            Self::Before { inputs, .. } => inputs.store.quarantine(),
            Self::After { _owner, .. } => _owner.quarantine_live_authorization(),
        }
    }
    pub(crate) fn error(&self) -> crate::live_invocation::source_journal::SourceJournalError {
        match self {
            Self::Before { error, .. } | Self::After { error, .. } => *error,
        }
    }
}
impl LiveReadyAuthorizationV8 {
    pub(crate) fn prepare_live_effect_v8<'j>(
        self,
        inputs: super::super::effect::OwnedEffectInputsV8<'j>,
        permit: &crate::live_invocation::source_journal::LiveEffectAuthorizationPermitV8<'_, 'j>,
    ) -> Result<
        super::super::effect::PreparedOwnedEffectV8<'j>,
        LiveReadyEffectPreparationRejectionV8<'j>,
    > {
        let consumed = self.consumed;
        super::super::effect::live_append::prepare_live_owned_effect_v8(inputs, self.ready, permit)
            .map_err(|failed| match failed {
                super::super::effect::live_append::LiveEffectPreparationRejectionV8::Before {
                    rejected,
                    error,
                } => {
                    let super::super::effect::OwnedEffectPreparationRejectionV8 {
                        ready,
                        inputs,
                        ..
                    } = rejected;
                    LiveReadyEffectPreparationRejectionV8::Before {
                        _owner: LiveReadyAuthorizationV8 { ready, consumed },
                        inputs,
                        error,
                    }
                }
                super::super::effect::live_append::LiveEffectPreparationRejectionV8::After {
                    prepared,
                    error,
                } => LiveReadyEffectPreparationRejectionV8::After {
                    _owner: prepared,
                    _consumed: consumed,
                    error,
                },
            })
    }
}
pub(crate) enum LiveReadyPromotionOutcomeV8 {
    Ready(LiveReadyAuthorizationV8),
    Refused(LiveStagedAuthorizationV8),
    GuardLost(LiveReadyAuthorizationV8),
}
pub(crate) fn promote_live_owned_authorization_v8(
    permit: LiveReadyPromotionPermitV8<'_, '_>,
    owner: LiveStagedAuthorizationV8,
) -> LiveReadyPromotionOutcomeV8 {
    let binding = permit.binding();
    let valid = owner.checked_facts(binding).is_some_and(|(_, decision)| {
        decision["case"].as_str() == Some(binding.authorize().granted().as_str())
    });
    let mut commits = binding
        .authorize()
        .function()
        .cleanup_plan
        .exits
        .iter()
        .filter(|e| {
            matches!(
                e.continuation,
                crate::cleanup_plan::ExitContinuation::CommitResult { .. }
            )
        });
    if !valid
        || commits
            .next()
            .is_none_or(|e| !e.finalize_in_order.is_empty())
        || commits.next().is_some()
        || permit.validate_guard().is_err()
    {
        return LiveReadyPromotionOutcomeV8::Refused(owner);
    }
    let consumed = owner.consumed;
    let ready = match settle_owned_authorize_v2(
        owner.staged,
        || permit.validate_guard().is_ok(),
        |_| unreachable!("proved empty success vector"),
    ) {
        Ok(OwnedAuthorizeSettledV2::Ready(ready)) => LiveReadyAuthorizationV8 { ready, consumed },
        Ok(OwnedAuthorizeSettledV2::Failed { .. }) => {
            unreachable!("preproved successful full Decision")
        }
        Err(rejected) => {
            return LiveReadyPromotionOutcomeV8::Refused(LiveStagedAuthorizationV8 {
                staged: rejected.staged,
                consumed,
            })
        }
    };
    if permit.validate_guard().is_err() {
        LiveReadyPromotionOutcomeV8::GuardLost(ready)
    } else {
        LiveReadyPromotionOutcomeV8::Ready(ready)
    }
}
impl LiveStagedAuthorizationV8 {
    pub(crate) fn failure(
        &self,
    ) -> Option<&crate::interpreter::resumable::owned_frame::OwnedFrameFailure> {
        self.staged.failure()
    }
    pub(crate) fn consumed(&self) -> u64 {
        self.consumed
    }
    pub(crate) fn checked_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Option<(serde_json::Value, serde_json::Value)> {
        self.staged.live_staged_facts(binding)
    }
    #[cfg(test)]
    pub(crate) fn test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        // Actual combined inventory, not inert Decision bytes.
        self.staged.live_test_weak()
    }
}
pub(crate) enum LiveAuthorizeOutcomeV8 {
    Staged(LiveStagedAuthorizationV8),
    Refused(LiveTransferredStateV8),
    Failed(LiveStagedAuthorizationV8),
    GuardLost(LiveStagedAuthorizationV8),
}
pub(crate) fn authorize_live_owned_state_v8(
    permit: LiveAuthorizePermitV8<'_>,
    owner: LiveTransferredStateV8,
) -> LiveAuthorizeOutcomeV8 {
    authorize_with_guard_v8(AuthorizeGuardV8::Initial(&permit), owner)
}
pub(in crate::interpreter::resumable::owned_frame::registered_stage) fn authorize_with_guard_v8(
    permit: AuthorizeGuardV8<'_, '_, '_>,
    owner: LiveTransferredStateV8,
) -> LiveAuthorizeOutcomeV8 {
    if permit.validate_guard().is_err() {
        return LiveAuthorizeOutcomeV8::Refused(owner);
    }
    let mut budget = OwnedFrameBudget::new(permit.fuel()).expect("checked exact Authorize F");
    #[cfg(test)]
    if matches!(&permit, AuthorizeGuardV8::Continued(_)) {
        super::test_continued_authorize_entry_v8();
    }
    let staged =
        match stage_owned_authorize_v2(owner.state, permit.binding().authorize(), &mut budget) {
            Ok(staged) => staged,
            Err(rejected) => {
                return LiveAuthorizeOutcomeV8::Refused(LiveTransferredStateV8 {
                    state: rejected.state,
                })
            }
        };
    let failed = staged.failure().is_some();
    let owner = LiveStagedAuthorizationV8 {
        staged,
        consumed: budget.consumed() as u64,
    };
    if permit.validate_guard().is_err() {
        LiveAuthorizeOutcomeV8::GuardLost(owner)
    } else if failed {
        LiveAuthorizeOutcomeV8::Failed(owner)
    } else {
        LiveAuthorizeOutcomeV8::Staged(owner)
    }
}

/// Exhaustive compiler-owned stage guard: no caller trait or closure authority.
pub(in crate::interpreter::resumable::owned_frame::registered_stage) enum TransferGuardV8<
    'g,
    'p,
    'j,
> {
    Initial(&'g LiveStateTransferPermitV8<'j>),
    Continued(
        &'g crate::live_invocation::source_journal::LiveContinuedStateTransferPermitV8<'p, 'j>,
    ),
}
impl TransferGuardV8<'_, '_, '_> {
    fn validate_guard(
        &self,
    ) -> Result<(), crate::live_invocation::source_journal::SourceJournalError> {
        match self {
            Self::Initial(x) => x.validate_guard(),
            Self::Continued(x) => x.validate_guard(),
        }
    }
    fn binding(&self) -> &CheckedOwnedAgentWaitBindingV8 {
        match self {
            Self::Initial(x) => x.binding(),
            Self::Continued(x) => x.binding(),
        }
    }
}
pub(in crate::interpreter::resumable::owned_frame::registered_stage) enum AuthorizeGuardV8<
    'g,
    'p,
    'j,
> {
    Initial(&'g LiveAuthorizePermitV8<'j>),
    Continued(&'g crate::live_invocation::source_journal::LiveContinuedAuthorizePermitV8<'p, 'j>),
}
impl AuthorizeGuardV8<'_, '_, '_> {
    fn validate_guard(
        &self,
    ) -> Result<(), crate::live_invocation::source_journal::SourceJournalError> {
        match self {
            Self::Initial(x) => x.validate_guard(),
            Self::Continued(x) => x.validate_guard(),
        }
    }
    fn binding(&self) -> &CheckedOwnedAgentWaitBindingV8 {
        match self {
            Self::Initial(x) => x.binding(),
            Self::Continued(x) => x.binding(),
        }
    }
    fn fuel(&self) -> usize {
        match self {
            Self::Initial(x) => x.fuel(),
            Self::Continued(x) => x.fuel(),
        }
    }
}
