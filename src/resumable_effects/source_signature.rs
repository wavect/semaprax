//! Derivation of the runtime resumable-effect signature from checked `.spx`
//! source.
//!
//! [`super::signature::EffectSignatureTable`] is intentionally useful for
//! arbitrary Rust-level resumable programs and therefore accepts caller-owned
//! shape strings. Source `yield` must not trust such strings: this module
//! reuses the compiler-owned resumable lowering, then derives the effect id and
//! both scalar shapes from the checked HIR. The resulting binding commits to
//! the lowering's exact checked-program identity and can be reverified after a
//! source or dependency change without dispatching an effect.
//!
//! This remains proof data, not authority. Deriving or verifying a signature
//! performs no effect, creates no handler, and grants no permission to resume.

use super::lowering;
use super::signature::{EffectSignature, EffectSignatureTable, EffectTag};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedFunction, ResolvedProgram, ResolvedType};

const INVALID_SOURCE_SIGNATURE: &str = "SPX-H006";

/// Versioned prefix for the exact checked scalar type identity used by source
/// resumable signatures. The suffix is [`ResolvedType::identity_key`].
pub const SOURCE_TYPE_SHAPE_PREFIX: &str = "semaprax.resolved-type.v1:";

/// One checked source function's effect signature and its exact lowering
/// identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEffectSignature {
    function_id: String,
    request_shape: String,
    answer_shape: String,
    plan_identity: [u8; 32],
    yield_count: u32,
    control_dependent: bool,
    carries_owned_bytes: bool,
    aggregate_channel: bool,
    aggregate_bytes_channel: bool,
    table: EffectSignatureTable,
}

impl SourceEffectSignature {
    pub fn function_id(&self) -> &str {
        &self.function_id
    }

    pub fn request_shape(&self) -> &str {
        &self.request_shape
    }

    pub fn answer_shape(&self) -> &str {
        &self.answer_shape
    }

    pub fn plan_identity(&self) -> &[u8; 32] {
        &self.plan_identity
    }

    /// Static yield-site count of the selected plan.
    pub fn yield_count(&self) -> u32 {
        self.yield_count
    }

    /// True when the plan is the control-dependent (v3 or v4) plan rather
    /// than the direct sequential one; the identities never coincide.
    pub fn is_control_dependent(&self) -> bool {
        self.control_dependent
    }

    /// True when the plan carries an owned `Bytes` local across some site
    /// (issue #296, spec section 11.6): fixes the checkpoint envelope to the
    /// v4 schema and the plan/binding identity to the v4 domain, rather than
    /// v3. Always `false` for a sequential plan.
    pub fn carries_owned_bytes(&self) -> bool {
        self.carries_owned_bytes
    }

    /// True when the request or response type is a bounded record/variant
    /// of Copy scalars (issue #296 R20) rather than a bare Copy scalar.
    /// Admitted only for the direct top-level (sequential) placement, so
    /// this is always `false` when [`Self::is_control_dependent`] is `true`.
    pub fn is_aggregate_channel(&self) -> bool {
        self.aggregate_channel
    }

    pub fn has_aggregate_bytes(&self) -> bool {
        self.aggregate_bytes_channel
    }

    pub fn table(&self) -> &EffectSignatureTable {
        &self.table
    }

    /// Tag a request for checking at an injected handler boundary.
    pub fn request_tag(&self) -> EffectTag {
        EffectTag::new(&self.function_id, &self.request_shape)
    }

    /// Tag an answer for checking before it is admitted as the suspension's
    /// observation.
    pub fn answer_tag(&self) -> EffectTag {
        EffectTag::new(&self.function_id, &self.answer_shape)
    }

    /// Re-derive every field from the current checked program. This is a pure
    /// equality check and never repairs a stale binding.
    pub fn verify(&self, program: &ResolvedProgram) -> Result<(), Diagnostic> {
        let observed = derive_source_effect_signature(program, &self.function_id)?;
        if observed != *self {
            return Err(invalid(
                "source effect signature is stale for the checked program",
            ));
        }
        Ok(())
    }
}

/// Derive the runtime signature for one checked source function.
///
/// The same lowering that prepares source continuations rechecks the admitted
/// Copy-scalar profile, persistent function identity, direct yield sites,
/// ownership/cleanup state, and reachable call closure. Consequently this
/// bridge cannot manufacture a signature for source that the resumable
/// lowering itself refuses.
pub fn derive_source_effect_signature(
    program: &ResolvedProgram,
    function_id: &str,
) -> Result<SourceEffectSignature, Diagnostic> {
    let function = selected_function(program, function_id)?;
    let control_dependent = lowering::control::is_control_dependent(function);
    let (plan_identity, site_count, carries_owned_bytes) = if control_dependent {
        let plan = lowering::control::lower_control(program, function)?;
        (
            *plan.identity.as_bytes(),
            plan.sites.len(),
            plan.carries_owned_bytes,
        )
    } else {
        let plan = lowering::lower_sequential(program, function)?;
        (*plan.identity.as_bytes(), plan.suspensions.len(), false)
    };
    let yields = function
        .yields
        .as_ref()
        .ok_or_else(|| invalid("selected function has no `yields` clause"))?;
    if crate::hir::yield_aggregate::has_bytes_leaf(&program.declarations, &yields.response_type) {
        return Err(invalid(
            "source signature does not admit a Bytes response channel",
        ));
    }
    let request_shape = source_shape(&program.declarations, &yields.request_type)?;
    let answer_shape = source_shape(&program.declarations, &yields.response_type)?;
    let effect = EffectSignature::new(function.id.as_str(), &request_shape, &answer_shape);
    let table = EffectSignatureTable::new(vec![effect]).map_err(|error| {
        invalid(format!(
            "compiler-derived effect signature table was invalid: {error:?}"
        ))
    })?;
    let yield_count = u32::try_from(site_count)
        .map_err(|_| invalid("resumable yield count does not fit its public field"))?;
    let aggregate_channel = matches!(yields.request_type, ResolvedType::Nominal { .. })
        || matches!(yields.response_type, ResolvedType::Nominal { .. });
    let aggregate_bytes_channel = aggregate_has_bytes(&program.declarations, &yields.request_type)
        || aggregate_has_bytes(&program.declarations, &yields.response_type);
    Ok(SourceEffectSignature {
        function_id: function.id.as_str().to_owned(),
        request_shape,
        answer_shape,
        plan_identity,
        yield_count,
        control_dependent,
        carries_owned_bytes,
        aggregate_channel,
        aggregate_bytes_channel,
        table,
    })
}

