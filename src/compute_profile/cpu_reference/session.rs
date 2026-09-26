//! The CPU-reference device session: the typed owned-device-buffer
//! lifecycle, kernel binding, dispatch, and settlement.
//!
//! # Lifecycle
//!
//! A session is opened only with an explicit [`ComputeCapability`] naming
//! the device and the device effects it grants. Buffers move through
//! allocate (`DeviceAlloc`, zero-filled) -> upload (`DeviceCopyIn`) ->
//! dispatch (`DeviceDispatch`, synchronous) -> download (`DeviceCopyOut`) ->
//! release (`DeviceRelease`). Every refused operation leaves the session
//! unchanged and journals nothing.
//!
//! # Failure selection
//!
//! The first dispatch that ends in a kernel status, a cancellation, or a
//! device loss selects that [`SessionFailure`]. Selection is sticky: every
//! later allocation, upload, dispatch, or download refuses
//! (`SPX-GC019`), so no result is published after a failure; release and
//! settlement stay admitted and cannot replace the selected failure.
//!
//! # Settlement
//!
//! [`CpuReferenceSession::settle`] consumes the session and releases every
//! still-live buffer in reverse allocation order. Together with explicit
//! releases, every allocated buffer appears in [`Settlement::releases`]
//! exactly once.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::cleanup_plan::StatusCase;
use crate::compute_profile::boundary_profile::MAX_BUFFER_ELEMENTS;
use crate::compute_profile::classifier::{
    classify, AddressSpace, AliasingClaim, DeviceEffect, GridShape, KernelCandidate, KernelOp,
    KernelParam, KernelType, ParamMode, ReductionOrder, Refusal, ScalarType,
};
use crate::hir::{self, IdentityOrigin, OwnershipMode, ResolvedFunction, ResolvedProgram};

use super::kernel_ir::{self, EvalStop, KernelIr, LowerStop, Scalar, ScalarKind};
use super::{ComputeRefusal, MAX_SESSION_LIVE_ELEMENTS};

/// The device a capability selects. Only the CPU reference exists; there is
/// no implicit or ambient device selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputeDevice {
    CpuReference,
}

/// The explicit authority to open one compute session. It names its device
/// and the closed set of device effects it grants, and is consumed by
/// [`CpuReferenceSession::open`].
#[derive(Debug, Eq, PartialEq)]
pub struct ComputeCapability {
    device: ComputeDevice,
    granted: Vec<DeviceEffect>,
}

impl ComputeCapability {
    /// Grant exactly `effects` on the CPU reference device. A grant that
    /// admits `DeviceAlloc` must also admit `DeviceRelease`, so a buffer
    /// can never be allocated without the authority to clean it up.
    pub fn cpu_reference(effects: &[DeviceEffect]) -> Result<Self, ComputeRefusal> {
        if effects.contains(&DeviceEffect::DeviceAlloc)
            && !effects.contains(&DeviceEffect::DeviceRelease)
        {
            return Err(ComputeRefusal::EffectNotGranted {
                effect: DeviceEffect::DeviceRelease,
            });
        }
        let mut granted = effects.to_vec();
        granted.dedup();
        Ok(Self {
            device: ComputeDevice::CpuReference,
            granted,
        })
    }

    /// Grant the complete closed device-effect vocabulary.
    pub fn cpu_reference_all() -> Self {
        Self {
            device: ComputeDevice::CpuReference,
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

/// One buffer handle. A plain value: misuse (use after release, double
/// release, a handle from another session) is refused by the session state
/// machine, not by the host type system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferHandle {
    session: u64,
    index: u32,
}

impl BufferHandle {
    /// The allocation ordinal within its session.
    pub fn index(self) -> u32 {
        self.index
    }
}

/// The two executable kernel shapes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelShape {
    /// `out[i] = f(in_0[i], ..., in_k[i])`, dispatched over one-dimensional
    /// workgroups of `workgroup_size` invocations.
    ElementwiseMap { workgroup_size: u32 },
    /// `acc = f(acc, in[i])` for ascending `i`, from an explicit initial
    /// value, as one sequential invocation.
    SequentialFold,
}

impl KernelShape {
    fn encode(self) -> Vec<u8> {
        match self {
            Self::ElementwiseMap { workgroup_size } => {
                let mut bytes = vec![1];
                bytes.extend_from_slice(&workgroup_size.to_le_bytes());
                bytes
            }
            Self::SequentialFold => vec![2],
        }
    }
}

/// A kernel bound to one checked declaration: its persistent identity, the
/// fingerprint of its lowered body, and the session it was loaded into.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelArtifact {
    session: u64,
    declaration: String,
    shape: KernelShape,
    fingerprint: String,
    ir: KernelIr,
}

