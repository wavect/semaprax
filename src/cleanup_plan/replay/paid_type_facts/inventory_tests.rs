use super::*;

#[test]
fn raw_inventory_classifier_keeps_complete_original_diagnostic_fields() {
    for program in tests::fixtures() {
        for function in &program.functions {
            let (results, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
                let mut budget = ReplayBudget::new();
                budget.prepare_builder(function).unwrap();
                reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget)
                    .unwrap();
                let mut results = Vec::new();
                for ty in function
                    .params
                    .iter()
                    .map(|p| &p.ty)
                    .chain(std::iter::once(&function.return_type))
                {
                    results.push((
                        crate::cleanup::type_needs_resource_cleanup(&program, ty)
                            .map_err(|m| Diagnostic::io("SPX-H006", m)),
                        inventory_needs_drop(
                            &program,
                            ty,
                            &mut tests::payer(function, &mut budget),
                        ),
                    ));
                }
                results
            });
            assert!(!refused);
            for (old, current) in results {
                assert_eq!(current, old);
            }
        }
    }
}

#[test]
fn raw_missing_cleanup_facts_refusal_keeps_message_without_replay_prefix() {
    let program = tests::fixtures().into_iter().next().unwrap();
    let function = program.functions.first().unwrap();
    let ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("missing.raw.inventory"),
        arguments: Vec::new(),
    };
    let old = crate::cleanup::type_needs_resource_cleanup(&program, &ty)
        .map_err(|m| Diagnostic::io("SPX-H006", m));
    let (current, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget).unwrap();
        inventory_needs_drop(&program, &ty, &mut tests::payer(function, &mut budget))
    });
    assert!(!refused);
    assert_eq!(current, old);
    assert!(current.is_err());
}

#[test]
fn full_current_classifier_source_equals_complete_saved_receiver_plus_raw_adapter() {
    let current = include_str!("../paid_type_facts.rs");
    assert_eq!(current.matches("pub(super) struct Payer<'a> {").count(), 1);
    let current = current.replacen("pub(super) struct Payer<'a> {", "struct Payer<'a> {", 1);
    let current = current
        .strip_suffix("\n#[cfg(test)]\npub(super) mod oracle_inventory;\n")
        .unwrap();
    let current = current.replace(
        "    #[cfg(test)]\n    if budget.oracle_lease().is_some() {\n        return oracle_inventory::flags(index, function, ty, budget);\n    }\n",
        "",
    );
    let current = current.replace(
        "    #[cfg(test)]\n    if budget.oracle_lease().is_some() {\n        return oracle_inventory::needs_drop(program, function, ty, budget);\n    }\n",
        "",
    );
    let (head, rest) = current
        .split_once("/// Raw ordinary inventory diagnostic adapter.")
        .unwrap();
    let (_, tail) = rest
        .split_once("#[cfg(test)]\nthread_local! { static SAVED:")
        .unwrap();
    let projected = format!("{head}#[cfg(test)]\nthread_local! {{ static SAVED:{tail}");
    let projected = projected.replacen(
        "#[cfg(test)]\npub(super) mod tests;",
        "#[cfg(test)]\nmod tests;",
        1,
    );
    let projected = projected
        .strip_suffix("\npub(super) mod inventory_summary;\n")
        .unwrap()
        .strip_suffix("\nmod inventory_lifecycle;\n")
        .unwrap()
        .strip_suffix("\nmod inventory_drop;\n")
        .unwrap()
        .strip_suffix("\nmod inventory_raw_key;\n")
        .unwrap()
        .strip_suffix("\nmod inventory_flags;\n")
        .unwrap()
        .strip_suffix("\nmod inventory_ordered;\n")
        .unwrap()
        .strip_suffix("\nmod inventory_encoded;\n")
        .unwrap()
        .strip_suffix("\nmod inventory_metadata;\n\nmod inventory_nested;\n")
        .unwrap()
        .strip_suffix("\nmod inventory_profile;\n")
        .unwrap()
        .strip_suffix("\nmod inventory_text;\n")
        .unwrap()
        .strip_suffix("\n#[cfg(test)]\nmod inventory_substitute_tests;\n")
        .unwrap()
        .strip_suffix("\nmod inventory_substitute;\n\nmod inventory_clone;\n")
        .unwrap()
        .strip_suffix("\n#[cfg(test)]\nmod inventory_tests;\n")
        .unwrap();
    assert_eq!(
        projected,
        include_str!("../../../../docs/evidence/schema-summary/paid_type_facts.rs.before")
    );
}

