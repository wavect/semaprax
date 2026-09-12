//! The pure [RFC 0005: Compute Kernel
//! Profile](../../docs/RFC-0005-COMPUTE-KERNEL-PROFILE.md) admission
//! classifier.
//!
//! [`classify`] takes one constructed [`KernelCandidate`] fixture — never
//! real parsed or checked program source, since no kernel syntax exists in
//! this compiler yet — and returns either a typed [`AdmittedKernel`] or one
//! closed [`Refusal`] reason, each backed by a real `SPX-GC0xx` diagnostic
//! code. It decides nothing about whether a kernel *runs* correctly on any
//! device: there is no CPU reference interpreter and no accelerator backend
//! behind this module, and none is claimed. It decides only whether a
//! candidate kernel shape is admissible under the profile that exists so
//! that, once real front-end integration is built, it has one trusted,
//! already-tested admission predicate to call rather than inventing one
//! under implementation pressure.
//!
//! # Precedence
//!
//! [`classify`] checks in this fixed order, matching the owning
//! specification's own precedence list: effect-vocabulary closure, explicit
//! device-capability presence, parameter-count bound, parameter-type
//! admission, buffer-capacity bound, parameter-ownership-mode admission,
//! grid/workgroup-shape bounds, buffer aliasing, then each kernel-body
//! operation in declaration order (impure host effect, unchecked indexed
//! access, floating-point operation, non-deterministic reduction order,
//! non-deterministic atomic). Every negative test in this module's `tests`
//! submodule mutates exactly one field of a fully admitted baseline fixture,
//! so every earlier-precedence check the mutation does not touch is
//! independently known to still pass — the admitted baseline itself proves
//! that — and the asserted [`Refusal`] can only be attributed to the one
//! rule the test actually targets.
//!
//! # Known limitations
//!
//! - **No real front-end integration.** [`KernelCandidate`] is a classifier
//!   fixture type, not a parsed or checked declaration. Nothing here
//!   constrains what any future parser, resolver, or HIR pass actually
//!   admits; it is the predicate those stages must implement, not evidence
//!   that they do yet.
//! - **No CPU reference execution and no accelerator backend.** This module
//!   never runs a kernel, on any device, real or simulated. A kernel this
//!   classifier admits is a shape the profile permits, not a program known
//!   to produce any particular output.
//! - **Host resource lifecycle is out of scope here.** Device buffer/queue
//!   allocation, copy, dispatch, and release as affine RFC-0003-style
//!   resources — including double-release, device-absence, and stale
//!   artifact rejection — are specified in the owning RFC's "Ownership and
//!   effects" section but have no classifier here: they require a real
//!   resource tracker and cleanup-plan integration that does not exist for
//!   a language feature not yet admitted into the grammar. [`DeviceEffect`]
//!   only names the closed effect vocabulary a kernel's declared effects
//!   must stay within; it does not track resource lifecycle state.

use crate::diagnostic::Diagnostic;

use super::boundary_profile::{
    COMPUTE_KERNEL_PROFILE_SCHEMA, MAX_BUFFER_ELEMENTS, MAX_GRID_DIM, MAX_KERNEL_PARAMS,
    MAX_WORKGROUP_DIM, MAX_WORKGROUP_INVOCATIONS,
};

/// A declared effect a kernel dispatch may name. Deliberately not the full
/// closed capability vocabulary [Build Capability Manifest
/// v1](../../docs/CAPABILITY-MANIFEST-V1.md) already defines for host
/// programs: a kernel body admits only this narrower, explicit device
/// subset, and nothing else, ever — see [`KernelCandidate::declared_other_effects`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceEffect {
    DeviceAlloc,
    DeviceCopyIn,
    DeviceCopyOut,
    DeviceDispatch,
    DeviceSynchronize,
    DeviceRelease,
}

/// The eight admitted kernel-safe integer/boolean scalars. No floating-point
/// scalar is admitted in v1: the owning RFC's numeric policy defers floats
/// to a separate tolerance/NaN/rounding profile and admits only an
/// exact-integer kernel subset today.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Bool,
    /// Not admitted in v1 — see [`is_admitted_scalar`]. Named here, rather
    /// than left unrepresentable, so a fixture can exercise "a
    /// floating-point *parameter type*"
    /// ([`Refusal::TypeOutsideKernelVocabulary`]) distinctly from "a
    /// floating-point *body operation*" on an otherwise-integer signature
    /// ([`KernelOp::FloatArithmetic`], [`Refusal::FloatingPointNotAdmittedInV1`]).
    F32,
    F64,
}

