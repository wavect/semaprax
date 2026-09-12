//! The [Compute Kernel Profile v1](../docs/RFC-0005-COMPUTE-KERNEL-PROFILE.md)
//! admission classifier — issue #210's design half.
//!
//! No kernel syntax exists anywhere in this compiler's parser, resolver, or
//! HIR yet, and no GPU or accelerator toolchain is available on any host that
//! builds this crate. This module is therefore **not** wired into
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
//! | Document | Module |
//! | --- | --- |
//! | [RFC 0005: Compute Kernel Profile](../docs/RFC-0005-COMPUTE-KERNEL-PROFILE.md) | [`boundary_profile`] (bounds only) and [`classifier`] (the admission predicate) |

pub mod boundary_profile;
pub mod classifier;
