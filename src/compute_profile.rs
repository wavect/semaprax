//! The [Compute Kernel Profile v1](../docs/RFC-0005-COMPUTE-KERNEL-PROFILE.md)
//! admission classifier — issue #210's design half.
//!
//! No kernel syntax exists anywhere in this compiler's parser, resolver, or
//! HIR yet, and no GPU or accelerator toolchain is available on any host that
//! builds this crate. The classifier is therefore **not** wired into
//! compilation, executes nothing, dispatches nothing, and reads no real
//! program source. It is the admission predicate a future front end must
//! implement, made executable today the same way
//! [`crate::public_generic_abi::classifier`] made its own boundary profile
//! executable before any descriptor producer existed: as a pure function
//! over a constructed fixture, so every refusal reason in the closed
//! `SPX-GC0xx` vocabulary is independently observed by a real test today,
//! without hardware, without a parser change, and without pretending any
//! kernel has ever run.
//!
//!
//! [`cpu_reference`] adds the executable half for a closed subset: a
//! deterministic, library-level CPU reference executor that binds an
//! ordinary checked function by its persistent `@id`, lowers its resolved
//! HIR, admits it through [`classifier::classify`], and runs it under a
//! typed owned-device-buffer lifecycle. It is still not wired into any
//! compilation route, CLI, or backend, and it is not an accelerator.
//!
//! | Document | Module |
//! | --- | --- |
//! | [RFC 0005: Compute Kernel Profile](../docs/RFC-0005-COMPUTE-KERNEL-PROFILE.md) | [`boundary_profile`] (bounds only), [`classifier`] (the admission predicate), and [`cpu_reference`] (executable CPU reference semantics v1) |

pub mod boundary_profile;
pub mod classifier;
pub mod cpu_reference;
