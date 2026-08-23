#[cfg(not(target_os = "ios"))]
use super::open_library;
use super::{
    allocate_instance_id, ModuleInstanceId, OpenError, MAX_CALL_WIRE_BYTES, MAX_DESCRIPTOR_BYTES,
    MAX_GETTER_SYMBOL_BYTES,
};
#[cfg(not(target_os = "ios"))]
use libloading::Library;
use std::cell::Cell;
use std::error::Error;
#[cfg(not(target_os = "ios"))]
use std::ffi::c_void;
#[cfg(all(unix, not(target_os = "ios")))]
use std::ffi::{c_char, c_int, CStr};
use std::fmt;
use std::marker::PhantomData;
#[cfg(not(target_os = "ios"))]
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

const PRE_EXECUTE_HOST_UNWIND_CODE: u32 = u32::MAX - 1;

const HEADER_BYTES: usize = 20;
const FINGERPRINT_COUNT: usize = 19;
const FINGERPRINT_BYTES: usize = 32;
const CAPACITY_COUNT: usize = 15;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_EVENT_COUNT: u32 = 65_536;
const MAX_DICTIONARY_BYTES: u32 = 1024 * 1024;
const MAX_DICTIONARY_ENTRIES: u32 = 65_536;
const MAX_RESOURCES: u32 = 4_096;
const MAX_CHECKPOINTS: u32 = 65_536;
const MAX_GRAPH_WORK_UNITS: u32 = 1_000_000;
const MAX_ACTIVE_FRAMES: u32 = 256;
const MAX_QUARANTINED_FRAMES: u32 = 64;
const MAX_INSTANCE_RESERVED_BYTES: u32 = 64 * 1024 * 1024;
const MAX_STATIC_SETTLEMENT_REGISTRATIONS: usize = 256;
const DECISION_BYTES: u32 = 172;
const ACTION_EVIDENCE_BYTES: u32 = 196;
const HOST_RECEIPT_BYTES: u32 = 524;

pub type StaticDescriptorGetter = unsafe extern "C" fn() -> *const u8;
pub type StaticExecuteEntry =
    unsafe extern "C" fn(*const u8, u32, *mut u8, u32, *mut u8, u32) -> u32;
pub type StaticSettleEntry =
    unsafe extern "C" fn(*mut u8, u32, *const u8, u32, *mut u8, u32) -> u32;

#[cfg(not(target_os = "ios"))]
type DescriptorGetter = StaticDescriptorGetter;
type ExecuteEntry = StaticExecuteEntry;
type SettleEntry = StaticSettleEntry;

/// Closed iOS-family target identities for static callable-v3 registration.
///
/// Device, simulator, and Mac Catalyst registrations never share an identity,
/// even when their linked entry points happen to have the same source-level
/// names. The target tag is authenticated by the complete host descriptor.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IosStaticTarget {
    DeviceArm64,
    SimulatorArm64,
    SimulatorX86_64,
    MacCatalystArm64,
    MacCatalystX86_64,
}

impl IosStaticTarget {
    #[must_use]
    pub const fn canonical_tag(self) -> &'static str {
        match self {
            Self::DeviceArm64 => "aarch64-ios-device-apple-macho-ptr64-little-callable-v3",
            Self::SimulatorArm64 => "aarch64-ios-simulator-apple-macho-ptr64-little-callable-v3",
            Self::SimulatorX86_64 => "x86_64-ios-simulator-apple-macho-ptr64-little-callable-v3",
            Self::MacCatalystArm64 => "aarch64-ios-catalyst-apple-macho-ptr64-little-callable-v3",
            Self::MacCatalystX86_64 => "x86_64-ios-catalyst-apple-macho-ptr64-little-callable-v3",
        }
    }

    /// Return the exact iOS-family identity of this compilation target.
    #[must_use]
    pub const fn current() -> Option<Self> {
        if cfg!(all(
            target_os = "ios",
            target_arch = "aarch64",
            target_abi = "macabi"
        )) {
            Some(Self::MacCatalystArm64)
        } else if cfg!(all(
            target_os = "ios",
            target_arch = "x86_64",
            target_abi = "macabi"
        )) {
            Some(Self::MacCatalystX86_64)
        } else if cfg!(all(
            target_os = "ios",
            target_arch = "aarch64",
            target_abi = "sim"
        )) {
            Some(Self::SimulatorArm64)
        } else if cfg!(all(
            target_os = "ios",
            target_arch = "x86_64",
            target_abi = "sim"
        )) {
            Some(Self::SimulatorX86_64)
        } else if cfg!(all(
            target_os = "ios",
            target_arch = "aarch64",
            not(any(target_abi = "macabi", target_abi = "sim"))
        )) {
            Some(Self::DeviceArm64)
        } else {
            None
        }
    }
}

/// Stable static-registration rejection without paths or loader diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticSettlementRegistrationError {
    InvalidDescriptor,
    WrongTarget,
    NullDescriptor,
    DescriptorAddressMismatch,
    DescriptorMismatch,
    AliasedAddresses,
    AddressConflict,
    WrongThread,
    RegistrationInProgress,
    RegistryFull,
    RegistryPoisoned,
    InstanceIdentityExhausted,
}

impl fmt::Display for StaticSettlementRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidDescriptor => "iOS static registration requires exact SPXNABI3 bytes",
            Self::WrongTarget => {
                "iOS static registration target does not match the descriptor or compilation target"
            }
            Self::NullDescriptor => "iOS static descriptor getter returned null",
            Self::DescriptorAddressMismatch => {
                "iOS static descriptor getter returned different storage"
            }
            Self::DescriptorMismatch => "iOS static descriptor bytes do not match exactly",
            Self::AliasedAddresses => {
                "iOS static descriptor and entry addresses are not pairwise distinct"
            }
            Self::AddressConflict => {
                "iOS static registration reuses an address with conflicting evidence"
            }
            Self::WrongThread => {
                "iOS static registration is bound to its original registering thread"
            }
            Self::RegistrationInProgress => {
                "iOS static registration for these addresses is already in progress"
            }
            Self::RegistryFull => "iOS static registration table reached its fixed process bound",
            Self::RegistryPoisoned => "iOS static registration table is unavailable",
            Self::InstanceIdentityExhausted => "native module instance identity space is exhausted",
        })
    }
}

impl Error for StaticSettlementRegistrationError {}

/// Exact descriptor-authenticated byte capacities for one settlement frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementBufferCapacities {
    request: usize,
    execute_response: usize,
    frame: usize,
    decision: usize,
    candidate_receipt: usize,
}

impl SettlementBufferCapacities {
    #[must_use]
    pub const fn request(self) -> usize {
        self.request
    }

    #[must_use]
    pub const fn execute_response(self) -> usize {
        self.execute_response
    }

    #[must_use]
    pub const fn frame(self) -> usize {
        self.frame
    }

    #[must_use]
    pub const fn decision(self) -> usize {
        self.decision
    }

    #[must_use]
    pub const fn candidate_receipt(self) -> usize {
        self.candidate_receipt
    }
}

