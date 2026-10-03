//! Selected native Rust metadata in retained Project/workspace graphs.

use super::*;

pub(super) fn has_facts(modules: &[WorkspaceGraphProjectionModule]) -> bool {
    modules
        .iter()
        .flat_map(|module| &module.interfaces)
        .flat_map(|interface| &interface.imports)
        .any(|import| import.index_selected)
}

pub(super) fn schema(
    base: &'static str,
    modules: &[WorkspaceGraphProjectionModule],
    project: bool,
) -> &'static str {
    if has_facts(modules) {
        if project {
            "semaprax.project-semantic-graph.v5"
        } else {
            "semaprax.workspace-semantic-graph.v5"
        }
    } else {
        base
    }
}

pub(super) fn append(
    output: &mut crate::bounded_output::CappedString,
    modules: &[WorkspaceGraphProjectionModule],
) {
    if !has_facts(modules) {
        return;
    }
    output.push_str(",\"indexed_rust_imports\":{\"authority\":\"none\",\"imports\":[");
    let mut first = true;
    for module in modules {
        let mut imports = module
            .interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
            .filter(|import| import.index_selected)
            .collect::<Vec<_>>();
        imports.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        for import in imports {
            if !first {
                output.push(',');
            }
            first = false;
            output.push_str("{\"id\":");
            push_json_string(output, import.id.as_str());
            output.push_str(",\"path\":");
            push_json_string(output, &module.path);
            output.push_str(",\"rust_path\":");
            push_json_string(
                output,
                import
                    .rust_path
                    .as_deref()
                    .expect("checked indexed Rust path"),
            );
            output.push_str(",\"selected_index_digest\":");
            push_json_string(
                output,
                import
                    .selected_index_digest
                    .as_deref()
                    .expect("checked indexed Rust identity"),
            );
            output.push_str(",\"receiver\":");
            push_json_string(
                output,
                import.selected_receiver.as_deref().unwrap_or("none"),
            );
            output.push_str(",\"result\":");
            push_json_string(
                output,
                match import.result.kind {
                    hir::ResolvedImportResultKind::Unit => "unit",
                    hir::ResolvedImportResultKind::I64 => "i64",
                    hir::ResolvedImportResultKind::Bool => "bool",
                    hir::ResolvedImportResultKind::ResultI64I64 => "Result<i64, i64>",
                },
            );
            output.push_str(",\"effects\":[");
            for (index, effect) in import.effects.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                push_json_string(output, effect);
            }
            output.push_str("],\"failure_domain\":");
            match &import.failure {
                hir::ResolvedImportFailure::Infallible => output.push_str("null"),
                hir::ResolvedImportFailure::Status { domain_id, .. } => {
                    push_json_string(output, domain_id)
                }
            }
            output.push('}');
        }
    }
    output.push_str("]}");
}