/// The v1 exact-integer profile's admitted scalar set: every [`ScalarType`]
/// variant except the two floating-point ones.
fn is_admitted_scalar(scalar: ScalarType) -> bool {
    !matches!(scalar, ScalarType::F32 | ScalarType::F64)
}

/// The address space one buffer parameter lives in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressSpace {
    /// Host-visible device-global memory, moved across the host/device
    /// boundary only through an explicit `DeviceCopyIn`/`DeviceCopyOut`
    /// effect at dispatch.
    Global,
    /// Workgroup-local scratch, visible only within one workgroup and only
    /// for the duration of one dispatch.
    Shared,
    /// Per-invocation private scratch.
    Private,
}

/// A kernel-signature parameter's type. Buffers are always fixed-length and
/// bounds-checked; no dynamically-sized array is admitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelType {
    Scalar(ScalarType),
    /// A fixed-width vector, 2, 3, or 4 lanes of one admitted scalar.
    Vector {
        elem: ScalarType,
        lanes: u8,
    },
    /// A fixed-length buffer of one admitted scalar in one address space.
    Buffer {
        elem: ScalarType,
        space: AddressSpace,
        len: usize,
    },
}

/// A kernel parameter's admitted access mode. Ownership *transfer* of a
/// device buffer is a host-side dispatch concern (see the module's Known
/// limitations); inside a kernel body, a buffer is only ever borrowed for
/// the duration of one dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParamMode {
    ReadOnlyView,
    ReadWriteView,
    /// An affine transfer into the kernel body itself. Never admitted: an
    /// owned aggregate transferred *into* a kernel invocation would need a
    /// per-invocation cleanup obligation the profile does not define, and
    /// no owning specification defines one.
    OwnedTransfer,
    /// A mutable alias reachable through more than one parameter at once,
    /// with no disjointness proof. Never admitted; see
    /// [`KernelCandidate::aliasing`] for the buffer-pair case this
    /// classifier can actually detect.
    SharedAlias,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelParam {
    pub name: &'static str,
    pub ty: KernelType,
    pub mode: ParamMode,
}

/// A dispatch grid: the fixed workgroup size and the fixed grid size (in
/// workgroups), both three-dimensional. A dimension of `1` collapses that
/// axis; no dimension may be `0`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GridShape {
    pub workgroup_size: [u32; 3],
    pub grid_size: [u32; 3],
}

/// The order a reduction combines its per-invocation partial results. Only
/// [`ReductionOrder::SequentialLeftToRight`] is deterministic under this
/// profile: it names one fixed combination order that does not depend on
/// scheduling, workgroup count, or driver-chosen tree shape, so the same
/// input always combines in the same order regardless of device or driver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReductionOrder {
    SequentialLeftToRight,
    /// A tree/pairwise combination whose shape depends on how many
    /// invocations or workgroups the driver schedules. Refused: the same
    /// logical reduction can combine operands in a different order — and,
    /// under floating point, produce a different result — on a different
    /// device, workgroup count, or driver version, silently.
    TreeAssociative,
    /// No order is named at all (for example, "whichever invocation
    /// finishes first wins"). Refused for the same reason, more acutely.
    Unspecified,
}

/// The ordering guarantee one atomic operation names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicOrder {
    /// A fixed, total order over every contending invocation for this exact
    /// memory location, defined independently of scheduling.
    SequentiallyConsistentFixedOrder,
    /// Whatever order the driver's own relaxed/undefined scheduling
    /// produces. Refused unconditionally: this profile admits no atomic
    /// without a total order it can name in advance.
    RelaxedDriverDefined,
}

/// One operation a kernel body performs, in declaration order. This is a
/// classifier fixture vocabulary, not a real IR or AST: it names exactly the
/// constructs the owning RFC's determinism policy must classify, at the
/// granularity that policy actually distinguishes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelOp {
    /// A checked integer/boolean arithmetic operation. Always admitted.
    IntegerArithmetic,
    /// An explicit workgroup barrier. Always admitted: a barrier is exactly
    /// the deterministic synchronization primitive this profile requires in
    /// place of implicit cross-invocation ordering.
    Barrier,
    /// A read from a buffer parameter at the given address space.
    /// `statically_in_bounds` is true only when the index is a compile-time
    /// constant proven within `len`; a dynamic index additionally needs
    /// `dynamically_checked` to name an admitted runtime bounds check.
    IndexedLoad {
        space: AddressSpace,
        statically_in_bounds: bool,
        dynamically_checked: bool,
    },
    /// A write to a buffer parameter; same admission rule as
    /// [`KernelOp::IndexedLoad`].
    IndexedStore {
        space: AddressSpace,
        statically_in_bounds: bool,
        dynamically_checked: bool,
    },
    /// Any floating-point arithmetic. Refused unconditionally in v1: the
    /// owning RFC's numeric policy admits only exact-integer kernels until a
    /// separate floating-point tolerance/NaN/rounding profile exists.
    FloatArithmetic,
    /// A reduction over some admitted scalar type, combined in the given
    /// order.
    Reduction {
        elem: ScalarType,
        order: ReductionOrder,
    },
    /// An atomic read-modify-write. `is_integer` false means the operand is
    /// not one of the admitted integer scalars (for example a
    /// floating-point atomic add, whose result additionally depends on
    /// operation order under reassociation).
    Atomic {
        is_integer: bool,
        order: AtomicOrder,
    },
    /// Any operation reaching outside the closed device-effect vocabulary
    /// from inside the kernel body itself (for example a direct host I/O
    /// call), independent of what the kernel's declared effect list claims.
    HostEffect,
}

