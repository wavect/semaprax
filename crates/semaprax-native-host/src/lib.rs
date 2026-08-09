#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
//! Unpublished, thread-confined physical ownership host for SEMAPRAX.
//!
//! This crate connects the pointer-free compiler descriptor, exact native
//! loader lease, authenticated capability codec, and independently tested host
//! ownership ledger. It deliberately exposes no callable symbol or raw loader
//! handle, and therefore does not weaken the compiler's `SPX-B104` gate.

#[cfg(test)]
mod postcommit_allocation_probe {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    std::thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
        static COUNT: Cell<usize> = const { Cell::new(0) };
        static LAST: Cell<Option<usize>> = const { Cell::new(None) };
    }

    pub(crate) struct ProbeGuard;

    impl ProbeGuard {
        pub(crate) fn finish(self) -> usize {
            let count = COUNT.with(Cell::get);
            ACTIVE.with(|active| active.set(false));
            LAST.with(|last| last.set(Some(count)));
            std::mem::forget(self);
            count
        }
    }

    impl Drop for ProbeGuard {
        fn drop(&mut self) {
            ACTIVE.with(|active| active.set(false));
        }
    }

    pub(crate) fn begin() -> ProbeGuard {
        ACTIVE.with(|active| assert!(!active.replace(true), "allocation probe nested"));
        COUNT.with(|count| count.set(0));
        LAST.with(|last| last.set(None));
        ProbeGuard
    }

    pub(crate) fn take_last() -> Option<usize> {
        LAST.with(|last| last.take())
    }

    pub(crate) struct CountingAllocator;

    impl CountingAllocator {
        fn record(&self) {
            ACTIVE.with(|active| {
                if active.get() {
                    COUNT.with(|count| count.set(count.get().saturating_add(1)));
                }
            });
        }
    }

    // SAFETY: Test-only forwarding allocator preserves `System`'s contract and
    // merely increments thread-local counters before allocation/reallocation.
    #[allow(unsafe_code)]
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            self.record();
            // SAFETY: Forwarding the caller-provided layout unchanged.
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            self.record();
            // SAFETY: Forwarding the caller-provided layout unchanged.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            // SAFETY: Forwarding the allocation and its original layout.
            unsafe { System.dealloc(pointer, layout) }
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            self.record();
            // SAFETY: Forwarding the allocation, original layout and new size.
            unsafe { System.realloc(pointer, layout, size) }
        }
    }
}

#[cfg(test)]
#[global_allocator]
static POSTCOMMIT_COUNTING_ALLOCATOR: postcommit_allocation_probe::CountingAllocator =
    postcommit_allocation_probe::CountingAllocator;

mod authority;
mod callable_semantics;
mod callable_wire;
mod callable_wire_v3;
mod descriptor;
mod descriptor_v2;
#[cfg(test)]
mod descriptor_v2_integration;
mod descriptor_v3;
#[cfg(test)]
mod descriptor_v3_integration;
mod receipt_authority;
mod settlement_host_v3;
#[cfg(test)]
mod settlement_host_v3_integration;
mod settlement_ledger;
mod settlement_proof;

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
use std::num::NonZeroU64;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use authority::{Authority, AuthorityError, Credential, ExactLeasePin};
use callable_semantics::{
    authenticate_dictionary, AuthenticatedSemanticDictionary, ResponseDecodeBuffers,
    ValidatedExecution, ValidatedOutcome,
};
use callable_wire::{encode_request, RequestArgument};
use descriptor::{Descriptor, DescriptorError, ResultShape, ScalarKind};
use descriptor_v2::{
    Descriptor as CallableDescriptor, DescriptorError as CallableDescriptorError,
    Parameter as CallableParameter, ResultShape as CallableResultShape,
    ScalarKind as CallableScalarKind, CALL_RESULT_COMPLETE,
};
use host_ownership::{
    HostBoundaryRejection, HostCallContract, HostCallOutcome, HostCallRequest, HostIdentity,
    HostOwnerToken, HostOwnershipRegistry, HostPreparedInvocation, HostPublishedValue,
    HostResourceProvenance, HostResourceRequirement, HostResultPlan,
};
use semaprax::conformance::{NormalizedStatus, TraceEvent};
use semaprax::semantic_trace::SemanticEventDictionary;
use semaprax::trace_path_certificate::TracePathCertificate;
use semaprax_native_loader::{
    open_admitted_callable_exact, open_admitted_exact, ModuleInstanceId, NativeCallableModuleLease,
    NativeModuleLease, OpenError, MAX_DESCRIPTOR_BYTES,
};

