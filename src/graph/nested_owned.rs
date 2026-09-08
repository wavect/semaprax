use crate::cleanup::FieldLivenessShape;
use crate::cleanup_plan::{
    StorageId, CLEANUP_PLAN_SCHEMA_V10, CLEANUP_PLAN_SCHEMA_V11, CLEANUP_PLAN_SCHEMA_V7,
    CLEANUP_PLAN_SCHEMA_V8, CLEANUP_PLAN_SCHEMA_V9,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{PlaceProjection, ResolvedFunction, ResolvedProgram};

pub(super) fn nested_cleanup_graph_schema<'a>(
    program: Option<&ResolvedProgram>,
    functions: impl IntoIterator<Item = &'a ResolvedFunction>,
    has_native_import: bool,
    generic_composition: bool,
) -> Result<Option<&'static str>, Diagnostic> {
    let functions = functions.into_iter().collect::<Vec<_>>();
    // v10 is selected by the iterator lifecycle.  It may compose with an
    // older nested storage shape, so its numeric selector cannot erase the
    // independently retained shape facts that guard Graph v26-v31.
    let has_nested_storage = functions.iter().try_fold(false, |found, function| {
        function
            .cleanup
            .slots
            .iter()
            .try_fold(found, |found, slot| {
                crate::cleanup::cleanup_shape_profile(&slot.shape)
                    .map(|profile| found || profile.has_nested_owned_bytes)
            })
    })?;
    let has_nested_update = functions
        .iter()
        .any(|function| function.cleanup_plan.schema == CLEANUP_PLAN_SCHEMA_V9);
    let has_nested_destructure = has_nested_update
        || functions
            .iter()
            .any(|function| function.cleanup_plan.schema == CLEANUP_PLAN_SCHEMA_V8);
    let has_nested_cleanup = has_nested_storage
        || has_nested_destructure
        || functions
            .iter()
            .any(|function| function.cleanup_plan.schema == CLEANUP_PLAN_SCHEMA_V7);
    if !has_nested_cleanup {
        return Ok(None);
    }
    reject_nested_native_flags(has_nested_cleanup, has_native_import)?;

    let has_attached_loans = functions
        .iter()
        .any(|function| !function.loan_plan.loans.is_empty());
    if has_attached_loans {
        if let Some(program) = program {
            crate::loan_plan::validate_program(program).map_err(|_| {
                composition_error(
                    "nested owned-record Graph composition contains forged shared-loan evidence",
                )
            })?;
        }
    }

    let mut has_authenticated_nested_projected_loan = false;
    for function in &functions {
        if function.loan_plan.loans.is_empty() {
            continue;
        }
        for loan in &function.loan_plan.loans {
            if loan.origin.projections.len() < 2
                || !matches!(
                    function.cleanup_plan.schema,
                    CLEANUP_PLAN_SCHEMA_V7
                        | CLEANUP_PLAN_SCHEMA_V8
                        | CLEANUP_PLAN_SCHEMA_V9
                        | CLEANUP_PLAN_SCHEMA_V10
                        | CLEANUP_PLAN_SCHEMA_V11
                )
                || !function_has_nested_storage(function)?
                || !loan_origin_is_nested_owned_leaf(function, loan)
                || program.is_some_and(|program| {
                    !crate::hir::is_authenticated_nested_projected_byte_loan(
                        program, function, loan,
                    )
                })
            {
                if generic_composition {
                    let program = program.ok_or_else(|| {
                        composition_error("generic loan composition requires retained checked HIR")
                    })?;
                    crate::hir::validate(program)?;
                    return Ok(Some("semaprax.graph.v34"));
                }
                return Err(composition_error(
                    "nested owned-record Graph composition contains a loan that is not an exactly authenticated nested projected Bytes loan",
                ));
            }
            has_authenticated_nested_projected_loan = true;
        }
    }
    Ok(Some(
        if has_nested_update && has_authenticated_nested_projected_loan {
            "semaprax.graph.v31"
        } else if has_nested_update {
            "semaprax.graph.v30"
        } else if has_nested_destructure && has_authenticated_nested_projected_loan {
            "semaprax.graph.v29"
        } else if has_nested_destructure {
            "semaprax.graph.v28"
        } else if has_authenticated_nested_projected_loan {
            "semaprax.graph.v27"
        } else {
            "semaprax.graph.v26"
        },
    ))
}

