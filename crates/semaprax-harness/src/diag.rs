//! Harness diagnostics: stable `SPX-HP<letter><3 digits>` codes (see the
//! specification's Diagnostics section for the per-work-item letter).
//!
//! The type is the decision core's `Diagnostic` (MR-07): one code-and-message
//! shape, so a routing refusal is identical through the harness and the
//! runtime boundary. The historical name stays the harness's public name.

pub use semaprax_decision_core::diag::Diagnostic as HarnessDiagnostic;

pub type HarnessResult<T> = Result<T, HarnessDiagnostic>;