/// Whether two or more buffer parameters may reach overlapping memory. This
/// classifier can only observe the coarse claim a fixture makes about its
/// own buffers; a real front end would derive this from checked ownership
/// and view facts the way [`crate::hir`] already does for ordinary borrows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AliasingClaim {
    /// No two buffer parameters can reach the same memory during one
    /// dispatch.
    Disjoint,
    /// At least two buffer parameters may reach overlapping memory, with no
    /// disjointness proof.
    MayOverlap,
}

/// One fixture kernel candidate: everything [`classify`] needs to decide
/// admission. Never a real parsed declaration; see the module
/// documentation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelCandidate {
    pub name: &'static str,
    /// The kernel's own declared effects, which must be a subset of the
    /// closed [`DeviceEffect`] vocabulary — see
    /// [`Self::declared_other_effects`] for the refusal case.
    pub effects: Vec<DeviceEffect>,
    /// Any effect token the fixture declares outside the closed
    /// [`DeviceEffect`] vocabulary (for example a host filesystem or network
    /// capability). Always empty on an admitted kernel.
    pub declared_other_effects: Vec<&'static str>,
    /// Whether an explicit device capability was named for this dispatch.
    /// A kernel that declares [`DeviceEffect::DeviceDispatch`] without one
    /// is refused: this profile admits no implicit or ambient device
    /// selection.
    pub explicit_device_capability: bool,
    pub params: Vec<KernelParam>,
    pub grid: GridShape,
    pub aliasing: AliasingClaim,
    pub body: Vec<KernelOp>,
}

/// The complete typed result of one successful classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedKernel {
    pub schema: &'static str,
    pub name: &'static str,
    pub param_count: usize,
    pub grid: GridShape,
}

/// A kernel body declares an effect outside the closed [`DeviceEffect`]
/// vocabulary.
pub const EFFECT_OUTSIDE_CLOSED_VOCABULARY: &str = "SPX-GC001";
/// A dispatch effect is declared without an explicit device capability.
pub const IMPLICIT_DEVICE_SELECTION: &str = "SPX-GC002";
/// The parameter count is zero or exceeds [`MAX_KERNEL_PARAMS`].
pub const PARAMETER_COUNT_OUT_OF_BOUNDS: &str = "SPX-GC003";
/// A parameter's type is outside the kernel-safe scalar/vector/buffer
/// vocabulary (this includes any floating-point scalar in v1).
pub const TYPE_OUTSIDE_KERNEL_VOCABULARY: &str = "SPX-GC004";
/// A parameter's access mode is not an admitted read-only or read-write
/// buffer view.
pub const OWNERSHIP_MODE_NOT_ADMITTED: &str = "SPX-GC005";
/// A workgroup or grid dimension is zero or exceeds its bound.
pub const GRID_SHAPE_OUT_OF_BOUNDS: &str = "SPX-GC006";
/// An indexed buffer access is neither statically proven in-bounds nor
/// guarded by an admitted runtime bounds check.
pub const UNCHECKED_INDEXED_ACCESS: &str = "SPX-GC007";
/// A reduction combines its operands in a non-deterministic order.
pub const NONDETERMINISTIC_REDUCTION_ORDER: &str = "SPX-GC008";
/// An atomic operation has no admitted deterministic total order, or
/// operates on a non-integer operand.
pub const NONDETERMINISTIC_ATOMIC: &str = "SPX-GC009";
/// The kernel body performs a floating-point operation, which the v1
/// exact-integer profile does not admit.
pub const FLOATING_POINT_NOT_ADMITTED_IN_V1: &str = "SPX-GC010";
/// Two or more buffer parameters may alias beyond the profile's checked
/// disjointness rule.
pub const ALIASING_BEYOND_CHECKED_RULE: &str = "SPX-GC011";
/// The kernel body performs a host effect directly, independent of its
/// declared effect list.
pub const KERNEL_BODY_NOT_PURE: &str = "SPX-GC012";
/// A buffer parameter's element count exceeds [`MAX_BUFFER_ELEMENTS`].
pub const CAPACITY_BOUND_EXCEEDED: &str = "SPX-GC013";