impl KernelArtifact {
    pub fn declaration(&self) -> &str {
        &self.declaration
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn shape(&self) -> KernelShape {
        self.shape
    }

    #[cfg(test)]
    pub(crate) fn ir(&self) -> &KernelIr {
        &self.ir
    }

    #[cfg(test)]
    pub(crate) fn ir_mut(&mut self) -> &mut KernelIr {
        &mut self.ir
    }
}

/// Deterministic cancellation and device-loss injection points for the CPU
/// reference. Each is observed immediately before the named invocation
/// ordinal starts. These model the outcomes; they are not hardware evidence.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DispatchControl {
    pub cancel_before_invocation: Option<usize>,
    pub device_loss_before_invocation: Option<usize>,
}

/// The selected, sticky failure of one session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionFailure {
    /// The lowest-ordinal failing invocation selected this checked status.
    KernelStatus {
        declaration: String,
        invocation: usize,
        status: StatusCase,
    },
    Cancelled {
        completed_invocations: usize,
    },
    DeviceLost {
        completed_invocations: usize,
    },
}

/// The executed result of one admitted dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchOutcome {
    /// Every invocation completed and the output buffer was published.
    Completed { invocations: usize },
    /// The dispatch selected this session failure; the output buffer was not
    /// written.
    Failed(SessionFailure),
}

/// One journaled device effect, in execution order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectEvent {
    pub effect: DeviceEffect,
    /// The allocation ordinal of the buffer the effect names; for a dispatch,
    /// its output buffer.
    pub buffer: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseCause {
    Explicit,
    Settlement,
}

/// One buffer release. `device_lost` marks a host-side settlement after
/// device loss, where no device memory remains to free.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseEvent {
    pub buffer: u32,
    pub cause: ReleaseCause,
    pub device_lost: bool,
}

/// The consumed session's complete, deterministic facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settlement {
    pub selected: Option<SessionFailure>,
    pub effects: Vec<EffectEvent>,
    pub releases: Vec<ReleaseEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum BufferState {
    Live { kind: ScalarKind, data: Vec<Scalar> },
    Released,
}

/// One CPU-reference compute session.
#[derive(Debug)]
pub struct CpuReferenceSession {
    id: u64,
    granted: Vec<DeviceEffect>,
    buffers: Vec<BufferState>,
    live_elements: usize,
    selected: Option<SessionFailure>,
    device_lost: bool,
    effects: Vec<EffectEvent>,
    releases: Vec<ReleaseEvent>,
}

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

const INPUT_NAMES: [&str; 8] = ["in0", "in1", "in2", "in3", "in4", "in5", "in6", "in7"];

fn profile(refusal: Refusal, detail: impl Into<String>) -> ComputeRefusal {
    ComputeRefusal::Profile {
        refusal,
        detail: detail.into(),
    }
}

impl CpuReferenceSession {
    pub fn open(capability: ComputeCapability) -> Self {
        let ComputeCapability { device, granted } = capability;
        match device {
            ComputeDevice::CpuReference => {}
        }
        Self {
            id: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
            granted,
            buffers: Vec::new(),
            live_elements: 0,
            selected: None,
            device_lost: false,
            effects: Vec::new(),
            releases: Vec::new(),
        }
    }

