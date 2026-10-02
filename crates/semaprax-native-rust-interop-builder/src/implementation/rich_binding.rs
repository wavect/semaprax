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
    bindings: Vec<RichBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RichBinding {
    semaprax_id: String,
    rust_path: String,
    rust_method: String,
    failure: RichFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RichFailure {
    Infallible,
    ImportStatus,
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
        bindings: vec![binding(
            "host.add",
            "fixture_math::add",
            rust_method,
            RichFailure::Infallible,
        )],
    })
}

/// Produces the complete fixture plan used by the physical rich round trip.
pub(super) fn bootstrap_rich_fixture_plan(
    add_method: &str,
    checked_div_method: &str,
) -> Result<RichBindingPlan, Diagnostic> {
    for method in [add_method, checked_div_method] {
        if !is_rust_identifier(method) {
            return Err(Diagnostic::io(
                "SPX-B117",
                "Native Rust rich BindingPlan contains an invalid generated import method",
            ));
        }
    }
    Ok(RichBindingPlan {
        bindings: vec![
            binding(
                "host.add",
                "fixture_math::add",
                add_method,
                RichFailure::Infallible,
            ),
            binding(
                "host.checked_div",
                "fixture_math::checked_div",
                checked_div_method,
                RichFailure::ImportStatus,
            ),
        ],
    })
}

impl RichBindingPlan {
    pub(super) fn canonical(&self) -> String {
        let mut canonical = format!("{{\"schema\":\"{RICH_BINDING_PLAN_SCHEMA}\",\"bindings\":[");
        for (index, binding) in self.bindings.iter().enumerate() {
            if index != 0 {
                canonical.push(',');
            }
            canonical.push_str("{\"semaprax_id\":\"");
            canonical.push_str(&binding.semaprax_id);
            canonical.push_str("\",\"rust_path\":\"");
            canonical.push_str(&binding.rust_path);
            canonical.push_str("\",\"receiver\":\"none\",\"arguments\":[{\"type\":\"i64\",\"mode\":\"copy\"},{\"type\":\"i64\",\"mode\":\"copy\"}],\"result\":{\"type\":\"i64\",\"mode\":\"copy\"},\"substitutions\":[],\"effects\":[\"host.math\"],\"failure\":\"");
            canonical.push_str(match binding.failure {
                RichFailure::Infallible => "infallible",
                RichFailure::ImportStatus => "status:fixture.math.v1",
            });
            canonical.push_str("\"}");
        }
        canonical.push_str("],\"nonclaims\":[\"no_ambient_authority\",\"no_rust_abi\"]}\n");
        canonical
    }

    pub(super) fn digest(&self) -> String {
        domain_digest(RICH_BINDING_PLAN_DOMAIN, self.canonical().as_bytes())
    }

    /// Renders the bootstrap adapter from every selected binding. Its trait
    /// implementation is the generated physical route from the C11-produced
    /// Semaprax bridge to ordinary Rust functions; it exposes no fixture FFI.
    pub(super) fn render_adapter(&self) -> String {
        let mut adapter = String::from(
            "struct GeneratedFixtureAdapter;\nimpl NativeRustImports for GeneratedFixtureAdapter{",
        );
        for binding in &self.bindings {
            adapter.push_str("fn ");
            adapter.push_str(&binding.rust_method);
            adapter.push_str("(&mut self,left:i64,right:i64)->NativeRustImportResult<i64>{");
            match binding.failure {
                RichFailure::Infallible => {
                    adapter.push_str("NativeRustImportResult::Success(");
                    adapter.push_str(&binding.rust_path);
                    adapter.push_str("(left,right))");
                }
                RichFailure::ImportStatus => {
                    adapter.push_str("match ");
                    adapter.push_str(&binding.rust_path);
                    adapter.push_str("(left,right){Ok(value)=>NativeRustImportResult::Success(value),Err(_)=>NativeRustImportResult::Status{code:core::num::NonZeroU32::new(7).unwrap(),class:NativeRustStatusClass::Import,retryable:false}}");
                }
            }
            adapter.push('}');
        }
        adapter.push_str("}\n");
        adapter
    }

    /// Refuses a descriptor that cannot be the plan's selected declaration
    /// before the generated adapter can be entered.
    pub(super) fn validate_descriptor(&self, descriptor: &str) -> Result<(), Diagnostic> {
        if self
            .bindings
            .iter()
            .all(|binding| descriptor.contains(&binding.semaprax_id))
        {
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

fn binding(
    semaprax_id: &str,
    rust_path: &str,
    rust_method: &str,
    failure: RichFailure,
) -> RichBinding {
    RichBinding {
        semaprax_id: semaprax_id.to_owned(),
        rust_path: rust_path.to_owned(),
        rust_method: rust_method.to_owned(),
        failure,
    }
}

fn is_rust_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}