fn function_has_nested_storage(function: &ResolvedFunction) -> Result<bool, Diagnostic> {
    function
        .cleanup
        .slots
        .iter()
        .try_fold(false, |found, slot| {
            crate::cleanup::cleanup_shape_profile(&slot.shape)
                .map(|profile| found || profile.has_nested_owned_bytes)
        })
}

pub(super) fn select_schema<'a>(
    program: Option<&ResolvedProgram>,
    functions: impl IntoIterator<Item = &'a ResolvedFunction>,
    has_native_import: bool,
    base_schema: &'static str,
    generic_composition: bool,
) -> Result<&'static str, Diagnostic> {
    let functions = functions.into_iter().collect::<Vec<_>>();
    if let Some(schema) = nested_cleanup_graph_schema(
        program,
        functions.iter().copied(),
        has_native_import,
        generic_composition,
    )? {
        return Ok(schema);
    }
    if has_native_import {
        return Ok(base_schema);
    }
    let has_owned_variant = functions.iter().any(|function| {
        function.cleanup.schema == crate::cleanup::CLEANUP_INVENTORY_SCHEMA_V2
            || function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V6
    });
    let has_loans = functions
        .iter()
        .any(|function| !function.loan_plan.loans.is_empty());
    let has_projected_loans = functions.iter().any(|function| {
        function
            .loan_plan
            .loans
            .iter()
            .any(|loan| !loan.origin.projections.is_empty())
    });
    if has_owned_variant && has_projected_loans {
        Ok("semaprax.graph.v33")
    } else if has_owned_variant && has_loans {
        Ok("semaprax.graph.v32")
    } else if has_projected_loans {
        Ok("semaprax.graph.v24")
    } else if has_loans {
        Ok("semaprax.graph.v23")
    } else if has_owned_variant {
        Ok("semaprax.graph.v22")
    } else {
        Ok(base_schema)
    }
}

fn loan_origin_is_nested_owned_leaf(
    function: &ResolvedFunction,
    loan: &crate::loan_plan::Loan,
) -> bool {
    let root = &loan.origin.root;
    let Some(slot) = function
        .cleanup_plan
        .slots
        .iter()
        .find(|slot| matches!(&slot.storage, StorageId::Value(candidate) if candidate == root))
    else {
        return false;
    };
    let mut shape = &slot.field_liveness_shape;
    for projection in &loan.origin.projections {
        let PlaceProjection::Field(field) = projection else {
            return false;
        };
        let FieldLivenessShape::Record { fields, .. } = shape else {
            return false;
        };
        let Some(next) = fields.iter().find(|candidate| candidate.field == *field) else {
            return false;
        };
        shape = &next.shape;
    }
    matches!(shape, FieldLivenessShape::Leaf { .. })
}

fn composition_error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-G410", message)
}

fn parts_use_iterator(
    types: &[crate::hir::ResolvedTypeDeclaration],
    functions: &[ResolvedFunction],
    templates: &[crate::hir::ResolvedFunctionTemplate],
    instances: &[crate::hir::ResolvedFunctionInstance],
) -> bool {
    fn function_uses_iterator(function: &ResolvedFunction) -> bool {
        let mut expressions = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        expressions.any(crate::iterator_ops::resolved_expression_uses_iterator)
            || crate::iterator_ops::resolved_type_uses_iterator(&function.return_type)
            || function
                .params
                .iter()
                .any(|parameter| crate::iterator_ops::resolved_type_uses_iterator(&parameter.ty))
    }
    types
        .iter()
        .filter(|declaration| {
            !matches!(
                declaration.id.as_str(),
                crate::iterator_ops::ITER_ID | crate::iterator_ops::STEP_ID
            )
        })
        .any(|declaration| match &declaration.kind {
            crate::hir::ResolvedTypeDeclarationKind::Record { fields }
            | crate::hir::ResolvedTypeDeclarationKind::Class { fields, .. } => fields
                .iter()
                .any(|field| crate::iterator_ops::resolved_type_uses_iterator(&field.ty)),
            crate::hir::ResolvedTypeDeclarationKind::Variant { cases } => cases
                .iter()
                .flat_map(|case| &case.fields)
                .any(|field| crate::iterator_ops::resolved_type_uses_iterator(&field.ty)),
            crate::hir::ResolvedTypeDeclarationKind::Resource { .. } => false,
        })
        || functions.iter().any(function_uses_iterator)
        || templates.iter().any(|template| {
            crate::iterator_ops::resolved_type_uses_iterator(&template.return_type)
                || template.params.iter().any(|parameter| {
                    crate::iterator_ops::resolved_type_uses_iterator(&parameter.ty)
                })
                || template
                    .requires
                    .iter()
                    .chain(std::iter::once(&template.body))
                    .chain(&template.ensures)
                    .any(crate::iterator_ops::resolved_expression_uses_iterator)
        })
        || instances.iter().any(|instance| {
            instance
                .type_arguments
                .iter()
                .any(crate::iterator_ops::resolved_type_uses_iterator)
                || function_uses_iterator(&instance.function)
        })
}