    fn admit(&self, effect: DeviceEffect) -> Result<(), ComputeRefusal> {
        if let Some(failure) = &self.selected {
            return Err(ComputeRefusal::FailureAlreadySelected {
                failure: failure.clone(),
            });
        }
        if !self.granted.contains(&effect) {
            return Err(ComputeRefusal::EffectNotGranted { effect });
        }
        Ok(())
    }

    fn live(&self, handle: BufferHandle) -> Result<(ScalarKind, &[Scalar]), ComputeRefusal> {
        if handle.session != self.id {
            return Err(ComputeRefusal::StaleHandle {
                detail: format!("buffer {} belongs to another session", handle.index),
            });
        }
        match self.buffers.get(handle.index as usize) {
            Some(BufferState::Live { kind, data }) => Ok((*kind, data)),
            Some(BufferState::Released) => Err(ComputeRefusal::BufferReleased {
                buffer: handle.index,
            }),
            None => Err(ComputeRefusal::StaleHandle {
                detail: format!("buffer {} was never allocated here", handle.index),
            }),
        }
    }

    fn data_mut(&mut self, handle: BufferHandle) -> &mut Vec<Scalar> {
        match &mut self.buffers[handle.index as usize] {
            BufferState::Live { data, .. } => data,
            BufferState::Released => unreachable!("checked live by the caller"),
        }
    }

    /// `DeviceAlloc`: a zero-filled buffer of `len` elements of `kind`.
    pub fn alloc(&mut self, kind: ScalarKind, len: usize) -> Result<BufferHandle, ComputeRefusal> {
        self.admit(DeviceEffect::DeviceAlloc)?;
        if len == 0 {
            return Err(ComputeRefusal::OutOfBounds {
                detail: "a buffer holds at least one element".to_owned(),
            });
        }
        if len > MAX_BUFFER_ELEMENTS
            || self
                .live_elements
                .checked_add(len)
                .is_none_or(|total| total > MAX_SESSION_LIVE_ELEMENTS)
        {
            return Err(profile(
                Refusal::CapacityBoundExceeded {
                    param: "allocation",
                },
                format!("{len} elements requested with {} live", self.live_elements),
            ));
        }
        let index = u32::try_from(self.buffers.len()).map_err(|_| ComputeRefusal::OutOfBounds {
            detail: "too many allocations".to_owned(),
        })?;
        self.buffers.push(BufferState::Live {
            kind,
            data: vec![kind.zero(); len],
        });
        self.live_elements += len;
        self.effects.push(EffectEvent {
            effect: DeviceEffect::DeviceAlloc,
            buffer: index,
        });
        Ok(BufferHandle {
            session: self.id,
            index,
        })
    }

    /// `DeviceCopyIn`: copy `values` into `[offset, offset + values.len())`.
    pub fn upload(
        &mut self,
        handle: BufferHandle,
        offset: usize,
        values: &[Scalar],
    ) -> Result<(), ComputeRefusal> {
        self.admit(DeviceEffect::DeviceCopyIn)?;
        let (kind, data) = self.live(handle)?;
        if let Some(value) = values.iter().find(|value| value.kind() != kind) {
            return Err(ComputeRefusal::ElementTypeMismatch {
                detail: format!(
                    "{:?} value uploaded into a {kind:?} buffer {}",
                    value.kind(),
                    handle.index
                ),
            });
        }
        let end = checked_end(offset, values.len(), data.len(), handle)?;
        self.data_mut(handle)[offset..end].copy_from_slice(values);
        self.effects.push(EffectEvent {
            effect: DeviceEffect::DeviceCopyIn,
            buffer: handle.index,
        });
        Ok(())
    }

    /// `DeviceCopyOut`: copy `[offset, offset + len)` back to the host.
    pub fn download(
        &mut self,
        handle: BufferHandle,
        offset: usize,
        len: usize,
    ) -> Result<Vec<Scalar>, ComputeRefusal> {
        self.admit(DeviceEffect::DeviceCopyOut)?;
        let (_, data) = self.live(handle)?;
        let end = checked_end(offset, len, data.len(), handle)?;
        let values = data[offset..end].to_vec();
        self.effects.push(EffectEvent {
            effect: DeviceEffect::DeviceCopyOut,
            buffer: handle.index,
        });
        Ok(values)
    }

