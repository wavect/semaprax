//! Actual typed execution plus independently retained physical registration.
//! A context proves expected journal facts; it grants no append/owner authority.
use super::*;
use crate::execution_revision::typed::CheckedTypedOwnedWaitExecutionV8;
use crate::resumable_effects::owned_frame::{
    SourceOwnedWaitLeaseV8, SourceOwnedWaitLimitsV8, SourceOwnedWaitStoreRegistrationV8,
};
use serde_json::json;
use std::sync::Arc;

pub(crate) struct CheckedOwnedWaitJournalContextV8 {
    execution: Arc<CheckedTypedOwnedWaitExecutionV8>,
    registration: SourceOwnedWaitStoreRegistrationV8,
    fold: FoldContextV8,
    creator: u32,
}
impl CheckedOwnedWaitJournalContextV8 {
    pub(super) fn fold(&self) -> &FoldContextV8 {
        &self.fold
    }
    pub(crate) fn generation(&self) -> &str {
        self.registration.generation()
    }
    pub(crate) fn ordinary(&self) -> &SourceInvocationBinding {
        &self.fold.ordinary
    }
    pub(crate) fn binding(&self) -> &str {
        self.execution.wait().binding()
    }
    pub(crate) fn validate_lease(
        &self,
        lease: &SourceOwnedWaitLeaseV8,
    ) -> Result<(), SourceJournalError> {
        if self.creator != std::process::id() {
            return Err(SourceJournalError::Binding);
        }
        lease
            .validate_registration(&self.registration)
            .map_err(|_| SourceJournalError::Binding)
    }
}
pub(crate) fn checked_owned_wait_journal_context_v8(
    execution: Arc<CheckedTypedOwnedWaitExecutionV8>,
    lease: &SourceOwnedWaitLeaseV8,
    registration: &SourceOwnedWaitStoreRegistrationV8,
) -> Result<CheckedOwnedWaitJournalContextV8, SourceJournalError> {
    let expected = registration.expected_facts();
    // PID/scope/profile and complete four physical pins are rechecked before
    // deriving any recorded facts. Later physical work must recheck the lease.
    lease
        .validate_registration(registration)
        .map_err(|_| SourceJournalError::Binding)?;
    let b = execution.wait();
    let ordinary = execution.ordinary();
    let limits = SourceOwnedWaitLimitsV8 {
        max_steps_per_stage: ordinary
            .max_steps_per_stage()
            .ok_or(SourceJournalError::Binding)?,
        max_total_steps: u64::try_from(
            ordinary
                .max_total_steps()
                .ok_or(SourceJournalError::Binding)?,
        )
        .map_err(|_| SourceJournalError::Capacity)?,
        max_stages: ordinary.max_stages() as usize,
        max_attempts: ordinary.max_attempts() as usize,
        response_limit: ordinary.response_limit(),
    };
    let invocation = wire::recipe_digest(
        wire::RecipeV8::Invocation,
        &json!({"execution":ordinary.invocation(),"owned_wait_binding":b.binding()}),
    )?;
    if expected.scope.invocation_id() != invocation
        || expected.execution != ordinary.invocation()
        || expected.binding != b.binding()
        || expected.scope.program_root() != b.lifecycle().source_revision()
        || expected.limits != limits
        || execution.evaluation_fuel() != limits.max_steps_per_stage
    {
        return Err(SourceJournalError::Binding);
    }
    let scope = json!({"program_root":expected.scope.program_root(),"invocation":expected.scope.invocation_id(),"policy_epoch":expected.scope.policy_epoch()});
    let created = model::OwnedBodyV8::OwnedRunCreated {
        scope,
        execution: ordinary.invocation().into(),
        binding: b.binding().into(),
        signature: b.signature().clone(),
        limits: limits.json().map_err(|_| SourceJournalError::Binding)?,
        store_identity: registration.identity().json(),
    };
    // The actual compiler vector decides whether Refused owns no leaf. No
    // caller boolean or observed empty receipt can replace this proof.
    let refused_cleanup_empty = b.authorize().disposal().iter().all(|action| {
        action
            .active_case
            .as_ref()
            .is_some_and(|case| case.case != *b.authorize().refused())
    });
    // E remains immutable. Only this closed constructor creates the separate
    // v8 ordinary validation binding whose attempt identities use exact I8.
    let validation_ordinary = SourceInvocationBinding {
        invocation,
        ..ordinary.clone()
    };
    let fold = FoldContextV8 {
        ordinary: validation_ordinary,
        created,
        plan_digest: b.binding().into(),
        cleanup_plan_digest: b.cleanup_digest().into(),
        signature: b.signature().clone(),
        helper: b.helper().function().id.as_str().into(),
        authorize: b.authorize().function().id.as_str().into(),
        granted: b.authorize().granted().as_str().into(),
        refused: b.authorize().refused().as_str().into(),
        refused_cleanup_empty,
        checked_binding: execution.wait_arc(),
    };
    Ok(CheckedOwnedWaitJournalContextV8 {
        execution,
        registration: registration.clone(),
        fold,
        creator: std::process::id(),
    })
}