/// Why one candidate kernel was refused admission under this profile.
/// Closed: a new reason is a new profile version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    EffectOutsideClosedVocabulary { token: &'static str },
    ImplicitDeviceSelection,
    ParameterCountOutOfBounds { found: usize },
    TypeOutsideKernelVocabulary { param: &'static str },
    OwnershipModeNotAdmitted { param: &'static str },
    GridShapeOutOfBounds,
    UncheckedIndexedAccess,
    NondeterministicReductionOrder,
    NondeterministicAtomic,
    FloatingPointNotAdmittedInV1,
    AliasingBeyondCheckedRule,
    KernelBodyNotPure,
    CapacityBoundExceeded { param: &'static str },
}

impl Refusal {
    /// The real, allocated diagnostic code for this reason.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::EffectOutsideClosedVocabulary { .. } => EFFECT_OUTSIDE_CLOSED_VOCABULARY,
            Self::ImplicitDeviceSelection => IMPLICIT_DEVICE_SELECTION,
            Self::ParameterCountOutOfBounds { .. } => PARAMETER_COUNT_OUT_OF_BOUNDS,
            Self::TypeOutsideKernelVocabulary { .. } => TYPE_OUTSIDE_KERNEL_VOCABULARY,
            Self::OwnershipModeNotAdmitted { .. } => OWNERSHIP_MODE_NOT_ADMITTED,
            Self::GridShapeOutOfBounds => GRID_SHAPE_OUT_OF_BOUNDS,
            Self::UncheckedIndexedAccess => UNCHECKED_INDEXED_ACCESS,
            Self::NondeterministicReductionOrder => NONDETERMINISTIC_REDUCTION_ORDER,
            Self::NondeterministicAtomic => NONDETERMINISTIC_ATOMIC,
            Self::FloatingPointNotAdmittedInV1 => FLOATING_POINT_NOT_ADMITTED_IN_V1,
            Self::AliasingBeyondCheckedRule => ALIASING_BEYOND_CHECKED_RULE,
            Self::KernelBodyNotPure => KERNEL_BODY_NOT_PURE,
            Self::CapacityBoundExceeded { .. } => CAPACITY_BOUND_EXCEEDED,
        }
    }

    /// Render as the [`Diagnostic`] a caller can surface directly.
    pub fn diagnostic(&self) -> Diagnostic {
        let message = match self {
            Self::EffectOutsideClosedVocabulary { token } => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: declared effect \
                 `{token}` is outside the closed device-effect vocabulary"
            ),
            Self::ImplicitDeviceSelection => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: a dispatch effect is \
                 declared without an explicit device capability"
            ),
            Self::ParameterCountOutOfBounds { found } => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: {found} parameters, v1 \
                 requires 1 to {MAX_KERNEL_PARAMS}"
            ),
            Self::TypeOutsideKernelVocabulary { param } => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: parameter `{param}` has a \
                 type outside the kernel-safe scalar/vector/buffer vocabulary"
            ),
            Self::OwnershipModeNotAdmitted { param } => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: parameter `{param}` does \
                 not use an admitted read-only or read-write buffer view"
            ),
            Self::GridShapeOutOfBounds => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: a workgroup or grid \
                 dimension is zero or exceeds its bound"
            ),
            Self::UncheckedIndexedAccess => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: an indexed buffer access \
                 is neither statically proven in-bounds nor guarded by a runtime bounds check"
            ),
            Self::NondeterministicReductionOrder => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: a reduction combines its \
                 operands in a non-deterministic order"
            ),
            Self::NondeterministicAtomic => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: an atomic operation has no \
                 admitted deterministic total order, or operates on a non-integer operand"
            ),
            Self::FloatingPointNotAdmittedInV1 => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: a floating-point \
                 operation is not admitted by the v1 exact-integer profile"
            ),
            Self::AliasingBeyondCheckedRule => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: two or more buffer \
                 parameters may alias beyond the profile's checked disjointness rule"
            ),
            Self::KernelBodyNotPure => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: the kernel body performs \
                 a host effect directly"
            ),
            Self::CapacityBoundExceeded { param } => format!(
                "{COMPUTE_KERNEL_PROFILE_SCHEMA} refuses this kernel: buffer parameter \
                 `{param}` exceeds the {MAX_BUFFER_ELEMENTS}-element bound"
            ),
        };
        Diagnostic::io(self.code(), message)
    }
}

