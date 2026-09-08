//! Replay omitted callable templates through the existing checked source path.
use super::*;

pub(crate) fn checked_source_callable_closures(
    sources: &[crate::project::ProjectSource],
    retained_templates: &BTreeSet<String>,
    defining_revision: &str,
) -> Result<Vec<serde_json::Value>, Vec<Diagnostic>> {
    if !sources
        .iter()
        .any(|source| source.source_graph_schema() == "semaprax.graph.v36")
    {
        return Ok(Vec::new());
    }
    let mut programs = Vec::with_capacity(sources.len());
    let mut candidates = BTreeSet::new();
    for source in sources {
        let parsed = crate::parse(source.source(), source.path()).map_err(|e| vec![e])?;
        if source.source_graph_schema() == "semaprax.graph.v36"
            && parsed.functions.iter().any(|f| {
                !f.type_parameters.is_empty() && !retained_templates.contains(&f.stable_id)
            })
        {
            candidates.insert(source.path().to_owned());
        }
        // Canonical reparse removes comments and their span shifts from checked
        // semantic facts. Exact original bytes remain in SourceProjection.
        let normalized = crate::format::canonical(&parsed);
        programs.push(crate::parse(&normalized, source.path()).map_err(|e| vec![e])?);
    }
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let authored = index_authored(&programs)?;
    let mut closures = Vec::new();
    let mut output_bytes = 0usize;
    for program in &programs {
        if !candidates.contains(&program.path) {
            continue;
        }
        let prebound = synthetic_builder_bytes(program, &authored, &programs)?;
        let (checked, overflowed) = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
            charge_builder_prebound(prebound.raw_clone_and_hir)?;
            let synthetic = synthetic_program(program, &authored, &programs)?;
            let resolved = crate::vec_ops::with_authenticated_linked_source(|| {
                crate::box_ops::with_authenticated_linked_source(|| hir::resolve(&synthetic))
            })?;
            verify_resolved_call_edges(program, &resolved, &authored)?;
            let omitted = resolved
                .function_templates
                .iter()
                .filter(|template| {
                    !retained_templates.contains(template.id.as_str())
                        && program
                            .functions
                            .iter()
                            .any(|authored| authored.stable_id == template.id.as_str())
                        && hir::function_value::template_uses_value(template)
                })
                .map(|template| template.id.as_str().to_owned())
                .collect::<Vec<_>>();
            if omitted.is_empty() {
                return Ok::<Option<(Vec<String>, String)>, Vec<Diagnostic>>(None);
            }
            let graph = graph::to_hir_json(&resolved, defining_revision).map_err(|e| vec![e])?;
            Ok(Some((omitted, graph)))
        });
        if overflowed {
            return Err(vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)]);
        }
        let Some((mut omitted, graph)) = checked? else {
            continue;
        };
        omitted.sort();
        output_bytes = checked_usage(output_bytes, graph.len(), "output_bytes", MAX_OUTPUT_BYTES)?;
        closures.push(serde_json::json!({
            "path": program.path,
            "defining_revision_kind": "normalized_source_workspace",
            "defining_revision": defining_revision,
            "omitted_callable_templates": omitted,
            "graph": graph,
        }));
    }
    Ok(closures)
}
