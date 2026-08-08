#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
//! Unpublished, thread-confined physical ownership host for SEMAPRAX.
//!
//! This crate connects the pointer-free compiler descriptor, exact native
//! loader lease, authenticated capability codec, and independently tested host
//! ownership ledger. It deliberately exposes no callable symbol or raw loader
//! handle, and therefore does not weaken the compiler's `SPX-B104` gate.

mod authority;
mod descriptor;

// Temporary audited source sharing keeps these protocol implementations
// private in both crates while avoiding a second security-critical codec or
// ledger. A private workspace protocol crate should replace these path modules
// before either host API becomes a supported external integration surface.
#[path = "../../../src/host_ownership.rs"]
mod host_ownership;
#[path = "../../../src/codegen/native_capability_token.rs"]
mod native_capability_token;

mod conformance {
    pub(crate) use semaprax::conformance::*;
}

#[cfg(test)]
mod cleanup_plan {
    pub(crate) use semaprax::cleanup_plan::*;
}

use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::rc::Rc;

use authority::{Authority, AuthorityError, Credential};
use descriptor::{Descriptor, DescriptorError, ResultShape};
use host_ownership::{
    HostBoundaryRejection, HostCallContract, HostCallOutcome, HostCallRequest, HostIdentity,
    HostOwnerToken, HostOwnershipRegistry, HostPublishedValue, HostResourceProvenance,
    HostResourceRequirement, HostResultPlan,
};
use semaprax::conformance::NormalizedStatus;
use semaprax_native_loader::{
    open_admitted_exact, ModuleInstanceId, NativeModuleLease, OpenError, MAX_DESCRIPTOR_BYTES,
};

/// One admitted descriptor shape supported by this physical host milestone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultKind {
    ScalarI64,
    OwnedInput { owner_ordinal: usize },
}

/// Stable admission rejection categories. Error text never contains secrets or
/// capability bytes.
#[derive(Debug)]
pub enum AdmissionError {
    Descriptor(DescriptorRejection),
    Loader(OpenError),
    InvalidHostContract,
    AuthorityUnavailable,
}

/// Stable descriptor rejection categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DescriptorRejection {
    Malformed,
    UnsupportedSchema,
    WrongTarget,
    NonCanonical,
    UnsupportedShape,
}

/// Stable rejection before an ownership commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallRejection {
    Draining,
    WrongShape,
    InputCountMismatch,
    WrongModuleInstance,
    WrongResourceType,
    WrongLifecycle,
    StaleOrInvalidCredential,
    GenerationExhausted,
    LedgerRejected,
}

/// A rejected call returns every still-live input credential to its caller.
pub struct RejectedCall {
    rejection: CallRejection,
    owners: Vec<NativeOwner>,
}

impl RejectedCall {
    pub fn rejection(&self) -> CallRejection {
        self.rejection
    }

    pub fn into_owners(self) -> Vec<NativeOwner> {
        self.owners
    }
}

/// Result of an executed scalar call. Rejection is represented separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalarExecution {
    Success(i64),
    Failure(NormalizedStatus),
}

/// Result of an executed owned-result call. Rejection is represented
/// separately and execution failure publishes no owner.
pub enum OwnedExecution {
    Success(NativeOwner),
    Failure(NormalizedStatus),
}

/// Opaque, noncopying owner credential retaining one exact module instance.
///
/// This type intentionally implements neither `Clone` nor `Debug`, and its
/// embedded loader lease makes it neither `Send` nor `Sync`.
///
/// ```compile_fail
/// use semaprax_native_host::NativeOwner;
/// fn clone_owner(owner: NativeOwner) { let _ = owner.clone(); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_host::NativeOwner;
/// fn format_owner(owner: NativeOwner) { let _ = format!("{owner:?}"); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_host::NativeOwner;
/// fn assert_send<T: Send>() {}
/// assert_send::<NativeOwner>();
/// ```
///
/// ```compile_fail
/// use semaprax_native_host::NativeOwner;
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<NativeOwner>();
/// ```
pub struct NativeOwner {
    token: HostOwnerToken,
    credential: Credential,
    resource: String,
    lifecycle: String,
    ledger: Rc<RefCell<LedgerState>>,
    armed: bool,
}

impl NativeOwner {
    /// Process-local identity of the exact admitted module retained by this
    /// owner. This is diagnostic identity, not a raw loader handle.
    pub fn module_instance_id(&self) -> ModuleInstanceId {
        self.credential.module_instance_id()
    }
}

