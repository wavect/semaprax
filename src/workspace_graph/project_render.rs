use super::{
    declaration_kind_text, identity_origin_text, push_json_string, push_optional_json_string,
    WorkspaceGraphProjection, PROJECT_GRAPH_NONCLAIMS, PROJECT_GRAPH_SCHEMA,
};

pub(super) fn render_project_graph_json(
    projection: &WorkspaceGraphProjection,
    project_schema: &str,
    project_name: &str,
    project_revision: &str,
    test_module: &str,
    digest: Option<&str>,
) -> String {
    use std::fmt::Write as _;

    let mut output = crate::bounded_output::CappedString::new();
    output.push_str("{\"schema\":");
    push_json_string(&mut output, PROJECT_GRAPH_SCHEMA);
    output.push_str(",\"project_schema\":");
    push_json_string(&mut output, project_schema);
    output.push_str(",\"project\":");
    push_json_string(&mut output, project_name);
    output.push_str(",\"project_revision\":");
    push_json_string(&mut output, project_revision);
    output.push_str(",\"workspace_revision\":");
    push_json_string(&mut output, projection.workspace_revision());
    if let Some(digest) = digest {
        output.push_str(",\"graph_digest\":");
        push_json_string(&mut output, digest);
    }
    output.push_str(",\"entry_module\":");
    push_json_string(&mut output, projection.entry_module());
    output.push_str(",\"test_module\":");
    push_json_string(&mut output, test_module);
    output.push_str(",\"modules\":[");
    for (index, module) in projection.modules.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"path\":");
        push_json_string(&mut output, &module.path);
        output.push_str(",\"module\":");
        push_json_string(&mut output, &module.module);
        output.push_str(",\"source_graph_schema\":");
        push_json_string(&mut output, &module.source_graph_schema);
        output.push_str(",\"source_revision\":");
        push_json_string(&mut output, &module.source_revision);
        output.push_str(",\"source_digest\":");
        push_json_string(&mut output, &module.source_digest);
        write!(
            output,
            ",\"dependency_depth\":{}}}",
            module.dependency_depth
        )
        .expect("writing to a string cannot fail");
    }
    output.push_str("],\"declarations\":[");
    for (index, declaration) in projection.declarations.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"id\":");
        push_json_string(&mut output, &declaration.id);
        output.push_str(",\"kind\":");
        push_json_string(&mut output, declaration_kind_text(declaration.kind));
        output.push_str(",\"identity_origin\":");
        push_json_string(&mut output, identity_origin_text(declaration.origin));
        output.push_str(",\"owner\":");
        push_optional_json_string(&mut output, declaration.owner.as_deref());
        output.push_str(",\"path\":");
        push_optional_json_string(&mut output, declaration.path.as_deref());
        output.push_str(",\"module\":");
        push_optional_json_string(&mut output, declaration.module.as_deref());
        output.push('}');
    }
    output.push_str("],\"edges\":[");
    for (index, edge) in projection.edges.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str("{\"caller_path\":");
        push_json_string(&mut output, &edge.caller_path);
        output.push_str(",\"caller\":");
        push_json_string(&mut output, &edge.caller);
        output.push_str(",\"target_path\":");
        push_json_string(&mut output, &edge.target_path);
        output.push_str(",\"target\":");
        push_json_string(&mut output, &edge.target);
        output.push_str(",\"kind\":");
        push_json_string(&mut output, edge.kind);
        output.push_str(",\"site\":");
        push_json_string(&mut output, edge.site);
        output.push_str(",\"expression\":");
        push_json_string(&mut output, &edge.expression);
        output.push_str(",\"ast_path\":");
        push_json_string(&mut output, &edge.ast_path);
        output.push_str(",\"alias\":");
        push_json_string(&mut output, &edge.alias);
        write!(output, ",\"ordinal\":{}}}", edge.ordinal).expect("writing to a string cannot fail");
    }
    let usage = projection.usage;
    output.push_str("],\"budget\":{");
    write!(
        output,
        "\"used_sources\":{},\"used_total_source_bytes\":{},\"used_declarations\":{},\"used_callables\":{},\"used_call_sites\":{},\"used_uses\":{},\"used_cross_file_edges\":{},\"used_dependency_depth\":{},\"used_builder_bytes\":{},\"used_manifest_bytes\":{}",
        usage.used_managed_files,
        usage.used_total_source_bytes,
        usage.used_declarations,
        usage.used_callables,
        usage.used_call_sites,
        usage.used_uses,
        usage.used_resolved_cross_file_edges,
        usage.used_dependency_depth,
        usage.used_builder_bytes,
        usage.used_manifest_bytes,
    )
    .expect("writing to a string cannot fail");
    output.push_str("},\"nonclaims\":[");
    for (index, nonclaim) in PROJECT_GRAPH_NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        push_json_string(&mut output, nonclaim);
    }
    output.push_str("]}");
    output.into_string()
}
