//! Public inert data and sealed consuming owners. No snapshot-to-owner route.
use super::{driver, plan, store, OwnedFrameError};
use crate::cleanup_plan::{FinalizeAction, OwnedFrameLiveness};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedFunction, ResolvedProgram};
use crate::interpreter::resumable::owned_frame as core;
use crate::interpreter::retained_call::RetainedValue;
use crate::interpreter::ArgumentValue;
use crate::resumable_effects::{
    source_checkpoint::{SourceCheckpointKey, SourceCheckpointScope},
    CapabilityPolicy,
};
use std::fs::File;

pub use core::{
    OwnedFrameInput, OwnedFrameInputField, OwnedFrameInputRejection, OwnedFrameInputValue,
};
#[derive(Clone)]
pub struct CheckedOwnedFramePlan {
    inner: plan::CheckedOwnedFramePlan,
}
impl CheckedOwnedFramePlan {
    pub fn binding(&self) -> &str {
        self.inner.binding()
    }
    pub(crate) fn program(&self) -> &ResolvedProgram {
        self.inner.program()
    }
    pub(crate) fn function(&self) -> &ResolvedFunction {
        self.inner.function()
    }
    pub(crate) fn liveness(&self) -> &OwnedFrameLiveness {
        self.inner.liveness()
    }
}
pub fn compile_owned_frame_plan(
    program: &ResolvedProgram,
    function: &DeclarationId,
) -> Result<CheckedOwnedFramePlan, Diagnostic> {
    plan::compile_owned_frame_plan(program, function).map(|inner| CheckedOwnedFramePlan { inner })
}
pub struct OwnedFrameArgument {
    inner: Option<core::OwnedFrameArgument>,
    creator: u32,
}
impl OwnedFrameArgument {
    fn wrap(inner: core::OwnedFrameArgument) -> Self {
        Self {
            inner: Some(inner),
            creator: std::process::id(),
        }
    }
    fn current(&self) -> Result<(), OwnedFrameError> {
        if self.creator == std::process::id() {
            Ok(())
        } else {
            Err(OwnedFrameError::Policy)
        }
    }
}
impl Drop for OwnedFrameArgument {
    fn drop(&mut self) {
        if self.creator != std::process::id() {
            if let Some(inner) = self.inner.take() {
                inner.drop_backing_only();
            }
        }
    }
}
pub fn admit_owned_frame_input(
    plan: &CheckedOwnedFramePlan,
    input: OwnedFrameInput,
) -> Result<OwnedFrameArgument, OwnedFrameInputRejection> {
    core::admit_owned_frame_input(plan, input).map(OwnedFrameArgument::wrap)
}
pub struct OwnedFrameArgumentRejection {
    pub input: RetainedValue,
    pub diagnostic: Diagnostic,
}
pub fn admit_owned_frame_argument(
    plan: &CheckedOwnedFramePlan,
    input: RetainedValue,
) -> Result<OwnedFrameArgument, OwnedFrameArgumentRejection> {
    core::admit_owned_frame_argument(plan, input)
        .map(OwnedFrameArgument::wrap)
        .map_err(|e| OwnedFrameArgumentRejection {
            input: e.input,
            diagnostic: e.diagnostic,
        })
}
pub struct OwnedFrameResult {
    inner: Option<core::OwnedFrameResult>,
    creator: u32,
}
pub struct OwnedFrameResultRejection {
    pub result: OwnedFrameResult,
    pub error: OwnedFrameError,
}
pub struct OwnedFrameReleaseReceipt {
    pub operations: Vec<FinalizeAction>,
}
impl OwnedFrameResult {
    fn wrap(inner: core::OwnedFrameResult) -> Self {
        Self {
            inner: Some(inner),
            creator: std::process::id(),
        }
    }
    pub fn dispose(mut self) -> Result<OwnedFrameReleaseReceipt, OwnedFrameResultRejection> {
        if self.creator != std::process::id() {
            return Err(OwnedFrameResultRejection {
                result: self,
                error: OwnedFrameError::Policy,
            });
        }
        match self.inner.take().expect("sealed result").dispose() {
            Ok(receipt) => Ok(OwnedFrameReleaseReceipt {
                operations: receipt.operations,
            }),
            Err(e) => {
                self.inner = Some(e.result);
                Err(OwnedFrameResultRejection {
                    result: self,
                    error: OwnedFrameError::Binding,
                })
            }
        }
    }
    pub fn into_argument(
        mut self,
        plan: &CheckedOwnedFramePlan,
    ) -> Result<OwnedFrameArgument, OwnedFrameResultRejection> {
        if self.creator != std::process::id() {
            return Err(OwnedFrameResultRejection {
                result: self,
                error: OwnedFrameError::Policy,
            });
        }
        match self
            .inner
            .take()
            .expect("sealed result")
            .into_argument(plan)
        {
            Ok(argument) => Ok(OwnedFrameArgument::wrap(argument)),
            Err(e) => {
                self.inner = Some(e.result);
                Err(OwnedFrameResultRejection {
                    result: self,
                    error: OwnedFrameError::Binding,
                })
            }
        }
    }
}
impl Drop for OwnedFrameResult {
    fn drop(&mut self) {
        if self.creator != std::process::id() {
            if let Some(inner) = self.inner.take() {
                inner.drop_backing_only();
            }
        }
    }
}
pub struct PreparedOwnedFrame {
    inner: driver::PreparedOwnedFrame,
    creator: u32,
}
impl PreparedOwnedFrame {
    pub fn new(
        plan: &CheckedOwnedFramePlan,
        argument: &OwnedFrameArgument,
        scope: SourceCheckpointScope,
        max_steps: u64,
        max_reserved_fuel: u64,
    ) -> Result<Self, OwnedFrameError> {
        argument.current()?;
        Ok(Self {
            inner: driver::PreparedOwnedFrame::new(
                plan,
                argument.inner.as_ref().expect("sealed argument"),
                scope,
                max_steps,
                max_reserved_fuel,
            )?,
            creator: std::process::id(),
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnedFrameStoreIdentity {
    pub directory_device: u64,
    pub directory_inode: u64,
    pub file_device: u64,
    pub file_inode: u64,
}
impl OwnedFrameStoreIdentity {
    fn raw(self) -> store::OwnedFrameStoreIdentity {
        store::OwnedFrameStoreIdentity {
            directory_device: self.directory_device,
            directory_inode: self.directory_inode,
            file_device: self.file_device,
            file_inode: self.file_inode,
        }
    }
    fn wrap(raw: store::OwnedFrameStoreIdentity) -> Self {
        Self {
            directory_device: raw.directory_device,
            directory_inode: raw.directory_inode,
            file_device: raw.file_device,
            file_inode: raw.file_inode,
        }
    }
}
/// The trusted caller must protect this one history for its entire lifetime
/// against rollback/copy/rebinding. This assertion is not a HMAC uniqueness proof.
pub struct OwnedFrameStoreRegistration {
    inner: store::OwnedFrameStoreRegistration,
    creator: u32,
}
impl OwnedFrameStoreRegistration {
    pub fn grant_for_trusted_host(
        identity: OwnedFrameStoreIdentity,
        scope: &SourceCheckpointScope,
        authoritative_history_available: bool,
    ) -> Result<Self, OwnedFrameError> {
        Ok(Self {
            inner: store::OwnedFrameStoreRegistration::grant_for_trusted_host(
                identity.raw(),
                scope,
                authoritative_history_available,
            )?,
            creator: std::process::id(),
        })
    }
}
pub struct RegisteredOwnedFrameJournalLease {
    inner: store::RegisteredJournalLease,
    creator: u32,
}
impl RegisteredOwnedFrameJournalLease {
    pub fn fresh(
        directory: File,
        expected_directory: (u64, u64),
        scope: &SourceCheckpointScope,
    ) -> Result<Self, OwnedFrameError> {
        Ok(Self {
            inner: store::RegisteredJournalLease::fresh(directory, expected_directory, scope)?,
            creator: std::process::id(),
        })
    }
    pub fn recover(
        directory: File,
        registration: OwnedFrameStoreRegistration,
        scope: &SourceCheckpointScope,
    ) -> Result<Self, OwnedFrameError> {
        if registration.creator != std::process::id() {
            return Err(OwnedFrameError::Policy);
        }
        Ok(Self {
            inner: store::RegisteredJournalLease::recover(directory, registration.inner, scope)?,
            creator: std::process::id(),
        })
    }
    pub fn identity(&self) -> Result<OwnedFrameStoreIdentity, OwnedFrameError> {
        self.inner.validate_current()?;
        Ok(OwnedFrameStoreIdentity::wrap(self.inner.identity()))
    }
}
pub struct OwnedFrameInvocation<'key> {
    inner: driver::OwnedFrameInvocation<'key>,
    creator: u32,
}
pub enum OwnedFrameStart<'key> {
    Rejected {
        argument: OwnedFrameArgument,
        error: OwnedFrameError,
    },
    Invocation {
        invocation: OwnedFrameInvocation<'key>,
        acknowledgement: Result<(), OwnedFrameError>,
    },
}
pub struct OwnedFrameCleanupConfirmation {
    inner: driver::OwnedFrameCleanupConfirmation,
    creator: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedFrameActivePhase {
    Committed,
    StartReserved,
    Yielded,
    DispatchedInDoubt,
    Answered,
    ResumeReserved,
}
/// Read-only classification. It never grants evaluator/cleanup/result authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedFrameInvocationStatus {
    UncommittedStart { created: bool },
    Active(OwnedFrameActivePhase),
    PendingCleanup { failed: bool },
    ReadyToClaim,
    Settled { failed: bool, host_confirmed: bool },
    CleanupInDoubt { failed: bool },
    ResultDeliveryInDoubt,
    PersistenceInDoubt,
}
impl<'key> OwnedFrameInvocation<'key> {
    pub fn status(&self) -> Result<OwnedFrameInvocationStatus, OwnedFrameError> {
        self.current()?;
        Ok(self.inner.status())
    }
    fn current(&self) -> Result<(), OwnedFrameError> {
        if self.creator == std::process::id() {
            Ok(())
        } else {
            Err(OwnedFrameError::Policy)
        }
    }
    pub fn start(
        prepared: PreparedOwnedFrame,
        mut argument: OwnedFrameArgument,
        lease: RegisteredOwnedFrameJournalLease,
        key: &'key SourceCheckpointKey,
        policy: &CapabilityPolicy,
    ) -> OwnedFrameStart<'key> {
        if argument.current().is_err()
            || prepared.creator != std::process::id()
            || lease.creator != std::process::id()
        {
            return OwnedFrameStart::Rejected {
                argument,
                error: OwnedFrameError::Policy,
            };
        }
        match driver::OwnedFrameInvocation::start(
            prepared.inner,
            argument.inner.take().expect("sealed argument"),
            lease.inner,
            key,
            policy,
        ) {
            driver::OwnedFrameStart::Rejected {
                argument: inner,
                error,
            } => OwnedFrameStart::Rejected {
                argument: OwnedFrameArgument::wrap(inner),
                error,
            },
            driver::OwnedFrameStart::Invocation {
                invocation: inner,
                acknowledgement,
            } => OwnedFrameStart::Invocation {
                invocation: Self {
                    inner,
                    creator: std::process::id(),
                },
                acknowledgement,
            },
        }
    }
    pub fn recover(
        lease: RegisteredOwnedFrameJournalLease,
        key: &'key SourceCheckpointKey,
        plan: &CheckedOwnedFramePlan,
        scope: SourceCheckpointScope,
        max_steps: u64,
        max_reserved_fuel: u64,
        policy: &CapabilityPolicy,
    ) -> Result<(Self, Result<(), OwnedFrameError>), OwnedFrameError> {
        if lease.creator != std::process::id() {
            return Err(OwnedFrameError::Policy);
        }
        driver::OwnedFrameInvocation::recover(
            lease.inner,
            key,
            plan,
            scope,
            max_steps,
            max_reserved_fuel,
            policy,
        )
        .map(|(inner, validation)| {
            (
                Self {
                    inner,
                    creator: std::process::id(),
                },
                validation,
            )
        })
    }
    pub fn begin(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
    ) -> Result<(), OwnedFrameError> {
        self.current()?;
        self.inner.begin(policy, scope)
    }
    pub fn dispatch(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
        handler: &mut dyn FnMut(&ArgumentValue) -> Result<ArgumentValue, ()>,
    ) -> Result<(), OwnedFrameError> {
        self.current()?;
        self.inner.dispatch(policy, scope, handler)
    }
    pub fn resume(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
    ) -> Result<(), OwnedFrameError> {
        self.current()?;
        self.inner.resume(policy, scope)
    }
    pub fn abandon(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
    ) -> Result<(), OwnedFrameError> {
        self.current()?;
        self.inner.abandon(policy, scope)
    }
    pub fn settle(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
        observer: &mut dyn FnMut(&FinalizeAction) -> bool,
    ) -> Result<(), OwnedFrameError> {
        self.current()?;
        self.inner.settle(policy, scope, observer)
    }
    pub fn claim(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
    ) -> Result<OwnedFrameResult, OwnedFrameError> {
        self.current()?;
        self.inner.claim(policy, scope).map(OwnedFrameResult::wrap)
    }
    pub fn grant_cleanup_confirmation_for_trusted_host(
        &self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
    ) -> Result<OwnedFrameCleanupConfirmation, OwnedFrameError> {
        self.current()?;
        Ok(OwnedFrameCleanupConfirmation {
            inner: self
                .inner
                .grant_cleanup_confirmation_for_trusted_host(policy, scope)?,
            creator: std::process::id(),
        })
    }
    pub fn confirm_cleanup(
        &mut self,
        policy: &CapabilityPolicy,
        scope: &SourceCheckpointScope,
        grant: OwnedFrameCleanupConfirmation,
    ) -> Result<(), OwnedFrameError> {
        self.current()?;
        if grant.creator != std::process::id() {
            return Err(OwnedFrameError::Policy);
        }
        self.inner.confirm_cleanup(policy, scope, grant.inner)
    }
    pub fn evidence(&self) -> Result<(Vec<u8>, String), OwnedFrameError> {
        self.current()?;
        self.inner.evidence()
    }
}

#[cfg(all(test, unix))]
mod tests;
