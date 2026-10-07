//! Documentation over immutable, admitted Project source and graph facts.
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;
use serde_json::{json, Value};

pub const SCHEMA: &str = "semaprax.project-doc.v1";

/// Render a complete Project or one declared source path. The caller retains
/// ordinary Project input authority and rechecks live inputs before delivery.
pub fn render(
    revision: &ProjectRevision,
    selected: Option<&str>,
    as_json: bool,
) -> Result<String, Vec<Diagnostic>> {
    revision.check()?;
    let graph: Value = serde_json::from_str(revision.semantic_graph()).map_err(|_| invalid())?;
    let mut modules = Vec::new();
    for source in revision.sources() {
        if source.source_graph_schema() == "semaprax.native-law.v1" {
            continue;
        }
        let (program, comments) = crate::parse_with_comments(source.source(), source.path())
            .map_err(|error| vec![error])?;
        let document = super::document(&program, &comments);
        // Project revisions bind retained comments as well as the AST. The
        // nested doc.v1 revision retains its standalone AST identity.
        if crate::graph::revision_from_canonical_program(source.source(), &program)
            != source.source_revision()
        {
            return Err(invalid());
        }
        let rendered: Value =
            serde_json::from_str(&super::render_json(&document)).map_err(|_| invalid())?;
        modules.push((source, document, rendered));
    }
    if selected.is_some_and(|path| !modules.iter().any(|(source, _, _)| source.path() == path)) {
        return Err(vec![Diagnostic::io(
            "SPX-V213",
            "documentation module must be a declared Project source path",
        )]);
    }
    // Relationship inventories remain complete even for a selected module.
    // They are the retained graph's facts, not another textual symbol index.
    if as_json {
        let documents: Vec<Value> = modules
            .iter()
            .filter(|(source, _, _)| selected.is_none_or(|path| source.path() == path))
            .map(|(source, _, document)| {
                json!({
                    "path": source.path(), "source_revision": source.source_revision(),
                    "source_digest": source.source_digest(), "document": document,
                })
            })
            .collect();
        return Ok(format!(
            "{}\n",
            json!({
                "schema": SCHEMA, "project": revision.manifest().name(),
                "project_revision": revision.project_revision(),
                "workspace_revision": revision.workspace_revision(),
                "graph_digest": revision.semantic_graph_digest(),
                "entry_module": graph["entry_module"], "test_module": graph["test_module"],
                "selected_path": selected, "modules": documents,
                "relationships": {"modules": graph["modules"], "declarations": graph["declarations"], "edges": graph["edges"]},
            })
        ));
    }
    let mut output = format!(
        "# Project `{}`\n\n- Project revision: `{}`\n- Graph digest: `{}`\n\n## Modules\n\n",
        revision.manifest().name(),
        revision.project_revision(),
        revision.semantic_graph_digest(),
    );
    for (index, (source, document, _)) in modules.iter().enumerate() {
        output.push_str(&format!(
            "- <a id=\"module-{index}\"></a>{} — `{}`\n",
            document.module,
            source.path()
        ));
    }
    for (source, document, _) in &modules {
        if selected.is_some_and(|path| source.path() != path) {
            continue;
        }
        output.push_str(&format!(
            "\n- Source: `{}`\n- Source digest: `{}`\n\n",
            source.path(),
            source.source_digest()
        ));
        output.push_str(&super::render_markdown(document));
        if !document.uses.is_empty() {
            output.push_str("### Resolved imports\n\n");
            for item in &document.uses {
                if let Some((target, _)) = modules
                    .iter()
                    .enumerate()
                    .find(|(_, (_, candidate, _))| candidate.module == item.module)
                {
                    output.push_str(&format!(
                        "- `{}` as `{}` from [{}](#module-{target})\n",
                        item.id, item.alias, item.module
                    ));
                }
            }
        }
    }
    Ok(output)
}

fn invalid() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-V213",
        "Project documentation disagrees with its authenticated graph",
    )]
}
