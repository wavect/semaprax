//! The real Apple Metal device session: kernel binding through the same
//! [`session::bind`] admission path the CPU reference uses, deterministic
//! MSL compilation, real buffer allocation, and dispatch.
//!
//! # Lifecycle
//!
//! Mirrors [`super::super::cpu_reference::session`] deliberately: a session
//! opens only with an explicit [`MetalCapability`]; buffers move through
//! allocate (zero-filled) -> upload -> dispatch (synchronous,
//! `waitUntilCompleted`) -> download -> release; the first dispatch that
//! selects a [`MetalSessionFailure`] is sticky (`SPX-GC019`, via
//! [`MetalRefusal::FailureAlreadySelected`]); [`MetalSession::settle`]
//! releases every still-live buffer in reverse allocation order exactly
//! once. Every refusal is one of the existing `SPX-GC0xx` codes carried by
//! [`ComputeRefusal`] (see [`MetalRefusal`] for the one addition the
//! differently-typed sticky failure needs).
//!
//! # Non-claims
//!
//! `Cancelled`/`DeviceLost` exist in [`MetalSessionFailure`] because a real
//! command buffer can genuinely report them (`MTLCommandBufferStatus`), and
//! this session classifies whatever the device actually reports — but no
//! test in this crate deliberately triggers either outcome on real
//! hardware: unlike the CPU reference's [`DispatchControl`] injection
//! points, there is no supported way to force a real device to lose itself
//! or a real command buffer to race a cancellation deterministically within
//! one process. Only [`MetalSessionFailure::KernelStatus`] (a checked
//! arithmetic failure) is exercised by a repeatable local test.
//!
//! [`DispatchControl`]: super::super::cpu_reference::DispatchControl

// This module owns real Metal device, buffer, and command-queue authority
// (via `objc2`/`objc2-metal` FFI): reading and writing raw GPU-visible
// buffer memory, and calling Objective-C methods whose safety the Rust
// compiler cannot check. Every unsafe block below carries its own `SAFETY`
// comment. Mirrors the existing precedent at
// `src/process_provider/registered/platform.rs`.
#![allow(unsafe_code)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::{NSProcessInfo, NSString};
use objc2_metal::{
    MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandEncoder, MTLCommandQueue,
    MTLComputeCommandEncoder, MTLComputePipelineState, MTLCreateSystemDefaultDevice, MTLDevice,
    MTLLibrary, MTLResource, MTLResourceOptions, MTLSize,
};

use crate::cleanup_plan::StatusCase;
use crate::compute_profile::boundary_profile::MAX_BUFFER_ELEMENTS;
use crate::compute_profile::classifier::{DeviceEffect, Refusal};
use crate::compute_profile::cpu_reference::kernel_ir::{self, KernelIr, Scalar, ScalarKind};
use crate::compute_profile::cpu_reference::session::{self, KernelShape};
use crate::compute_profile::cpu_reference::{
    ComputeRefusal, FAILURE_ALREADY_SELECTED, MAX_SESSION_LIVE_ELEMENTS,
};
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::msl;

// `MTLCreateSystemDefaultDevice` needs CoreGraphics linked; the `Metal`
// framework itself is already linked by `objc2-metal`'s own generated code.
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {}

/// The explicit authority to open one Metal session, naming the granted
/// device-effect vocabulary. Mirrors
/// `cpu_reference::ComputeCapability::cpu_reference` exactly and for the
/// same reason: a grant that admits `DeviceAlloc` without `DeviceRelease`
/// would let a buffer be allocated with no way to ever free it.
#[derive(Debug, Eq, PartialEq)]
pub struct MetalCapability {
    granted: Vec<DeviceEffect>,
}

impl MetalCapability {
    pub fn new(effects: &[DeviceEffect]) -> Result<Self, MetalRefusal> {
        if effects.contains(&DeviceEffect::DeviceAlloc)
            && !effects.contains(&DeviceEffect::DeviceRelease)
        {
            return Err(ComputeRefusal::EffectNotGranted {
                effect: DeviceEffect::DeviceRelease,
            }
            .into());
        }
        let mut granted = effects.to_vec();
        granted.dedup();
        Ok(Self { granted })
    }

    /// Grant the complete closed device-effect vocabulary.
    pub fn all() -> Self {
        Self {
            granted: vec![
                DeviceEffect::DeviceAlloc,
                DeviceEffect::DeviceCopyIn,
                DeviceEffect::DeviceCopyOut,
                DeviceEffect::DeviceDispatch,
                DeviceEffect::DeviceSynchronize,
                DeviceEffect::DeviceRelease,
            ],
        }
    }
}

/// No Metal device is present on this host, or it refused a resource the
/// session needs before any kernel-specific question can be asked. Never
/// one of the closed `SPX-GC0xx` refusals — those classify a *kernel*, and
/// there is no kernel yet to classify. Callers (in particular tests) treat
/// this as "skip, with this exact reason", never as evidence about any
/// other machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceUnavailable(pub String);

/// Real, locally observed Metal device facts, recorded so a claim of
/// hardware evidence is always attached to the exact device it came from.
/// Never hosted, never a simulator, never a claim about any other machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceProvenance {
    pub device_name: String,
    pub registry_id: u64,
    pub operating_system_version: String,
}

fn open_device() -> Option<Retained<ProtocolObject<dyn MTLDevice>>> {
    MTLCreateSystemDefaultDevice()
}

fn provenance_of(device: &ProtocolObject<dyn MTLDevice>) -> DeviceProvenance {
    let os_version = NSProcessInfo::processInfo()
        .operatingSystemVersionString()
        .to_string();
    DeviceProvenance {
        device_name: device.name().to_string(),
        registry_id: device.registryID(),
        operating_system_version: os_version,
    }
}

/// The real local Metal device's provenance, or `None` if this host has
/// none. Standalone (opens no session), so a test can decide to skip
/// before paying for anything else.
pub fn device_provenance() -> Option<DeviceProvenance> {
    open_device().as_deref().map(provenance_of)
}

