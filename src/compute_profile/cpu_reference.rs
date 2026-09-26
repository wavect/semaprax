//! The deterministic CPU reference executor for the executable subset of
//! [RFC 0005: Compute Kernel
//! Profile](../../docs/RFC-0005-COMPUTE-KERNEL-PROFILE.md#executable-cpu-reference-semantics-v1)
//! (`semaprax.compute-cpu-reference.v1`).
//!
//! A kernel is not new source syntax. It is one ordinary, already checked
//! SEMAPRAX function, selected by its explicit persistent `@id`, whose
//! resolved HIR lowers into the closed [`kernel_ir`] vocabulary: `Copy`
//! integer/boolean scalar parameters, literals, checked arithmetic,
//! comparisons, lazy boolean operators, `if`, and immutable `let` blocks.
//! Everything else refuses with the existing `SPX-GC0xx` admission codes,
//! reached through [`super::classifier::classify`] itself rather than a
//! restated predicate.
//!
//! Two kernel shapes execute:
//!
//! - [`KernelShape::ElementwiseMap`]: `out[i] = f(in_0[i], ..., in_k[i])`
//!   for every `i` in ascending invocation order;
//! - [`KernelShape::SequentialFold`]: `acc = f(acc, in[i])` for `i` in
//!   ascending order from an explicit initial value, published to a
//!   one-element output buffer.
//!
//! Device buffers follow one typed lifecycle inside a [`CpuReferenceSession`]
//! opened only with an explicit [`ComputeCapability`]:
//! allocate -> upload -> dispatch -> download -> release, with sticky
//! failure selection and settlement that releases every live buffer exactly
//! once in reverse allocation order. See [`session`].
//!
//! # Non-claims
//!
//! This is a library-level reference model on the host CPU. It is not an
//! accelerator backend, performs no driver or device call, and its
//! cancellation and device-loss outcomes are deterministic injection points
//! ([`DispatchControl`]), never evidence about real hardware. It is not wired
//! into any CLI, compilation route, or generated artifact.

use crate::diagnostic::Diagnostic;

use super::boundary_profile::MAX_BUFFER_ELEMENTS;
use super::classifier::{DeviceEffect, Refusal};

pub mod kernel_ir;
pub mod session;

pub use kernel_ir::{Scalar, ScalarKind};
pub use session::{
    BufferHandle, ComputeCapability, CpuReferenceSession, DispatchControl, DispatchOutcome,
    EffectEvent, KernelArtifact, KernelShape, ReleaseCause, ReleaseEvent, SessionFailure,
    Settlement,
};

/// The frozen executable-semantics schema identifier.
pub const CPU_REFERENCE_SCHEMA: &str = "semaprax.compute-cpu-reference.v1";

/// Max lowered kernel IR nodes in one kernel body.
pub const MAX_KERNEL_IR_NODES: usize = 4096;

/// Max nesting depth of one lowered kernel body.
pub const MAX_KERNEL_IR_DEPTH: usize = 64;

/// Max live elements across every buffer of one session. Equal to the
/// per-buffer bound so a hostile caller cannot multiply it by allocating
/// many buffers.
pub const MAX_SESSION_LIVE_ELEMENTS: usize = MAX_BUFFER_ELEMENTS;

/// The selected declaration is absent, has no explicit persistent identity,
/// or does not have the signature the requested kernel shape requires.
pub const KERNEL_SELECTION_REFUSED: &str = "SPX-GC014";
/// A kernel artifact or buffer handle is stale: from another session, or
/// an artifact whose recorded binding no longer matches the checked program.
pub const STALE_HANDLE: &str = "SPX-GC015";
/// A buffer handle was used after its release, or released twice.
pub const BUFFER_RELEASED: &str = "SPX-GC016";
/// A transfer range, allocation length, or dispatch extent is out of bounds.
pub const TRANSFER_OUT_OF_BOUNDS: &str = "SPX-GC017";
/// A value or buffer element type disagrees with the buffer or kernel
/// signature it is bound to.
pub const ELEMENT_TYPE_MISMATCH: &str = "SPX-GC018";
/// A failure (kernel status, cancellation, or device loss) is already
/// selected for this session; only release and settlement remain.
pub const FAILURE_ALREADY_SELECTED: &str = "SPX-GC019";
/// A kernel body exceeds the lowered-IR node or depth bound.
pub const KERNEL_BODY_BOUND_EXCEEDED: &str = "SPX-GC020";
/// An operation needs a device effect the session capability does not grant.
pub const EFFECT_NOT_GRANTED: &str = "SPX-GC021";

