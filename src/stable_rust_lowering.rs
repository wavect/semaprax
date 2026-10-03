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
/// The only admitted non-inert plan transfers the whole owned parameter to a
/// temporary, then that same temporary to the provisional result, with no
/// finalizers. The generated `Option::take` occurs at the first action. This
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
            at: first_at,
            source: first_source,
            destination: first_destination,
        }, CleanupTransition::Transfer {
            at: second_at,
            source: second_source,
            destination: second_destination,
        }] => {
            matches!(&first_source.storage, StorageId::Value(value) if value == &parameter.id)
                && matches!(
                    (&first_destination.storage, &second_source.storage),
                    (StorageId::Temporary(first), StorageId::Temporary(second))
                        if first == second && first == first_at && second == second_at
                )
                && second_destination.storage == StorageId::ProvisionalResult
                && first_source.projections.is_empty()
                && first_destination.projections.is_empty()
                && second_source.projections.is_empty()
                && second_destination.projections.is_empty()
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
        "pub const SPX_CLEANUP_ACTIONS: &[&str] = &[\"Transfer(parameter -> temporary)\", \"Transfer(temporary -> provisional-result)\"];\n\
pub fn spx_entry(mut value: Option<Vec<u8>>, trace: &mut Vec<&'static str>) -> Vec<u8> {\n\
    trace.push(SPX_CLEANUP_ACTIONS[0]);\n\
    let temporary = value.take().expect(\"verified owned parameter is live at transfer\");\n\
    trace.push(SPX_CLEANUP_ACTIONS[1]);\n\
    let result = temporary;\n\
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
    use std::{
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn binding() -> StableRustToolchainBinding {
        StableRustToolchainBinding {
            target: "aarch64-apple-darwin".into(),
            rustc_commit: "88d9e12ae178fab0fb5cc050a94da85685d449ea".into(),
        }
    }

    const OWNED_IDENTITY_DIFFERENTIAL: &str = r#"module ri14.transfer;
@id("ri14.transfer.identity") fn identity(value: own Bytes) -> Bytes { value }
@id("ri14.transfer.main") fn main() -> i64 {
    let source = [0u8, 255u8, 7u8, 0u8];
    let source_view = array_as_slice(source);
    let owned = bytes_copy(source_view);
    let forwarded = identity(owned);
    let forwarded_view = bytes_as_slice(forwarded);
    match byte_get(forwarded_view, 1usize) {
        Option::Some { value: byte } => if byte == 255u8 { 42 } else { 0 },
        Option::None {} => 0,
    }
}
"#;

    fn temporary_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "semaprax-ri14-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn command_output(command: &mut Command, label: &str) -> Vec<u8> {
        let output = command.output().unwrap_or_else(|error| {
            panic!("{label} did not start: {error}");
        });
        assert!(
            output.status.success(),
            "{label} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
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
    fn lowers_verified_owned_bytes_identity_through_its_canonical_transfers() {
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
            .contains("Transfer(parameter -> temporary)"));
        assert!(artifact
            .source()
            .contains("Transfer(temporary -> provisional-result)"));
        assert!(artifact.source().contains("value.take()"));
        assert!(artifact
            .source()
            .contains("spx_lexical_drop_negative_control"));
        assert!(artifact.source().contains("lexical.second"));
        assert!(artifact.source().contains("lexical.first"));
    }

    #[test]
    fn generated_owned_identity_matches_interpreter_and_c11_when_explicitly_enabled() {
        let Some(rustc) = std::env::var_os("SEMAPRAX_RI14_RUSTC") else {
            eprintln!("RI-14 generated Rust differential disabled; set SEMAPRAX_RI14_RUSTC");
            return;
        };
        let rustc = PathBuf::from(rustc);
        assert!(rustc.is_absolute() && rustc.is_file());
        let clang = PathBuf::from(std::env::var_os("CLANG").expect(
            "RI-14 generated Rust differential requires CLANG when SEMAPRAX_RI14_RUSTC is set",
        ));
        assert!(clang.is_absolute() && clang.is_file());

        let verbose = String::from_utf8(command_output(
            Command::new(&rustc).arg("--version").arg("--verbose"),
            "bound rustc identity",
        ))
        .unwrap();
        let expected = binding();
        assert!(verbose.contains(&format!("commit-hash: {}", expected.rustc_commit)));
        assert!(verbose.contains(&format!("host: {}", expected.target)));

        let parsed = crate::parse(
            OWNED_IDENTITY_DIFFERENTIAL,
            Path::new("ri14-differential.spx"),
        )
        .unwrap();
        assert!(crate::verify::verify(&parsed).is_empty());
        let resolved = hir::resolve(&parsed).unwrap();
        let artifact = lower_noninert_cleanup_plan(
            &resolved,
            &DeclarationId::new("ri14.transfer.identity"),
            &expected,
        )
        .unwrap();

        let root = temporary_root("owned-identity");
        std::fs::create_dir(&root).unwrap();
        let generated = root.join("generated.rs");
        let driver = root.join("main.rs");
        let rust_binary = root.join(format!("generated{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&generated, artifact.source()).unwrap();
        std::fs::write(
            &driver,
            r#"include!("generated.rs");
fn main() {
    let mut trace = Vec::new();
    let value = spx_entry(Some(vec![0, 255, 7, 0]), &mut trace);
    assert_eq!(value, vec![0, 255, 7, 0]);
    assert_eq!(trace.as_slice(), SPX_CLEANUP_ACTIONS);
    assert_eq!(spx_lexical_drop_negative_control(), vec!["lexical.second", "lexical.first"]);
    println!("{value:?}|{trace:?}");
}
"#,
        )
        .unwrap();
        command_output(
            Command::new(&rustc)
                .arg("--edition=2021")
                .arg(&driver)
                .arg("-o")
                .arg(&rust_binary),
            "generated stable Rust compilation",
        );
        assert_eq!(
            command_output(
                &mut Command::new(&rust_binary),
                "generated stable Rust execution"
            ),
            b"[0, 255, 7, 0]|[\"Transfer(parameter -> temporary)\", \"Transfer(temporary -> provisional-result)\"]\n"
        );

        let source_path = root.join("identity.spx");
        std::fs::write(&source_path, OWNED_IDENTITY_DIFFERENTIAL).unwrap();
        let interpreted = crate::interpreter::interpret(
            &source_path,
            "ri14.transfer.main",
            &[],
            &crate::interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(interpreted.returned);
        let envelope: serde_json::Value = serde_json::from_str(&interpreted.envelope).unwrap();
        assert_eq!(envelope["payload"]["outcome"]["value"], "42");

        let c_source = root.join("identity.c");
        let c_binary = root.join(format!("identity{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&c_source, crate::codegen::emit_c(&parsed).unwrap()).unwrap();
        assert_eq!(
            command_output(
                Command::new(&clang)
                    .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
                    .arg(&c_source)
                    .arg("-o")
                    .arg(&c_binary),
                "C11 identity compilation",
            ),
            b""
        );
        assert_eq!(
            command_output(&mut Command::new(&c_binary), "C11 identity execution"),
            b"42\n"
        );
        std::fs::remove_dir_all(root).unwrap();
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