/// One admitted descriptor shape supported by this physical host milestone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultKind {
    ScalarI64,
    OwnedInput { owner_ordinal: usize },
}

/// Canonical scalar values supplied in scalar-parameter order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarValue {
    I64(i64),
    Bool(bool),
}

/// Stable admission rejection categories. Error text never contains secrets or
/// capability bytes.
#[derive(Debug)]
pub enum AdmissionError {
    Descriptor(DescriptorRejection),
    Loader(OpenError),
    SemanticDictionary,
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
    ScalarInputCountMismatch,
    ScalarKindMismatch,
    WrongModuleInstance,
    WrongResourceType,
    WrongLifecycle,
    StaleOrInvalidCredential,
    GenerationExhausted,
    WireRejected,
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

/// Result and authenticated semantic events from one executed callable-v2
/// invocation. Rejection remains represented separately by [`RejectedCall`].
pub struct NativeCallableExecution<T> {
    result: Result<T, NormalizedStatus>,
    events: Vec<Arc<TraceEvent>>,
}

impl<T> NativeCallableExecution<T> {
    pub fn result(&self) -> Result<&T, &NormalizedStatus> {
        self.result.as_ref()
    }

    pub fn events(&self) -> &[Arc<TraceEvent>] {
        &self.events
    }

    pub fn into_parts(self) -> (Result<T, NormalizedStatus>, Vec<Arc<TraceEvent>>) {
        (self.result, self.events)
    }
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
            ExactLeasePin::descriptor_v1(module_lease.retain()),
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
        let credential = match self.authority.mint_owner(
            resource.as_bytes(),
            lifecycle.as_bytes(),
            token.slot(),
            token.generation(),
        ) {
            Ok(credential) => credential,
            Err(error) => {
                self.ledger
                    .borrow_mut()
                    .registry
                    .rollback_adapter_owner(token)
                    .expect("same-thread adoption rollback must restore the latest owner slot");
                return Err(map_authority_error(error));
            }
        };
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
        owners: Vec<NativeOwner>,
        execute: F,
    ) -> Result<ScalarExecution, RejectedCall>
    where
        F: FnOnce(&[u64]) -> Result<i64, NormalizedStatus>,
    {
        // SAFETY: This compatibility entry point preserves its existing
        // closure contract and supplies no scalar values. Scalar preflight
        // rejects scalar-bearing descriptors before ownership commit.
        unsafe {
            self.execute_scalar_with_values(owners, Vec::new(), |payloads, _| execute(payloads))
        }
    }

    /// Execute a trusted scalar-result body with canonical scalar arguments.
    /// Scalar values are ordered by scalar-parameter occurrence; owner values
    /// remain ordered by owner ordinal.
    ///
    /// # Safety
    ///
    /// The closure is part of the audited generated adapter. It must implement
    /// the admitted function template and may not retain, duplicate, finalize,
    /// or reinterpret the opaque payload integers outside that contract.
    pub unsafe fn execute_scalar_with_values<F>(
        &mut self,
        mut owners: Vec<NativeOwner>,
        scalars: Vec<ScalarValue>,
        execute: F,
    ) -> Result<ScalarExecution, RejectedCall>
    where
        F: FnOnce(&[u64], &[ScalarValue]) -> Result<i64, NormalizedStatus>,
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
        if let Err(rejection) = self.preflight_scalars(&scalars) {
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
        let outcome = catch_unwind(AssertUnwindSafe(|| execute(prepared.payloads(), &scalars)));
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
        owners: Vec<NativeOwner>,
        execute: F,
    ) -> Result<OwnedExecution, RejectedCall>
    where
        F: FnOnce(&[u64]) -> Result<(), NormalizedStatus>,
    {
        // SAFETY: This compatibility entry point preserves its existing
        // closure contract and supplies no scalar values. Scalar preflight
        // rejects scalar-bearing descriptors before ownership commit.
        unsafe {
            self.execute_owned_with_values(owners, Vec::new(), |payloads, _| execute(payloads))
        }
    }

