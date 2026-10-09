//! Graph v71 binds the new closed owned-String byte-slice root kind.
use super::*;

const SCHEMA: &str = "semaprax.graph.v71";

fn requires_function(function: &ResolvedFunction) -> bool {
    let mut owned_strings = function
        .params
        .iter()
        .filter(|parameter| {
            parameter.ty == ResolvedType::String && parameter.ownership == OwnershipMode::Own
        })
        .map(|parameter| parameter.id.clone())
        .collect::<BTreeSet<_>>();
    hir::function_value::walk(function, |expression| {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::Let { binding, .. } = statement {
                    if binding.ty == ResolvedType::String
                        && binding.ownership == OwnershipMode::Own
                    {
                        owned_strings.insert(binding.id.clone());
                    }
                }
            }
        }
        if let ResolvedExprKind::Closure { captures, .. } = &expression.kind {
            for capture in captures {
                if capture.binding.ty == ResolvedType::String
                    && capture.binding.ownership == OwnershipMode::Own
                {
                    owned_strings.insert(capture.binding.id.clone());
                }
            }
        }
    });

    let mut found = false;
    hir::function_value::walk(function, |expression| {
        if let ResolvedExprKind::BorrowPlace { operation, place } = &expression.kind {
            found |= operation.as_str() == crate::byte_ops::STR_AS_BYTES_ID
                && expression.ty == ResolvedType::SliceU8
                && expression.ownership == OwnershipMode::Borrow
                && place.projections.is_empty()
                && owned_strings.contains(&place.root);
        }
    });
    found
}

fn requires_functions<'a>(functions: impl IntoIterator<Item = &'a ResolvedFunction>) -> bool {
    functions.into_iter().any(requires_function)
}

pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if program
        .declarations
        .byte_slice_provenances()
        .any(|(_, provenance)| provenance.root_kind == ByteSliceRootKind::OwnedString)
    {
        Ok(SCHEMA)
    } else {
        super::byte_buffer_renewal::graph_schema(program)
    }
}

pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if program
        .declarations
        .byte_slice_provenances()
        .any(|(_, provenance)| provenance.root_kind == ByteSliceRootKind::OwnedString)
    {
        return Err(Diagnostic::io(
            "SPX-G411",
            "fused owned-String byte views require Graph v71",
        ));
    }
    super::nested_owned::legacy_graph_schema(program)
}

pub(crate) fn graph_schema_from_parts_and_instances(
    interfaces: &[hir::ResolvedInterface],
    types: &[hir::ResolvedTypeDeclaration],
    functions: &[ResolvedFunction],
    templates: &[hir::ResolvedFunctionTemplate],
    instances: &[hir::ResolvedFunctionInstance],
) -> Result<&'static str, Diagnostic> {
    if requires_functions(
        functions
            .iter()
            .chain(instances.iter().map(|instance| &instance.function)),
    ) {
        Ok(SCHEMA)
    } else {
        super::byte_buffer_renewal::graph_schema_from_parts_and_instances(
            interfaces, types, functions, templates, instances,
        )
    }
}

pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut graph = super::byte_buffer_renewal::graph_json(
        program, revision, functions, types, view,
    )?;
    if !program
        .declarations
        .byte_slice_provenances()
        .any(|(_, provenance)| provenance.root_kind == ByteSliceRootKind::OwnedString)
    {
        return Ok(graph);
    }
    let document: serde_json::Value = serde_json::from_str(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?;
    let previous = document["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "missing checked schema"))?;
    let prefix = format!("{{\"schema\":{}", quote_json(previous));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io("SPX-G411", "noncanonical checked graph"));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v71\"");
    Ok(graph)
}
