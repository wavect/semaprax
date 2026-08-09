use super::{
    allocate_instance_id, open_library, ModuleInstanceId, OpenError, MAX_CALL_WIRE_BYTES,
    MAX_DESCRIPTOR_BYTES, MAX_GETTER_SYMBOL_BYTES,
};
use libloading::Library;
use std::error::Error;
use std::ffi::c_void;
#[cfg(unix)]
use std::ffi::{c_char, c_int, CStr};
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

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
const DECISION_BYTES: u32 = 172;
const ACTION_EVIDENCE_BYTES: u32 = 196;
const HOST_RECEIPT_BYTES: u32 = 524;

type DescriptorGetter = unsafe extern "C" fn() -> *const u8;
type ExecuteEntry = unsafe extern "C" fn(*const u8, u32, *mut u8, u32, *mut u8, u32) -> u32;
type SettleEntry = unsafe extern "C" fn(*mut u8, u32, *const u8, u32, *mut u8, u32) -> u32;

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
pub struct NativeSettlementModuleLease {
    inner: Arc<LoadedSettlementModule>,
    _same_thread: PhantomData<Rc<()>>,
}

struct LoadedSettlementModule {
    instance_id: ModuleInstanceId,
    canonical_path: PathBuf,
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
        *unsafe { library.get(&projection.getter_symbol) }.map_err(OpenError::GetterLookup)?;
    let execute: ExecuteEntry =
        *unsafe { library.get(&projection.execute_symbol) }.map_err(OpenError::ExecuteLookup)?;
    let settle: SettleEntry =
        *unsafe { library.get(&projection.settle_symbol) }.map_err(OpenError::SettleLookup)?;

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
    getter_symbol: Vec<u8>,
    execute_symbol: Vec<u8>,
    settle_symbol: Vec<u8>,
    capacities: SettlementBufferCapacities,
}

impl DescriptorProjection {
    fn parse(bytes: &[u8]) -> Result<Self, OpenError> {
        let mut reader = Reader::new(bytes);
        if reader.take(8)? != b"SPXNABI3"
            || reader.u32()? != 3
            || reader.u32()? != HEADER_BYTES as u32
            || reader.usize()? != bytes.len()
        {
            return Err(OpenError::InvalidSettlementDescriptorSchema);
        }
        let _target = reader.text(MAX_TEXT_BYTES)?;
        if reader.u32()? != 1 {
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
            getter_symbol,
            execute_symbol,
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

#[cfg(unix)]
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
