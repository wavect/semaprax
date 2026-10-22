//! Complete authentic old selector, with explicit name/visibility/helper glue.
use super::*;
// Preserve the original owning source bytes, including literal contents.
#[rustfmt::skip]
pub(super) fn before_select<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    proofs: &mut byte_helper::Proofs<'a>,
    static_schema: Option<&'static str>,
    lexical_static: bool,
) -> Result<&'static str, Vec<Diagnostic>> {
    let helper_schema = byte_helper::before_static_guard_schema_with_proofs(module, modules, facts, proofs)?;
    let mut string_slots = false;
    for function in module.functions.iter().chain(
        module
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            hir::visit_resolved_calls(root, &mut |callee, _, _| {
                string_slots |= matches!(
                    callee.as_str(),
                    crate::vec_ops::string_slot::ID | crate::vec_ops::string_slot::BYTES_ID
                );
            });
        }
    }
    let finite_scratch = module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(crate::finite_byte_scratch::function_requires);
    let admission = |operation: crate::vec_ops::VecOp, element: &hir::ResolvedType| {
        if crate::vec_ops::resolved_element_is_admitted(element)
            && crate::vec_ops::resolved_operation_element_is_admitted(operation, element)
        {
            return Some(hir::OwnershipMode::Value);
        }
        if *element == hir::ResolvedType::String && operation.admits_owned_leaf()
            || *element == hir::ResolvedType::Bytes
                && crate::vec_ops::resolved_operation_element_is_admitted(operation, element)
        {
            return Some(hir::OwnershipMode::Own);
        }
        let mode = record_mode(element, module, modules, facts)?;
        match mode {
            hir::OwnershipMode::Value if !operation.owned_leaf_only() => Some(mode),
            hir::OwnershipMode::Own if operation.admits_owned_leaf() => Some(mode),
            _ => None,
        }
    };
    let authority = |function: &hir::ResolvedFunction| {
        hir::vec_loop_renewal::requires_with_admission(function, admission)
    };
    let consuming = module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(|function| {
            hir::vec_loop_renewal::consuming::requires_with_admission(function, admission)
        });
    // Static modules independently replay the other renewal obligations too.
    if static_schema.is_none() && !consuming && !string_slots {
        if finite_scratch {
            return Ok("semaprax.graph.v78");
        }
        if let Some(schema) = helper_schema {
            return Ok(schema);
        }
    }
    for function in module.functions.iter().chain(
        module
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        if lexical_static && hir::lexical_exit::requires(function).map_err(|error| vec![error])? {
            continue;
        }
        if helper_schema.is_some()
            && function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
        {
            continue;
        }
        let lexical = hir::lexical_exit::requires(function).map_err(|error| vec![error])?;
        if (function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18) != lexical
        {
            return Err(vec![graph_error(
                "SPX-G410",
                "lexical cleanup schema disagrees with authored exits",
            )]);
        }
        let byte = !lexical && crate::byte_ops::requires_same_owner_set(function);
        let string = !lexical && !byte && crate::string_ops::replacement::requires(function);
        let ordinary = !lexical && !byte && !string && authority(function);
        for (schema, expected, message) in [
            (
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17,
                byte,
                "byte-buffer renewal cleanup schema disagrees",
            ),
            (
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V16,
                string,
                "String replacement cleanup schema disagrees",
            ),
            (
                crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V15,
                ordinary,
                "ordinary Vec renewal cleanup schema disagrees",
            ),
        ] {
            if (function.cleanup_plan.schema == schema) != expected {
                return Err(vec![graph_error("SPX-G410", message)]);
            }
        }
    }
    if module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(|f| {
            std::iter::once(&f.body)
                .chain(&f.requires)
                .chain(&f.ensures)
                .any(crate::vec_field::expression_uses)
        })
    {
        return Ok("semaprax.graph.v85");
    }
    if module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(crate::hir::copy_discard_function_uses)
    {
        return Ok("semaprax.graph.v84");
    }
    // Byte traversal is the outer v83 layer, preserving row-loan v82 facts.
    if module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .any(crate::hir::byte_traversal_function_uses)
    {
        return Ok("semaprax.graph.v83");
    }
    if graph::row_loan::functions_require(
        module
            .functions
            .iter()
            .chain(module.function_instances.iter().map(|i| &i.function)),
    ) {
        return Ok(graph::row_loan::SCHEMA);
    }
    if module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
        .try_fold(false, |found, f| {
            crate::hir::vec_constructor::function_uses(f).map(|uses| found || uses)
        })
        .map_err(|e| vec![e])?
    {
        return Ok("semaprax.graph.v81");
    }
    if string_slots {
        return Ok("semaprax.graph.v80");
    }
    if consuming {
        return Ok("semaprax.graph.v79");
    }
    if let Some(schema) = static_schema {
        return Ok(if finite_scratch {
            "semaprax.graph.v78"
        } else {
            schema
        });
    }
    reserve_builder_structure(nominal_context::RECEIVING_CONTROLS)?;
    let types = nominal_context::Types {
        module,
        modules,
        facts,
    };
    if !module
        .functions
        .iter()
        .chain(
            module
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| authority(function) && !hir::vec_loop_renewal::requires(function))
    {
        return graph::graph_schema_from_parts_and_instances(
            &module.interfaces,
            &types,
            &module.functions,
            &module.function_templates,
            &module.function_instances,
        )
        .map_err(|error| vec![error]);
    }
    graph::graph_schema_from_parts_and_instances_with_renewal_authority(
        &module.interfaces,
        &types,
        &module.functions,
        &module.function_templates,
        &module.function_instances,
        Some(&authority),
    )
    .map_err(|error| vec![error])
}