/// Why one Metal-backend operation was refused. Every case except
/// [`Self::FailureAlreadySelected`] is an existing [`ComputeRefusal`] — the
/// same `SPX-GC0xx` vocabulary the CPU reference uses, reused rather than
/// reinvented. `FailureAlreadySelected` needs its own case only because a
/// [`MetalSessionFailure`] is not the CPU reference's `SessionFailure`
/// type; it still carries the identical `SPX-GC019` code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetalRefusal {
    Compute(ComputeRefusal),
    FailureAlreadySelected { failure: MetalSessionFailure },
}

impl From<ComputeRefusal> for MetalRefusal {
    fn from(value: ComputeRefusal) -> Self {
        Self::Compute(value)
    }
}

impl MetalRefusal {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Compute(inner) => inner.code(),
            Self::FailureAlreadySelected { .. } => FAILURE_ALREADY_SELECTED,
        }
    }

    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Compute(inner) => inner.diagnostic(),
            Self::FailureAlreadySelected { failure } => Diagnostic::io(
                self.code(),
                format!(
                    "{} refuses the operation: failure {failure:?} is already selected, only \
                     release and settlement remain",
                    super::METAL_BACKEND_SCHEMA
                ),
            ),
        }
    }
}

/// The selected, sticky failure of one Metal session. See the module's
/// Non-claims section for `Cancelled`/`DeviceLost`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetalSessionFailure {
    KernelStatus {
        declaration: String,
        invocation: usize,
        status: StatusCase,
    },
    Cancelled,
    DeviceLost,
}