    /// `DeviceRelease`: consume the buffer exactly once. Admitted after a
    /// selected failure and after device loss.
    pub fn release(&mut self, handle: BufferHandle) -> Result<(), ComputeRefusal> {
        let (_, data) = self.live(handle)?;
        let len = data.len();
        self.buffers[handle.index as usize] = BufferState::Released;
        self.live_elements -= len;
        self.record_release(handle.index, ReleaseCause::Explicit);
        Ok(())
    }

    fn record_release(&mut self, buffer: u32, cause: ReleaseCause) {
        if !self.device_lost {
            self.effects.push(EffectEvent {
                effect: DeviceEffect::DeviceRelease,
                buffer,
            });
        }
        self.releases.push(ReleaseEvent {
            buffer,
            cause,
            device_lost: self.device_lost,
        });
    }

    /// Bind the checked declaration `declaration` of `program` as a kernel
    /// of `shape`. Lowering reads only the validated HIR.
    pub fn load_kernel(
        &mut self,
        program: &ResolvedProgram,
        declaration: &str,
        shape: KernelShape,
    ) -> Result<KernelArtifact, ComputeRefusal> {
        if let Some(failure) = &self.selected {
            return Err(ComputeRefusal::FailureAlreadySelected {
                failure: failure.clone(),
            });
        }
        let (ir, fingerprint) = bind(program, declaration, shape)?;
        Ok(KernelArtifact {
            session: self.id,
            declaration: declaration.to_owned(),
            shape,
            fingerprint,
            ir,
        })
    }

    /// `DeviceDispatch` of an [`KernelShape::ElementwiseMap`] artifact.
    pub fn dispatch_map(
        &mut self,
        program: &ResolvedProgram,
        artifact: &KernelArtifact,
        inputs: &[BufferHandle],
        output: BufferHandle,
        control: DispatchControl,
    ) -> Result<DispatchOutcome, ComputeRefusal> {
        let KernelShape::ElementwiseMap { workgroup_size } = artifact.shape else {
            return Err(ComputeRefusal::KernelSelection {
                detail: "a fold artifact cannot be dispatched as a map".to_owned(),
            });
        };
        self.admit(DeviceEffect::DeviceDispatch)?;
        self.check_artifact(program, artifact)?;
        if inputs.len() != artifact.ir.params.len() {
            return Err(ComputeRefusal::KernelSelection {
                detail: format!(
                    "the kernel takes {} input buffers, {} were bound",
                    artifact.ir.params.len(),
                    inputs.len()
                ),
            });
        }
        let (output_kind, output_data) = self.live(output)?;
        let len = output_data.len();
        expect_kind(output_kind, artifact.ir.result, output)?;
        let mut columns = Vec::with_capacity(inputs.len());
        for (input, kind) in inputs.iter().zip(&artifact.ir.params) {
            let (input_kind, data) = self.live(*input)?;
            expect_kind(input_kind, *kind, *input)?;
            if data.len() != len {
                return Err(ComputeRefusal::OutOfBounds {
                    detail: format!(
                        "input buffer {} holds {} elements, the output holds {len}",
                        input.index,
                        data.len()
                    ),
                });
            }
            columns.push(data);
        }
        let grid = u32::try_from(len.div_ceil(workgroup_size.max(1) as usize)).unwrap_or(u32::MAX);
        let mut params: Vec<(ScalarKind, ParamMode, usize)> = artifact
            .ir
            .params
            .iter()
            .map(|kind| (*kind, ParamMode::ReadOnlyView, len))
            .collect();
        params.push((artifact.ir.result, ParamMode::ReadWriteView, len));
        let aliasing = if inputs.contains(&output) {
            AliasingClaim::MayOverlap
        } else {
            AliasingClaim::Disjoint
        };
        let candidate = candidate(
            &params,
            GridShape {
                workgroup_size: [workgroup_size, 1, 1],
                grid_size: [grid, 1, 1],
            },
            aliasing,
            map_ops(&artifact.ir),
        );
        classify(&candidate).map_err(|refusal| profile(refusal, "dispatch shape"))?;

        let outcome = run_map(&artifact.ir, &columns, len, control);
        self.finish(artifact, output, len, outcome)
    }

