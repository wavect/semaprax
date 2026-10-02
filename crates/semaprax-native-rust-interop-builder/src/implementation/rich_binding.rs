//! Additive RI-01 bootstrap BindingPlan and generated Rust callback adapter.
//!
//! The plan is deliberately smaller than the existing v1 Spec.  It is an
//! authenticated compiler input for the checked fixture only; it neither
//! changes the v1 bridge nor grants build or foreign-call authority.

use super::*;

pub(super) const RICH_BINDING_PLAN_SCHEMA: &str = "semaprax.native-rust-rich-binding-plan.v1";
pub(super) const RICH_BINDING_PLAN_DOMAIN: &[u8] = b"semaprax.native-rust-rich-interop.plan.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RichBindingPlan {
    binding: RichBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RichBinding {
    semaprax_id: String,
    rust_path: String,
    rust_method: String,
}

/// Produces the checked-in RI-01 fixture plan.  Automatic package indexing is
/// intentionally a later concern; the input is still rendered canonically and
/// consumed by the adapter generator rather than duplicated in a test host.
pub(super) fn bootstrap_fixture_plan(rust_method: &str) -> Result<RichBindingPlan, Diagnostic> {
    if !is_rust_identifier(rust_method) {
        return Err(Diagnostic::io(
            "SPX-B117",
            "Native Rust rich BindingPlan contains an invalid generated import method",
        ));
    }
    Ok(RichBindingPlan {
        binding: RichBinding {
            semaprax_id: "host.add".to_owned(),
            rust_path: "fixture_math::add".to_owned(),
            rust_method: rust_method.to_owned(),
        },
    })
}

impl RichBindingPlan {
    pub(super) fn canonical(&self) -> String {
        format!(
            "{{\"schema\":\"{RICH_BINDING_PLAN_SCHEMA}\",\"bindings\":[{{\"semaprax_id\":\"{}\",\"rust_path\":\"{}\",\"receiver\":\"none\",\"arguments\":[{{\"type\":\"i64\",\"mode\":\"copy\"}},{{\"type\":\"i64\",\"mode\":\"copy\"}}],\"result\":{{\"type\":\"i64\",\"mode\":\"copy\"}},\"substitutions\":[],\"effects\":[],\"failure\":\"infallible\"}}],\"nonclaims\":[\"no_ambient_authority\",\"no_rust_abi\"]}}\n",
            self.binding.semaprax_id, self.binding.rust_path
        )
    }

    pub(super) fn digest(&self) -> String {
        domain_digest(RICH_BINDING_PLAN_DOMAIN, self.canonical().as_bytes())
    }

    /// Renders the sole bootstrap adapter.  Its trait implementation is the
    /// generated physical route from the C11-produced Semaprax bridge to an
    /// ordinary Rust function; it does not expose an FFI item from the fixture.
    pub(super) fn render_adapter(&self) -> String {
        format!(
            "struct GeneratedFixtureAdapter;\nimpl NativeRustImports for GeneratedFixtureAdapter{{fn {}(&mut self,left:i64,right:i64)->NativeRustImportResult<i64>{{NativeRustImportResult::Success({}(left,right))}}}}\n",
            self.binding.rust_method, self.binding.rust_path
        )
    }

    /// Refuses a descriptor that cannot be the plan's selected declaration
    /// before the generated adapter can be entered.
    pub(super) fn validate_descriptor(&self, descriptor: &str) -> Result<(), Diagnostic> {
        if descriptor.contains(&self.binding.semaprax_id) {
            Ok(())
        } else {
            Err(Diagnostic::io(
                "SPX-B120",
                "Native Rust rich descriptor disagrees with BindingPlan",
            ))
        }
    }

    /// The bootstrap is a native static profile only.  Other targets fail
    /// during admission, before any generated callback exists.
    pub(super) fn validate_target(&self, target: &str) -> Result<(), Diagnostic> {
        if target
            == current_target()
                .map(|target| target.triple)
                .as_deref()
                .unwrap_or("")
        {
            Ok(())
        } else {
            Err(Diagnostic::io(
                "SPX-B118",
                "Native Rust rich BindingPlan target is unsupported",
            ))
        }
    }

    pub(super) fn validate_signature(
        &self,
        parameters: &[ScalarType],
        result: ScalarType,
    ) -> Result<(), Diagnostic> {
        if parameters == [ScalarType::I64, ScalarType::I64] && result == ScalarType::I64 {
            Ok(())
        } else {
            Err(Diagnostic::io(
                "SPX-B118",
                "Native Rust rich BindingPlan signature is unsupported",
            ))
        }
    }
}

fn is_rust_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}