/// Stable preparation or one-shot invocation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementCallError {
    AllocationFailed,
    WrongModuleInstance,
    ExecuteAlreadyInvoked,
    ExecuteNotInvoked,
    SettleAlreadyInvoked,
    BufferOverlap,
}

impl fmt::Display for SettlementCallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AllocationFailed => "native settlement buffer preallocation failed",
            Self::WrongModuleInstance => {
                "prepared native settlement call belongs to a different module instance"
            }
            Self::ExecuteAlreadyInvoked => "native settlement execute was already invoked",
            Self::ExecuteNotInvoked => "native settlement execute has not been invoked",
            Self::SettleAlreadyInvoked => "native settlement settle was already invoked",
            Self::BufferOverlap => "native settlement provider buffers overlap",
        })
    }
}

impl Error for SettlementCallError {}

/// One exact, thread-confined SPXNABI3 dynamic-image lease.
///
/// The type is intentionally non-`Clone`, non-formatting, `!Send`, and `!Sync`.
/// Explicit [`Self::retain`] is the only way to create another image pin.
///
/// ```compile_fail
/// use semaprax_native_loader::NativeSettlementModuleLease;
/// fn clone_is_not_implicit(lease: NativeSettlementModuleLease) { let _ = lease.clone(); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::NativeSettlementModuleLease;
/// fn state_is_not_formatted(lease: NativeSettlementModuleLease) { let _ = format!("{lease:?}"); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::NativeSettlementModuleLease;
/// fn assert_send<T: Send>() {}
/// assert_send::<NativeSettlementModuleLease>();
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::NativeSettlementModuleLease;
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<NativeSettlementModuleLease>();
/// ```
#[cfg(not(target_os = "ios"))]
pub struct NativeSettlementModuleLease {
    inner: Arc<LoadedSettlementModule>,
    _same_thread: PhantomData<Rc<()>>,
}

#[cfg(not(target_os = "ios"))]
struct LoadedSettlementModule {
    instance_id: ModuleInstanceId,
    canonical_path: PathBuf,
    root_image_allocation: usize,
    // Retain the exact immutable admission claim, not merely its capacities.
    // The descriptor is bounded before any image is opened and binds the
    // independent host parse byte-for-byte to this logical loader instance.
    descriptor: Box<[u8]>,
    capacities: SettlementBufferCapacities,
    execute: ExecuteEntry,
    settle: SettleEntry,
    // This is the single platform image pin. Keep it last so all copied entry
    // pointers and metadata disappear before native terminators may run.
    _library: Library,
}

/// One exact process-lifetime iOS-static callable-v3 registration.
///
/// This lease has no path, dynamic image, close operation, or unload
/// eligibility. The process table deliberately retains the registration even
/// after every explicit lease is dropped. Like the dynamic lease, it is
/// non-`Clone`, non-formatting, `!Send`, and `!Sync`; [`Self::retain`] is the
/// only way to create another exact-instance pin.
///
/// ```compile_fail
/// use semaprax_native_loader::NativeStaticSettlementLease;
/// fn clone_is_not_implicit(lease: NativeStaticSettlementLease) { let _ = lease.clone(); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::NativeStaticSettlementLease;
/// fn state_is_not_formatted(lease: NativeStaticSettlementLease) { let _ = format!("{lease:?}"); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::NativeStaticSettlementLease;
/// fn assert_send<T: Send>() {}
/// assert_send::<NativeStaticSettlementLease>();
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::NativeStaticSettlementLease;
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<NativeStaticSettlementLease>();
/// ```
pub struct NativeStaticSettlementLease {
    inner: Arc<LoadedStaticSettlement>,
    _same_thread: PhantomData<Rc<()>>,
}

struct LoadedStaticSettlement {
    instance_id: ModuleInstanceId,
    target: IosStaticTarget,
    descriptor_address: usize,
    getter_address: usize,
    execute_address: usize,
    settle_address: usize,
    registering_thread: std::thread::ThreadId,
    descriptor: Box<[u8]>,
    capacities: SettlementBufferCapacities,
    execute: ExecuteEntry,
    settle: SettleEntry,
}

struct PendingStaticSettlement {
    target: IosStaticTarget,
    addresses: [usize; 4],
    registering_thread: std::thread::ThreadId,
    descriptor: &'static [u8],
}

#[derive(Default)]
struct StaticSettlementRegistry {
    ready: Vec<Arc<LoadedStaticSettlement>>,
    pending: Vec<PendingStaticSettlement>,
}

impl StaticSettlementRegistry {
    fn find_existing(
        &self,
        target: IosStaticTarget,
        addresses: [usize; 4],
        registering_thread: std::thread::ThreadId,
        expected_descriptor: &'static [u8],
    ) -> Result<Option<Arc<LoadedStaticSettlement>>, StaticSettlementRegistrationError> {
        for existing in &self.ready {
            let existing_addresses = [
                existing.descriptor_address,
                existing.getter_address,
                existing.execute_address,
                existing.settle_address,
            ];
            if existing_addresses == addresses
                && existing.target == target
                && existing.descriptor.as_ref() == expected_descriptor
            {
                if existing.registering_thread != registering_thread {
                    return Err(StaticSettlementRegistrationError::WrongThread);
                }
                return Ok(Some(Arc::clone(existing)));
            }
            if addresses
                .iter()
                .any(|address| existing_addresses.contains(address))
            {
                return Err(StaticSettlementRegistrationError::AddressConflict);
            }
        }
        for pending in &self.pending {
            if pending.addresses == addresses
                && pending.target == target
                && pending.descriptor == expected_descriptor
            {
                return Err(if pending.registering_thread == registering_thread {
                    StaticSettlementRegistrationError::RegistrationInProgress
                } else {
                    StaticSettlementRegistrationError::WrongThread
                });
            }
            if addresses
                .iter()
                .any(|address| pending.addresses.contains(address))
            {
                return Err(StaticSettlementRegistrationError::AddressConflict);
            }
        }
        Ok(None)
    }

    fn entry_count(&self) -> usize {
        self.ready.len() + self.pending.len()
    }
}

static STATIC_SETTLEMENT_REGISTRATIONS: OnceLock<Mutex<StaticSettlementRegistry>> = OnceLock::new();