/// Why one CPU-reference operation was refused. A refusal happens before
/// any effect: session state is unchanged and nothing is journaled.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComputeRefusal {
    /// An existing RFC 0005 admission refusal, with the exact construct.
    Profile {
        refusal: Refusal,
        detail: String,
    },
    KernelSelection {
        detail: String,
    },
    StaleHandle {
        detail: String,
    },
    BufferReleased {
        buffer: u32,
    },
    OutOfBounds {
        detail: String,
    },
    ElementTypeMismatch {
        detail: String,
    },
    FailureAlreadySelected {
        failure: SessionFailure,
    },
    KernelBodyBoundExceeded {
        detail: String,
    },
    EffectNotGranted {
        effect: DeviceEffect,
    },
}

impl ComputeRefusal {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Profile { refusal, .. } => refusal.code(),
            Self::KernelSelection { .. } => KERNEL_SELECTION_REFUSED,
            Self::StaleHandle { .. } => STALE_HANDLE,
            Self::BufferReleased { .. } => BUFFER_RELEASED,
            Self::OutOfBounds { .. } => TRANSFER_OUT_OF_BOUNDS,
            Self::ElementTypeMismatch { .. } => ELEMENT_TYPE_MISMATCH,
            Self::FailureAlreadySelected { .. } => FAILURE_ALREADY_SELECTED,
            Self::KernelBodyBoundExceeded { .. } => KERNEL_BODY_BOUND_EXCEEDED,
            Self::EffectNotGranted { .. } => EFFECT_NOT_GRANTED,
        }
    }

    pub fn diagnostic(&self) -> Diagnostic {
        let message = match self {
            Self::Profile { refusal, detail } => {
                format!("{} ({detail})", refusal.diagnostic().message)
            }
            Self::KernelSelection { detail } => {
                format!("{CPU_REFERENCE_SCHEMA} refuses kernel selection: {detail}")
            }
            Self::StaleHandle { detail } => {
                format!("{CPU_REFERENCE_SCHEMA} refuses a stale handle: {detail}")
            }
            Self::BufferReleased { buffer } => {
                format!("{CPU_REFERENCE_SCHEMA} refuses buffer {buffer}: it was already released")
            }
            Self::OutOfBounds { detail } => {
                format!("{CPU_REFERENCE_SCHEMA} refuses an out-of-bounds extent: {detail}")
            }
            Self::ElementTypeMismatch { detail } => {
                format!("{CPU_REFERENCE_SCHEMA} refuses an element type mismatch: {detail}")
            }
            Self::FailureAlreadySelected { failure } => format!(
                "{CPU_REFERENCE_SCHEMA} refuses the operation: failure {failure:?} is already \
                 selected, only release and settlement remain"
            ),
            Self::KernelBodyBoundExceeded { detail } => format!(
                "{CPU_REFERENCE_SCHEMA} refuses the kernel body: {detail} (bounds \
                 {MAX_KERNEL_IR_NODES} nodes, depth {MAX_KERNEL_IR_DEPTH})"
            ),
            Self::EffectNotGranted { effect } => format!(
                "{CPU_REFERENCE_SCHEMA} refuses the operation: capability does not grant \
                 {effect:?}"
            ),
        };
        Diagnostic::io(self.code(), message)
    }
}

#[cfg(test)]
mod tests;
