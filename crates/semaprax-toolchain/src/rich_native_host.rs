//! Explicit host entry points for native build and callback refusals.
//!
//! These functions do not discover ambient authority. Callers supply an
//! admitted profile, exact prepared closure, and explicit invocation or grant.

use crate::rich_cargo_execution::{
    authorize_prepared_build, build_locked_offline, CargoExecutionError, ExplicitCargoInvocation,
};
use crate::rich_cargo_preparation::PreparedCargoClosure;
use semaprax::diagnostic::Diagnostic;
use semaprax_native_rust_interop::{
    NativeDispatchError, NativeExecutionGrant, NativeTrustError, TrustedNativeProfile,
};

/// Refusals from the separate Cargo, native-admission, and callback stages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeHostRefusal {
    Cargo(CargoExecutionError),
    Trust(NativeTrustError),
    Dispatch(NativeDispatchError),
}

impl NativeHostRefusal {
    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Cargo(error) => error.diagnostic(),
            Self::Trust(error) => error.diagnostic(),
            Self::Dispatch(error) => error.diagnostic(),
        }
    }

    /// Existing human/JSON diagnostic rendering with a failing CLI exit code.
    /// Neither form includes an input path, child stderr, or environment value.
    pub fn render(&self, json: bool) -> (String, u8) {
        let diagnostic = self.diagnostic();
        let line = if json {
            diagnostic.json()
        } else {
            diagnostic.to_string()
        };
        (format!("{line}\n"), 1)
    }
}

/// Execute only a previously admitted trusted-host Cargo build. The disclosure
/// is returned only after the build succeeds; a refusal keeps its exact stage.
pub fn build_admitted_native(
    profile: &TrustedNativeProfile,
    invocation: &ExplicitCargoInvocation,
    prepared: &PreparedCargoClosure,
    binding_plan: &[u8],
    tool_identity: &[u8],
) -> Result<&'static str, NativeHostRefusal> {
    let authority =
        authorize_prepared_build(profile, invocation, prepared, binding_plan, tool_identity)
            .map_err(NativeHostRefusal::Cargo)?;
    build_locked_offline(invocation, prepared, &authority).map_err(NativeHostRefusal::Cargo)?;
    Ok(authority.disclosure())
}

/// Enter the generated adapter through the admitted per-call capability gate.
pub fn dispatch_admitted_native<T>(
    profile: &TrustedNativeProfile,
    grant: &NativeExecutionGrant,
    required_effects: &[&str],
    adapter: impl FnOnce() -> T,
) -> Result<T, NativeHostRefusal> {
    profile
        .dispatch(grant, required_effects, adapter)
        .map_err(NativeHostRefusal::Dispatch)
}

/// Acquire a per-call grant without promoting an opaque metadata assertion.
pub fn grant_admitted_native(
    profile: &TrustedNativeProfile,
    capabilities: &[&str],
) -> Result<NativeExecutionGrant, NativeHostRefusal> {
    profile
        .grant(capabilities)
        .map_err(NativeHostRefusal::Trust)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semaprax_native_rust_interop::{NativeBuildPolicy, NativeEffectContract};
    use std::cell::Cell;

    #[test]
    fn host_renders_five_distinct_native_refusals_without_input_leakage() {
        let cases = [
            (
                NativeHostRefusal::Cargo(CargoExecutionError::UnsupportedApi),
                "SPX-B122",
                "unsupported",
            ),
            (
                NativeHostRefusal::Cargo(CargoExecutionError::MissingTool),
                "SPX-B125",
                "missing",
            ),
            (
                NativeHostRefusal::Dispatch(NativeDispatchError::MissingCapability),
                "SPX-B129",
                "capability",
            ),
            (
                NativeHostRefusal::Trust(NativeTrustError::OpaqueNativeBehavior),
                "SPX-B126",
                "opaque",
            ),
            (
                NativeHostRefusal::Cargo(CargoExecutionError::SandboxUnavailable),
                "SPX-B127",
                "no enforcing runner",
            ),
        ];
        for (refusal, code, message) in cases {
            let (human, exit) = refusal.render(false);
            assert_eq!(exit, 1);
            assert!(human.contains(code));
            assert!(human.contains(message));
            assert!(!human.contains("/private/"));
            let (json, exit) = refusal.render(true);
            assert_eq!(exit, 1);
            let value: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(value["code"], code);
            assert_eq!(value["path"], serde_json::Value::Null);
        }
    }

    #[test]
    fn host_dispatch_refuses_before_callback_entry() {
        let profile = TrustedNativeProfile::admit(
            b"plan",
            b"crate",
            b"tool",
            NativeEffectContract::Audited(&["fixture.read"]),
            NativeBuildPolicy::StrictDenyExecution,
        )
        .unwrap();
        let grant = grant_admitted_native(&profile, &[]).unwrap();
        let entries = Cell::new(0);
        let result = dispatch_admitted_native(&profile, &grant, &["fixture.read"], || {
            entries.set(entries.get() + 1);
        });
        assert_eq!(
            result,
            Err(NativeHostRefusal::Dispatch(
                NativeDispatchError::MissingCapability
            ))
        );
        assert_eq!(entries.get(), 0);
    }
}