thread_local! {
    static STATIC_DESCRIPTOR_GETTER_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

struct StaticDescriptorGetterGuard;

impl Drop for StaticDescriptorGetterGuard {
    fn drop(&mut self) {
        STATIC_DESCRIPTOR_GETTER_ACTIVE.with(|active| active.set(false));
    }
}

/// All five disjoint buffers reserved before the one-shot execute boundary.
///
/// ```compile_fail
/// use semaprax_native_loader::PreparedSettlementExecute;
/// fn clone_is_not_implicit(value: PreparedSettlementExecute) { let _ = value.clone(); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::PreparedSettlementExecute;
/// fn bytes_are_not_formatted(value: PreparedSettlementExecute) { let _ = format!("{value:?}"); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::PreparedSettlementExecute;
/// fn assert_send<T: Send>() {}
/// assert_send::<PreparedSettlementExecute>();
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::PreparedSettlementExecute;
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<PreparedSettlementExecute>();
/// ```
pub struct PreparedSettlementExecute {
    buffers: SettlementBuffers,
    execute_return: Option<u32>,
    _same_thread: PhantomData<Rc<()>>,
}

/// Post-execute buffers prepared for exactly one settle entry.
///
/// ```compile_fail
/// use semaprax_native_loader::PreparedSettlementCall;
/// fn clone_is_not_implicit(value: PreparedSettlementCall) { let _ = value.clone(); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::PreparedSettlementCall;
/// fn bytes_are_not_formatted(value: PreparedSettlementCall) { let _ = format!("{value:?}"); }
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::PreparedSettlementCall;
/// fn assert_send<T: Send>() {}
/// assert_send::<PreparedSettlementCall>();
/// ```
///
/// ```compile_fail
/// use semaprax_native_loader::PreparedSettlementCall;
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<PreparedSettlementCall>();
/// ```
pub struct PreparedSettlementCall {
    buffers: SettlementBuffers,
    execute_return: u32,
    settle_return: Option<u32>,
    _same_thread: PhantomData<Rc<()>>,
}

struct SettlementBuffers {
    module_instance: ModuleInstanceId,
    request: Vec<u8>,
    frame: Vec<u8>,
    response: Vec<u8>,
    decision: Vec<u8>,
    candidate: Vec<u8>,
}

#[cfg(not(target_os = "ios"))]
impl NativeSettlementModuleLease {
    #[must_use]
    pub fn retain(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            _same_thread: PhantomData,
        }
    }

    #[must_use]
    pub fn instance_id(&self) -> ModuleInstanceId {
        self.inner.instance_id
    }

    #[must_use]
    pub fn is_same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    #[must_use]
    pub fn canonical_path(&self) -> &Path {
        &self.inner.canonical_path
    }

    /// Private adapter predicate: both supplied live symbol addresses must
    /// resolve to the exact root allocation and canonical path admitted for
    /// this lease. No allocation address is exposed to the caller.
    #[doc(hidden)]
    #[must_use]
    pub fn private_addresses_share_admitted_root(
        &self,
        first: *const c_void,
        second: *const c_void,
    ) -> bool {
        !first.is_null()
            && !second.is_null()
            && matches!(
                root_image_allocation(first, &self.inner.canonical_path),
                Ok(allocation) if allocation == self.inner.root_image_allocation
            )
            && matches!(
                root_image_allocation(second, &self.inner.canonical_path),
                Ok(allocation) if allocation == self.inner.root_image_allocation
            )
    }

    #[must_use]
    pub fn descriptor_len(&self) -> usize {
        self.inner.descriptor.len()
    }

    /// Test whether bytes exactly equal the immutable descriptor admitted for
    /// this instance without exposing or formatting the stored claim.
    #[must_use]
    pub fn descriptor_matches(&self, candidate: &[u8]) -> bool {
        self.inner.descriptor.as_ref() == candidate
    }

    #[must_use]
    pub fn capacities(&self) -> SettlementBufferCapacities {
        self.inner.capacities
    }

    /// Preallocate every provider-visible buffer at its exact descriptor size.
    pub fn prepare_execute(&self) -> Result<PreparedSettlementExecute, SettlementCallError> {
        let capacities = self.capacities();
        let buffers = SettlementBuffers {
            module_instance: self.instance_id(),
            request: zeroed(capacities.request)?,
            frame: zeroed(capacities.frame)?,
            response: zeroed(capacities.execute_response)?,
            decision: zeroed(capacities.decision)?,
            candidate: zeroed(capacities.candidate_receipt)?,
        };
        if !buffers_are_disjoint(&buffers) {
            return Err(SettlementCallError::BufferOverlap);
        }
        Ok(PreparedSettlementExecute {
            buffers,
            execute_return: None,
            _same_thread: PhantomData,
        })
    }

    /// Invoke the exact eagerly bound execute entry at most once.
    pub fn invoke_execute(
        &self,
        call: &mut PreparedSettlementExecute,
    ) -> Result<u32, SettlementCallError> {
        if call.buffers.module_instance != self.instance_id() {
            return Err(SettlementCallError::WrongModuleInstance);
        }
        if call.execute_return.is_some() {
            return Err(SettlementCallError::ExecuteAlreadyInvoked);
        }
        if !buffers_are_disjoint(&call.buffers) {
            return Err(SettlementCallError::BufferOverlap);
        }
        call.execute_return = Some(u32::MAX);
        let request_len = wire_len(call.buffers.request.len());
        let frame_len = wire_len(call.buffers.frame.len());
        let response_len = wire_len(call.buffers.response.len());
        // SAFETY: Admission fixes this exact synchronous ABI and forbids
        // retained pointers and foreign unwind. The independently allocated,
        // nonempty ranges were rechecked pairwise disjoint immediately above.
        let result = unsafe {
            (self.inner.execute)(
                call.buffers.request.as_ptr(),
                request_len,
                call.buffers.frame.as_mut_ptr(),
                frame_len,
                call.buffers.response.as_mut_ptr(),
                response_len,
            )
        };
        call.execute_return = Some(result);
        Ok(result)
    }

    /// Invoke the exact eagerly bound settle entry at most once.
    pub fn invoke_settle(
        &self,
        call: &mut PreparedSettlementCall,
    ) -> Result<u32, SettlementCallError> {
        if call.buffers.module_instance != self.instance_id() {
            return Err(SettlementCallError::WrongModuleInstance);
        }
        if call.settle_return.is_some() {
            return Err(SettlementCallError::SettleAlreadyInvoked);
        }
        if !buffers_are_disjoint(&call.buffers) {
            return Err(SettlementCallError::BufferOverlap);
        }
        call.settle_return = Some(u32::MAX);
        let frame_len = wire_len(call.buffers.frame.len());
        let decision_len = wire_len(call.buffers.decision.len());
        let candidate_len = wire_len(call.buffers.candidate.len());
        // SAFETY: The same admission contract fixes settle's exact ABI and
        // synchronous bounded access. All three supplied nonempty ranges are
        // disjoint, and the provider cannot access the other two retained Vecs.
        let result = unsafe {
            (self.inner.settle)(
                call.buffers.frame.as_mut_ptr(),
                frame_len,
                call.buffers.decision.as_ptr(),
                decision_len,
                call.buffers.candidate.as_mut_ptr(),
                candidate_len,
            )
        };
        call.settle_return = Some(result);
        Ok(result)
    }
}

