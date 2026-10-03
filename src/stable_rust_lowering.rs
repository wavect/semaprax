//! RI-14's deliberately tiny, nondefault stable-Rust lowering seam.
//!
//! It consumes only validated HIR and its attached canonical cleanup plan.
//! The admitted islands are one parameter-free `i64` literal with an inert
//! plan and one whole-value owned-`Bytes` transfer. Every other shape is
//! rejected before source exists.

use crate::{
    cleanup_plan::{CleanupTransition, StorageId},
    hir::{
        self, DeclarationId, OwnershipMode, ResolvedExprKind, ResolvedFunction, ResolvedProgram,
        ResolvedType,
    },
};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableRustArtifact {
    source: String,
    function: DeclarationId,
    cleanup_schema: &'static str,
    digest: String,
    target: String,
    rustc_commit: String,
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

    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn rustc_commit(&self) -> &str {
        &self.rustc_commit
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableRustToolchainBinding {
    pub target: String,
    pub rustc_commit: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableRustLoweringError {
    InvalidHir,
    FunctionMissing,
    UnsupportedSignature,
    UnsupportedContractsOrEffects,
    UnsupportedOwnershipPlan,
    UnsupportedExpression,
    InvalidToolchainBinding,
    InertCleanupPlan,
}

/// Render stable Rust for one explicitly admitted HIR island.
///
/// This is not selected by the normal compiler and has no publication or
/// execution authority.  Validation precedes all inspection so a forged HIR
/// or cleanup plan cannot be lowered as if it were compiler-derived.
pub fn lower_i64_literal(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    binding: &StableRustToolchainBinding,
) -> Result<StableRustArtifact, StableRustLoweringError> {
    validate_binding(binding)?;
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
    let value = match &function.body.kind {
        ResolvedExprKind::Int(value) => value,
        ResolvedExprKind::Block { statements, tail } if statements.is_empty() => match &tail.kind {
            ResolvedExprKind::Int(value) => value,
            _ => return Err(StableRustLoweringError::UnsupportedExpression),
        },
        _ => return Err(StableRustLoweringError::UnsupportedExpression),
    };
    if !inert_cleanup_plan(function) {
        return Err(StableRustLoweringError::UnsupportedOwnershipPlan);
    }
    let source = format!(
            "// RI-14 validated HIR function: {}\n// cleanup-plan schema: {}\npub fn spx_entry() -> i64 {{ {value} }}\n",
            function.id.as_str(), function.cleanup_plan.schema
        );
    Ok(artifact(source, function, binding))
}

/// Lower one verified owned-`Bytes` return through its canonical transfer.
///
/// The only admitted non-inert plan has exactly one `Transfer`, from the
/// whole owned parameter to the whole provisional result, and no finalizers.
/// The generated `Option::take` is deliberately placed at that action. This
/// is a physical move in generated stable Rust, rather than a reconstruction
/// from lexical `Drop`. Every other cleanup plan remains refused.
pub fn lower_noninert_cleanup_plan(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    binding: &StableRustToolchainBinding,
) -> Result<StableRustArtifact, StableRustLoweringError> {
    validate_binding(binding)?;
    hir::validate(program).map_err(|_| StableRustLoweringError::InvalidHir)?;
    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.id == *function_id)
        .ok_or(StableRustLoweringError::FunctionMissing)?;
    if function.params.len() != 1
        || function.params[0].ownership != OwnershipMode::Own
        || function.params[0].ty != ResolvedType::Bytes
        || function.return_type != ResolvedType::Bytes
    {
        return Err(StableRustLoweringError::UnsupportedSignature);
    }
    if !function.requires.is_empty()
        || !function.ensures.is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
    {
        return Err(StableRustLoweringError::UnsupportedContractsOrEffects);
    }
    let returned = match &function.body.kind {
        ResolvedExprKind::Place(place) => place,
        ResolvedExprKind::Block { statements, tail } if statements.is_empty() => match &tail.kind {
            ResolvedExprKind::Place(place) => place,
            _ => return Err(StableRustLoweringError::UnsupportedExpression),
        },
        _ => return Err(StableRustLoweringError::UnsupportedExpression),
    };
    if returned.root != function.params[0].id || !returned.projections.is_empty() {
        return Err(StableRustLoweringError::UnsupportedExpression);
    }
    let transitions = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| block.transitions.iter())
        .collect::<Vec<_>>();
    let has_finalizer = function
        .cleanup_plan
        .exits
        .iter()
        .any(|exit| !exit.finalize_in_order.is_empty());
    if transitions.is_empty() && !has_finalizer {
        return Err(StableRustLoweringError::InertCleanupPlan);
    }
    let parameter = &function.params[0];
    let admitted_transfer = match transitions.as_slice() {
        [CleanupTransition::Transfer {
            source,
            destination,
            ..
        }] => {
            matches!(
                (&source.storage, &destination.storage),
                (StorageId::Value(value), StorageId::ProvisionalResult)
                    if value == &parameter.id
            ) && source.projections.is_empty()
                && destination.projections.is_empty()
        }
        _ => false,
    };
    if !admitted_transfer || has_finalizer {
        return Err(StableRustLoweringError::UnsupportedOwnershipPlan);
    }
    let mut source = format!(
        "// RI-14 validated HIR function: {}\n// cleanup-plan schema: {}\n",
        function.id.as_str(),
        function.cleanup_plan.schema
    );
    source.push_str(
        "pub const SPX_CLEANUP_ACTIONS: &[&str] = &[\"Transfer(parameter -> provisional-result)\"];\n\
pub fn spx_entry(mut value: Option<Vec<u8>>, trace: &mut Vec<&'static str>) -> Vec<u8> {\n\
    trace.push(SPX_CLEANUP_ACTIONS[0]);\n\
    let result = value.take().expect(\"verified owned parameter is live at transfer\");\n\
    result\n\
}\n\n\
pub fn spx_lexical_drop_negative_control() -> Vec<&'static str> {\n\
    use std::{cell::RefCell, rc::Rc};\n\
    struct LexicalDrop(Rc<RefCell<Vec<&'static str>>>, &'static str);\n\
    impl Drop for LexicalDrop {\n\
        fn drop(&mut self) { self.0.borrow_mut().push(self.1); }\n\
    }\n\
    let trace = Rc::new(RefCell::new(Vec::new()));\n\
    {\n\
        let _first = LexicalDrop(trace.clone(), \"lexical.first\");\n\
        let _second = LexicalDrop(trace.clone(), \"lexical.second\");\n\
    }\n\
    Rc::try_unwrap(trace).expect(\"lexical controls released\").into_inner()\n\
}\n",
    );
    Ok(artifact(source, function, binding))
}

fn artifact(
    source: String,
    function: &ResolvedFunction,
    binding: &StableRustToolchainBinding,
) -> StableRustArtifact {
    let digest = format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    );
    StableRustArtifact {
        source,
        function: function.id.clone(),
        cleanup_schema: function.cleanup_plan.schema,
        digest,
        target: binding.target.clone(),
        rustc_commit: binding.rustc_commit.clone(),
    }
}

