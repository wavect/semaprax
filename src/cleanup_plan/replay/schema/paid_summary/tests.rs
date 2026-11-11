use super::*;

#[test]
fn complete_previous_paid_schema_source_and_executable_are_bound() {
    use crate::source_reflows_merge::main_e177_final_inputs::declared_history_inputs as history;
    const PATH: &str = "src/cleanup_plan/replay/schema/paid_summary.rs";
    const SHA: &str = "dcdc3881d12701c95ce3fc353b4a063d12fcec3de665c16014a2890585c66440";
    let old = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/dcdc3881d12701c95ce3fc353b4a063d12fcec3de665c16014a2890585c66440"
    ));
    assert_eq!(
        history::expected_value(PATH, SHA, include_str!("../paid_summary.rs")).as_deref(),
        Some(old)
    );
    let retained = include_str!("before_selectors.rs")
        .strip_suffix('\n')
        .unwrap();
    assert_eq!(
        old.strip_suffix("\n#[cfg(test)]\nmod original_receiver;\n#[cfg(test)]\nmod tests;\n"),
        Some(retained),
    );
}

#[test]
fn complete_predecessor_and_live_selector_keep_the_same_schema() {
    for program in paid_type_facts::tests::fixtures() {
        for function in &program.functions {
            let mut tampered = function.clone();
            tampered.cleanup_plan.schema = "forged.schema";
            for subject in [function, &tampered] {
                let expected = crate::bounded_output::with_limit_usage(usize::MAX, || {
                    let mut budget = ReplayBudget::new();
                    before_selectors::selected_with_budget(&program, subject, None, &mut budget)
                });
                let current = crate::bounded_output::with_limit_usage(usize::MAX, || {
                    let mut budget = ReplayBudget::new();
                    selected_with_budget(&program, subject, None, &mut budget)
                });
                assert_eq!((current.0, current.1), (expected.0, expected.1));
            }
        }
    }
}

#[test]
fn complete_schema_original_and_receiving_body_remain_byte_exact() {
    let restored_schema = super::super::ordinary_tests::before_summary(ci_str!("../../schema.rs"));
    let root = restored_schema.as_str();
    assert_eq!(
        root.strip_suffix("\npub(super) mod paid_summary;\n")
            .unwrap(),
        ci_str!("../../../../../docs/evidence/schema-summary/schema.rs.before")
    );
    let restored =
        crate::cleanup_plan::replay::expression_facts::receiving::before_status_retention(
            "src/cleanup_plan/replay.rs",
            ci_str!("../../../replay.rs"),
        );
    let mut restored = restored;
    assert_eq!(restored.matches("    expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;").count(), 1);
    restored = restored.replacen(
        "    expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;",
        "    validate_entry(function, &storage, &leaves)?;",
        1,
    );
    assert_eq!(
        restored
            .matches(
                "    expression_facts::receiving::validate_regions(function, &storage, budget)?;"
            )
            .count(),
        1
    );
    restored = restored.replacen(
        "    expression_facts::receiving::validate_regions(function, &storage, budget)?;",
        "    validate_regions(function, &storage)?;",
        1,
    );
    assert_eq!(restored.matches("    expression_facts::receiving::validate_exits(\n        program,\n        function,\n        &storage,\n        &leaves,\n        &BTreeSet::new(),\n        budget,\n    )?;").count(), 1);
    restored = restored.replacen("    expression_facts::receiving::validate_exits(\n        program,\n        function,\n        &storage,\n        &leaves,\n        &BTreeSet::new(),\n        budget,\n    )?;", "    validate_exits(program, function, &storage, &leaves, &BTreeSet::new())?;", 1);
    let current = restored.as_str();
    let start = current
        .find("fn validate_structure_with_budget_storage<'hir>(")
        .unwrap();
    let end = current[start..].find("\nstruct CfgReplayBounds {").unwrap() + start;
    let current = &current[start..end];
    let new_call = "let ordinary_schema =\n        schema::paid_summary::selected_with_budget(program, function, helpers, budget)?;";
    assert_eq!(current.matches(new_call).count(), 1);
    assert_eq!(current.replace(new_call, "let ordinary_schema = schema::selected_schema_with_index(program, function, helpers)?;"),
        ci_str!("../../../../../docs/evidence/schema-summary/validation-receiver.rs.before"));
    let old_reference =
        super::super::ordinary_tests::before_original_receiver(ci_str!("original_receiver.rs"));
    let old=old_reference.strip_prefix("//! Complete pre-adoption receiving function, independently compiled for controls.\nuse super::*;\n").unwrap();
    // Relocation changes only the exact namespace of the same V20 constant.
    let qualified = "crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20";
    assert_eq!(old.matches(qualified).count(), 1);
    let old = old.replacen(qualified, "super::CLEANUP_PLAN_SCHEMA_V20", 1);
    assert_eq!(
        old.replacen(
            "pub(super) fn validate_original<'hir>(",
            "fn validate_structure_with_budget_storage<'hir>(",
            1
        ),
        ci_str!("../../../../../docs/evidence/schema-summary/validation-receiver.rs.before")
    );
}
fn original(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget)?;
    original_receiver::validate_original(program, function, &mut budget, None, None)
}
fn current(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget)?;
    validate_structure_with_budget(program, function, &mut budget, None)
}
#[test]
fn full_schema_and_receiving_oracle_keep_late_tampering_all_diagnostic_fields() {
    for program in paid_type_facts::tests::fixtures() {
        for function in &program.functions {
            for replacement in [None, Some("forged.schema")] {
                let mut subject = function.clone();
                if let Some(schema) = replacement {
                    subject.cleanup_plan.schema = schema;
                }
                let expected = crate::bounded_output::with_limit_usage(usize::MAX, || {
                    original(&program, &subject)
                })
                .0;
                let (actual, refused, _) =
                    crate::bounded_output::with_limit_usage(usize::MAX, || {
                        current(&program, &subject)
                    });
                assert!(!refused);
                assert_eq!(actual, expected);
            }
            if !function.cleanup_plan.slots.is_empty() {
                let mut subject = function.clone();
                subject.cleanup_plan.slots[0].storage_index =
                    subject.cleanup_plan.slots[0].storage_index.wrapping_add(1);
                let expected = crate::bounded_output::with_limit_usage(usize::MAX, || {
                    original(&program, &subject)
                })
                .0;
                let (actual, refused, _) =
                    crate::bounded_output::with_limit_usage(usize::MAX, || {
                        current(&program, &subject)
                    });
                assert!(!refused);
                assert_eq!(actual, expected);
            }
        }
    }
}
#[test]
fn full_unfunded_compatibility_and_selected_reservation_receiver_stay_original() {
    for program in paid_type_facts::tests::fixtures() {
        for function in &program.functions {
            let expected = super::super::selected_schema_with_index(&program, function, None);
            let mut budget = ReplayBudget::new();
            assert!(!budget.pays_builder());
            assert_eq!(
                selected_with_budget(&program, function, None, &mut budget),
                expected
            );
            let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
            assert!(!budget.pays_builder());
            assert_eq!(
                selected_with_budget(&program, function, None, &mut budget),
                expected
            );
        }
    }
}