// Frozen workspace source metadata retains its versioned pre-v34 contract.
pub(crate) fn graph_schema_from_parts_and_instances(
    interfaces: &[crate::hir::ResolvedInterface],
    types: &[crate::hir::ResolvedTypeDeclaration],
    functions: &[ResolvedFunction],
    function_templates: &[crate::hir::ResolvedFunctionTemplate],
    function_instances: &[crate::hir::ResolvedFunctionInstance],
) -> Result<&'static str, Diagnostic> {
    let iterator_schema = iterator_loop_schema(
        functions
            .iter()
            .chain(function_instances.iter().map(|instance| &instance.function)),
        function_templates,
    )?;
    if functions
        .iter()
        .chain(function_instances.iter().map(|instance| &instance.function))
        .any(|function| {
            matches!(
                function.cleanup_plan.schema,
                CLEANUP_PLAN_SCHEMA_V10 | CLEANUP_PLAN_SCHEMA_V11
            )
        })
        && super::native_import::declares_native_rust_import(interfaces)
    {
        return Err(Diagnostic::io(
            "SPX-G410",
            "native Rust import Graph v25 cannot mask CleanupPlan v10 iterator semantics",
        ));
    }
    if parts_use_iterator(types, functions, function_templates, function_instances) {
        // Validate all older cleanup/loan composition first.  v38 is the
        // projection version for the iterator facts, never a bypass for the
        // retained cleanup evidence.
        select_schema(
            None,
            functions
                .iter()
                .chain(function_instances.iter().map(|instance| &instance.function)),
            super::native_import::declares_native_rust_import(interfaces),
            super::graph_schema_from_parts_without_loans(
                interfaces,
                types,
                functions,
                function_templates,
            )?,
            false,
        )?;
        return Ok(iterator_schema);
    }
    if functions
        .iter()
        .chain(function_instances.iter().map(|instance| &instance.function))
        .any(super::function_values::function_has_closure)
        || function_templates
            .iter()
            .any(crate::hir::closure::template_has_closure)
    {
        return Ok("semaprax.graph.v37");
    }
    if functions
        .iter()
        .chain(function_instances.iter().map(|instance| &instance.function))
        .any(crate::hir::function_value::function_uses_value)
        || function_templates
            .iter()
            .any(crate::hir::function_value::template_uses_value)
    {
        return Ok("semaprax.graph.v36");
    }
    let schema = select_schema(
        None,
        functions
            .iter()
            .chain(function_instances.iter().map(|instance| &instance.function)),
        super::native_import::declares_native_rust_import(interfaces),
        super::graph_schema_from_parts_without_loans(
            interfaces,
            types,
            functions,
            function_templates,
        )?,
        false,
    )?;
    Ok(
        if super::generic_mapping::requires_v35(function_templates) {
            "semaprax.graph.v35"
        } else if function_templates
            .iter()
            .any(crate::hir::generic_result::profile)
        {
            "semaprax.graph.v34"
        } else {
            schema
        },
    )
}

pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    let iterator_schema = iterator_loop_schema(
        program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ),
        &program.function_templates,
    )?;
    if program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| {
            matches!(
                function.cleanup_plan.schema,
                CLEANUP_PLAN_SCHEMA_V10 | CLEANUP_PLAN_SCHEMA_V11
            )
        })
        && super::native_import::declares_native_rust_import(&program.interfaces)
    {
        return Err(Diagnostic::io(
            "SPX-G410",
            "native Rust import Graph v25 cannot mask CleanupPlan v10 iterator semantics",
        ));
    }
    if super::prelude_binding::uses_iterator(program) {
        program_schema(
            program,
            !program.function_instances.is_empty()
                || requires_generic_result_schema(program)
                || super::generic_mapping::requires_v35(&program.function_templates)
                || crate::hir::function_value::requires_function_values(program),
        )?;
        return Ok(iterator_schema);
    }
    if crate::hir::closure::requires_closure_projection(program) {
        return Ok("semaprax.graph.v37");
    }
    if crate::hir::function_value::requires_function_values(program) {
        return Ok("semaprax.graph.v36");
    }
    if program.function_instances.is_empty()
        && !requires_generic_result_schema(program)
        && !super::generic_mapping::requires_v35(&program.function_templates)
    {
        return legacy_graph_schema(program);
    }
    generic_payload_schema(program)?;
    Ok(
        if super::generic_mapping::requires_v35(&program.function_templates) {
            "semaprax.graph.v35"
        } else {
            "semaprax.graph.v34"
        },
    )
}

pub(super) fn requires_generic_result_schema(program: &ResolvedProgram) -> bool {
    program
        .function_templates
        .iter()
        .any(crate::hir::generic_result::profile)
}

/// The additive generic graph can compose authenticated ordinary local loans
/// with nested cleanup. Frozen nested graph versions admitted only projected
/// nested-leaf loans; their renderer and rejection remain unchanged.
pub(super) fn generic_payload_schema(
    program: &ResolvedProgram,
) -> Result<&'static str, Diagnostic> {
    program_schema(
        program,
        !program.function_instances.is_empty()
            || requires_generic_result_schema(program)
            || super::generic_mapping::requires_v35(&program.function_templates)
            || crate::hir::function_value::requires_function_values(program),
    )
}

pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    program_schema(program, false)
}

fn program_schema(
    program: &ResolvedProgram,
    generic_composition: bool,
) -> Result<&'static str, Diagnostic> {
    let iterator_schema = iterator_loop_schema(
        program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ),
        &program.function_templates,
    )?;
    let has_iterator = super::prelude_binding::uses_iterator(program);
    let schema = select_schema(
        Some(program),
        program
            .functions
            .iter()
            .chain(program.function_instances.iter().map(|i| &i.function)),
        super::native_import::declares_native_rust_import(&program.interfaces),
        super::graph_schema_from_parts_without_loans(
            &program.interfaces,
            &program.types,
            &program.functions,
            &program.function_templates,
        )?,
        generic_composition,
    )?;
    if has_iterator {
        return Ok(iterator_schema);
    }
    if crate::hir::function_value::requires_function_values(program) {
        if !generic_composition {
            return Err(composition_error("function values require Graph v36"));
        }
        return Ok(
            if crate::hir::closure::requires_closure_projection(program) {
                "semaprax.graph.v37"
            } else {
                "semaprax.graph.v36"
            },
        );
    }
    if super::generic_mapping::requires_v35(&program.function_templates) {
        if !generic_composition {
            return Err(composition_error(
                "explicit generic forwarding requires Graph v35",
            ));
        }
        return Ok("semaprax.graph.v35");
    }
    if requires_generic_result_schema(program) {
        if !generic_composition {
            return Err(composition_error(
                "generic owned Result templates require additive Graph v34",
            ));
        }
        return Ok("semaprax.graph.v34");
    }
    Ok(schema)
}

pub(super) fn graph_schema_includes_modern_composite_facts(schema: &str) -> bool {
    matches!(
        schema,
        "semaprax.graph.v21"
            | "semaprax.graph.v22"
            | "semaprax.graph.v23"
            | "semaprax.graph.v24"
            | "semaprax.graph.v26"
            | "semaprax.graph.v27"
            | "semaprax.graph.v28"
            | "semaprax.graph.v29"
            | "semaprax.graph.v30"
            | "semaprax.graph.v31"
            | "semaprax.graph.v32"
            | "semaprax.graph.v33"
            | "semaprax.graph.v34"
            | "semaprax.graph.v35"
            | "semaprax.graph.v36"
            | "semaprax.graph.v37"
            | "semaprax.graph.v38"
            | "semaprax.graph.v39"
    )
}