fn kernel_type_admitted(ty: &KernelType) -> bool {
    match ty {
        KernelType::Scalar(scalar) => is_admitted_scalar(*scalar),
        KernelType::Vector { elem, lanes } => is_admitted_scalar(*elem) && (2..=4).contains(lanes),
        KernelType::Buffer { elem, .. } => is_admitted_scalar(*elem),
    }
}

fn buffer_len_if_present(ty: &KernelType) -> Option<usize> {
    match ty {
        KernelType::Buffer { len, .. } => Some(*len),
        _ => None,
    }
}

/// Classify one fixture kernel candidate against [RFC 0005: Compute Kernel
/// Profile](../../docs/RFC-0005-COMPUTE-KERNEL-PROFILE.md). See the module
/// documentation's [Precedence](#precedence) section for the exact check
/// order.
pub fn classify(candidate: &KernelCandidate) -> Result<AdmittedKernel, Refusal> {
    if let Some(&token) = candidate.declared_other_effects.first() {
        return Err(Refusal::EffectOutsideClosedVocabulary { token });
    }

    if candidate.effects.contains(&DeviceEffect::DeviceDispatch)
        && !candidate.explicit_device_capability
    {
        return Err(Refusal::ImplicitDeviceSelection);
    }

    if candidate.params.is_empty() || candidate.params.len() > MAX_KERNEL_PARAMS {
        return Err(Refusal::ParameterCountOutOfBounds {
            found: candidate.params.len(),
        });
    }

    for param in &candidate.params {
        if !kernel_type_admitted(&param.ty) {
            return Err(Refusal::TypeOutsideKernelVocabulary { param: param.name });
        }
    }

    for param in &candidate.params {
        if let Some(len) = buffer_len_if_present(&param.ty) {
            if len > MAX_BUFFER_ELEMENTS {
                return Err(Refusal::CapacityBoundExceeded { param: param.name });
            }
        }
    }

    for param in &candidate.params {
        if !matches!(
            param.mode,
            ParamMode::ReadOnlyView | ParamMode::ReadWriteView
        ) {
            return Err(Refusal::OwnershipModeNotAdmitted { param: param.name });
        }
    }

    let grid = &candidate.grid;
    let dims_in_bounds =
        |dims: [u32; 3]| dims.iter().all(|&d| (1..=MAX_WORKGROUP_DIM).contains(&d));
    let workgroup_invocations: u64 = grid.workgroup_size.iter().map(|&d| d as u64).product();
    if !dims_in_bounds(grid.workgroup_size)
        || grid.grid_size.iter().any(|&d| d == 0 || d > MAX_GRID_DIM)
        || workgroup_invocations > MAX_WORKGROUP_INVOCATIONS as u64
    {
        return Err(Refusal::GridShapeOutOfBounds);
    }

    if candidate.aliasing == AliasingClaim::MayOverlap {
        return Err(Refusal::AliasingBeyondCheckedRule);
    }

    for op in &candidate.body {
        match op {
            KernelOp::IntegerArithmetic | KernelOp::Barrier => {}
            KernelOp::HostEffect => return Err(Refusal::KernelBodyNotPure),
            KernelOp::IndexedLoad {
                statically_in_bounds,
                dynamically_checked,
                ..
            }
            | KernelOp::IndexedStore {
                statically_in_bounds,
                dynamically_checked,
                ..
            } => {
                if !statically_in_bounds && !dynamically_checked {
                    return Err(Refusal::UncheckedIndexedAccess);
                }
            }
            KernelOp::FloatArithmetic => return Err(Refusal::FloatingPointNotAdmittedInV1),
            KernelOp::Reduction { order, .. } => {
                if *order != ReductionOrder::SequentialLeftToRight {
                    return Err(Refusal::NondeterministicReductionOrder);
                }
            }
            KernelOp::Atomic { is_integer, order } => {
                if !is_integer || *order != AtomicOrder::SequentiallyConsistentFixedOrder {
                    return Err(Refusal::NondeterministicAtomic);
                }
            }
        }
    }

    Ok(AdmittedKernel {
        schema: COMPUTE_KERNEL_PROFILE_SCHEMA,
        name: candidate.name,
        param_count: candidate.params.len(),
        grid: candidate.grid,
    })
}

#[path = "classifier/tests.rs"]
#[cfg(test)]
mod tests;
