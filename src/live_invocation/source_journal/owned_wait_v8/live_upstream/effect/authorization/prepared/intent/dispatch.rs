//! Actual one-use target dispatch. No settlement/cleanup ACK or next owner.
use super::activation::IntentLineageV8;
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::{TargetAccounting, TargetHostHandler};
use crate::interpreter::resumable::owned_frame::registered_stage::effect::{
    dispatch_live_owned_effect_v8, StagedOwnedEffectV8,
};

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveDispatchedOwnedEffectV8<'j>
{
    staged: StagedOwnedEffectV8<'j>,
    accounting: TargetAccounting,
    lineage: IntentLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveEffectDispatchFailureV8<'j> {
    Before {
        _owner: LiveActivatedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
    After {
        _owner: LiveDispatchedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
}
impl<'j> LiveActivatedOwnedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn dispatch(
        self,
        handler: &mut dyn TargetHostHandler,
    ) -> Result<LiveDispatchedOwnedEffectV8<'j>, LiveEffectDispatchFailureV8<'j>> {
        if let Err(error) = self.validate_live() {
            return Err(LiveEffectDispatchFailureV8::Before {
                _owner: self,
                error,
            });
        }
        if self.accounting != TargetAccounting::default() {
            self.activated.quarantine_live_intent();
            return Err(LiveEffectDispatchFailureV8::Before {
                _owner: self,
                error: SourceJournalError::Binding,
            });
        }
        let LiveActivatedOwnedEffectV8 {
            activated,
            mut accounting,
            lineage,
        } = self;
        let (staged, selected) = {
            // The permit borrows lineage only for validation/dispatch. Its
            // held-store lifetime remains 'j; no returned owner borrows it.
            let permit = lineage.permit();
            dispatch_live_owned_effect_v8(activated, &mut accounting, &permit, handler)
        };
        let actual = LiveDispatchedOwnedEffectV8 {
            staged,
            accounting,
            lineage,
        };
        if let Some(error) = selected {
            actual.staged.quarantine_live_dispatch();
            return Err(LiveEffectDispatchFailureV8::After {
                _owner: actual,
                error,
            });
        }
        if let Err(error) = actual.validate_live() {
            return Err(LiveEffectDispatchFailureV8::After {
                _owner: actual,
                error,
            });
        }
        Ok(actual)
    }
}
impl LiveDispatchedOwnedEffectV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.lineage.validate_live()?;
            self.staged.validate_live_dispatch(&self.lineage.permit())?;
            if let Some(actual) = self.staged.dispatch() {
                if actual.evidence().accounting() != self.accounting {
                    return Err(SourceJournalError::Binding);
                }
            } else if self.accounting != TargetAccounting::default() {
                return Err(SourceJournalError::Binding);
            }
            self.lineage.validate_live()
        })();
        if result.is_err() {
            self.staged.quarantine_live_dispatch();
        }
        result
    }
    /// Observation only. This immutable ledger grants no dispatch or refund.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        &self.accounting
    }
}
#[cfg(all(test, unix))]
mod tests;

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod settlement;