    /// Execute a trusted owned-result body with canonical scalar arguments.
    ///
    /// # Safety
    ///
    /// The closure is part of the audited generated adapter. It must implement
    /// the admitted function template and may not retain, duplicate, finalize,
    /// or reinterpret the opaque payload integers outside that contract.
    pub unsafe fn execute_owned_with_values<F>(
        &mut self,
        mut owners: Vec<NativeOwner>,
        scalars: Vec<ScalarValue>,
        execute: F,
    ) -> Result<OwnedExecution, RejectedCall>
    where
        F: FnOnce(&[u64], &[ScalarValue]) -> Result<(), NormalizedStatus>,
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
        if let Err(rejection) = self.preflight_scalars(&scalars) {
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
        let outcome = catch_unwind(AssertUnwindSafe(|| execute(prepared.payloads(), &scalars)));
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

    fn preflight_scalars(&self, scalars: &[ScalarValue]) -> Result<(), CallRejection> {
        let expected = self.descriptor.scalar_kinds();
        if scalars.len() != expected.len() {
            return Err(CallRejection::ScalarInputCountMismatch);
        }
        for (value, kind) in scalars.iter().zip(expected) {
            if !matches!(
                (value, kind),
                (ScalarValue::I64(_), ScalarKind::I64) | (ScalarValue::Bool(_), ScalarKind::Bool)
            ) {
                return Err(CallRejection::ScalarKindMismatch);
            }
        }
        Ok(())
    }
}

/// Same-thread ownership host for one exact callable descriptor-v2 module.
///
/// Unlike [`NativeHost`]'s compatibility lane, this type never accepts a Rust
/// execution closure: safe calls always enter the eagerly admitted generated
/// native callable exactly once.
///
/// ```compile_fail
/// use semaprax_native_host::NativeCallableHost;
/// fn assert_send<T: Send>() {}
/// assert_send::<NativeCallableHost>();
/// ```
///
/// ```compile_fail
/// use semaprax_native_host::NativeCallableHost;
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<NativeCallableHost>();
/// ```
///
/// ```compile_fail
/// use semaprax_native_host::NativeCallableHost;
/// fn clone_is_not_implicit(host: NativeCallableHost) { let _ = host.clone(); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_host::NativeCallableHost;
/// fn secret_host_state_is_not_formatted(host: NativeCallableHost) {
///     let _ = format!("{host:?}");
/// }
/// ```
///
/// ```compile_fail
/// use semaprax_native_host::NativeCallableHost;
/// fn cannot_inject_shadow_execution(mut host: NativeCallableHost) {
///     let _ = host.call_scalar(Vec::new(), |_| Ok(7));
/// }
/// ```
pub struct NativeCallableHost {
    module_lease: NativeCallableModuleLease,
    descriptor: CallableDescriptor,
    semantics: AuthenticatedSemanticDictionary,
    failure_status_templates: Vec<Option<NormalizedStatus>>,
    authority: Authority,
    ledger: Rc<RefCell<LedgerState>>,
    module_identity: HostIdentity,
    adapter_identity: HostIdentity,
    contract: HostCallContract,
    host_thread_identity: u64,
    draining: bool,
}

impl NativeCallableHost {
    /// Admit one exact generated callable and bind its semantic dictionary.
    ///
    /// # Safety
    ///
    /// The caller must satisfy every requirement of
    /// [`semaprax_native_loader::open_admitted_callable_exact`], including the
    /// complete trusted-image/dependency contract, exact getter and callable C
    /// ABIs, bounded synchronous pointer access, and absence of foreign unwind,
    /// callbacks, retained pointers, traps, or process termination.
    #[allow(
        clippy::result_large_err,
        reason = "admission preserves the exact loader rejection as its source"
    )]
    pub unsafe fn open_admitted_callable_exact(
        canonical_library_path: &Path,
        getter_symbol: &[u8],
        callable_symbol: &[u8],
        expected_descriptor: &[u8],
        dictionary: SemanticEventDictionary,
        trace_path_certificate: TracePathCertificate,
    ) -> Result<Self, AdmissionError> {
        if expected_descriptor.is_empty() || expected_descriptor.len() > MAX_DESCRIPTOR_BYTES {
            return Err(AdmissionError::Loader(
                OpenError::InvalidExpectedDescriptorLength {
                    actual: expected_descriptor.len(),
                    maximum: MAX_DESCRIPTOR_BYTES,
                },
            ));
        }
        let descriptor = CallableDescriptor::parse(expected_descriptor)
            .map_err(callable_descriptor_admission_error)?;
        if getter_symbol != descriptor.getter_symbol.as_bytes()
            || callable_symbol != descriptor.callable_symbol.as_bytes()
        {
            return Err(AdmissionError::Descriptor(
                DescriptorRejection::NonCanonical,
            ));
        }
        let semantics = authenticate_dictionary(&descriptor, dictionary, trace_path_certificate)
            .map_err(|_| AdmissionError::SemanticDictionary)?;
        let failure_status_templates = semantics
            .try_failure_status_templates()
            .map_err(|_| AdmissionError::SemanticDictionary)?;
        // SAFETY: This constructor preserves the loader's complete unsafe
        // contract and passes only independently authenticated descriptor-v2
        // bytes and their exact derived symbols.
        let module_lease = unsafe {
            open_admitted_callable_exact(
                canonical_library_path,
                getter_symbol,
                callable_symbol,
                expected_descriptor,
            )
        }
        .map_err(AdmissionError::Loader)?;
        let instance_id = module_lease.instance_id();
        let host_thread_identity = instance_id.get();
        let adapter_text = format!(
            "semaprax.native-callable-host.v2:{}:{}:{}",
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
            .parameters
            .iter()
            .filter_map(|parameter| match parameter {
                CallableParameter::Owned {
                    resource,
                    lifecycle,
                    ..
                } => Some((resource, lifecycle)),
                CallableParameter::Scalar { .. } => None,
            })
            .map(|(resource, lifecycle)| {
                Ok(HostResourceRequirement::new(
                    HostIdentity::try_new(resource.clone())?,
                    HostIdentity::try_new(lifecycle.clone())?,
                ))
            })
            .collect::<Result<Vec<_>, HostBoundaryRejection>>()
            .map_err(|_| AdmissionError::InvalidHostContract)?;
        let result = match descriptor.result {
            CallableResultShape::ScalarI64 => HostResultPlan::Scalar,
            CallableResultShape::OwnedInput { owner_ordinal, .. } => HostResultPlan::OwnedInput {
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
            ExactLeasePin::callable_v2(module_lease.retain()),
            descriptor.fingerprints.physical_module,
            adapter_text.as_bytes(),
        )
        .map_err(|_| AdmissionError::AuthorityUnavailable)?;
        let registry =
            HostOwnershipRegistry::try_new().map_err(|_| AdmissionError::InvalidHostContract)?;
        Ok(Self {
            module_lease,
            descriptor,
            semantics,
            failure_status_templates,
            authority,
            ledger: Rc::new(RefCell::new(LedgerState { registry })),
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
            CallableResultShape::ScalarI64 => ResultKind::ScalarI64,
            CallableResultShape::OwnedInput { owner_ordinal, .. } => {
                ResultKind::OwnedInput { owner_ordinal }
            }
        }
    }

    pub fn is_draining(&self) -> bool {
        self.draining
    }

    pub fn live_owner_count(&self) -> usize {
        self.ledger.borrow().registry.live_owner_count()
    }

    pub fn begin_draining(&mut self) {
        self.draining = true;
    }

    /// Adopt one newly created payload from this exact trusted callable.
    ///
    /// # Safety
    ///
    /// The payload must be uniquely owned by the selected descriptor resource
    /// and must not already be represented by another owner credential.
    pub unsafe fn adopt_trusted_owner(
        &mut self,
        owner_ordinal: usize,
        payload: u64,
    ) -> Result<NativeOwner, CallRejection> {
        if self.draining {
            return Err(CallRejection::Draining);
        }
        let (resource, lifecycle) = self
            .owned_parameter(owner_ordinal)
            .ok_or(CallRejection::WrongShape)?;
        let resource = resource.to_owned();
        let lifecycle = lifecycle.to_owned();
        let provenance = HostResourceProvenance::try_new(
            self.module_identity.clone(),
            self.adapter_identity.clone(),
            HostIdentity::try_new(resource.clone())
                .map_err(|_| CallRejection::WrongResourceType)?,
            HostIdentity::try_new(lifecycle.clone()).map_err(|_| CallRejection::WrongLifecycle)?,
            self.host_thread_identity,
        )
        .map_err(map_ledger_error)?;
        let token = self
            .ledger
            .borrow_mut()
            .registry
            .register_adapter_owner(provenance, payload)
            .map_err(map_ledger_error)?;
        let credential = match self.authority.mint_owner(
            resource.as_bytes(),
            lifecycle.as_bytes(),
            token.slot(),
            token.generation(),
        ) {
            Ok(credential) => credential,
            Err(error) => {
                self.ledger
                    .borrow_mut()
                    .registry
                    .rollback_adapter_owner(token)
                    .expect("same-thread adoption rollback must restore the latest owner slot");
                return Err(map_authority_error(error));
            }
        };
        Ok(NativeOwner {
            token,
            credential,
            resource,
            lifecycle,
            ledger: Rc::clone(&self.ledger),
            armed: true,
        })
    }

    pub fn call_scalar(
        &mut self,
        owners: Vec<NativeOwner>,
    ) -> Result<NativeCallableExecution<i64>, RejectedCall> {
        self.call_scalar_with_values(owners, Vec::new())
    }

    pub fn call_scalar_with_values(
        &mut self,
        mut owners: Vec<NativeOwner>,
        scalars: Vec<ScalarValue>,
    ) -> Result<NativeCallableExecution<i64>, RejectedCall> {
        if self.draining {
            return Err(rejected(CallRejection::Draining, owners));
        }
        if self.descriptor.result != CallableResultShape::ScalarI64 {
            return Err(rejected(CallRejection::WrongShape, owners));
        }
        if let Err(error) = self.preflight_owners(&owners) {
            return Err(rejected(error, owners));
        }
        if let Err(error) = self.preflight_scalars(&scalars) {
            return Err(rejected(error, owners));
        }
        let tokens = owners.iter().map(|owner| owner.token).collect();
        let request =
            HostCallRequest::new(self.contract.clone(), self.host_thread_identity, tokens);
        let plan = match self.ledger.borrow().registry.plan_scalar(&request) {
            Ok(plan) => plan,
            Err(error) => return Err(rejected(map_ledger_error(error), owners)),
        };
        let invocation = NonZeroU64::new(plan.sequence())
            .expect("ownership registry invocation identities are nonzero");
        let arguments = match self.request_arguments(plan.payloads(), &scalars) {
            Ok(arguments) => arguments,
            Err(error) => return Err(rejected(error, owners)),
        };
        let request_bytes = match encode_request(&self.descriptor, invocation, &arguments) {
            Ok(bytes) => bytes,
            Err(_) => return Err(rejected(CallRejection::WireRejected, owners)),
        };
        let mut physical_call = match self.module_lease.prepare_call(
            request_bytes,
            self.descriptor.capacities.max_response_bytes as usize,
        ) {
            Ok(call) => call,
            Err(_) => return Err(rejected(CallRejection::WireRejected, owners)),
        };
        let mut response_buffers = match ResponseDecodeBuffers::try_new(&self.descriptor) {
            Ok(buffers) => buffers,
            Err(_) => return Err(rejected(CallRejection::WireRejected, owners)),
        };
        let adapter_status = HostOwnershipRegistry::prepare_abandonment_status();
        let mut failure_statuses = self.failure_status_templates.clone();
        let ledger_pin = Rc::clone(&self.ledger);
        let prepared = match self.ledger.borrow_mut().registry.commit_plan(plan) {
            Ok(prepared) => prepared,
            Err(error) => return Err(rejected(map_ledger_error(error), owners)),
        };
        let guard = PreparedLedgerGuard::new(ledger_pin, prepared);
        for owner in &mut owners {
            owner.armed = false;
        }
        let physical_result = self.module_lease.invoke(&mut physical_call);
        if !matches!(physical_result, Ok(CALL_RESULT_COMPLETE)) {
            guard.abandon();
            return Ok(adapter_execution(adapter_status));
        }
        let response = match response_buffers.decode_response_into(
            &self.descriptor,
            invocation,
            physical_call.response_storage(),
        ) {
            Ok(response) => response,
            Err(_) => {
                guard.abandon();
                return Ok(adapter_execution(adapter_status));
            }
        };
        let outcome = match self
            .semantics
            .validate_response_into(response, &mut response_buffers)
        {
            Ok(outcome) => outcome,
            Err(_) => {
                guard.abandon();
                return Ok(adapter_execution(adapter_status));
            }
        };
        let ValidatedExecution { outcome, events } = response_buffers.into_execution(outcome);
        match outcome {
            ValidatedOutcome::ScalarSuccess(value) => {
                let ledger_outcome = guard.complete_scalar(Ok(value));
                if ledger_outcome
                    != HostCallOutcome::ExecutedSuccess(HostPublishedValue::Scalar(value))
                {
                    return Ok(adapter_execution(adapter_status));
                }
                Ok(NativeCallableExecution {
                    result: Ok(value),
                    events,
                })
            }
            ValidatedOutcome::Failure {
                selected_ordinal,
                status,
                ..
            } => {
                let Some(status_value) = take_prepared_failure_status(
                    &mut failure_statuses,
                    selected_ordinal,
                    status.as_ref(),
                ) else {
                    guard.abandon();
                    return Ok(adapter_execution(adapter_status));
                };
                let ledger_outcome = guard.complete_scalar(Err(status_value));
                let HostCallOutcome::ExecutedFailure(status) = ledger_outcome else {
                    return Ok(adapter_execution(adapter_status));
                };
                Ok(NativeCallableExecution {
                    result: Err(status),
                    events,
                })
            }
            ValidatedOutcome::OwnedSuccess { .. } => {
                guard.abandon();
                Ok(adapter_execution(adapter_status))
            }
        }
    }

    pub fn call_owned(
        &mut self,
        owners: Vec<NativeOwner>,
    ) -> Result<NativeCallableExecution<NativeOwner>, RejectedCall> {
        self.call_owned_with_values(owners, Vec::new())
    }

    pub fn call_owned_with_values(
        &mut self,
        mut owners: Vec<NativeOwner>,
        scalars: Vec<ScalarValue>,
    ) -> Result<NativeCallableExecution<NativeOwner>, RejectedCall> {
        if self.draining {
            return Err(rejected(CallRejection::Draining, owners));
        }
        let CallableResultShape::OwnedInput { owner_ordinal, .. } = self.descriptor.result else {
            return Err(rejected(CallRejection::WrongShape, owners));
        };
        if let Err(error) = self.preflight_owners(&owners) {
            return Err(rejected(error, owners));
        }
        if let Err(error) = self.preflight_scalars(&scalars) {
            return Err(rejected(error, owners));
        }
        let selected = &owners[owner_ordinal];
        let selected_token = selected.token;
        let result_resource = selected.resource.clone();
        let result_lifecycle = selected.lifecycle.clone();
        let Some(expected_result_token) = selected_token.next_generation() else {
            return Err(rejected(CallRejection::GenerationExhausted, owners));
        };
        let next_generation = expected_result_token.generation();
        let result_credential = match self.authority.mint_result(
            &self.descriptor.fingerprints.function_template,
            result_resource.as_bytes(),
            result_lifecycle.as_bytes(),
            selected_token.slot(),
            next_generation,
        ) {
            Ok(credential) => credential,
            Err(error) => return Err(rejected(map_authority_error(error), owners)),
        };
        if let Err(error) = self.authority.authenticate_result(
            &self.descriptor.fingerprints.function_template,
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
            Ok(credential) => credential,
            Err(error) => return Err(rejected(map_authority_error(error), owners)),
        };
        let tokens = owners.iter().map(|owner| owner.token).collect();
        let request =
            HostCallRequest::new(self.contract.clone(), self.host_thread_identity, tokens);
        let plan = match self.ledger.borrow().registry.plan_owned(&request) {
            Ok(plan) => plan,
            Err(error) => return Err(rejected(map_ledger_error(error), owners)),
        };
        let invocation = NonZeroU64::new(plan.sequence())
            .expect("ownership registry invocation identities are nonzero");
        let arguments = match self.request_arguments(plan.payloads(), &scalars) {
            Ok(arguments) => arguments,
            Err(error) => return Err(rejected(error, owners)),
        };
        let request_bytes = match encode_request(&self.descriptor, invocation, &arguments) {
            Ok(bytes) => bytes,
            Err(_) => return Err(rejected(CallRejection::WireRejected, owners)),
        };
        let mut physical_call = match self.module_lease.prepare_call(
            request_bytes,
            self.descriptor.capacities.max_response_bytes as usize,
        ) {
            Ok(call) => call,
            Err(_) => return Err(rejected(CallRejection::WireRejected, owners)),
        };
        let mut response_buffers = match ResponseDecodeBuffers::try_new(&self.descriptor) {
            Ok(buffers) => buffers,
            Err(_) => return Err(rejected(CallRejection::WireRejected, owners)),
        };
        let adapter_status = HostOwnershipRegistry::prepare_abandonment_status();
        let mut failure_statuses = self.failure_status_templates.clone();
        let ledger_pin = Rc::clone(&self.ledger);
        let prepared = match self.ledger.borrow_mut().registry.commit_plan(plan) {
            Ok(prepared) => prepared,
            Err(error) => return Err(rejected(map_ledger_error(error), owners)),
        };
        let guard = PreparedLedgerGuard::new(ledger_pin, prepared);
        for owner in &mut owners {
            owner.armed = false;
        }
        let physical_result = self.module_lease.invoke(&mut physical_call);
        if !matches!(physical_result, Ok(CALL_RESULT_COMPLETE)) {
            guard.abandon();
            return Ok(adapter_execution(adapter_status));
        }
        let response = match response_buffers.decode_response_into(
            &self.descriptor,
            invocation,
            physical_call.response_storage(),
        ) {
            Ok(response) => response,
            Err(_) => {
                guard.abandon();
                return Ok(adapter_execution(adapter_status));
            }
        };
        let outcome = match self
            .semantics
            .validate_response_into(response, &mut response_buffers)
        {
            Ok(outcome) => outcome,
            Err(_) => {
                guard.abandon();
                return Ok(adapter_execution(adapter_status));
            }
        };
        let ValidatedExecution { outcome, events } = response_buffers.into_execution(outcome);
        match outcome {
            ValidatedOutcome::OwnedSuccess {
                owner_ordinal: actual,
            } if actual == owner_ordinal => {
                let token = match guard.complete_owned_expected(expected_result_token) {
                    Ok(token) => token,
                    Err(()) => return Ok(adapter_execution(adapter_status)),
                };
                drop(result_credential);
                Ok(NativeCallableExecution {
                    result: Ok(NativeOwner {
                        token,
                        credential: owner_credential,
                        resource: result_resource,
                        lifecycle: result_lifecycle,
                        ledger: Rc::clone(&self.ledger),
                        armed: true,
                    }),
                    events,
                })
            }
            ValidatedOutcome::Failure {
                selected_ordinal,
                status,
                ..
            } => {
                let Some(status_value) = take_prepared_failure_status(
                    &mut failure_statuses,
                    selected_ordinal,
                    status.as_ref(),
                ) else {
                    guard.abandon();
                    return Ok(adapter_execution(adapter_status));
                };
                let ledger_outcome = guard.complete_owned(Err(status_value));
                let HostCallOutcome::ExecutedFailure(status) = ledger_outcome else {
                    return Ok(adapter_execution(adapter_status));
                };
                Ok(NativeCallableExecution {
                    result: Err(status),
                    events,
                })
            }
            ValidatedOutcome::ScalarSuccess(_) | ValidatedOutcome::OwnedSuccess { .. } => {
                guard.abandon();
                Ok(adapter_execution(adapter_status))
            }
        }
    }

    fn owned_parameter(&self, ordinal: usize) -> Option<(&str, &str)> {
        self.descriptor
            .parameters
            .iter()
            .find_map(|parameter| match parameter {
                CallableParameter::Owned {
                    owner_ordinal,
                    resource,
                    lifecycle,
                    ..
                } if *owner_ordinal == ordinal => Some((resource.as_str(), lifecycle.as_str())),
                _ => None,
            })
    }

    fn preflight_owners(&self, owners: &[NativeOwner]) -> Result<(), CallRejection> {
        let requirements = self
            .descriptor
            .parameters
            .iter()
            .filter_map(|parameter| match parameter {
                CallableParameter::Owned {
                    resource,
                    lifecycle,
                    ..
                } => Some((resource.as_str(), lifecycle.as_str())),
                CallableParameter::Scalar { .. } => None,
            })
            .collect::<Vec<_>>();
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

    fn preflight_scalars(&self, scalars: &[ScalarValue]) -> Result<(), CallRejection> {
        let expected = self
            .descriptor
            .parameters
            .iter()
            .filter_map(|parameter| match parameter {
                CallableParameter::Scalar { kind, .. } => Some(*kind),
                CallableParameter::Owned { .. } => None,
            })
            .collect::<Vec<_>>();
        if scalars.len() != expected.len() {
            return Err(CallRejection::ScalarInputCountMismatch);
        }
        for (value, kind) in scalars.iter().zip(expected) {
            if !matches!(
                (value, kind),
                (ScalarValue::I64(_), CallableScalarKind::I64)
                    | (ScalarValue::Bool(_), CallableScalarKind::Bool)
            ) {
                return Err(CallRejection::ScalarKindMismatch);
            }
        }
        Ok(())
    }

    fn request_arguments(
        &self,
        payloads: &[u64],
        scalars: &[ScalarValue],
    ) -> Result<Vec<RequestArgument>, CallRejection> {
        let mut arguments = Vec::with_capacity(self.descriptor.parameters.len());
        let mut owner_index = 0_usize;
        let mut scalar_index = 0_usize;
        for parameter in &self.descriptor.parameters {
            match parameter {
                CallableParameter::Scalar {
                    kind: CallableScalarKind::I64,
                    ..
                } => match scalars.get(scalar_index) {
                    Some(ScalarValue::I64(value)) => arguments.push(RequestArgument::I64(*value)),
                    _ => return Err(CallRejection::ScalarKindMismatch),
                },
                CallableParameter::Scalar {
                    kind: CallableScalarKind::Bool,
                    ..
                } => match scalars.get(scalar_index) {
                    Some(ScalarValue::Bool(value)) => arguments.push(RequestArgument::Bool(*value)),
                    _ => return Err(CallRejection::ScalarKindMismatch),
                },
                CallableParameter::Owned { owner_ordinal, .. } => {
                    if *owner_ordinal != owner_index {
                        return Err(CallRejection::WireRejected);
                    }
                    let payload = payloads
                        .get(owner_index)
                        .copied()
                        .ok_or(CallRejection::InputCountMismatch)?;
                    arguments.push(RequestArgument::OwnedPayload(payload));
                    owner_index += 1;
                    continue;
                }
            }
            scalar_index += 1;
        }
        if owner_index != payloads.len() || scalar_index != scalars.len() {
            return Err(CallRejection::WireRejected);
        }
        Ok(arguments)
    }
}

/// RAII completion capability for postcommit Rust-side failures. Foreign
/// unwind remains forbidden by callable admission, but any ordinary Rust panic
/// in decoding/reconciliation still consumes the committed owner set.
struct PreparedLedgerGuard {
    ledger: Rc<RefCell<LedgerState>>,
    prepared: Option<HostPreparedInvocation>,
}

impl PreparedLedgerGuard {
    fn new(ledger: Rc<RefCell<LedgerState>>, prepared: HostPreparedInvocation) -> Self {
        Self {
            ledger,
            prepared: Some(prepared),
        }
    }

    fn complete_scalar(mut self, result: Result<i64, NormalizedStatus>) -> HostCallOutcome {
        let prepared = self
            .prepared
            .as_ref()
            .expect("prepared invocation is linear");
        let outcome = self
            .ledger
            .borrow_mut()
            .registry
            .complete_prepared_scalar_ref(prepared, result);
        self.prepared.take();
        outcome
    }

    fn complete_owned(mut self, result: Result<(), NormalizedStatus>) -> HostCallOutcome {
        let prepared = self
            .prepared
            .as_ref()
            .expect("prepared invocation is linear");
        let outcome = self
            .ledger
            .borrow_mut()
            .registry
            .complete_prepared_owned_ref(prepared, result);
        self.prepared.take();
        outcome
    }

    fn complete_owned_expected(
        mut self,
        expected_owner: HostOwnerToken,
    ) -> Result<HostOwnerToken, ()> {
        let completion = {
            let prepared = self
                .prepared
                .as_ref()
                .expect("prepared invocation is linear");
            self.ledger
                .borrow_mut()
                .registry
                .complete_prepared_owned_expected_ref(prepared, expected_owner)
        };
        match completion {
            Ok(HostCallOutcome::ExecutedSuccess(HostPublishedValue::Owner(owner)))
                if owner == expected_owner =>
            {
                self.prepared.take();
                Ok(owner)
            }
            Ok(HostCallOutcome::ExecutedSuccess(HostPublishedValue::Owner(owner))) => {
                self.prepared.take();
                let _ = self.ledger.borrow_mut().registry.retire_owner(owner);
                Err(())
            }
            Ok(
                HostCallOutcome::ExecutedSuccess(HostPublishedValue::Scalar(_))
                | HostCallOutcome::ExecutedFailure(_),
            ) => {
                self.prepared.take();
                Err(())
            }
            Err(_) => {
                self.abandon();
                Err(())
            }
        }
    }

    fn abandon(mut self) {
        let prepared = self
            .prepared
            .as_ref()
            .expect("prepared invocation is linear");
        let mut ledger = self.ledger.borrow_mut();
        ledger.registry.abandon_prepared_ref(prepared);
        assert!(
            ledger.registry.take_last_abandonment_flag(),
            "every detached abandonment records its canonical adapter failure"
        );
        drop(ledger);
        self.prepared.take();
    }
}

impl Drop for PreparedLedgerGuard {
    fn drop(&mut self) {
        if let Some(prepared) = self.prepared.take() {
            let mut ledger = self.ledger.borrow_mut();
            ledger.registry.abandon_prepared(prepared);
            let _ = ledger.registry.take_last_abandonment_flag();
        }
    }
}

fn adapter_execution<T>(status: NormalizedStatus) -> NativeCallableExecution<T> {
    NativeCallableExecution {
        result: Err(status),
        events: Vec::new(),
    }
}

fn take_prepared_failure_status(
    statuses: &mut [Option<NormalizedStatus>],
    selected_ordinal: u32,
    authenticated: &NormalizedStatus,
) -> Option<NormalizedStatus> {
    let index = usize::try_from(selected_ordinal).ok()?.checked_sub(1)?;
    let status = statuses.get_mut(index)?.take()?;
    (status == *authenticated).then_some(status)
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

fn callable_descriptor_admission_error(value: CallableDescriptorError) -> AdmissionError {
    AdmissionError::Descriptor(match value {
        CallableDescriptorError::Malformed => DescriptorRejection::Malformed,
        CallableDescriptorError::UnsupportedSchema => DescriptorRejection::UnsupportedSchema,
        CallableDescriptorError::WrongTarget => DescriptorRejection::WrongTarget,
        CallableDescriptorError::NonCanonical => DescriptorRejection::NonCanonical,
    })
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Descriptor(_) => "native adapter descriptor was rejected",
            Self::Loader(_) => "native module loading or descriptor admission failed",
            Self::SemanticDictionary => "native callable semantic dictionary was rejected",
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
