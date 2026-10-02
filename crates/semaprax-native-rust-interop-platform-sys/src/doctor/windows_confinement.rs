//! Windows confinement and settlement contract for
//! [`DOCTOR-PRODUCTION-PROVISIONER-WINDOWS-V1`][doc] (issue #236).
//!
//! This is a standalone confinement primitive, analogous to
//! `doctor::darwin_confinement`: it is not the ordinary `--version` probe in
//! `doctor::windows` (which already assigns a suspended leader to a
//! non-breakaway job before any user code runs, and is not touched by this
//! module), and it is not wired into any ordinary CLI route or into
//! `provisioned_doctor_*`.
//!
//! This module is split so that the parts with no Win32 dependency --
//! signed-capsule verification plus a non-authoritative structural corpus ([`capsule`]), fail-closed admission
//! ordering ([`refusal`]), and the sticky settlement state machine
//! ([`settlement`]) -- compile and run their tests on every host this crate
//! builds on, this one included. Only [`primitive`], the actual restricted
//! token / tightened job object / ACL'd scratch root / sealed-capsule
//! consumption wiring, is `#[cfg(windows)]`. The historical ten-case native
//! selector passed at `f4d3291f`; the signed-image binding continuation and
//! authenticated request/bundle handoff have no native execution receipt. See
//! the owning specification for the retained historical evidence and remaining
//! nonclaims.
//!
//! [doc]: https://github.com/wavect/semaprax/blob/main/docs/DOCTOR-PRODUCTION-PROVISIONER-WINDOWS-V1.md
//!
//! `dead_code` is allowed because the crate root deliberately does not
//! re-export this internal Windows primitive as a public API or ordinary CLI
//! route. Its contract and tests remain available inside the owning crate.
#![allow(dead_code)]
pub mod capsule;
#[cfg(windows)]
pub mod carrier;
#[cfg(windows)]
pub mod primitive;
pub mod refusal;
pub mod settlement;