impl NativeStaticSettlementLease {
    #[must_use]
    pub fn retain(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            _same_thread: PhantomData,
        }
    }

    #[must_use]
    pub fn instance_id(&self) -> ModuleInstanceId {
        self.inner.instance_id
    }

    #[must_use]
    pub fn is_same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    #[must_use]
    pub fn target(&self) -> IosStaticTarget {
        self.inner.target
    }

    #[must_use]
    pub fn descriptor_len(&self) -> usize {
        self.inner.descriptor.len()
    }

    #[must_use]
    pub fn descriptor_matches(&self, candidate: &[u8]) -> bool {
        self.inner.descriptor.as_ref() == candidate
    }

    #[must_use]
    pub fn capacities(&self) -> SettlementBufferCapacities {
        self.inner.capacities
    }

    /// Preallocate the same five disjoint buffers used by dynamic admission.
    pub fn prepare_execute(&self) -> Result<PreparedSettlementExecute, SettlementCallError> {
        prepare_execute(self.instance_id(), self.capacities())
    }

    /// Invoke the exact statically registered execute address at most once.
    pub fn invoke_execute(
        &self,
        call: &mut PreparedSettlementExecute,
    ) -> Result<u32, SettlementCallError> {
        invoke_execute(self.instance_id(), self.inner.execute, call)
    }

    /// Invoke the exact statically registered settle address at most once.
    pub fn invoke_settle(
        &self,
        call: &mut PreparedSettlementCall,
    ) -> Result<u32, SettlementCallError> {
        invoke_settle(self.instance_id(), self.inner.settle, call)
    }
}

impl PreparedSettlementExecute {
    #[must_use]
    pub fn request_storage(&self) -> &[u8] {
        &self.buffers.request
    }

    pub fn request_storage_mut(&mut self) -> &mut [u8] {
        &mut self.buffers.request
    }

    #[must_use]
    pub fn frame_storage(&self) -> &[u8] {
        &self.buffers.frame
    }

    pub fn frame_storage_mut(&mut self) -> &mut [u8] {
        &mut self.buffers.frame
    }

    #[must_use]
    pub fn response_storage(&self) -> &[u8] {
        &self.buffers.response
    }

    #[must_use]
    pub fn execute_return(&self) -> Option<u32> {
        self.execute_return
    }

    /// Move the already-reserved buffers into the one-shot settlement stage.
    pub fn into_settlement(self) -> Result<PreparedSettlementCall, SettlementCallError> {
        let Some(execute_return) = self.execute_return else {
            return Err(SettlementCallError::ExecuteNotInvoked);
        };
        Ok(PreparedSettlementCall {
            buffers: self.buffers,
            execute_return,
            settle_return: None,
            _same_thread: PhantomData,
        })
    }

    /// Private callable-v3 recovery lane when host control unwinds before the
    /// provider execute entry is entered. The distinct sentinel is evidence,
    /// not an execute return value; the response buffer remains canonical zero.
    #[doc(hidden)]
    pub fn into_pre_execute_host_unwind_settlement(
        self,
    ) -> Result<PreparedSettlementCall, SettlementCallError> {
        if self.execute_return.is_some() {
            return Err(SettlementCallError::ExecuteAlreadyInvoked);
        }
        if self.buffers.response.iter().any(|byte| *byte != 0) {
            return Err(SettlementCallError::ExecuteNotInvoked);
        }
        Ok(PreparedSettlementCall {
            buffers: self.buffers,
            execute_return: PRE_EXECUTE_HOST_UNWIND_CODE,
            settle_return: None,
            _same_thread: PhantomData,
        })
    }
}

impl PreparedSettlementCall {
    #[must_use]
    pub fn execute_return(&self) -> u32 {
        self.execute_return
    }

    #[must_use]
    pub fn settle_return(&self) -> Option<u32> {
        self.settle_return
    }

    #[must_use]
    pub fn request_storage(&self) -> &[u8] {
        &self.buffers.request
    }

    #[must_use]
    pub fn response_storage(&self) -> &[u8] {
        &self.buffers.response
    }

    #[must_use]
    pub fn frame_storage(&self) -> &[u8] {
        &self.buffers.frame
    }

    pub fn frame_storage_mut(&mut self) -> &mut [u8] {
        &mut self.buffers.frame
    }

    #[must_use]
    pub fn decision_storage(&self) -> &[u8] {
        &self.buffers.decision
    }

    pub fn decision_storage_mut(&mut self) -> &mut [u8] {
        &mut self.buffers.decision
    }

    #[must_use]
    pub fn candidate_storage(&self) -> &[u8] {
        &self.buffers.candidate
    }
}

unsafe fn validate_static_descriptor_getter(
    expected_descriptor: &'static [u8],
    getter: StaticDescriptorGetter,
) -> Result<(), StaticSettlementRegistrationError> {
    let was_active = STATIC_DESCRIPTOR_GETTER_ACTIVE.with(|active| active.replace(true));
    if was_active {
        return Err(StaticSettlementRegistrationError::RegistrationInProgress);
    }
    let _guard = StaticDescriptorGetterGuard;
    // SAFETY: The caller establishes the getter ABI and process-lifetime
    // storage. Admission checks both exact address and bytes.
    let actual_pointer = unsafe { getter() };
    if actual_pointer.is_null() {
        return Err(StaticSettlementRegistrationError::NullDescriptor);
    }
    if actual_pointer != expected_descriptor.as_ptr() {
        return Err(StaticSettlementRegistrationError::DescriptorAddressMismatch);
    }
    // SAFETY: Pointer identity above and the caller's static-lifetime contract
    // establish this complete readable range.
    let actual_descriptor =
        unsafe { std::slice::from_raw_parts(actual_pointer, expected_descriptor.len()) };
    if actual_descriptor != expected_descriptor {
        return Err(StaticSettlementRegistrationError::DescriptorMismatch);
    }
    Ok(())
}