pub(super) fn graph_schema_includes_loans(schema: &str) -> bool {
    matches!(
        schema,
        "semaprax.graph.v23"
            | "semaprax.graph.v24"
            | "semaprax.graph.v27"
            | "semaprax.graph.v29"
            | "semaprax.graph.v31"
            | "semaprax.graph.v32"
            | "semaprax.graph.v33"
            | "semaprax.graph.v34"
            | "semaprax.graph.v35"
            | "semaprax.graph.v36"
            | "semaprax.graph.v37"
            | "semaprax.graph.v38"
            | "semaprax.graph.v39"
    )
}

pub(super) fn graph_schema_includes_projected_provenance(schema: &str) -> bool {
    matches!(
        schema,
        "semaprax.graph.v24"
            | "semaprax.graph.v27"
            | "semaprax.graph.v29"
            | "semaprax.graph.v31"
            | "semaprax.graph.v33"
            | "semaprax.graph.v34"
            | "semaprax.graph.v35"
            | "semaprax.graph.v36"
            | "semaprax.graph.v37"
            | "semaprax.graph.v38"
            | "semaprax.graph.v39"
    )
}

pub(super) fn rejected_evidence_schema(schema: &str) -> Option<Diagnostic> {
    let message = match schema {
        "semaprax.graph.v27" => "nested owned-record programs composed with shared loans select `semaprax.graph.v27`, which is outside this evidence flow's admission",
        "semaprax.graph.v26" => "nested owned-record programs select `semaprax.graph.v26`, which is outside this evidence flow's admission",
        "semaprax.graph.v29" => "nested owned-record destructuring composed with authenticated projected loans selects `semaprax.graph.v29`, which is outside this evidence flow's admission",
        "semaprax.graph.v28" => "nested owned-record destructuring selects `semaprax.graph.v28`, which is outside this evidence flow's admission",
        "semaprax.graph.v31" => "nested owned-record update composed with authenticated projected loans selects `semaprax.graph.v31`, which is outside this evidence flow's admission",
        "semaprax.graph.v30" => "nested owned-record update selects `semaprax.graph.v30`, which is outside this evidence flow's admission",
        "semaprax.graph.v33" => "owned-variant programs composed with projected shared loans select `semaprax.graph.v33`, which is outside this evidence flow's admission",
        "semaprax.graph.v32" => "owned-variant programs composed with shared loans select `semaprax.graph.v32`, which is outside this evidence flow's admission",
        _ => return None,
    };
    Some(Diagnostic::io("SPX-G410", message))
}

pub(super) fn reject_nested_native_flags(
    has_nested_cleanup: bool,
    has_native_import: bool,
) -> Result<(), Diagnostic> {
    if has_nested_cleanup && has_native_import {
        return Err(Diagnostic::io(
            "SPX-G410",
            "native Rust import Graph v25 cannot mask nested owned-record Graph v26-v31 semantics",
        ));
    }
    Ok(())
}

fn iterator_loop_schema<'a>(
    functions: impl IntoIterator<Item = &'a ResolvedFunction>,
    templates: &[crate::hir::ResolvedFunctionTemplate],
) -> Result<&'static str, Diagnostic> {
    let mut has_loop = templates
        .iter()
        .any(crate::hir::iterator_loop::template_contains);
    for function in functions {
        let expected = crate::hir::iterator_loop::function_contains(function);
        if expected != (function.cleanup_plan.schema == CLEANUP_PLAN_SCHEMA_V11) {
            return Err(composition_error(
                "iterator loop shape and CleanupPlan v11 selection disagree",
            ));
        }
        has_loop |= expected;
    }
    Ok(if has_loop {
        "semaprax.graph.v39"
    } else {
        "semaprax.graph.v38"
    })
}

pub(super) fn has_iterator_cleanup(function: &ResolvedFunction) -> bool {
    matches!(
        function.cleanup_plan.schema,
        CLEANUP_PLAN_SCHEMA_V10 | CLEANUP_PLAN_SCHEMA_V11
    )
}

pub(super) fn has_nested_cleanup(function: &ResolvedFunction) -> bool {
    matches!(
        function.cleanup_plan.schema,
        CLEANUP_PLAN_SCHEMA_V7 | CLEANUP_PLAN_SCHEMA_V8 | CLEANUP_PLAN_SCHEMA_V9
    )
}
