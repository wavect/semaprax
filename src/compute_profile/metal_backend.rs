//! A real Apple Metal accelerator backend for the executable subset of
//! [RFC 0005: Compute Kernel
//! Profile](../../docs/RFC-0005-COMPUTE-KERNEL-PROFILE.md)
//! (`semaprax.compute-metal.v1`), for the same closed elementwise-map
//! kernel vocabulary [`super::cpu_reference`] already admits and executes
//! on the host CPU.
//!
//! Unlike [`super::cpu_reference`], this module dispatches real GPU work
//! through Apple's Metal framework (via the maintained `objc2-metal`
//! bindings): a kernel is compiled at runtime from deterministically
//! generated Metal Shading Language (MSL) source
//! ([`msl::generate`]) and executed on whatever `MTLDevice` this host's
//! `MTLCreateSystemDefaultDevice` returns.
//!
//! # Scope (v1)
//!
//! - **Shapes:** both [`KernelShape::ElementwiseMap`]
//!   ([`device::MetalSession::load_kernel`]/[`device::MetalSession::dispatch_map`],
//!   one GPU thread per element) and [`KernelShape::SequentialFold`]
//!   ([`device::MetalSession::load_fold_kernel`]/[`device::MetalSession::dispatch_fold`]).
//!   A fold is inherently sequential — the CPU reference's own
//!   `acc = f(acc, in[i])` folds left to right one element at a time, and a
//!   checked failure must select the exact same lowest-ordinal element a
//!   reordering parallel reduction could not guarantee — so it compiles to
//!   one single-thread kernel that loops internally (see
//!   [`msl::generate_fold`]), never one GPU thread per element.
//! - **Types:** every [`ScalarKind`] the CPU reference admits: `i64`/`i32`,
//!   `u8`, `usize` (lowered to MSL's `ulong`; MSL has no type spelled
//!   `usize` — see [`msl::msl_type`]), and `bool`, as both buffer
//!   parameters/results and internal values.
//! - **Checked failures:** MSL has no traps. Every checked arithmetic
//!   operation the CPU reference recognizes (`+`, `-`, `*`, `/`, `%`,
//!   unary `-`) is preceded by an explicit MSL guard that writes the exact
//!   `semaprax.status.v1` code and returns from the invocation immediately
//!   on failure — see [`msl`] for the generator and its checked-helper
//!   preamble. 64-bit unsigned (`usize`) division and remainder never emit
//!   a native MSL `/`/`%`: the system Metal compiler service crashed,
//!   deterministically, on 64-bit unsigned division reached through one such
//!   use (see [`msl`]'s module docs), so every `checked_div_u64`/
//!   `checked_rem_u64` computes its result with an ordinary bit-at-a-time
//!   binary long division instead.
//!
//! Every refusal this backend reaches uses an existing `SPX-GC0xx` code
//! (see [`device::MetalRefusal`]): the same admission and lowering path as
//! the CPU reference (`session::bind`, reused verbatim, made `pub(crate)`
//! for exactly this reuse) plus this backend's own narrower buffer-type
//! restriction, itself only a tighter case of the classifier's existing
//! `TypeOutsideKernelVocabulary` (`SPX-GC004`). There is no silent
//! CPU fallback anywhere in this module: an unsupported shape or type
//! refuses, it never runs on the host instead.
//!
//! # Non-claims
//!
//! This module compiles and dispatches on whatever real device
//! `MTLCreateSystemDefaultDevice` returns on the host that builds it with
//! `metal-device` enabled — nothing else. It is not wired into any
//! compilation route, CLI, or generated artifact; it claims no hosted,
//! simulated, or continuously-verified device, only whatever this exact
//! process observed on this exact host at the moment a test ran (see
//! [`device::DeviceProvenance`], recorded in test output). It is not built
//! at all — the module is not even declared — on a non-macOS target or
//! without the `metal-device` feature, so the default build and every
//! other target are byte-for-byte unaffected by its existence.

pub mod device;
pub mod msl;

#[cfg(test)]
mod tests;

pub use device::{
    DeviceProvenance, DeviceUnavailable, MetalBufferHandle, MetalCapability, MetalDispatchOutcome,
    MetalKernelArtifact, MetalRefusal, MetalReleaseCause, MetalReleaseEvent, MetalSession,
    MetalSessionFailure, MetalSettlement,
};

/// The frozen executable-semantics schema identifier for this backend,
/// distinct from [`super::cpu_reference::CPU_REFERENCE_SCHEMA`]: it names a
/// real accelerator, not a host reference model.
pub const METAL_BACKEND_SCHEMA: &str = "semaprax.compute-metal.v1";
