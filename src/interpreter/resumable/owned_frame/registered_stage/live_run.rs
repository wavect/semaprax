//! Actual roots for the private initialized live actor. No inert snapshot can
//! construct these holders; the journal supplies a one-use ACKed entry permit.
use super::initialize::{
    admit_owned_task_input_v2, settle_owned_initialize_v2, stage_owned_initialize_v2,
    OwnedInitializeSettledV2, StagedOwnedInitializeV2,
};
use super::*;
use crate::live_invocation::source_journal::LiveInitializePermitV8;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedInitializeV2;

pub(crate) enum LiveInitializeOutcomeV8 {
    Initialized(LiveInitializedStateV8),
    Failed(LiveFailedInitializeV8),
}
pub(crate) struct LiveInitializedStateV8 {
    state: Option<OwnedAgentStateArgument>,
    facts: serde_json::Value,
    consumed: u64,
    #[cfg(test)]
    task_backings: Vec<std::sync::Weak<[u8]>>,
}
pub(crate) struct LiveFailedInitializeV8 {
    staged: StagedOwnedInitializeV2,
    consumed: u64,
}
impl Drop for LiveInitializedStateV8 {
    fn drop(&mut self) {
        // The ordinary foundation's Argument Drop is a semantic disposer.
        // Live abandonment has no CleanupStarted ACK: disarm it first, then
        // release process backing only while the enclosing actor holds store.
        if let Some(state) = self.state.as_mut() {
            drop(state.root.take());
        }
    }
}
impl LiveInitializedStateV8 {
    pub(crate) fn facts(&self) -> &serde_json::Value {
        &self.facts
    }
    pub(crate) fn consumed(&self) -> u64 {
        self.consumed
    }
    #[cfg(test)]
    pub(crate) fn test_same_task_backings(&self) -> bool {
        let state = self.test_weak();
        state.len() == self.task_backings.len()
            && state.iter().all(|s| {
                self.task_backings
                    .iter()
                    .any(|t| std::sync::Weak::ptr_eq(s, t))
            })
    }
    #[cfg(test)]
    pub(crate) fn test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        super::super::snapshot::weak_leaves(
            self.state
                .as_ref()
                .expect("live State")
                .root
                .as_ref()
                .expect("actual initialized State"),
        )
    }
}
impl LiveFailedInitializeV8 {
    pub(crate) fn consumed(&self) -> u64 {
        self.consumed
    }
    pub(crate) fn failure(&self) -> Option<&OwnedFrameFailure> {
        self.staged.failure()
    }
}
/// Uses the permit's actual held guard before evaluation and publication.
/// A failure remains a partial/provisional holder; no durable receipt exists.
pub(crate) fn initialize_live_owned_run_v8(
    permit: LiveInitializePermitV8<'_>,
    plan: &CheckedOwnedInitializeV2,
    input: OwnedFrameInput,
) -> Result<LiveInitializeOutcomeV8, OwnedFrameInputRejection> {
    if permit.validate_guard().is_err() {
        return Err(OwnedFrameInputRejection {
            input,
            diagnostic: rejected("live initializer entry authority differs"),
        });
    }
    let argument = admit_owned_task_input_v2(plan, input)?;
    #[cfg(test)]
    let task_backings = argument.test_weak();
    let mut fuel = OwnedFrameBudget::new(permit.fuel()).expect("checked positive stage allowance");
    if permit.validate_guard().is_err() {
        fuel.cancel();
    }
    let mut staged = stage_owned_initialize_v2(argument, plan, &mut fuel)
        .unwrap_or_else(|_| panic!("fresh checked Task and same initializer proof"));
    let consumed = fuel.consumed() as u64;
    if permit.validate_guard().is_err() {
        staged.abandon_after_live_guard();
    }
    if staged.failure().is_some() {
        return Ok(LiveInitializeOutcomeV8::Failed(LiveFailedInitializeV8 {
            staged,
            consumed,
        }));
    }
    let settled = settle_owned_initialize_v2(staged, || permit.validate_guard().is_ok(), |_| {})
        .map_err(|rejection| rejection.staged);
    match settled {
        Ok(OwnedInitializeSettledV2::Initialized(state)) => {
            let facts = record_facts(&state).expect("checked complete State profile");
            Ok(LiveInitializeOutcomeV8::Initialized(
                LiveInitializedStateV8 {
                    state: Some(state),
                    facts,
                    consumed,
                    #[cfg(test)]
                    task_backings,
                },
            ))
        }
        Err(mut staged) => {
            staged.abandon_after_live_guard();
            Ok(LiveInitializeOutcomeV8::Failed(LiveFailedInitializeV8 {
                staged,
                consumed,
            }))
        }
        Ok(OwnedInitializeSettledV2::Failed { .. }) => {
            unreachable!("successful compiler-empty settlement")
        }
    }
}
fn record_facts(state: &OwnedAgentStateArgument) -> Option<serde_json::Value> {
    root_facts(&state.plan, state.root.as_ref()?)
}
pub(super) fn root_facts(
    plan: &CheckedOwnedFrameHelperV2,
    root: &Value,
) -> Option<serde_json::Value> {
    if !root_valid(plan, root) {
        return None;
    }
    let Value::Record(record) = root else {
        return None;
    };
    let fields = plan.program().declarations.record_fields(&record.record)?;
    let values = fields.iter().map(|field| {
        let value = match record.fields.get(&field.id)? {
            Value::Bytes(bytes) => serde_json::json!({"kind":"bytes","hex":crate::live_invocation::identity::hex(&bytes.bytes)}),
            Value::Bool(v) => serde_json::json!({"tag":"bool","value":v}),
            Value::Int32(v) => serde_json::json!({"tag":"i32","value":v}),
            Value::Int(v) => serde_json::json!({"tag":"i64","value":v}),
            Value::Uint8(v) => serde_json::json!({"tag":"u8","value":v}),
            Value::Usize(v) => serde_json::json!({"tag":"usize","value":v}),
            _ => return None,
        };
        Some(serde_json::json!({"identity":field.id.as_str(),"value":value}))
    }).collect::<Option<Vec<_>>>()?;
    Some(serde_json::json!({"declaration":record.record.as_str(),"fields":values}))
}

mod observe;
pub(crate) use observe::{observe_live_owned_run_v8, LiveObserveOutcomeV8, LiveObservedStateV8};

mod wait;
pub(crate) use wait::{begin_live_owned_wait_v8, LiveParkedStateV8, LiveWaitStartOutcomeV8};

mod resume;
pub(crate) use resume::{resume_live_owned_wait_v8, LiveResumedStateV8, LiveWaitResumeOutcomeV8};

mod authorize;
pub(crate) use authorize::{
    authorize_live_owned_state_v8, promote_live_owned_authorization_v8,
    transfer_live_owned_state_v8, LiveAuthorizeOutcomeV8, LiveReadyAuthorizationV8,
    LiveReadyEffectPreparationRejectionV8, LiveReadyPromotionOutcomeV8, LiveStagedAuthorizationV8,
    LiveStateTransferOutcomeV8, LiveTransferredStateV8,
};

pub(crate) use observe::{
    InitialObserveStateCleanupFailureV8, LiveFailedObserveV8, ReleasedInitialObserveStateV8,
};

pub(crate) use wait::{
    begin_live_continued_wait_v8, LiveContinuedParkedStateV8, LiveContinuedTerminalStateV8,
    LiveContinuedWaitStartOutcomeV8,
};