    /// `DeviceDispatch` of a [`KernelShape::SequentialFold`] artifact.
    pub fn dispatch_fold(
        &mut self,
        program: &ResolvedProgram,
        artifact: &KernelArtifact,
        initial: Scalar,
        input: BufferHandle,
        output: BufferHandle,
        control: DispatchControl,
    ) -> Result<DispatchOutcome, ComputeRefusal> {
        if artifact.shape != KernelShape::SequentialFold {
            return Err(ComputeRefusal::KernelSelection {
                detail: "a map artifact cannot be dispatched as a fold".to_owned(),
            });
        }
        self.admit(DeviceEffect::DeviceDispatch)?;
        self.check_artifact(program, artifact)?;
        let accumulator = artifact.ir.result;
        if initial.kind() != accumulator {
            return Err(ComputeRefusal::ElementTypeMismatch {
                detail: format!(
                    "{:?} initial value for a {accumulator:?} fold",
                    initial.kind()
                ),
            });
        }
        let (output_kind, output_data) = self.live(output)?;
        expect_kind(output_kind, accumulator, output)?;
        if output_data.len() != 1 {
            return Err(ComputeRefusal::OutOfBounds {
                detail: format!(
                    "a fold publishes one element, output buffer {} holds {}",
                    output.index,
                    output_data.len()
                ),
            });
        }
        let (input_kind, data) = self.live(input)?;
        expect_kind(input_kind, artifact.ir.params[1], input)?;
        let len = data.len();
        let aliasing = if input == output {
            AliasingClaim::MayOverlap
        } else {
            AliasingClaim::Disjoint
        };
        let candidate = candidate(
            &[
                (artifact.ir.params[1], ParamMode::ReadOnlyView, len),
                (accumulator, ParamMode::ReadWriteView, 1),
            ],
            GridShape {
                workgroup_size: [1, 1, 1],
                grid_size: [1, 1, 1],
            },
            aliasing,
            fold_ops(&artifact.ir),
        );
        classify(&candidate).map_err(|refusal| profile(refusal, "dispatch shape"))?;

        let outcome = run_fold(&artifact.ir, initial, data, control);
        self.finish(artifact, output, len, outcome)
    }

    fn check_artifact(
        &self,
        program: &ResolvedProgram,
        artifact: &KernelArtifact,
    ) -> Result<(), ComputeRefusal> {
        if artifact.session != self.id {
            return Err(ComputeRefusal::StaleHandle {
                detail: format!(
                    "kernel `{}` was loaded into another session",
                    artifact.declaration
                ),
            });
        }
        let recorded = kernel_ir::fingerprint(
            &artifact.declaration,
            &artifact.shape.encode(),
            &artifact.ir,
        );
        if recorded != artifact.fingerprint {
            return Err(ComputeRefusal::StaleHandle {
                detail: format!(
                    "kernel `{}` no longer matches its recorded fingerprint",
                    artifact.declaration
                ),
            });
        }
        match bind(program, &artifact.declaration, artifact.shape) {
            Ok((_, current)) if current == artifact.fingerprint => Ok(()),
            Ok(_) => Err(ComputeRefusal::StaleHandle {
                detail: format!(
                    "kernel `{}` was bound to a different checked body",
                    artifact.declaration
                ),
            }),
            Err(refusal) => Err(ComputeRefusal::StaleHandle {
                detail: format!(
                    "kernel `{}` no longer binds to the checked program: {}",
                    artifact.declaration,
                    refusal.code()
                ),
            }),
        }
    }

