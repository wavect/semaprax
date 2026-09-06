//! Additive checked generic-instance closure in the existing semantic-program node.
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
        for (role, program) in [
            ("entry", revision.entry_program()),
            ("public_api", revision.public_api_program()),
            ("tests", revision.test_program()),
        ] {
            if program.function_instances.is_empty()
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
        let (schema, domain): (&str, &[u8]) = if closures.is_empty() {
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