/// Register one trusted, statically linked iOS-family callable-v3 provider.
///
/// Exact re-registration is idempotent and returns another retain of the same
/// logical instance. Reusing any descriptor/getter/execute/settle address with
/// different evidence fails closed. The registry itself retains the exact
/// entry for the process lifetime, so dropping leases has no unload meaning.
///
/// # Safety
///
/// The four supplied addresses and descriptor storage must be linked into the
/// process for its complete lifetime. The getter must synchronously return
/// exactly `expected_descriptor.as_ptr()` and must never unwind or `longjmp`.
/// Execute and settle must implement the exact synchronous SPXNABI3 ABIs, never
/// unwind, `longjmp`, retain pointers, invoke callbacks, or access outside the
/// supplied disjoint ranges. Exact
/// re-registration and every resulting lease remain bound to the first
/// registering thread. The caller must trust all provider and finalizer code
/// reachable through these entries.
pub unsafe fn register_admitted_ios_static_settlement_exact(
    target: IosStaticTarget,
    expected_descriptor: &'static [u8],
    getter: StaticDescriptorGetter,
    execute: StaticExecuteEntry,
    settle: StaticSettleEntry,
) -> Result<NativeStaticSettlementLease, StaticSettlementRegistrationError> {
    if expected_descriptor.is_empty() || expected_descriptor.len() > MAX_DESCRIPTOR_BYTES {
        return Err(StaticSettlementRegistrationError::InvalidDescriptor);
    }
    if let Some(current) = IosStaticTarget::current() {
        if current != target {
            return Err(StaticSettlementRegistrationError::WrongTarget);
        }
    }
    let projection = DescriptorProjection::parse_ios_static(expected_descriptor, target)?;

    let descriptor_address = expected_descriptor.as_ptr() as usize;
    let getter_address = getter as usize;
    let execute_address = execute as usize;
    let settle_address = settle as usize;
    let addresses = [
        descriptor_address,
        getter_address,
        execute_address,
        settle_address,
    ];
    if addresses.contains(&0)
        || addresses
            .iter()
            .enumerate()
            .any(|(index, address)| addresses[index + 1..].contains(address))
    {
        return Err(StaticSettlementRegistrationError::AliasedAddresses);
    }

    let registry = STATIC_SETTLEMENT_REGISTRATIONS
        .get_or_init(|| Mutex::new(StaticSettlementRegistry::default()));
    let registering_thread = std::thread::current().id();
    {
        let mut registrations = registry
            .lock()
            .map_err(|_| StaticSettlementRegistrationError::RegistryPoisoned)?;
        if let Some(existing) = registrations.find_existing(
            target,
            addresses,
            registering_thread,
            expected_descriptor,
        )? {
            drop(registrations);
            // SAFETY: The caller contract is unchanged, and the pure registry
            // thread check precedes provider entry. No lock spans that entry.
            unsafe { validate_static_descriptor_getter(expected_descriptor, getter)? };
            return Ok(NativeStaticSettlementLease {
                inner: existing,
                _same_thread: PhantomData,
            });
        }
        if registrations.entry_count() >= MAX_STATIC_SETTLEMENT_REGISTRATIONS {
            return Err(StaticSettlementRegistrationError::RegistryFull);
        }
        registrations.pending.push(PendingStaticSettlement {
            target,
            addresses,
            registering_thread,
            descriptor: expected_descriptor,
        });
    }

    // SAFETY: The caller establishes this new provider's ABI and lifetime.
    // A pending pure-data reservation prevents conflicting registration, and
    // no registry lock spans this foreign call.
    let getter_validation =
        unsafe { validate_static_descriptor_getter(expected_descriptor, getter) };
    let mut registrations = registry
        .lock()
        .map_err(|_| StaticSettlementRegistrationError::RegistryPoisoned)?;
    let pending_index = registrations.pending.iter().position(|pending| {
        pending.target == target
            && pending.addresses == addresses
            && pending.registering_thread == registering_thread
            && pending.descriptor == expected_descriptor
    });
    let Some(pending_index) = pending_index else {
        return Err(StaticSettlementRegistrationError::RegistryPoisoned);
    };
    registrations.pending.remove(pending_index);
    getter_validation?;

    let instance_id = allocate_instance_id().map_err(|error| match error {
        OpenError::InstanceIdentityExhausted => {
            StaticSettlementRegistrationError::InstanceIdentityExhausted
        }
        _ => StaticSettlementRegistrationError::InvalidDescriptor,
    })?;
    let registration = Arc::new(LoadedStaticSettlement {
        instance_id,
        target,
        descriptor_address,
        getter_address,
        execute_address,
        settle_address,
        registering_thread,
        descriptor: expected_descriptor.to_vec().into_boxed_slice(),
        capacities: projection.capacities,
        execute,
        settle,
    });
    registrations.ready.push(Arc::clone(&registration));
    Ok(NativeStaticSettlementLease {
        inner: registration,
        _same_thread: PhantomData,
    })
}

/// Open one already-admitted canonical SPXNABI3 dynamic provider.
///
/// The expected descriptor supplies all three symbol names and all exact wire
/// capacities. This loader independently validates the bounded structural
/// projection it consumes, but the caller must first run the complete canonical
/// host descriptor decoder and authenticate the artifact. Getter, execute,
/// settle, and immutable descriptor storage must all resolve to the admitted
/// root image; dependency interposition fails closed.
///
/// # Safety
///
/// The caller must trust the exact root artifact and dependency namespace for
/// arbitrary initializer, provider, finalizer, and terminator execution. The
/// descriptor getter and six-argument execute/settle functions must have the
/// exact ABIs documented by SPXNABI3, return synchronously without unwind,
/// `longjmp`, callbacks, reentrancy, or retained pointers, and access only the
/// supplied bounded disjoint ranges. Descriptor storage must remain immutable
/// for the complete lease lifetime. The root path and dependency namespace
/// must remain non-adversarially stable from admission through final release.
#[cfg(not(target_os = "ios"))]
pub unsafe fn open_admitted_settlement_exact(
    canonical_path: &Path,
    expected_descriptor: &[u8],
) -> Result<NativeSettlementModuleLease, OpenError> {
    if !canonical_path.is_absolute() {
        return Err(OpenError::PathNotAbsolute);
    }
    if expected_descriptor.is_empty() || expected_descriptor.len() > MAX_DESCRIPTOR_BYTES {
        return Err(OpenError::InvalidExpectedDescriptorLength {
            actual: expected_descriptor.len(),
            maximum: MAX_DESCRIPTOR_BYTES,
        });
    }
    let projection = DescriptorProjection::parse(expected_descriptor)?;
    let resolved_path =
        std::fs::canonicalize(canonical_path).map_err(OpenError::PathCanonicalization)?;
    if resolved_path != canonical_path {
        return Err(OpenError::PathNotCanonical);
    }

    // SAFETY: The caller admits the exact image, its initializers/terminators,
    // and the complete dependency namespace. Unix resolution remains NOW/LOCAL
    // and Windows keeps root/default-safe dependency search.
    let library = unsafe { open_library(canonical_path) }.map_err(OpenError::LibraryOpen)?;

    // SAFETY: The complete caller contract fixes all three symbol ABIs. Each
    // pointer is copied while the sole Library pin remains live below.
    let getter: DescriptorGetter =
        *unsafe { library.get(projection.getter_symbol.as_slice()) }.map_err(OpenError::GetterLookup)?;
    let execute: ExecuteEntry =
        *unsafe { library.get(projection.execute_symbol.as_slice()) }.map_err(OpenError::ExecuteLookup)?;
    let settle: SettleEntry =
        *unsafe { library.get(projection.settle_symbol.as_slice()) }.map_err(OpenError::SettleLookup)?;

    let getter_address = getter as *const () as *const c_void;
    let execute_address = execute as *const () as *const c_void;
    let settle_address = settle as *const () as *const c_void;
    if getter_address == execute_address
        || getter_address == settle_address
        || execute_address == settle_address
    {
        return Err(OpenError::AliasedSettlementSymbols);
    }
    let root_allocation = root_image_allocation(getter_address, &resolved_path)?;
    if root_image_allocation(execute_address, &resolved_path)? != root_allocation
        || root_image_allocation(settle_address, &resolved_path)? != root_allocation
    {
        return Err(OpenError::RootImageProvenanceMismatch);
    }

    // SAFETY: The admitted getter is synchronous, non-unwinding, and returns
    // immutable storage valid for the exact expected descriptor length.
    let descriptor_pointer = unsafe { getter() };
    if descriptor_pointer.is_null() {
        return Err(OpenError::NullDescriptor);
    }
    if root_image_allocation(descriptor_pointer.cast(), &resolved_path)? != root_allocation {
        return Err(OpenError::RootImageProvenanceMismatch);
    }
    // SAFETY: The caller establishes this exact immutable readable range for
    // the entire lifetime of the still-live Library.
    let actual_descriptor =
        unsafe { std::slice::from_raw_parts(descriptor_pointer, expected_descriptor.len()) };
    if actual_descriptor != expected_descriptor {
        return Err(OpenError::DescriptorMismatch);
    }

    let instance_id = allocate_instance_id()?;
    Ok(NativeSettlementModuleLease {
        inner: Arc::new(LoadedSettlementModule {
            instance_id,
            canonical_path: resolved_path,
            root_image_allocation: root_allocation,
            descriptor: expected_descriptor.to_vec().into_boxed_slice(),
            capacities: projection.capacities,
            execute,
            settle,
            _library: library,
        }),
        _same_thread: PhantomData,
    })
}

