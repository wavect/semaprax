use crate::cleanup::FieldLivenessShape;
use crate::cleanup_plan::{StorageId, CLEANUP_PLAN_SCHEMA_V7};
use crate::diagnostic::Diagnostic;
use crate::hir::{PlaceProjection, ResolvedFunction, ResolvedProgram};

pub(super) fn nested_cleanup_graph_schema<'a>(
    program: Option<&ResolvedProgram>,
    functions: impl IntoIterator<Item = &'a ResolvedFunction>,
    has_native_import: bool,
) -> Result<Option<&'static str>, Diagnostic> {
    let functions = functions.into_iter().collect::<Vec<_>>();
    let has_nested_cleanup = functions
        .iter()
        .any(|function| function.cleanup_plan.schema == CLEANUP_PLAN_SCHEMA_V7);
    if !has_nested_cleanup {
        return Ok(None);
    }
    reject_nested_native_flags(has_nested_cleanup, has_native_import)?;

    let mut has_any_loan = false;
    let mut locally_authenticated_nested_projected_loan = false;
    for function in &functions {
        for loan in &function.loan_plan.loans {
            has_any_loan = true;
            if loan.origin.projections.len() < 2 {
                continue;
            }
            if function.cleanup_plan.schema != CLEANUP_PLAN_SCHEMA_V7
                || !loan_origin_is_nested_owned_leaf(function, loan)
            {
                return Err(composition_error(
                    "nested owned-record Graph composition contains an unauthenticated projected loan",
                ));
            }
            locally_authenticated_nested_projected_loan = true;
        }
    }
    let has_authenticated_nested_projected_loan =
        program.map_or(locally_authenticated_nested_projected_loan, |program| {
            functions.iter().any(|function| {
                crate::hir::has_authenticated_nested_projected_byte_loan(program, function)
            })
        });
    if has_any_loan && !has_authenticated_nested_projected_loan {
        return Err(composition_error(
            "nested owned-record Graph composition has loans but no authenticated nested projected loan",
        ));
    }
    Ok(Some(if has_authenticated_nested_projected_loan {
        "semaprax.graph.v27"
    } else {
        "semaprax.graph.v26"
    }))
}

pub(super) fn select_schema<'a>(
    program: Option<&ResolvedProgram>,
    functions: impl IntoIterator<Item = &'a ResolvedFunction>,
    has_native_import: bool,
    base_schema: &'static str,
) -> Result<&'static str, Diagnostic> {
    let functions = functions.into_iter().collect::<Vec<_>>();
    if let Some(schema) =
        nested_cleanup_graph_schema(program, functions.iter().copied(), has_native_import)?
    {
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
    if has_owned_variant && has_loans {
        return Err(composition_error(
            "Shared Loan Plan v1 cannot mask owned-variant Graph v22 semantics",
        ));
    }
    if functions.iter().any(|function| {
        function
            .loan_plan
            .loans
            .iter()
            .any(|loan| !loan.origin.projections.is_empty())
    }) {
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

pub(crate) fn graph_schema_from_parts_and_instances(
    interfaces: &[crate::hir::ResolvedInterface],
    types: &[crate::hir::ResolvedTypeDeclaration],
    functions: &[ResolvedFunction],
    function_templates: &[crate::hir::ResolvedFunctionTemplate],
    function_instances: &[crate::hir::ResolvedFunctionInstance],
) -> Result<&'static str, Diagnostic> {
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
    )
}

pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    select_schema(
        Some(program),
        program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ),
        super::native_import::declares_native_rust_import(&program.interfaces),
        super::graph_schema_from_parts_without_loans(
            &program.interfaces,
            &program.types,
            &program.functions,
            &program.function_templates,
        )?,
    )
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
    )
}

pub(super) fn graph_schema_includes_loans(schema: &str) -> bool {
    matches!(
        schema,
        "semaprax.graph.v23" | "semaprax.graph.v24" | "semaprax.graph.v27"
    )
}

pub(super) fn graph_schema_includes_projected_provenance(schema: &str) -> bool {
    matches!(schema, "semaprax.graph.v24" | "semaprax.graph.v27")
}

pub(super) fn rejected_evidence_schema(schema: &str) -> Option<Diagnostic> {
    let message = match schema {
        "semaprax.graph.v27" => "nested owned-record programs composed with shared loans select `semaprax.graph.v27`, which is outside this evidence flow's admission",
        "semaprax.graph.v26" => "nested owned-record programs select `semaprax.graph.v26`, which is outside this evidence flow's admission",
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
            "native Rust import Graph v25 cannot mask nested owned-record Graph v26/v27 semantics",
        ));
    }
    Ok(())
}
