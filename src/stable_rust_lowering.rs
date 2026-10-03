//! RI-14's deliberately tiny, nondefault stable-Rust lowering seam.
//!
//! It consumes only validated HIR and its attached canonical cleanup plan.
//! The admitted island is one parameter-free `i64` literal function with an
//! inert ownership plan.  Every other shape is rejected before source exists.

use crate::hir::{
    self, DeclarationId, ResolvedExprKind, ResolvedFunction, ResolvedProgram, ResolvedType,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableRustArtifact {
    source: String,
    function: DeclarationId,
    cleanup_schema: &'static str,
}

impl StableRustArtifact {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn function(&self) -> &DeclarationId {
        &self.function
    }

    pub fn cleanup_schema(&self) -> &'static str {
        self.cleanup_schema
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableRustLoweringError {
    InvalidHir,
    FunctionMissing,
    UnsupportedSignature,
    UnsupportedContractsOrEffects,
    UnsupportedOwnershipPlan,
    UnsupportedExpression,
}

/// Render stable Rust for one explicitly admitted HIR island.
///
/// This is not selected by the normal compiler and has no publication or
/// execution authority.  Validation precedes all inspection so a forged HIR
/// or cleanup plan cannot be lowered as if it were compiler-derived.
pub fn lower_i64_literal(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
) -> Result<StableRustArtifact, StableRustLoweringError> {
    hir::validate(program).map_err(|_| StableRustLoweringError::InvalidHir)?;
    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.id == *function_id)
        .ok_or(StableRustLoweringError::FunctionMissing)?;
    if !function.params.is_empty() || function.return_type != ResolvedType::I64 {
        return Err(StableRustLoweringError::UnsupportedSignature);
    }
    if !function.requires.is_empty()
        || !function.ensures.is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
    {
        return Err(StableRustLoweringError::UnsupportedContractsOrEffects);
    }
    if !inert_cleanup_plan(function) {
        return Err(StableRustLoweringError::UnsupportedOwnershipPlan);
    }
    let value = match &function.body.kind {
        ResolvedExprKind::Int(value) => value,
        ResolvedExprKind::Block { statements, tail } if statements.is_empty() => match &tail.kind {
            ResolvedExprKind::Int(value) => value,
            _ => return Err(StableRustLoweringError::UnsupportedExpression),
        },
        _ => return Err(StableRustLoweringError::UnsupportedExpression),
    };
    Ok(StableRustArtifact {
        source: format!(
            "// RI-14 validated HIR function: {}\n// cleanup-plan schema: {}\npub fn spx_entry() -> i64 {{ {value} }}\n",
            function.id.as_str(), function.cleanup_plan.schema
        ),
        function: function.id.clone(),
        cleanup_schema: function.cleanup_plan.schema,
    })
}

fn inert_cleanup_plan(function: &ResolvedFunction) -> bool {
    function.cleanup.slots.is_empty()
        && function
            .cleanup
            .entry_state
            .live_owned_parameters
            .is_empty()
        && function
            .cleanup
            .entry_state
            .conditional_owned_parameters
            .is_empty()
        && function.cleanup_plan.slots.is_empty()
        && function
            .cleanup_plan
            .entry_state
            .live_owned_parameters
            .is_empty()
        && function
            .cleanup_plan
            .entry_state
            .conditional_owned_parameters
            .is_empty()
        && function
            .cleanup_plan
            .blocks
            .iter()
            .all(|block| block.transitions.is_empty())
        && function
            .cleanup_plan
            .exits
            .iter()
            .all(|exit| exit.finalize_in_order.is_empty())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn lowers_a_validated_literal_and_retains_its_cleanup_schema() {
        let source = "module ri14.literal;\n@id(\"ri14.literal.answer\") fn answer() -> i64 { 42 }\n@id(\"ri14.literal.main\") fn main() -> i64 { 0 }\n";
        let parsed = crate::parse(source, Path::new("ri14-literal.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        let artifact =
            lower_i64_literal(&resolved, &DeclarationId::new("ri14.literal.answer")).unwrap();
        assert_eq!(artifact.function().as_str(), "ri14.literal.answer");
        assert!(artifact
            .source()
            .contains("pub fn spx_entry() -> i64 { 42 }"));
        assert!(artifact.source().contains(artifact.cleanup_schema()));
    }

    #[test]
    fn rejects_an_actual_hir_expression_outside_the_literal_island() {
        let source = "module ri14.reject;\n@id(\"ri14.reject.answer\") fn answer() -> i64 { 40 + 2 }\n@id(\"ri14.reject.main\") fn main() -> i64 { 0 }\n";
        let parsed = crate::parse(source, Path::new("ri14-reject.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        assert_eq!(
            lower_i64_literal(&resolved, &DeclarationId::new("ri14.reject.answer")),
            Err(StableRustLoweringError::UnsupportedExpression)
        );
    }
}
