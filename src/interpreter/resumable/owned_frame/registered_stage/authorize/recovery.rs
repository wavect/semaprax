//! One-use restoration of the exact authenticated TransferCompleted State.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    restore_transferred_state_v8 as restore_live_transferred_state_v8,
    LiveTransferredRestoreFailureV8, LiveTransferredStateV8,
};
use crate::live_invocation::source_journal::LiveRecoveredStateTransferPermitV8;

pub(crate) fn restore_transferred_state_v8(
    permit: LiveRecoveredStateTransferPermitV8<'_, '_>,
    input: OwnedFrameInput,
) -> Result<LiveTransferredStateV8, LiveTransferredRestoreFailureV8> {
    restore_live_transferred_state_v8(permit, input)
}
