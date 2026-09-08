//! Additive checked generic and callable closures in the semantic-program node.
use serde_json::{json, Value};

use super::{canonical_json, framed_digest};
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticProgram {
    json: String,
    digest: String,
    schema: &'static str,
}

impl SemanticProgram {
    pub const SCHEMA: &'static str = "semaprax.semantic-workspace-revision.semantic-program.v1";
    pub const SCHEMA_V2: &'static str = "semaprax.semantic-workspace-revision.semantic-program.v2";

    pub const SCHEMA_V3: &'static str = "semaprax.semantic-workspace-revision.semantic-program.v3";

    pub const SCHEMA_V4: &'static str = "semaprax.semantic-workspace-revision.semantic-program.v4";

    pub const SCHEMA_V5: &'static str = "semaprax.semantic-workspace-revision.semantic-program.v5";

    pub(super) fn derive(
        revision: &ProjectRevision,
        mut payload: Value,
    ) -> Result<Self, Vec<Diagnostic>> {
        // Exact source bytes belong to SourceProjection and ProgramRoot. The
        // semantic instance key must not turn a comment-only edit into changed
        // checked meaning. Bind every linked role to this canonical program
        // subject before adding the graphs (there is no digest cycle).
        let subject = canonical_json(json!({
            "semantic_source": payload,
            "manifest": revision.manifest().to_canonical_toml(),
        }))?;
        let defining_revision = framed_digest(
            b"semaprax.generic-instance-program-revision.v1\0",
            subject.as_bytes(),
        );
        let mut closures = Vec::new();
        let mut has_callable_closure = false;
        let mut has_snapshot_closure = false;
        for (role, program) in [
            ("entry", revision.entry_program()),
            ("public_api", revision.public_api_program()),
            ("tests", revision.test_program()),
        ] {
            has_snapshot_closure |= crate::hir::closure::requires_closure_projection(program);
            let has_callables = crate::hir::function_value::requires_function_values(program);
            has_callable_closure |= has_callables;
            if !has_callables
                && program.function_instances.is_empty()
                && !crate::graph::requires_v35(&program.function_templates)
            {
                continue;
            }
            // The retained linked HIR is independently authenticated by the graph
            // boundary. Layout and discovery caches supply no identity facts.
            let graph = crate::graph::to_hir_json(program, &defining_revision)
                .map_err(|error| vec![error])?;
            closures.push(json!({
                "role": role,
                "defining_revision_kind": "normalized_project_semantics",
                "defining_revision": defining_revision,
                "graph": graph,
            }));
        }
        let retained_templates = [
            revision.entry_program(),
            revision.public_api_program(),
            revision.test_program(),
        ]
        .into_iter()
        .flat_map(|program| {
            program
                .function_templates
                .iter()
                .map(|template| template.id.as_str().to_owned())
        })
        .collect();
        let source_closures =
            crate::workspace_graph::source_callables::checked_source_callable_closures(
                revision.sources(),
                &retained_templates,
                &defining_revision,
            )?;
        let source_has_snapshot_closure = source_closures.iter().any(|closure| {
            closure
                .get("graph")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|graph| {
                    graph.contains("\"schema\":\"semaprax.graph.v37\"")
                        || graph.contains("\"schema\":\"semaprax.graph.v38\"")
                })
        });
        let (schema, domain): (&str, &[u8]) = if has_snapshot_closure || source_has_snapshot_closure
        {
            payload["checked_callable_closures"] = json!(closures);
            payload["checked_source_callable_closures"] = json!(source_closures);
            (
                Self::SCHEMA_V5,
                b"semaprax.semantic-workspace-revision.semantic-program.digest.v5\0",
            )
        } else if !source_closures.is_empty() {
            payload["checked_callable_closures"] = json!(closures);
            payload["checked_source_callable_closures"] = json!(source_closures);
            (
                Self::SCHEMA_V4,
                b"semaprax.semantic-workspace-revision.semantic-program.digest.v4\0",
            )
        } else if has_callable_closure {
            payload["checked_callable_closures"] = json!(closures);
            (
                Self::SCHEMA_V3,
                b"semaprax.semantic-workspace-revision.semantic-program.digest.v3\0",
            )
        } else if closures.is_empty() {
            (
                Self::SCHEMA,
                b"semaprax.semantic-workspace-revision.semantic-program.digest.v1\0",
            )
        } else {
            payload["generic_instance_closures"] = json!(closures);
            (
                Self::SCHEMA_V2,
                b"semaprax.semantic-workspace-revision.semantic-program.digest.v2\0",
            )
        };
        let json = canonical_json(json!({"schema": schema, "payload": payload}))?;
        let digest = framed_digest(domain, json.as_bytes());
        Ok(Self {
            json,
            digest,
            schema,
        })
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
}
