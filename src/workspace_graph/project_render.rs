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
    law_modules: &[crate::assurance_manifest::law_set::LawModule],
    digest: Option<&str>,
) -> String {
    use std::fmt::Write as _;

    let mut output = crate::bounded_output::CappedString::new();
    output.push_str("{\"schema\":");
    let has_protocols = has_tag(projection, "{\"stable_id\":");
    let has_follows = has_tag(projection, "{\"function\":");
    let base = if has_follows {
        "semaprax.project-semantic-graph.v3"
    } else if has_protocols {
        "semaprax.project-semantic-graph.v2"
    } else {
        PROJECT_GRAPH_SCHEMA
    };
    let schema = super::indexed_rust::schema(
        if super::agent_execution::has_facts(&projection.modules) {
            "semaprax.project-semantic-graph.v4"
        } else {
            base
        },
        &projection.modules,
        true,
    );
    push_json_string(
        &mut output,
        if law_modules.is_empty() || schema == "semaprax.project-semantic-graph.v7" {
            schema
        } else {
            "semaprax.project-semantic-graph.v6"
        },
    );
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
    output.push(']');
    append_facts(
        &mut output,
        projection,
        "{\"stable_id\":",
        "session_protocols",
        PROJECT_GRAPH_SCHEMA,
        "declarations",
    );
    append_facts(
        &mut output,
        projection,
        "{\"function\":",
        "session_protocol_follows",
        "semaprax.project-semantic-graph.v2",
        "bindings",
    );
    output.push_str(&super::agent_execution::render_trailing(
        base,
        &projection.modules,
    ));
    super::indexed_rust::append(&mut output, &projection.modules);
    if !law_modules.is_empty() {
        output.push_str(",\"law_modules\":");
        output.push_str(&serde_json::to_string(law_modules).expect("typed law modules serialize"));
        let mut dependencies = law_modules.iter().flat_map(|module| module.laws.iter().filter_map(|law| {
            if let crate::assurance_manifest::law_set::LawSelector::Contract { declaration_id, clause, .. } = &law.selector {
                Some(serde_json::json!({"law_id":law.law_id,"declaration_id":declaration_id,"clause":clause}))
            } else {
                None
            }
        })).collect::<Vec<_>>();
        dependencies.sort_by(|left, right| left["law_id"].as_str().cmp(&right["law_id"].as_str()));
        output.push_str(",\"law_dependencies\":");
        output.push_str(
            &serde_json::to_string(&dependencies).expect("typed law dependencies serialize"),
        );
    }
    output.push('}');
    output.into_string()
}

fn has_tag(projection: &WorkspaceGraphProjection, tag: &str) -> bool {
    projection.modules.iter().any(|module| {
        module
            .session_protocol_facts
            .iter()
            .any(|fact| fact.starts_with(tag))
    })
}

/// Borrow canonical checked facts directly; preserve their compiler order.
fn append_facts(
    output: &mut crate::bounded_output::CappedString,
    projection: &WorkspaceGraphProjection,
    tag: &str,
    section: &str,
    base: &str,
    rows: &str,
) {
    if !has_tag(projection, tag) {
        return;
    }
    output.push(',');
    push_json_string(output, section);
    output.push_str(":{\"base_schema\":");
    push_json_string(output, base);
    output.push_str(",\"authority\":\"none\",");
    push_json_string(output, rows);
    output.push_str(":[");
    let mut first = true;
    for module in &projection.modules {
        for fact in module
            .session_protocol_facts
            .iter()
            .filter(|fact| fact.starts_with(tag))
        {
            if !first {
                output.push(',');
            }
            first = false;
            output.push_str("{\"module\":");
            push_json_string(output, &module.module);
            output.push_str(",\"path\":");
            push_json_string(output, &module.path);
            output.push(',');
            output.push_str(&fact[1..]);
        }
    }
    output.push_str("]}");
}