    fn finish(
        &mut self,
        artifact: &KernelArtifact,
        output: BufferHandle,
        invocations: usize,
        outcome: RunOutcome,
    ) -> Result<DispatchOutcome, ComputeRefusal> {
        let failure = match outcome {
            RunOutcome::Completed(values) => {
                self.data_mut(output).copy_from_slice(&values);
                self.effects.push(EffectEvent {
                    effect: DeviceEffect::DeviceDispatch,
                    buffer: output.index,
                });
                return Ok(DispatchOutcome::Completed { invocations });
            }
            RunOutcome::Status { invocation, status } => SessionFailure::KernelStatus {
                declaration: artifact.declaration.clone(),
                invocation,
                status,
            },
            RunOutcome::Cancelled(completed_invocations) => SessionFailure::Cancelled {
                completed_invocations,
            },
            RunOutcome::DeviceLost(completed_invocations) => {
                self.device_lost = true;
                SessionFailure::DeviceLost {
                    completed_invocations,
                }
            }
            RunOutcome::Guard => {
                return Err(ComputeRefusal::StaleHandle {
                    detail: "the kernel reached an impossible post-lowering state".to_owned(),
                })
            }
        };
        self.effects.push(EffectEvent {
            effect: DeviceEffect::DeviceDispatch,
            buffer: output.index,
        });
        self.selected = Some(failure.clone());
        Ok(DispatchOutcome::Failed(failure))
    }

    /// Consume the session: release every live buffer in reverse allocation
    /// order and return the complete facts.
    pub fn settle(mut self) -> Settlement {
        for index in (0..self.buffers.len()).rev() {
            if let BufferState::Live { data, .. } = &self.buffers[index] {
                self.live_elements -= data.len();
                self.buffers[index] = BufferState::Released;
                self.record_release(index as u32, ReleaseCause::Settlement);
            }
        }
        Settlement {
            selected: self.selected,
            effects: self.effects,
            releases: self.releases,
        }
    }
}

fn checked_end(
    offset: usize,
    len: usize,
    capacity: usize,
    handle: BufferHandle,
) -> Result<usize, ComputeRefusal> {
    offset
        .checked_add(len)
        .filter(|end| *end <= capacity)
        .ok_or_else(|| ComputeRefusal::OutOfBounds {
            detail: format!(
                "range {offset}+{len} exceeds buffer {} of {capacity} elements",
                handle.index
            ),
        })
}

fn expect_kind(
    found: ScalarKind,
    expected: ScalarKind,
    handle: BufferHandle,
) -> Result<(), ComputeRefusal> {
    if found == expected {
        Ok(())
    } else {
        Err(ComputeRefusal::ElementTypeMismatch {
            detail: format!(
                "buffer {} holds {found:?}, the kernel binds {expected:?}",
                handle.index
            ),
        })
    }
}

fn candidate(
    params: &[(ScalarKind, ParamMode, usize)],
    grid: GridShape,
    aliasing: AliasingClaim,
    body: Vec<KernelOp>,
) -> KernelCandidate {
    KernelCandidate {
        name: "cpu_reference_kernel",
        effects: vec![DeviceEffect::DeviceDispatch],
        declared_other_effects: Vec::new(),
        // The session exists only through an explicit capability.
        explicit_device_capability: true,
        params: params
            .iter()
            .enumerate()
            .map(|(index, (kind, mode, len))| KernelParam {
                name: INPUT_NAMES.get(index).copied().unwrap_or("param"),
                ty: KernelType::Buffer {
                    elem: kind.classifier_scalar(),
                    space: AddressSpace::Global,
                    len: *len,
                },
                mode: *mode,
            })
            .collect(),
        grid,
        aliasing,
        body,
    }
}

fn guarded_access(store: bool) -> KernelOp {
    let (space, statically_in_bounds, dynamically_checked) = (AddressSpace::Global, false, true);
    if store {
        KernelOp::IndexedStore {
            space,
            statically_in_bounds,
            dynamically_checked,
        }
    } else {
        KernelOp::IndexedLoad {
            space,
            statically_in_bounds,
            dynamically_checked,
        }
    }
}

fn map_ops(ir: &KernelIr) -> Vec<KernelOp> {
    let mut ops: Vec<KernelOp> = ir.params.iter().map(|_| guarded_access(false)).collect();
    ops.extend(ir.ops.iter().cloned());
    ops.push(guarded_access(true));
    ops
}

