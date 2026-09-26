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
//! - **Shape:** [`KernelShape::ElementwiseMap`] only. Sequential folds are
//!   inherently sequential and are not attempted on the GPU in this pass;
//!   [`device::MetalSession::load_kernel`] only ever binds a map.
//! - **Types:** only `i64`/`i32` buffer parameters and results. An
//!   internal `bool` (from a comparison, `if` condition, or `&&`/`||`) is
//!   admitted, since no non-trivial kernel body can avoid one; `u8` and
//!   `usize`, anywhere in the body, are refused rather than given an
//!   unproven MSL lowering.
//! - **Checked failures:** MSL has no traps. Every checked arithmetic
//!   operation the CPU reference recognizes (`+`, `-`, `*`, `/`, `%`,
//!   unary `-`) is preceded by an explicit MSL guard that writes the exact
//!   `semaprax.status.v1` code and returns from the invocation immediately
//!   on failure — see [`msl`] for the generator and its checked-helper
//!   preamble.
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
