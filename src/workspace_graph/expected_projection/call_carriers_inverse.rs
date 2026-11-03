//! Exact finite inverse for the source fragments owned by OPT#898.
//! Existing older inverse tests still inspect their entire original subject.
pub(in crate::workspace_graph) fn pre898(source: &str) -> String {
    let mut restored = source.to_owned();
    for (before, after) in [
        (
            r####""####,
            r####"// Imported intrinsic wrappers are retained as templates by the synthetic
// projection too. Each actual typed call constructs a full instance, just as
// a call to an authored local template does; count its real provider body.
fn generic_instance_source_cost_with_imports(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    include_imports: bool,
) -> Result<GenericInstanceCost, Vec<Diagnostic>> {
    let mut total = generic_instance_source_cost(program)?;
    for module_use in program.module_uses.iter().filter(|_| include_imports) {
        if module_use.kind != ModuleUseKind::Function {
            continue;
        }
        let Some(target) = authored.get(module_use.persistent_id.as_str()) else {
            continue;
        };
        let Some(function) = target.function.filter(|f| !f.type_parameters.is_empty()) else {
            continue;
        };
        let Some(provider) = programs.iter().find(|p| p.module == target.module) else {
            continue;
        };
        if crate::vec_ops::source_wrapper(provider, function).is_none()
            && crate::box_ops::source_wrapper(provider, function).is_none()
        {
            continue;
        }
        let mut instances = 0usize;
        for caller in program
            .functions
            .iter()
            .filter(|f| f.type_parameters.is_empty())
        {
            for expression in caller
                .requires
                .iter()
                .chain(std::iter::once(&caller.body))
                .chain(&caller.ensures)
            {
                expression.visit_call_instances(&mut |name, arguments, _| {
                    if name == module_use.alias && !arguments.is_empty() {
                        instances = instances.saturating_add(1);
                    }
                });
            }
        }
        if instances == 0 {
            continue;
        }
        let mut cost = StructuralCost::new();
        ast_function_cost(function, &mut cost)?;
        let slots = ast_function_identity_slots(function)?;
        for (sum, per_instance) in [
            (&mut total.bytes, cost.total),
            (&mut total.string_bytes, cost.string_bytes),
            (&mut total.identity_slots, slots),
            (&mut total.identity_carriers, cost.identity_carriers),
        ] {
            *sum = instances
                .checked_mul(per_instance)
                .and_then(|bytes| sum.checked_add(bytes))
                .filter(|bytes| *bytes <= active_builder_limit())
                .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
        }
    }
    Ok(total)
}

"####,
        ),
        (
            r####"    let generic_instances = generic_instance_source_cost(program)?;
"####,
            r####"    let generic_instances =
        generic_instance_source_cost_with_imports(program, authored, programs, include_imports)?;
"####,
        ),
        (
            r####"    default_expr, generic_instance_source_cost, graph_error, import_stub, limit_error,
"####,
            r####"    default_expr, generic_instance_source_cost_with_imports, graph_error, import_stub, limit_error,
"####,
        ),
        (
            r####"#[path = "expected_projection/cached_exact.rs"]
pub(super) mod cached_exact;
#[path = "expected_projection/call_identity.rs"]
mod call_identity;
"####,
            r####"#[path = "expected_projection/cached_exact.rs"]
pub(super) mod cached_exact;
#[path = "expected_projection/call_carriers.rs"]
mod call_carriers;
#[cfg(test)]
#[path = "expected_projection/call_carriers_original.rs"]
#[allow(dead_code, reason = "complete independent pre-898 forecaster control")]
mod call_carriers_original;
#[path = "expected_projection/call_identity.rs"]
mod call_identity;
"####,
        ),
        (
            r####"    let mut raw = StructuralCost::raw_ast(layout_mode >= 1).with_inline_values(layout_mode >= 2);
    ast_program_cost(program, &mut raw, include_imports)?;
    let fresh;
    let held;
"####,
            r####"    let mut raw = StructuralCost::raw_ast(layout_mode >= 1).with_inline_values(layout_mode >= 2);
    ast_program_cost(program, &mut raw, include_imports)?;
    let call_proof = if layout_mode >= 5 {
        let proof = super::call_carriers::count(program, authored, include_imports);
        raw.exact_carrier_free_calls = proof.roots;
        proof.bytes
    } else {
        0
    };
    let fresh;
    let held;
"####,
        ),
        (
            r####"    )?;
    let hir_upper = fixed_hir_upper
        .checked_add(prelude_hir_upper)
        .and_then(|bytes| bytes.checked_add(identity_carrier_bytes))
        .and_then(|bytes| bytes.checked_add(identity_occurrence_upper))
"####,
            r####"    )?;
    let hir_upper = fixed_hir_upper
        .checked_add(call_proof)
        .and_then(|bytes| bytes.checked_add(prelude_hir_upper))
        .and_then(|bytes| bytes.checked_add(identity_carrier_bytes))
        .and_then(|bytes| bytes.checked_add(identity_occurrence_upper))
"####,
        ),
        (
            r####"                include_str!("../../src/workspace_graph/expected_projection/vec_constructor.rs"),
                include_str!("../../src/workspace_graph/expected_projection/held_projection.rs"),
                include_str!("../../src/workspace_graph/expected_projection/prelude_forecast.rs"),
                include_str!("../../src/workspace_graph/expected_projection/prelude_forecast_tests.rs"),
"####,
            r####"                include_str!("../../src/workspace_graph/expected_projection/vec_constructor.rs"),
                include_str!("../../src/workspace_graph/expected_projection/held_projection.rs"),
                include_str!("../../src/workspace_graph/expected_projection/call_carriers.rs"),
                include_str!("../../src/workspace_graph/expected_projection/call_carriers_tests.rs"),
                include_str!("../../src/workspace_graph/expected_projection/call_carriers_inverse.rs"),
                include_str!("../../src/workspace_graph/expected_projection/call_carriers_original.rs"),
                include_str!("../../src/workspace_graph/expected_projection/call_carriers_historical_tests.rs"),
                include_str!("../../src/workspace_graph/expected_projection/prelude_forecast.rs"),
                include_str!("../../src/workspace_graph/expected_projection/prelude_forecast_tests.rs"),
"####,
        ),
    ] {
        if restored.contains(after) {
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, before, 1);
        }
    }
    restored
}
#[test]
fn complete_forecaster_inverse_reconstructs_entire_pre898_algorithm() {
    let actual = include_str!("held_projection.rs");
    let original = include_str!("call_carriers_original.rs");
    let original = original
        .split("// Finite helper wiring")
        .next()
        .unwrap()
        .trim_end();
    assert_eq!(pre898(actual).trim_end(), original);
}