fn fold_ops(ir: &KernelIr) -> Vec<KernelOp> {
    let mut ops = vec![guarded_access(false)];
    ops.extend(ir.ops.iter().cloned());
    ops.push(KernelOp::Reduction {
        elem: ir.result.classifier_scalar(),
        order: ReductionOrder::SequentialLeftToRight,
    });
    ops.push(guarded_access(true));
    ops
}

/// Select, admit, and lower one declaration, returning its IR and canonical
/// fingerprint. Refusals follow the classifier precedence.
fn bind(
    program: &ResolvedProgram,
    declaration: &str,
    shape: KernelShape,
) -> Result<(KernelIr, String), ComputeRefusal> {
    hir::validate(program).map_err(|diagnostic| ComputeRefusal::KernelSelection {
        detail: format!("the program failed HIR validation: {}", diagnostic.code),
    })?;
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == declaration)
        .ok_or_else(|| ComputeRefusal::KernelSelection {
            detail: format!("`{declaration}` is not a checked monomorphic function"),
        })?;
    let explicit = program
        .declarations
        .declaration(&function.id)
        .is_some_and(|entry| entry.identity_origin == IdentityOrigin::Explicit);
    if !explicit {
        return Err(ComputeRefusal::KernelSelection {
            detail: format!("`{declaration}` has no explicit persistent @id"),
        });
    }
    if let Some(effect) = function.effects.first() {
        return Err(profile(
            Refusal::EffectOutsideClosedVocabulary {
                token: "host-capability",
            },
            format!("`{declaration}` declares `uses {{ {effect} }}`"),
        ));
    }
    // Parameter count, then type, then mode: the classifier's precedence.
    let buffer_count = match shape {
        KernelShape::ElementwiseMap { .. } => function.params.len() + 1,
        KernelShape::SequentialFold => 2,
    };
    if buffer_count > crate::compute_profile::boundary_profile::MAX_KERNEL_PARAMS {
        return Err(profile(
            Refusal::ParameterCountOutOfBounds {
                found: buffer_count,
            },
            format!("`{declaration}` binds one buffer per parameter plus its output"),
        ));
    }
    let (params, result) = signature(function, declaration)?;
    if let KernelShape::SequentialFold = shape {
        if params.len() != 2 || params[0] != result {
            return Err(ComputeRefusal::KernelSelection {
                detail: format!(
                    "a fold kernel is `fn(acc: T, element: U) -> T`; `{declaration}` is not"
                ),
            });
        }
    }
    let (ir, body, refused_detail) = match kernel_ir::lower(function, &params, result) {
        Ok(ir) => {
            let body = match shape {
                KernelShape::ElementwiseMap { .. } => map_ops(&ir),
                KernelShape::SequentialFold => fold_ops(&ir),
            };
            (Some(ir), body, None)
        }
        Err(LowerStop::Bound { detail }) => {
            return Err(ComputeRefusal::KernelBodyBoundExceeded { detail })
        }
        Err(LowerStop::Refused { op, detail }) => (None, vec![op], Some(detail)),
    };
    let modes: Vec<(ScalarKind, ParamMode, usize)> = match shape {
        KernelShape::ElementwiseMap { .. } => function
            .params
            .iter()
            .zip(&params)
            .map(|(param, kind)| (*kind, mode_of(param.ownership), 1))
            .chain(std::iter::once((result, ParamMode::ReadWriteView, 1)))
            .collect(),
        KernelShape::SequentialFold => vec![
            (params[1], mode_of(function.params[1].ownership), 1),
            (result, ParamMode::ReadWriteView, 1),
        ],
    };
    let workgroup = match shape {
        KernelShape::ElementwiseMap { workgroup_size } => workgroup_size,
        KernelShape::SequentialFold => 1,
    };
    let admission = candidate(
        &modes,
        GridShape {
            workgroup_size: [workgroup, 1, 1],
            grid_size: [1, 1, 1],
        },
        AliasingClaim::Disjoint,
        body,
    );
    if let Err(refusal) = classify(&admission) {
        let detail =
            refused_detail.unwrap_or_else(|| format!("`{declaration}` signature or shape"));
        return Err(profile(refusal, detail));
    }
    let Some(ir) = ir else {
        return Err(ComputeRefusal::KernelSelection {
            detail: "lowering stopped without a classifier refusal".to_owned(),
        });
    };
    let fingerprint = kernel_ir::fingerprint(declaration, &shape.encode(), &ir);
    Ok((ir, fingerprint))
}