struct DescriptorProjection {
    #[cfg(not(target_os = "ios"))]
    getter_symbol: Vec<u8>,
    #[cfg(not(target_os = "ios"))]
    execute_symbol: Vec<u8>,
    #[cfg(not(target_os = "ios"))]
    settle_symbol: Vec<u8>,
    capacities: SettlementBufferCapacities,
}

impl DescriptorProjection {
    #[cfg(not(target_os = "ios"))]
    fn parse(bytes: &[u8]) -> Result<Self, OpenError> {
        Self::parse_for_profile(bytes, 1)
    }

    fn parse_ios_static(
        bytes: &[u8],
        target: IosStaticTarget,
    ) -> Result<Self, StaticSettlementRegistrationError> {
        if !descriptor_has_target_and_profile(bytes, target.canonical_tag(), 2) {
            return Err(StaticSettlementRegistrationError::WrongTarget);
        }
        Self::parse_for_profile(bytes, 2)
            .map_err(|_| StaticSettlementRegistrationError::InvalidDescriptor)
    }

    fn parse_for_profile(bytes: &[u8], expected_profile: u32) -> Result<Self, OpenError> {
        let mut reader = Reader::new(bytes);
        if reader.take(8)? != b"SPXNABI3"
            || reader.u32()? != 3
            || reader.u32()? != HEADER_BYTES as u32
            || reader.usize()? != bytes.len()
        {
            return Err(OpenError::InvalidSettlementDescriptorSchema);
        }
        let _target = reader.text(MAX_TEXT_BYTES)?;
        if reader.u32()? != expected_profile {
            return Err(OpenError::InvalidSettlementDescriptorSchema);
        }
        for _ in 0..FINGERPRINT_COUNT {
            if reader.take(FINGERPRINT_BYTES)? == [0; FINGERPRINT_BYTES] {
                return Err(OpenError::InvalidSettlementDescriptorSchema);
            }
        }
        let _module = reader.text(MAX_TEXT_BYTES)?;
        let _function = reader.text(MAX_TEXT_BYTES)?;
        let getter_symbol = reader.symbol()?;
        let execute_symbol = reader.symbol()?;
        let settle_symbol = reader.symbol()?;
        if getter_symbol == execute_symbol
            || getter_symbol == settle_symbol
            || execute_symbol == settle_symbol
        {
            return Err(OpenError::InvalidSettlementSymbols);
        }
        if reader.u32()? != 3 || reader.u32()? != 0x03ff {
            return Err(OpenError::InvalidSettlementDescriptorSchema);
        }
        let mut raw = [0_u32; CAPACITY_COUNT];
        for value in &mut raw {
            *value = reader.u32()?;
        }
        let parameter_count = reader.usize()?;
        let mut request = 104_u32;
        let mut owned = Vec::new();
        for expected_index in 0..parameter_count {
            let tag = reader.u32()?;
            if reader.usize()? != expected_index {
                return Err(OpenError::InvalidSettlementDescriptorSchema);
            }
            let value = reader.text(MAX_TEXT_BYTES)?;
            match tag {
                1 => {
                    request = request
                        .checked_add(match reader.u32()? {
                            1 => 16,
                            2 => 12,
                            _ => return Err(OpenError::InvalidSettlementDescriptorSchema),
                        })
                        .ok_or(OpenError::InvalidSettlementCapacities)?;
                }
                2 => {
                    let ordinal = reader.usize()?;
                    if ordinal != owned.len() {
                        return Err(OpenError::InvalidSettlementDescriptorSchema);
                    }
                    let _resource = reader.text(MAX_TEXT_BYTES)?;
                    let _lifecycle = reader.text(MAX_TEXT_BYTES)?;
                    if reader.u32()? != 1 {
                        return Err(OpenError::InvalidSettlementDescriptorSchema);
                    }
                    request = request
                        .checked_add(20)
                        .ok_or(OpenError::InvalidSettlementCapacities)?;
                    owned.push((expected_index, value, ordinal));
                }
                _ => return Err(OpenError::InvalidSettlementDescriptorSchema),
            }
        }
        match reader.u32()? {
            1 => {}
            2 => {
                let index = reader.usize()?;
                let value = reader.text(MAX_TEXT_BYTES)?;
                let ordinal = reader.usize()?;
                if !owned
                    .iter()
                    .any(|entry| entry.0 == index && entry.1 == value && entry.2 == ordinal)
                {
                    return Err(OpenError::InvalidSettlementDescriptorSchema);
                }
            }
            _ => return Err(OpenError::InvalidSettlementDescriptorSchema),
        }
        let graph_len = reader.usize()?;
        if graph_len == 0 {
            return Err(OpenError::InvalidSettlementDescriptorSchema);
        }
        let _graph = reader.take(graph_len)?;
        if !reader.is_finished() {
            return Err(OpenError::InvalidSettlementDescriptorSchema);
        }
        validate_capacities(raw, request, owned.len())?;
        Ok(Self {
            #[cfg(not(target_os = "ios"))]
            getter_symbol,
            #[cfg(not(target_os = "ios"))]
            execute_symbol,
            #[cfg(not(target_os = "ios"))]
            settle_symbol,
            capacities: SettlementBufferCapacities {
                request: usize::try_from(raw[0])
                    .map_err(|_| OpenError::InvalidSettlementCapacities)?,
                execute_response: usize::try_from(raw[1])
                    .map_err(|_| OpenError::InvalidSettlementCapacities)?,
                frame: usize::try_from(raw[2])
                    .map_err(|_| OpenError::InvalidSettlementCapacities)?,
                decision: usize::try_from(raw[3])
                    .map_err(|_| OpenError::InvalidSettlementCapacities)?,
                candidate_receipt: usize::try_from(raw[5])
                    .map_err(|_| OpenError::InvalidSettlementCapacities)?,
            },
        })
    }
}