/// The executed result of one admitted dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetalDispatchOutcome {
    Completed { invocations: usize },
    Failed(MetalSessionFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetalReleaseCause {
    Explicit,
    Settlement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetalReleaseEvent {
    pub buffer: u32,
    pub cause: MetalReleaseCause,
}

/// The consumed session's complete, deterministic facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetalSettlement {
    pub selected: Option<MetalSessionFailure>,
    pub releases: Vec<MetalReleaseEvent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetalBufferHandle {
    session: u64,
    index: u32,
}

impl MetalBufferHandle {
    pub fn index(self) -> u32 {
        self.index
    }
}

enum MetalBufferState {
    Live {
        kind: ScalarKind,
        buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
        len: usize,
    },
    Released,
}

/// A kernel compiled and bound to one checked declaration: its persistent
/// identity, its lowered-body fingerprint (shared with the CPU reference's
/// own [`kernel_ir::fingerprint`]), the SHA-256 of the exact MSL it was
/// compiled from, and the session it was loaded into.
pub struct MetalKernelArtifact {
    session: u64,
    declaration: String,
    shape: KernelShape,
    fingerprint: String,
    msl_source: String,
    msl_sha256: String,
    ir: KernelIr,
    pipeline: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
}

impl MetalKernelArtifact {
    pub fn declaration(&self) -> &str {
        &self.declaration
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn msl_source(&self) -> &str {
        &self.msl_source
    }

    pub fn msl_sha256(&self) -> &str {
        &self.msl_sha256
    }
}

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// The Metal v1 buffer byte width of every admitted [`ScalarKind`]. `Bool`
/// and `U8` are each one byte (MSL's `bool` and `uchar`); `Usize` is eight
/// (MSL's `ulong`, the closest native type to the checked 64-bit unsigned
/// `usize` — see [`msl::msl_type`]).
fn element_size(kind: ScalarKind) -> usize {
    match kind {
        ScalarKind::I64 | ScalarKind::Usize => 8,
        ScalarKind::I32 => 4,
        ScalarKind::U8 | ScalarKind::Bool => 1,
    }
}

/// SAFETY: `base` points to at least `byte_offset + element_size(value.kind())`
/// writable bytes; every call site derives `byte_offset` from a
/// bounds-checked `(index, element_size)` pair. `Usize` writes as `u64`
/// (MSL's `ulong`, see [`msl::msl_type`]); `Bool` writes as a single `0`/`1`
/// byte (MSL's `bool` is one byte).
unsafe fn write_scalar(base: *mut u8, byte_offset: usize, value: Scalar) {
    match value {
        Scalar::I64(value) => unsafe { base.add(byte_offset).cast::<i64>().write_unaligned(value) },
        Scalar::I32(value) => unsafe { base.add(byte_offset).cast::<i32>().write_unaligned(value) },
        Scalar::Usize(value) => unsafe {
            base.add(byte_offset).cast::<u64>().write_unaligned(value)
        },
        Scalar::U8(value) => unsafe { base.add(byte_offset).write(value) },
        Scalar::Bool(value) => unsafe { base.add(byte_offset).write(u8::from(value)) },
    }
}

/// SAFETY: see [`write_scalar`]; the same bound holds for reads.
unsafe fn read_scalar(base: *const u8, byte_offset: usize, kind: ScalarKind) -> Scalar {
    match kind {
        ScalarKind::I64 => {
            Scalar::I64(unsafe { base.add(byte_offset).cast::<i64>().read_unaligned() })
        }
        ScalarKind::I32 => {
            Scalar::I32(unsafe { base.add(byte_offset).cast::<i32>().read_unaligned() })
        }
        ScalarKind::Usize => {
            Scalar::Usize(unsafe { base.add(byte_offset).cast::<u64>().read_unaligned() })
        }
        ScalarKind::U8 => Scalar::U8(unsafe { base.add(byte_offset).read() }),
        ScalarKind::Bool => Scalar::Bool(unsafe { base.add(byte_offset).read() } != 0),
    }
}

fn status_from_code(code: u32) -> Option<StatusCase> {
    match code {
        1 => Some(StatusCase::AddOverflow),
        2 => Some(StatusCase::SubOverflow),
        3 => Some(StatusCase::MulOverflow),
        4 => Some(StatusCase::DivisionByZero),
        5 => Some(StatusCase::DivisionOverflow),
        6 => Some(StatusCase::RemainderByZero),
        7 => Some(StatusCase::RemainderOverflow),
        8 => Some(StatusCase::NegationOverflow),
        _ => None,
    }
}

/// Every [`ScalarKind`] the CPU reference admits now has a Metal v1 buffer
/// and literal lowering (see [`msl::msl_type`]), so this can only refuse a
/// future kind added there before this backend is taught it. Refuses
/// through the same `SPX-GC004` (`TypeOutsideKernelVocabulary`) the
/// classifier already uses for "a parameter's type is outside the
/// kernel-safe vocabulary" — this is defense in depth for that same closed
/// reason, not a new one, and (unlike lowering itself) never the primary
/// admission path: `session::bind`'s own classifier call already refused
/// anything outside the CPU reference's vocabulary before this runs.
fn assert_metal_admits(ir: &KernelIr) -> Result<(), MetalRefusal> {
    for kind in &ir.params {
        if msl::msl_type(*kind).is_err() {
            return Err(ComputeRefusal::Profile {
                refusal: Refusal::TypeOutsideKernelVocabulary { param: "param" },
                detail: format!("the Metal v1 backend has no buffer lowering for {kind:?}"),
            }
            .into());
        }
    }
    if msl::msl_type(ir.result).is_err() {
        return Err(ComputeRefusal::Profile {
            refusal: Refusal::TypeOutsideKernelVocabulary { param: "result" },
            detail: format!(
                "the Metal v1 backend has no buffer lowering for {:?}",
                ir.result
            ),
        }
        .into());
    }
    Ok(())
}

fn checked_end(
    offset: usize,
    len: usize,
    capacity: usize,
    buffer: u32,
) -> Result<usize, MetalRefusal> {
    offset
        .checked_add(len)
        .filter(|end| *end <= capacity)
        .ok_or_else(|| {
            ComputeRefusal::OutOfBounds {
                detail: format!(
                    "range {offset}+{len} exceeds buffer {buffer} of {capacity} elements"
                ),
            }
            .into()
        })
}

/// One real Metal compute session. See the module documentation's
/// [Lifecycle](self#lifecycle) section.
pub struct MetalSession {
    id: u64,
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    granted: Vec<DeviceEffect>,
    buffers: Vec<MetalBufferState>,
    live_elements: usize,
    selected: Option<MetalSessionFailure>,
    releases: Vec<MetalReleaseEvent>,
}

impl MetalSession {
    pub fn open(capability: MetalCapability) -> Result<Self, DeviceUnavailable> {
        let device = open_device().ok_or_else(|| {
            DeviceUnavailable("no Metal device is available on this host".to_owned())
        })?;
        let queue = device.newCommandQueue().ok_or_else(|| {
            DeviceUnavailable("the Metal device refused to create a command queue".to_owned())
        })?;
        Ok(Self {
            id: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
            device,
            queue,
            granted: capability.granted,
            buffers: Vec::new(),
            live_elements: 0,
            selected: None,
            releases: Vec::new(),
        })
    }

    pub fn provenance(&self) -> DeviceProvenance {
        provenance_of(&self.device)
    }

    fn admit(&self, effect: DeviceEffect) -> Result<(), MetalRefusal> {
        if let Some(failure) = &self.selected {
            return Err(MetalRefusal::FailureAlreadySelected {
                failure: failure.clone(),
            });
        }
        if !self.granted.contains(&effect) {
            return Err(ComputeRefusal::EffectNotGranted { effect }.into());
        }
        Ok(())
    }

    fn live(
        &self,
        handle: MetalBufferHandle,
    ) -> Result<(ScalarKind, usize, &Retained<ProtocolObject<dyn MTLBuffer>>), MetalRefusal> {
        if handle.session != self.id {
            return Err(ComputeRefusal::StaleHandle {
                detail: format!("buffer {} belongs to another session", handle.index),
            }
            .into());
        }
        match self.buffers.get(handle.index as usize) {
            Some(MetalBufferState::Live { kind, buffer, len }) => Ok((*kind, *len, buffer)),
            Some(MetalBufferState::Released) => Err(ComputeRefusal::BufferReleased {
                buffer: handle.index,
            }
            .into()),
            None => Err(ComputeRefusal::StaleHandle {
                detail: format!("buffer {} was never allocated here", handle.index),
            }
            .into()),
        }
    }

    /// `DeviceAlloc`: a zero-filled buffer of `len` elements of `kind`.
    /// Only `i64`/`i32` are constructible; anything else refuses
    /// (`SPX-GC018`) rather than silently degrading.
    pub fn alloc(
        &mut self,
        kind: ScalarKind,
        len: usize,
    ) -> Result<MetalBufferHandle, MetalRefusal> {
        self.admit(DeviceEffect::DeviceAlloc)?;
        if len == 0 {
            return Err(ComputeRefusal::OutOfBounds {
                detail: "a buffer holds at least one element".to_owned(),
            }
            .into());
        }
        if len > MAX_BUFFER_ELEMENTS
            || self
                .live_elements
                .checked_add(len)
                .is_none_or(|total| total > MAX_SESSION_LIVE_ELEMENTS)
        {
            return Err(ComputeRefusal::Profile {
                refusal: Refusal::CapacityBoundExceeded {
                    param: "allocation",
                },
                detail: format!("{len} elements requested with {} live", self.live_elements),
            }
            .into());
        }
        let element_size = element_size(kind);
        let byte_len = len * element_size;
        let buffer = self
            .device
            .newBufferWithLength_options(byte_len, MTLResourceOptions::StorageModeShared)
            .ok_or_else(|| {
                MetalRefusal::from(ComputeRefusal::OutOfBounds {
                    detail: "the Metal device refused the allocation".to_owned(),
                })
            })?;
        // SAFETY: a freshly allocated `MTLResourceStorageModeShared` buffer's
        // `contents()` is `byte_len` host-visible bytes.
        unsafe {
            core::ptr::write_bytes(buffer.contents().as_ptr().cast::<u8>(), 0u8, byte_len);
        }
        let index = u32::try_from(self.buffers.len()).map_err(|_| {
            MetalRefusal::from(ComputeRefusal::OutOfBounds {
                detail: "too many allocations".to_owned(),
            })
        })?;
        self.buffers
            .push(MetalBufferState::Live { kind, buffer, len });
        self.live_elements += len;
        Ok(MetalBufferHandle {
            session: self.id,
            index,
        })
    }

    /// `DeviceCopyIn`: copy `values` into `[offset, offset + values.len())`.
    pub fn upload(
        &mut self,
        handle: MetalBufferHandle,
        offset: usize,
        values: &[Scalar],
    ) -> Result<(), MetalRefusal> {
        self.admit(DeviceEffect::DeviceCopyIn)?;
        let (kind, capacity, buffer) = self.live(handle)?;
        if let Some(value) = values.iter().find(|value| value.kind() != kind) {
            return Err(ComputeRefusal::ElementTypeMismatch {
                detail: format!(
                    "{:?} value uploaded into a {kind:?} buffer {}",
                    value.kind(),
                    handle.index
                ),
            }
            .into());
        }
        checked_end(offset, values.len(), capacity, handle.index)?;
        let element_size = element_size(kind);
        let base = buffer.contents().as_ptr() as *mut u8;
        for (position, value) in values.iter().enumerate() {
            // SAFETY: `(offset + position) < capacity` (checked above), so
            // the byte range lies within the buffer's allocated length.
            unsafe {
                write_scalar(base, (offset + position) * element_size, *value);
            }
        }
        Ok(())
    }

    /// `DeviceCopyOut`: copy `[offset, offset + len)` back to the host.
    pub fn download(
        &mut self,
        handle: MetalBufferHandle,
        offset: usize,
        len: usize,
    ) -> Result<Vec<Scalar>, MetalRefusal> {
        self.admit(DeviceEffect::DeviceCopyOut)?;
        let (kind, capacity, buffer) = self.live(handle)?;
        let end = checked_end(offset, len, capacity, handle.index)?;
        let element_size = element_size(kind);
        let base = buffer.contents().as_ptr() as *const u8;
        let mut values = Vec::with_capacity(end - offset);
        for position in offset..end {
            // SAFETY: `position < capacity` (checked above).
            values.push(unsafe { read_scalar(base, position * element_size, kind) });
        }
        Ok(values)
    }

    /// `DeviceRelease`: consume the buffer exactly once. Admitted after a
    /// selected failure, mirroring the CPU reference.
    pub fn release(&mut self, handle: MetalBufferHandle) -> Result<(), MetalRefusal> {
        let (_, len, _) = self.live(handle)?;
        self.buffers[handle.index as usize] = MetalBufferState::Released;
        self.live_elements -= len;
        self.releases.push(MetalReleaseEvent {
            buffer: handle.index,
            cause: MetalReleaseCause::Explicit,
        });
        Ok(())
    }

    /// Write one scalar directly into element `0` of a live buffer, without
    /// going through the ordinary `DeviceCopyIn`-gated [`Self::upload`].
    /// [`Self::dispatch_fold`] uses this to seed a fold's one-element
    /// accumulator buffer with its explicit initial value, mirroring the CPU
    /// reference's [`session::CpuReferenceSession::dispatch_fold`]: there,
    /// `initial` is a plain argument the dispatch itself consumes, not a
    /// separately admitted device transfer, so a fold dispatch needs only
    /// `DeviceDispatch` (matching `session::bind`'s own signature check that
    /// a fold's accumulator parameter is `ReadWriteView`, never a value the
    /// caller must have separately copied in).
    fn write_direct(&self, handle: MetalBufferHandle, value: Scalar) -> Result<(), MetalRefusal> {
        let (kind, capacity, buffer) = self.live(handle)?;
        if value.kind() != kind {
            return Err(ComputeRefusal::ElementTypeMismatch {
                detail: format!(
                    "{:?} initial value written into a {kind:?} buffer {}",
                    value.kind(),
                    handle.index
                ),
            }
            .into());
        }
        checked_end(0, 1, capacity, handle.index)?;
        let base = buffer.contents().as_ptr() as *mut u8;
        // SAFETY: `capacity >= 1` (checked by `checked_end` above), so byte
        // offset 0 lies within the buffer's allocated length.
        unsafe {
            write_scalar(base, 0, value);
        }
        Ok(())
    }

    /// Compile `source` into a library. No retry: a compile failure here is
    /// surfaced exactly once. An earlier version of this method retried on
    /// `XPC_ERROR_CONNECTION_INTERRUPTED` on the theory that the shared
    /// system Metal compiler service was merely overloaded; that was never
    /// confirmed (the offline `xcrun metal` compiler is not installed on
    /// this host — `MetalToolchain` is a separate, network-fetched
    /// component this task is not authorized to download — so the
    /// transient-vs-deterministic question could not be settled that way)
    /// and a bounded retry that might paper over a real, deterministic
    /// generated-MSL defect is worse than a visible failure.
    /// The failure was in fact deterministic generated-MSL input (see the
    /// `msl` module docs), which a retry would have hidden.
    fn compile_library(
        &self,
        source: &str,
    ) -> Result<Retained<ProtocolObject<dyn MTLLibrary>>, MetalRefusal> {
        let ns_source = NSString::from_str(source);
        self.device
            .newLibraryWithSource_options_error(&ns_source, None)
            .map_err(|error| {
                MetalRefusal::from(ComputeRefusal::KernelSelection {
                    detail: format!("MSL compilation failed: {}", error.localizedDescription()),
                })
            })
    }

    /// Generate MSL from `ir` for `shape` (an elementwise map through
    /// [`msl::generate`], a sequential fold through [`msl::generate_fold`]),
    /// compile it on this device, and bundle the compiled pipeline with the
    /// given `fingerprint` into an artifact. Shared by [`Self::load_kernel`]
    /// and [`Self::load_fold_kernel`] (whose `fingerprint` always comes from
    /// [`session::bind`] against a real checked declaration) and the
    /// `#[cfg(test)]` bypass [`Self::load_kernel_from_ir_for_test`] (whose
    /// caller derives `fingerprint` from a deliberately mutated `ir` for
    /// the negative-control test — never from any real checked source).
    fn build_artifact(
        &self,
        declaration: &str,
        shape: KernelShape,
        ir: KernelIr,
        fingerprint: String,
    ) -> Result<MetalKernelArtifact, MetalRefusal> {
        assert_metal_admits(&ir)?;
        let generated = match shape {
            KernelShape::ElementwiseMap { .. } => msl::generate(declaration, &ir),
            KernelShape::SequentialFold => msl::generate_fold(declaration, &ir),
        }
        .map_err(|detail| MetalRefusal::from(ComputeRefusal::KernelSelection { detail }))?;
        let library = self.compile_library(&generated.source)?;
        let function = library
            .newFunctionWithName(&NSString::from_str(msl::FUNCTION_NAME))
            .ok_or_else(|| {
                MetalRefusal::from(ComputeRefusal::KernelSelection {
                    detail: "the compiled library has no semaprax_kernel function".to_owned(),
                })
            })?;
        let pipeline = self
            .device
            .newComputePipelineStateWithFunction_error(&function)
            .map_err(|error| {
                MetalRefusal::from(ComputeRefusal::KernelSelection {
                    detail: format!("pipeline creation failed: {}", error.localizedDescription()),
                })
            })?;
        Ok(MetalKernelArtifact {
            session: self.id,
            declaration: declaration.to_owned(),
            shape,
            fingerprint,
            msl_source: generated.source,
            msl_sha256: generated.sha256,
            ir,
            pipeline,
        })
    }

    /// Bind the checked declaration `declaration` of `program` as an
    /// elementwise-map kernel, generate its MSL, and compile it on this
    /// device. Lowering and admission reuse [`session::bind`] verbatim, then
    /// this backend additionally admits only what [`assert_metal_admits`]
    /// (in practice, every [`ScalarKind`] the CPU reference itself admits)
    /// has a Metal buffer lowering for.
    pub fn load_kernel(
        &mut self,
        program: &ResolvedProgram,
        declaration: &str,
        workgroup_size: u32,
    ) -> Result<MetalKernelArtifact, MetalRefusal> {
        if let Some(failure) = &self.selected {
            return Err(MetalRefusal::FailureAlreadySelected {
                failure: failure.clone(),
            });
        }
        let shape = KernelShape::ElementwiseMap { workgroup_size };
        let (ir, fingerprint) =
            session::bind(program, declaration, shape).map_err(MetalRefusal::from)?;
        self.build_artifact(declaration, shape, ir, fingerprint)
    }

    /// Bind the checked declaration `declaration` of `program` as a
    /// [`KernelShape::SequentialFold`] kernel (`fn(acc: T, element: U) -> T`),
    /// generate its single-thread, order-preserving MSL loop, and compile it
    /// on this device. Lowering and admission reuse [`session::bind`]
    /// verbatim, exactly as [`Self::load_kernel`] does for a map.
    pub fn load_fold_kernel(
        &mut self,
        program: &ResolvedProgram,
        declaration: &str,
    ) -> Result<MetalKernelArtifact, MetalRefusal> {
        if let Some(failure) = &self.selected {
            return Err(MetalRefusal::FailureAlreadySelected {
                failure: failure.clone(),
            });
        }
        let shape = KernelShape::SequentialFold;
        let (ir, fingerprint) =
            session::bind(program, declaration, shape).map_err(MetalRefusal::from)?;
        self.build_artifact(declaration, shape, ir, fingerprint)
    }

    /// Test-only bypass of [`Self::load_kernel`]/[`Self::load_fold_kernel`]'s
    /// admission path: builds an artifact directly from a caller-supplied
    /// `(shape, ir, fingerprint)` triple instead of deriving all three from
    /// `session::bind` against a real checked program. Exists solely so the
    /// negative-control test can compile and run a deliberately mutated
    /// kernel body on real hardware; see
    /// [`Self::dispatch_map_unchecked_for_test`] for why the ordinary
    /// dispatch path cannot be used for that.
    #[cfg(test)]
    pub(crate) fn load_kernel_from_ir_for_test(
        &mut self,
        declaration: &str,
        shape: KernelShape,
        ir: KernelIr,
        fingerprint: String,
    ) -> Result<MetalKernelArtifact, MetalRefusal> {
        self.build_artifact(declaration, shape, ir, fingerprint)
    }

    /// Refuse a stale artifact (`SPX-GC015`): re-derives the fingerprint,
    /// the MSL digest, and the checked binding independently, exactly
    /// mirroring the CPU reference's own `check_artifact`.
    fn check_artifact(
        &self,
        program: &ResolvedProgram,
        artifact: &MetalKernelArtifact,
    ) -> Result<(), MetalRefusal> {
        if artifact.session != self.id {
            return Err(ComputeRefusal::StaleHandle {
                detail: format!(
                    "kernel `{}` was loaded into another session",
                    artifact.declaration
                ),
            }
            .into());
        }
        let shape = artifact.shape;
        let recorded = kernel_ir::fingerprint(&artifact.declaration, &shape.encode(), &artifact.ir);
        if recorded != artifact.fingerprint {
            return Err(ComputeRefusal::StaleHandle {
                detail: format!(
                    "kernel `{}` no longer matches its recorded fingerprint",
                    artifact.declaration
                ),
            }
            .into());
        }
        let regenerated = match shape {
            KernelShape::ElementwiseMap { .. } => {
                msl::generate(&artifact.declaration, &artifact.ir)
            }
            KernelShape::SequentialFold => msl::generate_fold(&artifact.declaration, &artifact.ir),
        }
        .map_err(|detail| MetalRefusal::from(ComputeRefusal::StaleHandle { detail }))?;
        if regenerated.sha256 != artifact.msl_sha256 {
            return Err(ComputeRefusal::StaleHandle {
                detail: format!(
                    "kernel `{}` no longer matches its recorded MSL digest",
                    artifact.declaration
                ),
            }
            .into());
        }
        match session::bind(program, &artifact.declaration, shape) {
            Ok((_, current)) if current == artifact.fingerprint => Ok(()),
            Ok(_) => Err(ComputeRefusal::StaleHandle {
                detail: format!(
                    "kernel `{}` was bound to a different checked body",
                    artifact.declaration
                ),
            }
            .into()),
            Err(refusal) => Err(ComputeRefusal::StaleHandle {
                detail: format!(
                    "kernel `{}` no longer binds to the checked program: {}",
                    artifact.declaration,
                    refusal.code()
                ),
            }
            .into()),
        }
    }

    /// `DeviceDispatch`: compile-bound, synchronous (`waitUntilCompleted`).
    /// Every invocation runs as an independent GPU thread guarded by
    /// `if (gid >= n) return`; a failing invocation writes its checked
    /// status and returns before writing any output. The lowest failing
    /// invocation ordinal is selected host-side by scanning the status
    /// buffer, matching the CPU reference's left-to-right selection.
    pub fn dispatch_map(
        &mut self,
        program: &ResolvedProgram,
        artifact: &MetalKernelArtifact,
        inputs: &[MetalBufferHandle],
        output: MetalBufferHandle,
    ) -> Result<MetalDispatchOutcome, MetalRefusal> {
        if !matches!(artifact.shape, KernelShape::ElementwiseMap { .. }) {
            return Err(MetalRefusal::from(ComputeRefusal::KernelSelection {
                detail: "a fold artifact cannot be dispatched as a map".to_owned(),
            }));
        }
        self.admit(DeviceEffect::DeviceDispatch)?;
        self.check_artifact(program, artifact)?;
        self.execute_map(artifact, inputs, output)
    }

    /// Test-only bypass of [`Self::dispatch_map`]'s `check_artifact` step:
    /// dispatches `artifact` exactly as encoded, without re-deriving
    /// whether it still matches any real checked program. The
    /// negative-control test needs this because its mutated artifact is
    /// *supposed* to fail that check (that failure is independently
    /// asserted); this method lets the mutant actually run on the GPU so
    /// its output can be compared to the interpreter reference instead.
    #[cfg(test)]
    pub(crate) fn dispatch_map_unchecked_for_test(
        &mut self,
        artifact: &MetalKernelArtifact,
        inputs: &[MetalBufferHandle],
        output: MetalBufferHandle,
    ) -> Result<MetalDispatchOutcome, MetalRefusal> {
        self.admit(DeviceEffect::DeviceDispatch)?;
        self.execute_map(artifact, inputs, output)
    }

    fn execute_map(
        &mut self,
        artifact: &MetalKernelArtifact,
        inputs: &[MetalBufferHandle],
        output: MetalBufferHandle,
    ) -> Result<MetalDispatchOutcome, MetalRefusal> {
        // Both public callers (`dispatch_map`, and the `#[cfg(test)]`
        // `dispatch_map_unchecked_for_test` bypass) only ever reach this
        // private method with a map-shaped artifact: `dispatch_map` refuses
        // any other shape before calling it, and the test bypass exists
        // solely to skip `check_artifact`, never the shape itself.
        let KernelShape::ElementwiseMap { workgroup_size } = artifact.shape else {
            return Err(ComputeRefusal::KernelSelection {
                detail: "a fold artifact cannot be dispatched as a map".to_owned(),
            }
            .into());
        };
        if inputs.len() != artifact.ir.params.len() {
            return Err(ComputeRefusal::KernelSelection {
                detail: format!(
                    "the kernel takes {} input buffers, {} were bound",
                    artifact.ir.params.len(),
                    inputs.len()
                ),
            }
            .into());
        }
        let (output_kind, len, output_buffer) = self.live(output)?;
        if output_kind != artifact.ir.result {
            return Err(ComputeRefusal::ElementTypeMismatch {
                detail: format!(
                    "buffer {} holds {output_kind:?}, the kernel binds {:?}",
                    output.index, artifact.ir.result
                ),
            }
            .into());
        }
        let output_buffer = output_buffer.clone();
        let mut input_buffers = Vec::with_capacity(inputs.len());
        for (handle, kind) in inputs.iter().zip(&artifact.ir.params) {
            let (input_kind, input_len, buffer) = self.live(*handle)?;
            if input_kind != *kind {
                return Err(ComputeRefusal::ElementTypeMismatch {
                    detail: format!(
                        "buffer {} holds {input_kind:?}, the kernel binds {kind:?}",
                        handle.index
                    ),
                }
                .into());
            }
            if input_len != len {
                return Err(ComputeRefusal::OutOfBounds {
                    detail: format!(
                        "input buffer {} holds {input_len} elements, the output holds {len}",
                        handle.index
                    ),
                }
                .into());
            }
            input_buffers.push(buffer.clone());
        }

        // Classify the real dispatch shape and the real aliasing claim
        // through the exact same code the CPU reference calls
        // (`session::classify_map_dispatch`), before any Metal allocation or
        // dispatch below: an aliased input/output buffer or a grid outside
        // `MAX_GRID_DIM` must be refused here exactly as it would be on the
        // CPU reference, never merely accepted because this backend never
        // asked.
        let aliased = inputs.contains(&output);
        session::classify_map_dispatch(&artifact.ir, workgroup_size, len, aliased)
            .map_err(MetalRefusal::from)?;

        let status_byte_len = len * 4;
        let status_buffer = self
            .device
            .newBufferWithLength_options(status_byte_len, MTLResourceOptions::StorageModeShared)
            .ok_or_else(|| {
                MetalRefusal::from(ComputeRefusal::OutOfBounds {
                    detail: "the Metal device refused the status buffer".to_owned(),
                })
            })?;
        // SAFETY: a freshly allocated shared buffer's `contents()` is
        // `status_byte_len` host-visible bytes.
        unsafe {
            core::ptr::write_bytes(
                status_buffer.contents().as_ptr().cast::<u8>(),
                0u8,
                status_byte_len,
            );
        }

        let element_count = u32::try_from(len).map_err(|_| {
            MetalRefusal::from(ComputeRefusal::OutOfBounds {
                detail: "too many elements for one dispatch".to_owned(),
            })
        })?;

        let command_buffer = self.queue.commandBuffer().ok_or_else(|| {
            MetalRefusal::from(ComputeRefusal::KernelSelection {
                detail: "the Metal device refused a command buffer".to_owned(),
            })
        })?;
        let encoder = command_buffer.computeCommandEncoder().ok_or_else(|| {
            MetalRefusal::from(ComputeRefusal::KernelSelection {
                detail: "the Metal device refused a compute encoder".to_owned(),
            })
        })?;
        encoder.setComputePipelineState(&artifact.pipeline);
        for (index, buffer) in input_buffers.iter().enumerate() {
            // SAFETY: `buffer` is kept alive by `input_buffers` until after
            // `waitUntilCompleted`; `index` is within the kernel's declared
            // buffer-argument count.
            unsafe {
                encoder.setBuffer_offset_atIndex(Some(&**buffer), 0, index);
            }
        }
        let output_index = input_buffers.len();
        // SAFETY: see above; `output_buffer`/`status_buffer` are kept alive
        // in this stack frame past `waitUntilCompleted`.
        unsafe {
            encoder.setBuffer_offset_atIndex(Some(&*output_buffer), 0, output_index);
            encoder.setBuffer_offset_atIndex(Some(&*status_buffer), 0, output_index + 1);
        }
        let mut element_count_value = element_count;
        let count_ptr = NonNull::from(&mut element_count_value).cast::<c_void>();
        // SAFETY: `count_ptr` is a valid, live pointer to 4 bytes for the
        // duration of this call (Metal copies the bytes immediately).
        unsafe {
            encoder.setBytes_length_atIndex(count_ptr, 4, output_index + 2);
        }

        let max_threads = artifact.pipeline.maxTotalThreadsPerThreadgroup().max(1);
        let threadgroup_width = (workgroup_size as usize).clamp(1, max_threads);
        encoder.dispatchThreads_threadsPerThreadgroup(
            MTLSize {
                width: len,
                height: 1,
                depth: 1,
            },
            MTLSize {
                width: threadgroup_width,
                height: 1,
                depth: 1,
            },
        );
        encoder.endEncoding();
        command_buffer.commit();
        command_buffer.waitUntilCompleted();

        if command_buffer.status() == MTLCommandBufferStatus::Error {
            let detail = command_buffer
                .error()
                .map(|error| error.localizedDescription().to_string())
                .unwrap_or_else(|| "no further detail".to_owned());
            eprintln!("Metal command buffer reported MTLCommandBufferStatusError: {detail}");
            let failure = MetalSessionFailure::DeviceLost;
            self.selected = Some(failure.clone());
            return Ok(MetalDispatchOutcome::Failed(failure));
        }

        // SAFETY: the command buffer completed; the status buffer's
        // `status_byte_len` bytes are host-visible and fully initialized
        // (zeroed above, then written to at most once per invocation by the
        // kernel itself). `read_unaligned` because `status_base.add(invocation
        // * 4)` is not guaranteed 4-byte aligned for every `invocation` on
        // every allocator, unlike a `u32` place.
        let status_base = status_buffer.contents().as_ptr() as *const u8;
        let mut first_failure = None;
        for invocation in 0..len {
            let code = unsafe {
                status_base
                    .add(invocation * 4)
                    .cast::<u32>()
                    .read_unaligned()
            };
            if code != 0 {
                first_failure = Some((invocation, code));
                break;
            }
        }

        match first_failure {
            None => Ok(MetalDispatchOutcome::Completed { invocations: len }),
            Some((invocation, code)) => {
                // `StaleHandle`, not a new refusal: no other `ComputeRefusal`
                // case fits a status the generated kernel was never emitted
                // to write, and this mirrors the CPU reference's own
                // precedent for the same class of problem (`RunOutcome::Guard`
                // in `cpu_reference::session::finish`, "the kernel reached an
                // impossible post-lowering state") — an internal invariant
                // violation in the compiled artifact or the host/device
                // synchronization around it, never an ordinary admission
                // refusal a checked program can trigger.
                let status = status_from_code(code).ok_or_else(|| {
                    MetalRefusal::from(ComputeRefusal::StaleHandle {
                        detail: format!("kernel wrote an unrecognized status code {code}"),
                    })
                })?;
                let failure = MetalSessionFailure::KernelStatus {
                    declaration: artifact.declaration.clone(),
                    invocation,
                    status,
                };
                self.selected = Some(failure.clone());
                Ok(MetalDispatchOutcome::Failed(failure))
            }
        }
    }

    /// `DeviceDispatch` of a [`KernelShape::SequentialFold`] artifact: seeds
    /// the one-element `output` accumulator buffer with `initial` (see
    /// [`Self::write_direct`]), then runs the single-thread, sequential,
    /// order-preserving generated loop over `input`'s elements. Mirrors
    /// `cpu_reference::session::CpuReferenceSession::dispatch_fold` exactly:
    /// the same `session::classify_fold_dispatch` call, the same sticky
    /// failure selection, the same left-to-right invocation ordinal for
    /// whichever element first selects a checked status.
    pub fn dispatch_fold(
        &mut self,
        program: &ResolvedProgram,
        artifact: &MetalKernelArtifact,
        initial: Scalar,
        input: MetalBufferHandle,
        output: MetalBufferHandle,
    ) -> Result<MetalDispatchOutcome, MetalRefusal> {
        if artifact.shape != KernelShape::SequentialFold {
            return Err(ComputeRefusal::KernelSelection {
                detail: "a map artifact cannot be dispatched as a fold".to_owned(),
            }
            .into());
        }
        self.admit(DeviceEffect::DeviceDispatch)?;
        self.check_artifact(program, artifact)?;
        self.execute_fold(artifact, initial, input, output)
    }

    /// Test-only bypass of [`Self::dispatch_fold`]'s `check_artifact` step;
    /// see [`Self::dispatch_map_unchecked_for_test`] for why the
    /// negative-control test needs this.
    #[cfg(test)]
    pub(crate) fn dispatch_fold_unchecked_for_test(
        &mut self,
        artifact: &MetalKernelArtifact,
        initial: Scalar,
        input: MetalBufferHandle,
        output: MetalBufferHandle,
    ) -> Result<MetalDispatchOutcome, MetalRefusal> {
        self.admit(DeviceEffect::DeviceDispatch)?;
        self.execute_fold(artifact, initial, input, output)
    }

    fn execute_fold(
        &mut self,
        artifact: &MetalKernelArtifact,
        initial: Scalar,
        input: MetalBufferHandle,
        output: MetalBufferHandle,
    ) -> Result<MetalDispatchOutcome, MetalRefusal> {
        let accumulator_kind = artifact.ir.result;
        if initial.kind() != accumulator_kind {
            return Err(ComputeRefusal::ElementTypeMismatch {
                detail: format!(
                    "{:?} initial value for a {accumulator_kind:?} fold",
                    initial.kind()
                ),
            }
            .into());
        }
        let (output_kind, out_len, output_buffer) = self.live(output)?;
        if output_kind != accumulator_kind {
            return Err(ComputeRefusal::ElementTypeMismatch {
                detail: format!(
                    "buffer {} holds {output_kind:?}, the kernel binds {accumulator_kind:?}",
                    output.index
                ),
            }
            .into());
        }
        if out_len != 1 {
            return Err(ComputeRefusal::OutOfBounds {
                detail: format!(
                    "a fold publishes one element, output buffer {} holds {out_len}",
                    output.index
                ),
            }
            .into());
        }
        let output_buffer = output_buffer.clone();

        let element_kind = artifact.ir.params[1];
        let (input_kind, len, input_buffer) = self.live(input)?;
        if input_kind != element_kind {
            return Err(ComputeRefusal::ElementTypeMismatch {
                detail: format!(
                    "buffer {} holds {input_kind:?}, the kernel binds {element_kind:?}",
                    input.index
                ),
            }
            .into());
        }
        let input_buffer = input_buffer.clone();

        // Classify the real dispatch shape and the real aliasing claim
        // through the exact same code the CPU reference calls
        // (`session::classify_fold_dispatch`), before any Metal allocation
        // or dispatch below, exactly as `execute_map` does for a map.
        let aliased = input == output;
        session::classify_fold_dispatch(&artifact.ir, len, aliased).map_err(MetalRefusal::from)?;

        // Seeded after classification (never before an admission refusal),
        // exactly as `execute_map` allocates its device buffers only after
        // `classify_map_dispatch` admits the dispatch.
        self.write_direct(output, initial)?;

        let status_buffer = self
            .device
            .newBufferWithLength_options(4, MTLResourceOptions::StorageModeShared)
            .ok_or_else(|| {
                MetalRefusal::from(ComputeRefusal::OutOfBounds {
                    detail: "the Metal device refused the status buffer".to_owned(),
                })
            })?;
        let invocation_buffer = self
            .device
            .newBufferWithLength_options(4, MTLResourceOptions::StorageModeShared)
            .ok_or_else(|| {
                MetalRefusal::from(ComputeRefusal::OutOfBounds {
                    detail: "the Metal device refused the invocation buffer".to_owned(),
                })
            })?;
        // SAFETY: both are freshly allocated shared buffers whose
        // `contents()` is 4 host-visible bytes.
        unsafe {
            core::ptr::write_bytes(status_buffer.contents().as_ptr().cast::<u8>(), 0u8, 4);
            core::ptr::write_bytes(invocation_buffer.contents().as_ptr().cast::<u8>(), 0u8, 4);
        }

        let element_count = u32::try_from(len).map_err(|_| {
            MetalRefusal::from(ComputeRefusal::OutOfBounds {
                detail: "too many elements for one fold dispatch".to_owned(),
            })
        })?;

        let command_buffer = self.queue.commandBuffer().ok_or_else(|| {
            MetalRefusal::from(ComputeRefusal::KernelSelection {
                detail: "the Metal device refused a command buffer".to_owned(),
            })
        })?;
        let encoder = command_buffer.computeCommandEncoder().ok_or_else(|| {
            MetalRefusal::from(ComputeRefusal::KernelSelection {
                detail: "the Metal device refused a compute encoder".to_owned(),
            })
        })?;
        encoder.setComputePipelineState(&artifact.pipeline);
        // SAFETY: every named buffer is kept alive (by a local binding or
        // `input_buffer`/`output_buffer`/`status_buffer`/`invocation_buffer`
        // themselves) in this stack frame past `waitUntilCompleted`.
        unsafe {
            encoder.setBuffer_offset_atIndex(Some(&*input_buffer), 0, 0);
            encoder.setBuffer_offset_atIndex(Some(&*output_buffer), 0, 1);
            encoder.setBuffer_offset_atIndex(Some(&*status_buffer), 0, 2);
            encoder.setBuffer_offset_atIndex(Some(&*invocation_buffer), 0, 3);
        }
        let mut element_count_value = element_count;
        let count_ptr = NonNull::from(&mut element_count_value).cast::<c_void>();
        // SAFETY: `count_ptr` is a valid, live pointer to 4 bytes for the
        // duration of this call (Metal copies the bytes immediately).
        unsafe {
            encoder.setBytes_length_atIndex(count_ptr, 4, 4);
        }

        // A fold is one single-threaded invocation that loops over every
        // element itself (see `msl::generate_fold`), never one GPU thread per
        // element: dispatching more than one thread here would run the whole
        // sequential loop redundantly in parallel, not divide it.
        encoder.dispatchThreads_threadsPerThreadgroup(
            MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            },
            MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            },
        );
        encoder.endEncoding();
        command_buffer.commit();
        command_buffer.waitUntilCompleted();

        if command_buffer.status() == MTLCommandBufferStatus::Error {
            let detail = command_buffer
                .error()
                .map(|error| error.localizedDescription().to_string())
                .unwrap_or_else(|| "no further detail".to_owned());
            eprintln!("Metal command buffer reported MTLCommandBufferStatusError: {detail}");
            let failure = MetalSessionFailure::DeviceLost;
            self.selected = Some(failure.clone());
            return Ok(MetalDispatchOutcome::Failed(failure));
        }

        // SAFETY: the command buffer completed; both buffers' 4 bytes are
        // host-visible and fully initialized (zeroed above, then written to
        // at most once by the kernel itself).
        let status_code = unsafe {
            (status_buffer.contents().as_ptr() as *const u8)
                .cast::<u32>()
                .read_unaligned()
        };
        if status_code == 0 {
            return Ok(MetalDispatchOutcome::Completed { invocations: len });
        }
        let invocation = unsafe {
            (invocation_buffer.contents().as_ptr() as *const u8)
                .cast::<u32>()
                .read_unaligned()
        } as usize;
        let status = status_from_code(status_code).ok_or_else(|| {
            MetalRefusal::from(ComputeRefusal::StaleHandle {
                detail: format!("kernel wrote an unrecognized status code {status_code}"),
            })
        })?;
        let failure = MetalSessionFailure::KernelStatus {
            declaration: artifact.declaration.clone(),
            invocation,
            status,
        };
        self.selected = Some(failure.clone());
        Ok(MetalDispatchOutcome::Failed(failure))
    }

    /// Consume the session: release every live buffer in reverse allocation
    /// order and return the complete facts.
    pub fn settle(mut self) -> MetalSettlement {
        for index in (0..self.buffers.len()).rev() {
            if let MetalBufferState::Live { len, .. } = &self.buffers[index] {
                self.live_elements -= *len;
                self.buffers[index] = MetalBufferState::Released;
                self.releases.push(MetalReleaseEvent {
                    buffer: index as u32,
                    cause: MetalReleaseCause::Settlement,
                });
            }
        }
        MetalSettlement {
            selected: self.selected.clone(),
            releases: std::mem::take(&mut self.releases),
        }
    }
}