fn signature(
    function: &ResolvedFunction,
    declaration: &str,
) -> Result<(Vec<ScalarKind>, ScalarKind), ComputeRefusal> {
    let mut params = Vec::with_capacity(function.params.len());
    for (index, param) in function.params.iter().enumerate() {
        let kind = ScalarKind::from_type(&param.ty).ok_or_else(|| {
            profile(
                Refusal::TypeOutsideKernelVocabulary {
                    param: INPUT_NAMES.get(index).copied().unwrap_or("param"),
                },
                format!("`{declaration}` parameter `{}`", param.name),
            )
        })?;
        params.push(kind);
    }
    let result = ScalarKind::from_type(&function.return_type).ok_or_else(|| {
        profile(
            Refusal::TypeOutsideKernelVocabulary { param: "result" },
            format!("`{declaration}` result type"),
        )
    })?;
    Ok((params, result))
}

fn mode_of(ownership: OwnershipMode) -> ParamMode {
    match ownership {
        OwnershipMode::Value => ParamMode::ReadOnlyView,
        OwnershipMode::Own => ParamMode::OwnedTransfer,
        OwnershipMode::Borrow | OwnershipMode::Shared => ParamMode::SharedAlias,
    }
}

/// The executed result of one run before session publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RunOutcome {
    Completed(Vec<Scalar>),
    Status {
        invocation: usize,
        status: StatusCase,
    },
    Cancelled(usize),
    DeviceLost(usize),
    Guard,
}

fn interrupted(control: DispatchControl, invocation: usize) -> Option<RunOutcome> {
    if control.device_loss_before_invocation == Some(invocation) {
        return Some(RunOutcome::DeviceLost(invocation));
    }
    if control.cancel_before_invocation == Some(invocation) {
        return Some(RunOutcome::Cancelled(invocation));
    }
    None
}

/// Run every map invocation in ascending ordinal order into a staged output.
/// The first stop in that order is selected, so a kernel status is always
/// the lowest failing ordinal regardless of how a device schedules.
pub(crate) fn run_map(
    ir: &KernelIr,
    columns: &[&[Scalar]],
    len: usize,
    control: DispatchControl,
) -> RunOutcome {
    let mut staged = Vec::with_capacity(len);
    let mut arguments = Vec::with_capacity(columns.len());
    for invocation in 0..len {
        if let Some(stop) = interrupted(control, invocation) {
            return stop;
        }
        arguments.clear();
        arguments.extend(columns.iter().map(|column| column[invocation]));
        match kernel_ir::evaluate(ir, &arguments) {
            Ok(value) => staged.push(value),
            Err(EvalStop::Status(status)) => return RunOutcome::Status { invocation, status },
            Err(EvalStop::Guard) => return RunOutcome::Guard,
        }
    }
    RunOutcome::Completed(staged)
}

/// Fold left to right from `initial`; each element is one invocation ordinal.
pub(crate) fn run_fold(
    ir: &KernelIr,
    initial: Scalar,
    input: &[Scalar],
    control: DispatchControl,
) -> RunOutcome {
    let mut accumulator = initial;
    for (invocation, element) in input.iter().enumerate() {
        if let Some(stop) = interrupted(control, invocation) {
            return stop;
        }
        match kernel_ir::evaluate(ir, &[accumulator, *element]) {
            Ok(value) => accumulator = value,
            Err(EvalStop::Status(status)) => return RunOutcome::Status { invocation, status },
            Err(EvalStop::Guard) => return RunOutcome::Guard,
        }
    }
    RunOutcome::Completed(vec![accumulator])
}
