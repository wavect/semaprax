//! Complete a417 final constructor; only entry name and full old index call differ.
use super::{
    analyze_byte_data_capacity, compiler_prelude, derive_byte_slice_provenance, link_error,
    rebuild_cleanup_metadata, validate, BTreeSet, DeclarationId, Diagnostic, IdentityOrigin,
    LinkedOwnedDataParts, LinkedScalarFunction, ResolvedProgram, ResolvedType,
};
pub(crate) fn link_owned_data_api_workspace_before_legacy_coordinates(
    module: String,
    entrypoint: DeclarationId,
    mut linked_functions: Vec<LinkedScalarFunction>,
    parts: LinkedOwnedDataParts,
) -> Result<ResolvedProgram, Diagnostic> {
    if linked_functions.is_empty() {
        return Err(link_error("workspace owned-data closure has no functions"));
    }
    linked_functions.sort_by(|left, right| left.function.id.cmp(&right.function.id));
    let mut seen = BTreeSet::new();
    let mut entry_origin = None;
    for linked in &linked_functions {
        if !seen.insert(linked.function.id.clone()) {
            return Err(link_error(format!(
                "workspace owned-data closure duplicates function `{}`",
                linked.function.id
            )));
        }
        if linked.function.id == entrypoint {
            entry_origin = Some(linked.origin);
            if linked.function.name != "main"
                || !linked.function.params.is_empty()
                || linked.function.return_type != ResolvedType::I64
            {
                return Err(link_error(
                    "workspace owned-data entry point must be an authored `fn main() -> i64`",
                ));
            }
        }
    }
    if entry_origin != Some(IdentityOrigin::Explicit) {
        return Err(link_error(
            "workspace owned-data entry point must have an explicit authored identity",
        ));
    }

    let functions = linked_functions
        .drain(..)
        .map(|linked| linked.function)
        .collect::<Vec<_>>();
    let uses_private_collections = functions.iter().any(|f| {
        crate::map_ops::is_collection(&f.return_type)
            || f.params
                .iter()
                .any(|p| crate::map_ops::is_collection(&p.ty))
    }) || functions.iter().any(|f| {
        f.requires
            .iter()
            .chain(std::iter::once(&f.body))
            .chain(&f.ensures)
            .any(|root| {
                let mut found = false;
                crate::hir::visit_resolved_calls(root, &mut |callee, _, _| {
                    found |= crate::map_ops::by_id(callee.as_str()).is_some()
                });
                found
            })
    }) || parts
        .types
        .iter()
        .any(|t| crate::map_ops::is_declaration(t.id.as_str()));
    let uses_stream = functions
        .iter()
        .any(crate::stdin_stream_ops::resolved_function_uses);
    let uses_vec = compiler_prelude::owned_data_uses_vec(&functions, &parts);
    let (mut declarations, mut types) = if uses_private_collections && uses_vec {
        compiler_prelude::workspace_compiler_prelude_for_stream_data_collections()?
    } else if uses_private_collections {
        compiler_prelude::workspace_compiler_prelude_for_collections()?
    } else if uses_stream && uses_vec {
        compiler_prelude::workspace_compiler_prelude_for_stream_data()?
    } else if uses_stream {
        compiler_prelude::workspace_compiler_prelude_for_stream()?
    } else {
        compiler_prelude::selected_for_owned_data(&functions, &parts)?
    };
    declarations.extend_linked_owned_data_before_legacy_coordinates(
        &parts.types,
        &parts.interfaces,
        &functions,
        &parts.function_templates,
        &parts.declaration_facts,
    )?;
    declarations.byte_slice_roots = derive_byte_slice_provenance(&functions, &declarations)?;
    if !declarations.populate_type_facts() {
        return Err(link_error(
            "workspace owned-data linker could not construct exact type facts",
        ));
    }
    types.extend(parts.types);
    let mut linked = ResolvedProgram {
        module,
        permits: parts.permits,
        agents: Vec::new(),
        entrypoint,
        declarations,
        types,
        interfaces: parts.interfaces,
        function_templates: parts.function_templates,
        functions,
        function_instances: parts.function_instances,
    };
    analyze_byte_data_capacity(&linked)?;
    rebuild_cleanup_metadata(&mut linked)?;
    validate(&linked)?;
    Ok(linked)
}