impl Drop for NativeOwner {
    fn drop(&mut self) {
        if self.armed {
            let retired = self.ledger.borrow_mut().registry.retire_owner(self.token);
            assert!(
                retired.is_ok(),
                "live native owner guard must retire exactly one ledger generation"
            );
            self.armed = false;
        }
    }
}

struct LedgerState {
    registry: HostOwnershipRegistry,
}

/// One same-thread physical host for one exact admitted function descriptor.
///
/// The type intentionally implements neither `Clone` nor `Debug`; its exact
/// loader lease makes it neither `Send` nor `Sync`.
///
/// ```compile_fail
/// use semaprax_native_host::NativeHost;
/// fn assert_send<T: Send>() {}
/// assert_send::<NativeHost>();
/// ```
///
/// ```compile_fail
/// use semaprax_native_host::NativeHost;
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<NativeHost>();
/// ```
pub struct NativeHost {
    module_lease: NativeModuleLease,
    descriptor: Descriptor,
    authority: Authority,
    ledger: Rc<RefCell<LedgerState>>,
    module_identity: HostIdentity,
    adapter_identity: HostIdentity,
    contract: HostCallContract,
    host_thread_identity: u64,
    draining: bool,
}

impl NativeHost {
    /// Open a trusted native module and bind its exact descriptor to a fresh
    /// same-thread authority and owner registry.
    ///
    /// # Safety
    ///
    /// The caller must satisfy every safety requirement of
    /// [`semaprax_native_loader::open_admitted_exact`]. In particular, loading
    /// may execute initializers before descriptor admission, so the exact root
    /// artifact, dependency namespace, descriptor getter ABI, immutable byte
    /// range, and absence of foreign unwind must already be trusted.
    #[allow(
        clippy::result_large_err,
        reason = "admission keeps the exact typed loader rejection available as its source"
    )]
    pub unsafe fn open_admitted_exact(
        canonical_library_path: &Path,
        getter_symbol: &[u8],
        expected_descriptor: &[u8],
    ) -> Result<Self, AdmissionError> {
        if expected_descriptor.is_empty() || expected_descriptor.len() > MAX_DESCRIPTOR_BYTES {
            return Err(AdmissionError::Loader(
                OpenError::InvalidExpectedDescriptorLength {
                    actual: expected_descriptor.len(),
                    maximum: MAX_DESCRIPTOR_BYTES,
                },
            ));
        }
        let descriptor = Descriptor::parse(expected_descriptor).map_err(AdmissionError::from)?;
        if descriptor.has_scalar_parameters() {
            return Err(AdmissionError::Descriptor(
                DescriptorRejection::UnsupportedShape,
            ));
        }
        if getter_symbol != descriptor.getter_symbol().as_bytes() {
            return Err(AdmissionError::Descriptor(
                DescriptorRejection::NonCanonical,
            ));
        }
        // SAFETY: This constructor preserves and documents the complete unsafe
        // contract of the quarantined loader above; no weaker safe path exists.
        let module_lease = unsafe {
            open_admitted_exact(canonical_library_path, getter_symbol, expected_descriptor)
        }
        .map_err(AdmissionError::Loader)?;
        let instance_id = module_lease.instance_id();
        let host_thread_identity = instance_id.get();
        let adapter_text = format!(
            "semaprax.native-host-adapter.v1:{}:{}:{}",
            descriptor.module,
            descriptor.function,
            instance_id.get()
        );
        let module_identity = HostIdentity::try_new(descriptor.module.clone())
            .map_err(|_| AdmissionError::InvalidHostContract)?;
        let adapter_identity = HostIdentity::try_new(adapter_text.clone())
            .map_err(|_| AdmissionError::InvalidHostContract)?;
        let function_identity = HostIdentity::try_new(descriptor.function.clone())
            .map_err(|_| AdmissionError::InvalidHostContract)?;
        let inputs = descriptor
            .owner_requirements()
            .into_iter()
            .map(|(resource, lifecycle)| {
                Ok(HostResourceRequirement::new(
                    HostIdentity::try_new(resource.to_owned())?,
                    HostIdentity::try_new(lifecycle.to_owned())?,
                ))
            })
            .collect::<Result<Vec<_>, HostBoundaryRejection>>()
            .map_err(|_| AdmissionError::InvalidHostContract)?;
        let result = match descriptor.result {
            ResultShape::ScalarI64 => HostResultPlan::Scalar,
            ResultShape::OwnedInput { owner_ordinal, .. } => HostResultPlan::OwnedInput {
                input_index: owner_ordinal,
            },
        };
        let contract = HostCallContract::try_new(
            module_identity.clone(),
            adapter_identity.clone(),
            function_identity,
            host_thread_identity,
            inputs,
            result,
        )
        .map_err(|_| AdmissionError::InvalidHostContract)?;
        let authority = Authority::from_os(
            module_lease.retain(),
            descriptor.physical_module,
            adapter_text.as_bytes(),
        )
        .map_err(|_| AdmissionError::AuthorityUnavailable)?;
        let registry =
            HostOwnershipRegistry::try_new().map_err(|_| AdmissionError::InvalidHostContract)?;
        let ledger = Rc::new(RefCell::new(LedgerState { registry }));
        Ok(Self {
            module_lease,
            descriptor,
            authority,
            ledger,
            module_identity,
            adapter_identity,
            contract,
            host_thread_identity,
            draining: false,
        })
    }

    pub fn module_instance_id(&self) -> ModuleInstanceId {
        self.module_lease.instance_id()
    }

    pub fn result_kind(&self) -> ResultKind {
        match self.descriptor.result {
            ResultShape::ScalarI64 => ResultKind::ScalarI64,
            ResultShape::OwnedInput { owner_ordinal, .. } => {
                ResultKind::OwnedInput { owner_ordinal }
            }
        }
    }

    pub fn is_draining(&self) -> bool {
        self.draining
    }

    /// Current logical live-owner count for quiescence and audit checks.
    pub fn live_owner_count(&self) -> usize {
        self.ledger.borrow().registry.live_owner_count()
    }

    /// Enter the one-way draining state. Existing owners keep their exact
    /// loader pins, while new owners and calls are rejected.
    pub fn begin_draining(&mut self) {
        self.draining = true;
    }

    /// Adopt a payload created by this already admitted trusted adapter.
    ///
    /// # Safety
    ///
    /// `payload` must be one newly created, live instance of the resource at
    /// `owner_ordinal`, owned exclusively by this exact adapter/module binding.
    /// It must not already be represented by any owner credential. The host
    /// does not interpret zero as an ownership sentinel.
    pub unsafe fn adopt_trusted_owner(
        &mut self,
        owner_ordinal: usize,
        payload: u64,
    ) -> Result<NativeOwner, CallRejection> {
        if self.draining {
            return Err(CallRejection::Draining);
        }
        let (resource, lifecycle, _) = self
            .descriptor
            .owned_parameter(owner_ordinal)
            .ok_or(CallRejection::WrongShape)?;
        let provenance = HostResourceProvenance::try_new(
            self.module_identity.clone(),
            self.adapter_identity.clone(),
            HostIdentity::try_new(resource.to_owned())
                .map_err(|_| CallRejection::WrongResourceType)?,
            HostIdentity::try_new(lifecycle.to_owned())
                .map_err(|_| CallRejection::WrongLifecycle)?,
            self.host_thread_identity,
        )
        .map_err(map_ledger_error)?;
        let token = self
            .ledger
            .borrow_mut()
            .registry
            .register_adapter_owner(provenance, payload)
            .map_err(map_ledger_error)?;
        let credential = self
            .authority
            .mint_owner(
                resource.as_bytes(),
                lifecycle.as_bytes(),
                token.slot(),
                token.generation(),
            )
            .map_err(map_authority_error)?;
        Ok(NativeOwner {
            token,
            credential,
            resource: resource.to_owned(),
            lifecycle: lifecycle.to_owned(),
            ledger: Rc::clone(&self.ledger),
            armed: true,
        })
    }

    /// Execute the trusted scalar body after exact non-mutating credential
    /// preflight. All owners are consumed after execution begins.
    ///
    /// # Safety
    ///
    /// The closure is part of the audited generated adapter. It must implement
    /// the admitted function template and may not retain, duplicate, finalize,
    /// or reinterpret the opaque payload integers outside that contract.
    pub unsafe fn execute_scalar_with<F>(
        &mut self,
        mut owners: Vec<NativeOwner>,
        execute: F,
    ) -> Result<ScalarExecution, RejectedCall>
    where
        F: FnOnce(&[u64]) -> Result<i64, NormalizedStatus>,
    {
        if self.draining {
            return Err(rejected(CallRejection::Draining, owners));
        }
        if self.descriptor.result != ResultShape::ScalarI64 {
            return Err(rejected(CallRejection::WrongShape, owners));
        }
        if let Err(rejection) = self.preflight_owners(&owners) {
            return Err(rejected(rejection, owners));
        }
        let tokens = owners.iter().map(|owner| owner.token).collect();
        let request =
            HostCallRequest::new(self.contract.clone(), self.host_thread_identity, tokens);
        let prepared = match self.ledger.borrow_mut().registry.prepare_scalar(request) {
            Ok(prepared) => prepared,
            Err(rejection) => return Err(rejected(map_ledger_error(rejection), owners)),
        };
        for owner in &mut owners {
            owner.armed = false;
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| execute(prepared.payloads())));
        let outcome = match outcome {
            Ok(result) => self
                .ledger
                .borrow_mut()
                .registry
                .complete_prepared_scalar(prepared, result),
            Err(payload) => {
                // A hostile panic payload may itself panic from `Drop`.
                // Forgetting the already-unwound payload is the only way to
                // preserve this containment boundary without executing
                // attacker-controlled destruction outside `catch_unwind`.
                std::mem::forget(payload);
                let mut ledger = self.ledger.borrow_mut();
                ledger.registry.abandon_prepared(prepared);
                let status = ledger
                    .registry
                    .take_last_abandonment()
                    .expect("detached ledger invocation records every adapter panic");
                return Ok(ScalarExecution::Failure(status));
            }
        };
        match outcome {
            HostCallOutcome::ExecutedSuccess(HostPublishedValue::Scalar(value)) => {
                Ok(ScalarExecution::Success(value))
            }
            HostCallOutcome::ExecutedFailure(status) => Ok(ScalarExecution::Failure(status)),
            HostCallOutcome::ExecutedSuccess(HostPublishedValue::Owner(_)) => {
                unreachable!("validated scalar contract cannot publish an owner")
            }
        }
    }

    /// Execute the trusted owned-result body after exact non-mutating
    /// credential preflight. The next-generation result and owner credentials
    /// are both prepared before ingress commit.
    ///
    /// # Safety
    ///
    /// The closure is part of the audited generated adapter. It must implement
    /// the admitted function template and may not retain, duplicate, finalize,
    /// or reinterpret the opaque payload integers outside that contract.
    pub unsafe fn execute_owned_with<F>(
        &mut self,
        mut owners: Vec<NativeOwner>,
        execute: F,
    ) -> Result<OwnedExecution, RejectedCall>
    where
        F: FnOnce(&[u64]) -> Result<(), NormalizedStatus>,
    {
        if self.draining {
            return Err(rejected(CallRejection::Draining, owners));
        }
        let ResultShape::OwnedInput { owner_ordinal, .. } = self.descriptor.result else {
            return Err(rejected(CallRejection::WrongShape, owners));
        };
        if let Err(rejection) = self.preflight_owners(&owners) {
            return Err(rejected(rejection, owners));
        }
        let selected = &owners[owner_ordinal];
        let selected_token = selected.token;
        let result_resource = selected.resource.clone();
        let result_lifecycle = selected.lifecycle.clone();
        let Some(next_generation) = selected_token.generation().checked_add(1) else {
            return Err(rejected(CallRejection::GenerationExhausted, owners));
        };
        let result_credential = match self.authority.mint_result(
            &self.descriptor.function_template,
            result_resource.as_bytes(),
            result_lifecycle.as_bytes(),
            selected_token.slot(),
            next_generation,
        ) {
            Ok(value) => value,
            Err(error) => return Err(rejected(map_authority_error(error), owners)),
        };
        if let Err(error) = self.authority.authenticate_result(
            &self.descriptor.function_template,
            &result_credential,
            result_resource.as_bytes(),
            result_lifecycle.as_bytes(),
            selected_token.slot(),
            next_generation,
        ) {
            return Err(rejected(map_authority_error(error), owners));
        }
        let owner_credential = match self.authority.mint_owner(
            result_resource.as_bytes(),
            result_lifecycle.as_bytes(),
            selected_token.slot(),
            next_generation,
        ) {
            Ok(value) => value,
            Err(error) => return Err(rejected(map_authority_error(error), owners)),
        };
        let tokens = owners.iter().map(|owner| owner.token).collect();
        let request =
            HostCallRequest::new(self.contract.clone(), self.host_thread_identity, tokens);
        let prepared = match self.ledger.borrow_mut().registry.prepare_owned(request) {
            Ok(prepared) => prepared,
            Err(rejection) => return Err(rejected(map_ledger_error(rejection), owners)),
        };
        for owner in &mut owners {
            owner.armed = false;
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| execute(prepared.payloads())));
        let outcome = match outcome {
            Ok(result) => self
                .ledger
                .borrow_mut()
                .registry
                .complete_prepared_owned(prepared, result),
            Err(payload) => {
                // See the scalar path: never drop an attacker-controlled
                // panic payload after its unwind has been contained.
                std::mem::forget(payload);
                let mut ledger = self.ledger.borrow_mut();
                ledger.registry.abandon_prepared(prepared);
                let status = ledger
                    .registry
                    .take_last_abandonment()
                    .expect("detached ledger invocation records every adapter panic");
                return Ok(OwnedExecution::Failure(status));
            }
        };
        match outcome {
            HostCallOutcome::ExecutedFailure(status) => Ok(OwnedExecution::Failure(status)),
            HostCallOutcome::ExecutedSuccess(HostPublishedValue::Owner(token)) => {
                debug_assert_eq!(token.slot(), selected_token.slot());
                debug_assert_eq!(token.generation(), next_generation);
                drop(result_credential);
                Ok(OwnedExecution::Success(NativeOwner {
                    token,
                    credential: owner_credential,
                    resource: result_resource,
                    lifecycle: result_lifecycle,
                    ledger: Rc::clone(&self.ledger),
                    armed: true,
                }))
            }
            HostCallOutcome::ExecutedSuccess(HostPublishedValue::Scalar(_)) => {
                unreachable!("validated owned contract cannot publish a scalar")
            }
        }
    }

    fn preflight_owners(&self, owners: &[NativeOwner]) -> Result<(), CallRejection> {
        let requirements = self.descriptor.owner_requirements();
        if owners.len() != requirements.len() {
            return Err(CallRejection::InputCountMismatch);
        }
        for (owner, (resource, lifecycle)) in owners.iter().zip(requirements) {
            if owner.resource != resource {
                return Err(CallRejection::WrongResourceType);
            }
            if owner.lifecycle != lifecycle {
                return Err(CallRejection::WrongLifecycle);
            }
            self.authority
                .authenticate_owner(
                    &owner.credential,
                    resource.as_bytes(),
                    lifecycle.as_bytes(),
                    owner.token.slot(),
                    owner.token.generation(),
                )
                .map_err(map_authority_error)?;
        }
        Ok(())
    }
}