fn validate_binding(binding: &StableRustToolchainBinding) -> Result<(), StableRustLoweringError> {
    if binding.target.is_empty()
        || binding.rustc_commit.len() != 40
        || !binding
            .rustc_commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(StableRustLoweringError::InvalidToolchainBinding);
    }
    Ok(())
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

    fn binding() -> StableRustToolchainBinding {
        StableRustToolchainBinding {
            target: "aarch64-apple-darwin".into(),
            rustc_commit: "88d9e12ae178fab0fb5cc050a94da85685d449ea".into(),
        }
    }

    #[test]
    fn lowers_a_validated_literal_and_retains_its_cleanup_schema() {
        let source = "module ri14.literal;\n@id(\"ri14.literal.answer\") fn answer() -> i64 { 42 }\n@id(\"ri14.literal.main\") fn main() -> i64 { 0 }\n";
        let parsed = crate::parse(source, Path::new("ri14-literal.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        let artifact = lower_i64_literal(
            &resolved,
            &DeclarationId::new("ri14.literal.answer"),
            &binding(),
        )
        .unwrap();
        assert_eq!(artifact.function().as_str(), "ri14.literal.answer");
        assert!(artifact
            .source()
            .contains("pub fn spx_entry() -> i64 { 42 }"));
        assert!(artifact.source().contains(artifact.cleanup_schema()));
        assert!(artifact.digest().starts_with("sha256:"));
    }

    #[test]
    fn rejects_an_actual_hir_expression_outside_the_literal_island() {
        let source = "module ri14.reject;\n@id(\"ri14.reject.answer\") fn answer() -> i64 { 40 + 2 }\n@id(\"ri14.reject.main\") fn main() -> i64 { 0 }\n";
        let parsed = crate::parse(source, Path::new("ri14-reject.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        assert_eq!(
            lower_i64_literal(
                &resolved,
                &DeclarationId::new("ri14.reject.answer"),
                &binding()
            ),
            Err(StableRustLoweringError::UnsupportedExpression)
        );
    }

    #[test]
    fn lowers_verified_owned_bytes_identity_through_its_single_transfer() {
        let source = "module ri14.transfer;\n@id(\"ri14.transfer.identity\") fn identity(value: own Bytes) -> Bytes { value }\n@id(\"ri14.transfer.main\") fn main() -> i64 { 0 }\n";
        let parsed = crate::parse(source, Path::new("ri14-transfer.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        let artifact = lower_noninert_cleanup_plan(
            &resolved,
            &DeclarationId::new("ri14.transfer.identity"),
            &binding(),
        )
        .unwrap();
        assert!(artifact
            .source()
            .contains("Transfer(parameter -> provisional-result)"));
        assert!(artifact.source().contains("value.take()"));
        assert!(artifact
            .source()
            .contains("spx_lexical_drop_negative_control"));
        assert!(artifact.source().contains("lexical.second"));
        assert!(artifact.source().contains("lexical.first"));
    }

    #[test]
    fn rejects_empty_target_and_noncanonical_compiler_commit_before_hir_use() {
        let invalid = StableRustToolchainBinding {
            target: String::new(),
            rustc_commit: "ABC".into(),
        };
        let source = "module ri14.binding;\n@id(\"ri14.binding.answer\") fn answer() -> i64 { 42 }\n@id(\"ri14.binding.main\") fn main() -> i64 { 0 }\n";
        let parsed = crate::parse(source, Path::new("ri14-binding.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        assert_eq!(
            lower_i64_literal(
                &resolved,
                &DeclarationId::new("ri14.binding.answer"),
                &invalid
            ),
            Err(StableRustLoweringError::InvalidToolchainBinding)
        );
    }
}
