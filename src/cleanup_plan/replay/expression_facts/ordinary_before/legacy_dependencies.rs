//! Historical Tree helpers, independent of the current Arena payer routes.
use super::*;
pub(super) mod ordinary_inventory {
    use super::*;
    pub(in crate::cleanup_plan::replay::expression_facts::ordinary_before) fn validate_inventory_coverage(
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        _budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        crate::cleanup_plan::replay::ordinary_inventory::owned::validate_inventory_coverage(
            program, function,
        )
    }
}
pub(super) fn validate_typed_control_skeleton_with_index<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    mut continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<(), Diagnostic> {
    if function.cleanup_plan.lexical_control.is_none()
        && factored::validate_with_index_storage(
            program,
            function,
            budget,
            helpers,
            continuations.as_deref_mut(),
        )?
    {
        return Ok(());
    }
    let mut expected =
        hir_skeleton_paths_with_index_storage(program, function, budget, helpers, continuations)?;
    // This saved receiver uses the original flat Vec ordering. Keep the
    // historical producer selected only for PLAN construction.
    let mut actual = ordinary_plan_history::completed_fork::emission::with_before(|| {
        plan_skeleton_paths(function, budget)
    })?;
    expected.sort();
    actual.sort();
    if actual != expected {
        if std::env::var_os("SPX_PROBE_SKELETON").is_some() {
            for (index, path) in expected.iter().enumerate() {
                eprintln!("EXP[{index}] {path:?}");
            }
            for (index, path) in actual.iter().enumerate() {
                eprintln!("ACT[{index}] {path:?}");
            }
        }
        return Err(replay_error(
            function,
            "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
        ));
    }
    Ok(())
}

#[test]
fn tree_control_helper_keeps_the_complete_original_body() {
    let current_parent = include_str!("../ordinary_before.rs");
    let physical = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/cleanup_plan/replay/expression_facts/ordinary_before.rs"),
    )
    .unwrap();
    assert_eq!(physical, current_parent);
    let hook = "mod legacy_dependencies;\nuse legacy_dependencies::{ordinary_inventory, validate_typed_control_skeleton_with_index};\n";
    assert_eq!(current_parent.matches(hook).count(), 1);
    assert_eq!(
        current_parent.replacen(hook, "", 1),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/51f538872b19ed820193c426e4407436a74b7b69f07b95ef0697ff213af66691"
        ))
    );
    use sha2::{Digest, Sha256};
    let original = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/main09013-source-namespaces-v1/main21-current-checkpoints/src/cleanup_plan/replay.rs"));
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "6db5b0ed361258023e3d7cad058c3de6d737b416b67e98e7113ecb0a993c86c1"
    );
    let complete = &original[109388..110771];
    let current = include_str!("legacy_dependencies.rs");
    let start = current
        .find("pub(super) fn validate_typed_control_skeleton_with_index")
        .unwrap();
    let end = current[start..].find("\n}\n").unwrap() + start + 3;
    let current = &current[start..end];
    let transport = concat!(
        "    // This saved receiver uses the original flat Vec ordering. Keep the\n",
        "    // historical producer selected only for PLAN construction.\n",
        "    let mut actual = ordinary_plan_history::completed_fork::emission::with_before(|| {\n",
        "        plan_skeleton_paths(function, budget)\n",
        "    })?;",
    );
    assert_eq!(current.matches(transport).count(), 1);
    assert_eq!(
        current.replacen(
            transport,
            "    let mut actual = plan_skeleton_paths(function, budget)?;",
            1
        ),
        format!("pub(super) {complete}")
    );
}