#[test]
fn full_droppability_source_keeps_complete_original_plus_raw_text_adapter() {
    let current = include_str!("droppability.rs");
    let (original, adapter) = current
        .split_once("\n// Raw ordinary inventory text")
        .expect("documented additive raw text adapter");
    assert_eq!(
        original,
        include_str!("../../../../docs/evidence/schema-summary/droppability.rs.before")
    );
    assert!(adapter.contains("text::raw(payer, message)"));
}

// The complete prior setup/body stays compiled against its original pure bool
// receiver. It grants no backing authority and makes no funded-owner claim.
mod original_raw_owner {
    use super::*;
    fn inventory_needs_drop(
        program: &dyn crate::cleanup::ReplayContext,
        ty: &ResolvedType,
        _payer: &mut Payer<'_>,
    ) -> Result<bool, Diagnostic> {
        crate::cleanup::type_needs_resource_cleanup(program, ty)
            .map_err(|message| Diagnostic::io("SPX-H006", message))
    }
    #[test]
    fn raw_inventory_classifier_keeps_complete_original_diagnostic_fields() {
        for program in tests::fixtures() {
            for function in &program.functions {
                let mut budget = ReplayBudget::new();
                let (results, refused, _) =
                    crate::bounded_output::with_limit_usage(usize::MAX, || {
                        reserve_program_skeleton_work(
                            &program,
                            std::iter::once(function),
                            &mut budget,
                        )
                        .unwrap();
                        let mut results = Vec::new();
                        for ty in function
                            .params
                            .iter()
                            .map(|p| &p.ty)
                            .chain(std::iter::once(&function.return_type))
                        {
                            results.push((
                                crate::cleanup::type_needs_resource_cleanup(&program, ty)
                                    .map_err(|m| Diagnostic::io("SPX-H006", m)),
                                inventory_needs_drop(
                                    &program,
                                    ty,
                                    &mut tests::payer(function, &mut budget),
                                ),
                            ));
                        }
                        results
                    });
                assert!(!refused);
                for (old, current) in results {
                    assert_eq!(current, old);
                }
            }
        }
    }
}

#[test]
fn complete_prior_raw_and_zero_work_receivers_and_identity_formatter_remain_source_bound() {
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../docs/evidence/paid-classifier-receiving-f8/compiled-original-projection.json"
    ))
    .unwrap();
    let body = proof["compiled_raw_body"].as_str().unwrap();
    assert!(include_str!("inventory_tests.rs").contains(body));
    // Invert only the enclosing module's four-space indentation, then the
    // one complete callback reflow (including rustfmt's trailing call comma).
    let mut projected = body
        .lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    projected.push('\n');
    let reflow = proof["reflow_before"].as_str().unwrap();
    assert_eq!(projected.matches(reflow).count(), 1);
    projected = projected.replacen(reflow, proof["reflow_after"].as_str().unwrap(), 1);
    projected.push('\n');
    assert_eq!(
        projected,
        include_str!(
            "../../../../docs/evidence/paid-classifier-receiving-f8/raw-inventory-owner.before.txt"
        )
    );
    let current = include_str!("../allocation/oracle/tests.rs");
    let old = include_str!(
        "../../../../docs/evidence/paid-classifier-receiving-f8/oracle-fast-owner.before.txt"
    );
    // The complete old zero-backing owner uses the declared prior receiver;
    // the independent funded current owner remains on its real production path.
    let dispatch = "                let received = nested_shape::type_needs_drop(&program, function, ty).unwrap();";
    let original_dispatch = "                let received = crate::cleanup_plan::replay::nested_shape::type_needs_drop(\n                    &program, function, ty,\n                )\n                .unwrap();";
    assert_eq!(current.matches(dispatch).count(), 1);
    let current = current.replacen(dispatch, original_dispatch, 1);
    let alias = "    use original_fast_type_probe as nested_shape;\n";
    assert_eq!(current.matches(alias).count(), 1);
    assert!(current.replacen(alias, "", 1).contains(old));
    let current =
        include_str!("../../../hir/declaration_index/flags/paid/inventory_constructor.rs");
    let hook = "    // A logical sink can select the existing sticky refusal without returning\n    // a physical error. Authenticate before publishing only a successful value;\n    // Storage::work leaves an already selected first error untouched.\n    let _ = storage.work(0, \"inventory identity receiving authority\");\n";
    assert_eq!(current.matches(hook).count(), 1);
    assert_eq!(
        current.replacen(hook, "", 1),
        include_str!(
        "../../../../docs/evidence/paid-classifier-receiving-f8/inventory-constructor.before.txt"
    )
    );
}