fn aggregate_has_bytes(declarations: &crate::hir::DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if !arguments.is_empty() {
        return false;
    }
    declarations
        .record_fields(declaration)
        .is_some_and(|fields| fields.iter().any(|field| field.ty == ResolvedType::Bytes))
        || declarations
            .variant_cases(declaration)
            .is_some_and(|cases| {
                cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .any(|field| field.ty == ResolvedType::Bytes)
            })
}

fn selected_function<'a>(
    program: &'a ResolvedProgram,
    function_id: &str,
) -> Result<&'a ResolvedFunction, Diagnostic> {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == function_id)
        .ok_or_else(|| invalid(format!("resumable function `{function_id}` was not found")))
}

fn source_shape(
    declarations: &crate::hir::DeclarationIndex,
    ty: &ResolvedType,
) -> Result<String, Diagnostic> {
    // Issue #296 R20: a bounded record/variant of Copy scalars
    // (`hir::yield_aggregate::bounded_aggregate_refusal`) is an admitted
    // shape alongside a bare Copy scalar; `hir::resolve_yield` already
    // checked it for the direct top-level (sequential) placement this
    // derivation reuses. The `identity_key()` shape string already
    // distinguishes any two distinct nominal types, so no separate encoding
    // is needed for the aggregate case.
    let is_bounded_aggregate = matches!(ty, ResolvedType::Nominal { .. })
        && crate::hir::yield_aggregate::bounded_aggregate_refusal(declarations, ty).is_ok();
    if !crate::hir::is_scalar_resolved_type(ty) && !is_bounded_aggregate {
        return Err(invalid(format!(
            "source effect signature type `{}` is outside the admitted Copy-scalar or bounded \
             aggregate profile",
            ty.identity_key()
        )));
    }
    Ok(format!("{SOURCE_TYPE_SHAPE_PREFIX}{}", ty.identity_key()))
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        INVALID_SOURCE_SIGNATURE,
        format!("invalid source resumable signature: {}", message.into()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
module test.source_signature;
@id("app.ask")
fn ask(seed: i64) -> bool yields i64 -> bool {
    let first = yield seed + 1;
    let second = yield seed + 2;
    first && second
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program(source: &str) -> ResolvedProgram {
        let parsed = crate::parse(source, "source-signature.spx").unwrap();
        crate::hir::resolve(&parsed).unwrap()
    }

    #[test]
    fn checked_source_derives_the_exact_effect_id_shapes_and_site_count() {
        let program = program(SOURCE);
        let signature = derive_source_effect_signature(&program, "app.ask").unwrap();
        assert_eq!(signature.function_id(), "app.ask");
        assert_eq!(signature.request_shape(), "semaprax.resolved-type.v1:i64");
        assert_eq!(signature.answer_shape(), "semaprax.resolved-type.v1:bool");
        assert_eq!(signature.yield_count(), 2);
        assert_eq!(signature.table().signatures().len(), 1);
        assert!(signature
            .table()
            .check_request(&signature.request_tag())
            .is_ok());
        assert!(signature
            .table()
            .check_answer(&signature.request_tag(), &signature.answer_tag())
            .is_ok());
        signature.verify(&program).unwrap();
    }

    #[test]
    fn response_type_or_checked_program_drift_stales_the_binding() {
        let original = program(SOURCE);
        let signature = derive_source_effect_signature(&original, "app.ask").unwrap();
        let changed = program(
            &SOURCE
                .replace(
                    "fn ask(seed: i64) -> bool yields i64 -> bool {",
                    "fn ask(seed: i64) -> i64 yields i64 -> i64 {",
                )
                .replace("first && second", "first + second"),
        );
        let observed = derive_source_effect_signature(&changed, "app.ask").unwrap();
        assert_eq!(observed.answer_shape(), "semaprax.resolved-type.v1:i64");
        assert_ne!(signature.plan_identity(), observed.plan_identity());
        assert_eq!(signature.verify(&changed).unwrap_err().code, "SPX-H006");
    }

    #[test]
    fn ordinary_or_unknown_functions_cannot_manufacture_source_signatures() {
        let program = program(SOURCE);
        assert_eq!(
            derive_source_effect_signature(&program, "app.main")
                .unwrap_err()
                .code,
            "SPX-H006"
        );
        assert_eq!(
            derive_source_effect_signature(&program, "app.missing")
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
}