fn descriptor_has_target_and_profile(bytes: &[u8], expected_target: &str, profile: u32) -> bool {
    let Some(header) = bytes.get(..HEADER_BYTES) else {
        return false;
    };
    if &header[..8] != b"SPXNABI3"
        || u32::from_le_bytes(header[8..12].try_into().expect("fixed header")) != 3
        || u32::from_le_bytes(header[12..16].try_into().expect("fixed header"))
            != HEADER_BYTES as u32
        || usize::try_from(u32::from_le_bytes(
            header[16..20].try_into().expect("fixed header"),
        ))
        .ok()
            != Some(bytes.len())
    {
        return false;
    }
    let Some(length_bytes) = bytes.get(HEADER_BYTES..HEADER_BYTES + 4) else {
        return false;
    };
    let Ok(length) = usize::try_from(u32::from_le_bytes(
        length_bytes.try_into().expect("fixed text length"),
    )) else {
        return false;
    };
    let Some(target_end) = HEADER_BYTES
        .checked_add(4)
        .and_then(|start| start.checked_add(length))
    else {
        return false;
    };
    let Some(profile_end) = target_end.checked_add(4) else {
        return false;
    };
    bytes.get(HEADER_BYTES + 4..target_end) == Some(expected_target.as_bytes())
        && bytes
            .get(target_end..profile_end)
            .map(|value| u32::from_le_bytes(value.try_into().expect("fixed linkage width")))
            == Some(profile)
}

fn validate_capacities(
    raw: [u32; CAPACITY_COUNT],
    exact_request: u32,
    owned_count: usize,
) -> Result<(), OpenError> {
    let [request, response, frame, decision, action, candidate, events, dictionary_bytes, dictionary_entries, resources, checkpoints, graph_work, active, quarantined, reserved] =
        raw;
    let max_wire = u32::try_from(MAX_CALL_WIRE_BYTES).expect("wire bound fits u32");
    if [request, response, frame, decision, action, candidate]
        .iter()
        .any(|value| *value == 0 || *value > max_wire)
        || events == 0
        || events > MAX_EVENT_COUNT
        || dictionary_bytes == 0
        || dictionary_bytes > MAX_DICTIONARY_BYTES
        || dictionary_entries == 0
        || dictionary_entries > MAX_DICTIONARY_ENTRIES
        || resources == 0
        || resources > MAX_RESOURCES
        || usize::try_from(resources).ok() != Some(owned_count)
        || checkpoints == 0
        || checkpoints > MAX_CHECKPOINTS
        || graph_work == 0
        || graph_work > MAX_GRAPH_WORK_UNITS
    {
        return Err(OpenError::InvalidSettlementCapacities);
    }
    let exact_response = 156_u32.checked_add(
        events
            .checked_mul(4)
            .ok_or(OpenError::InvalidSettlementCapacities)?,
    );
    let exact_frame = 388_u32.checked_add(
        resources
            .checked_mul(12)
            .ok_or(OpenError::InvalidSettlementCapacities)?,
    );
    let exact_candidate = 372_u32.checked_add(
        resources
            .checked_mul(12)
            .ok_or(OpenError::InvalidSettlementCapacities)?,
    );
    let exact_work = resources.checked_mul(checkpoints);
    let per_frame = request
        .checked_add(response)
        .and_then(|value| value.checked_add(frame))
        .and_then(|value| value.checked_add(DECISION_BYTES))
        .and_then(|value| value.checked_add(ACTION_EVIDENCE_BYTES))
        .and_then(|value| value.checked_add(candidate))
        .and_then(|value| value.checked_add(HOST_RECEIPT_BYTES));
    let exact_reserved = per_frame.and_then(|value| {
        MAX_ACTIVE_FRAMES
            .checked_add(MAX_QUARANTINED_FRAMES)
            .and_then(|count| count.checked_mul(value))
    });
    if request != exact_request
        || Some(response) != exact_response
        || Some(frame) != exact_frame
        || decision != DECISION_BYTES
        || action != ACTION_EVIDENCE_BYTES
        || Some(candidate) != exact_candidate
        || Some(graph_work) != exact_work
        || active != MAX_ACTIVE_FRAMES
        || quarantined != MAX_QUARANTINED_FRAMES
        || Some(reserved) != exact_reserved
        || reserved > MAX_INSTANCE_RESERVED_BYTES
    {
        return Err(OpenError::InvalidSettlementCapacities);
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], OpenError> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(OpenError::InvalidSettlementDescriptorSchema)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(OpenError::InvalidSettlementDescriptorSchema)?;
        self.cursor = end;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, OpenError> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .expect("reader returned exact u32 width"),
        ))
    }

    fn usize(&mut self) -> Result<usize, OpenError> {
        usize::try_from(self.u32()?).map_err(|_| OpenError::InvalidSettlementDescriptorSchema)
    }

    fn text(&mut self, maximum: usize) -> Result<Vec<u8>, OpenError> {
        let length = self.usize()?;
        if length == 0 || length > maximum {
            return Err(OpenError::InvalidSettlementDescriptorSchema);
        }
        let value = self.take(length)?.to_vec();
        if value.contains(&0) || std::str::from_utf8(&value).is_err() {
            return Err(OpenError::InvalidSettlementDescriptorSchema);
        }
        Ok(value)
    }

    fn symbol(&mut self) -> Result<Vec<u8>, OpenError> {
        let symbol = self.text(MAX_GETTER_SYMBOL_BYTES)?;
        if !is_c_symbol(&symbol) {
            return Err(OpenError::InvalidSettlementSymbols);
        }
        Ok(symbol)
    }

    fn is_finished(&self) -> bool {
        self.cursor == self.bytes.len()
    }
}

fn is_c_symbol(value: &[u8]) -> bool {
    let Some(first) = value.first() else {
        return false;
    };
    (first.is_ascii_alphabetic() || *first == b'_')
        && value[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
}

fn zeroed(length: usize) -> Result<Vec<u8>, SettlementCallError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| SettlementCallError::AllocationFailed)?;
    bytes.resize(length, 0);
    Ok(bytes)
}

fn prepare_execute(
    instance_id: ModuleInstanceId,
    capacities: SettlementBufferCapacities,
) -> Result<PreparedSettlementExecute, SettlementCallError> {
    let buffers = SettlementBuffers {
        module_instance: instance_id,
        request: zeroed(capacities.request)?,
        frame: zeroed(capacities.frame)?,
        response: zeroed(capacities.execute_response)?,
        decision: zeroed(capacities.decision)?,
        candidate: zeroed(capacities.candidate_receipt)?,
    };
    if !buffers_are_disjoint(&buffers) {
        return Err(SettlementCallError::BufferOverlap);
    }
    Ok(PreparedSettlementExecute {
        buffers,
        execute_return: None,
        _same_thread: PhantomData,
    })
}