impl Credential {
    fn module_instance_id(&self) -> ModuleInstanceId {
        self.instance_id()
    }
}

impl From<DescriptorError> for AdmissionError {
    fn from(value: DescriptorError) -> Self {
        Self::Descriptor(match value {
            DescriptorError::Malformed => DescriptorRejection::Malformed,
            DescriptorError::UnsupportedSchema => DescriptorRejection::UnsupportedSchema,
            DescriptorError::WrongTarget => DescriptorRejection::WrongTarget,
            DescriptorError::NonCanonical => DescriptorRejection::NonCanonical,
        })
    }
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Descriptor(_) => "native adapter descriptor was rejected",
            Self::Loader(_) => "native module loading or descriptor admission failed",
            Self::InvalidHostContract => "compiler-derived host contract was rejected",
            Self::AuthorityUnavailable => "native capability authority is unavailable",
        };
        formatter.write_str(message)
    }
}

impl Error for AdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Loader(error) => Some(error),
            _ => None,
        }
    }
}

fn rejected(rejection: CallRejection, owners: Vec<NativeOwner>) -> RejectedCall {
    RejectedCall { rejection, owners }
}

fn map_authority_error(error: AuthorityError) -> CallRejection {
    match error {
        AuthorityError::WrongModuleInstance => CallRejection::WrongModuleInstance,
        AuthorityError::WrongThread
        | AuthorityError::Token(_)
        | AuthorityError::InvalidBinding
        | AuthorityError::EntropyUnavailable
        | AuthorityError::InvalidEntropy => CallRejection::StaleOrInvalidCredential,
    }
}

fn map_ledger_error(_error: HostBoundaryRejection) -> CallRejection {
    CallRejection::LedgerRejected
}