fn invoke_execute(
    instance_id: ModuleInstanceId,
    execute: ExecuteEntry,
    call: &mut PreparedSettlementExecute,
) -> Result<u32, SettlementCallError> {
    if call.buffers.module_instance != instance_id {
        return Err(SettlementCallError::WrongModuleInstance);
    }
    if call.execute_return.is_some() {
        return Err(SettlementCallError::ExecuteAlreadyInvoked);
    }
    if !buffers_are_disjoint(&call.buffers) {
        return Err(SettlementCallError::BufferOverlap);
    }
    call.execute_return = Some(u32::MAX);
    let request_len = wire_len(call.buffers.request.len());
    let frame_len = wire_len(call.buffers.frame.len());
    let response_len = wire_len(call.buffers.response.len());
    // SAFETY: Static admission fixes the same synchronous, non-retaining ABI
    // as dynamic admission and all ranges were rechecked disjoint above.
    let result = unsafe {
        execute(
            call.buffers.request.as_ptr(),
            request_len,
            call.buffers.frame.as_mut_ptr(),
            frame_len,
            call.buffers.response.as_mut_ptr(),
            response_len,
        )
    };
    call.execute_return = Some(result);
    Ok(result)
}

fn invoke_settle(
    instance_id: ModuleInstanceId,
    settle: SettleEntry,
    call: &mut PreparedSettlementCall,
) -> Result<u32, SettlementCallError> {
    if call.buffers.module_instance != instance_id {
        return Err(SettlementCallError::WrongModuleInstance);
    }
    if call.settle_return.is_some() {
        return Err(SettlementCallError::SettleAlreadyInvoked);
    }
    if !buffers_are_disjoint(&call.buffers) {
        return Err(SettlementCallError::BufferOverlap);
    }
    call.settle_return = Some(u32::MAX);
    let frame_len = wire_len(call.buffers.frame.len());
    let decision_len = wire_len(call.buffers.decision.len());
    let candidate_len = wire_len(call.buffers.candidate.len());
    // SAFETY: The statically registered settle entry has the admitted ABI and
    // receives only the three nonempty disjoint ranges reserved above.
    let result = unsafe {
        settle(
            call.buffers.frame.as_mut_ptr(),
            frame_len,
            call.buffers.decision.as_ptr(),
            decision_len,
            call.buffers.candidate.as_mut_ptr(),
            candidate_len,
        )
    };
    call.settle_return = Some(result);
    Ok(result)
}

fn wire_len(length: usize) -> u32 {
    u32::try_from(length).expect("validated SPXNABI3 wire capacity fits u32")
}

fn buffers_are_disjoint(buffers: &SettlementBuffers) -> bool {
    let ranges = [
        range(&buffers.request),
        range(&buffers.frame),
        range(&buffers.response),
        range(&buffers.decision),
        range(&buffers.candidate),
    ];
    ranges.iter().enumerate().all(|(index, left)| {
        ranges[index + 1..]
            .iter()
            .all(|right| left.1 <= right.0 || right.1 <= left.0)
    })
}

fn range(bytes: &[u8]) -> (usize, usize) {
    let start = bytes.as_ptr() as usize;
    let end = start
        .checked_add(bytes.len())
        .expect("validated wire allocation address range cannot wrap");
    (start, end)
}

#[cfg(all(unix, not(target_os = "ios")))]
fn root_image_allocation(address: *const c_void, expected_path: &Path) -> Result<usize, OpenError> {
    #[repr(C)]
    struct DlInfo {
        filename: *const c_char,
        base: *mut c_void,
        symbol_name: *const c_char,
        symbol_address: *mut c_void,
    }

    #[cfg_attr(any(target_os = "linux", target_os = "android"), link(name = "dl"))]
    unsafe extern "C" {
        fn dladdr(address: *const c_void, info: *mut DlInfo) -> c_int;
    }

    let mut info = DlInfo {
        filename: std::ptr::null(),
        base: std::ptr::null_mut(),
        symbol_name: std::ptr::null(),
        symbol_address: std::ptr::null_mut(),
    };
    // SAFETY: `info` is writable for the complete platform structure and the
    // address is a live function or immutable byte inside a pinned image.
    if unsafe { dladdr(address, &mut info) } == 0 || info.filename.is_null() || info.base.is_null()
    {
        return Err(OpenError::RootImageProvenanceMismatch);
    }
    // SAFETY: Successful dladdr supplies a NUL-terminated filename for the
    // lifetime of the corresponding loaded image.
    let filename = unsafe { CStr::from_ptr(info.filename) };
    use std::os::unix::ffi::OsStrExt;
    let path = Path::new(std::ffi::OsStr::from_bytes(filename.to_bytes()));
    let actual = std::fs::canonicalize(path).map_err(|_| OpenError::RootImageProvenanceMismatch)?;
    if actual != expected_path {
        return Err(OpenError::RootImageProvenanceMismatch);
    }
    Ok(info.base as usize)
}

#[cfg(windows)]
fn root_image_allocation(address: *const c_void, expected_path: &Path) -> Result<usize, OpenError> {
    type ModuleHandle = *mut c_void;
    const FROM_ADDRESS: u32 = 0x0000_0004;
    const UNCHANGED_REFCOUNT: u32 = 0x0000_0002;
    const INITIAL_PATH_UNITS: usize = 260;
    const MAX_PATH_UNITS: usize = 32_768;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleExW(
            flags: u32,
            address_as_name: *const u16,
            module: *mut ModuleHandle,
        ) -> i32;
        fn GetModuleFileNameW(module: ModuleHandle, path: *mut u16, size: u32) -> u32;
    }

    let mut module = std::ptr::null_mut();
    // SAFETY: FROM_ADDRESS reinterprets the second argument as a live code/data
    // address and returns its allocation-base module handle. UNCHANGED_REFCOUNT
    // ensures the provenance check creates no second image pin.
    if unsafe {
        GetModuleHandleExW(
            FROM_ADDRESS | UNCHANGED_REFCOUNT,
            address.cast(),
            &mut module,
        )
    } == 0
        || module.is_null()
    {
        return Err(OpenError::RootImageProvenanceMismatch);
    }
    let mut capacity = INITIAL_PATH_UNITS;
    loop {
        let mut path = vec![0_u16; capacity];
        // SAFETY: `path` is a writable UTF-16 buffer with the declared length,
        // and `module` is the allocation base returned for the live address.
        let length = unsafe {
            GetModuleFileNameW(
                module,
                path.as_mut_ptr(),
                u32::try_from(path.len()).expect("Windows path bound fits u32"),
            )
        };
        if length == 0 {
            return Err(OpenError::RootImageProvenanceMismatch);
        }
        let length = usize::try_from(length).map_err(|_| OpenError::RootImageProvenanceMismatch)?;
        if length < path.len() {
            path.truncate(length);
            use std::os::windows::ffi::OsStringExt;
            let actual_path = PathBuf::from(std::ffi::OsString::from_wide(&path));
            let actual = std::fs::canonicalize(actual_path)
                .map_err(|_| OpenError::RootImageProvenanceMismatch)?;
            return if actual == expected_path {
                Ok(module as usize)
            } else {
                Err(OpenError::RootImageProvenanceMismatch)
            };
        }
        capacity = capacity
            .checked_mul(2)
            .filter(|value| *value <= MAX_PATH_UNITS)
            .ok_or(OpenError::RootImageProvenanceMismatch)?;
    }
}
