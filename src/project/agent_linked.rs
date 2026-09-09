//! Private Agent role closure derived only from an immutable admitted Project.
use super::ProjectRevision;
use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::semantic_workspace::{self, SemanticWorkspaceSource};

pub(crate) struct LinkedAgentProgram {
    pub(crate) program: hir::ResolvedProgram,
    pub(crate) revision: String,
    pub(crate) source_revision: String,
    pub(crate) association: String,
}

impl ProjectRevision {
    /// Derive the existing Proposal grammar from this Project's linked Agent
    /// roles. This read-only product conveys no runtime or provider authority.
    pub fn linked_agent_proposal_schema(
        &self,
        source_path: &str,
        agent_id: &str,
    ) -> Result<crate::agent_proposal::CompiledAgentProposalSchema, Vec<Diagnostic>> {
        let source = self
            .sources()
            .iter()
            .find(|source| source.path() == source_path)
            .ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-G582",
                    "linked Agent source is not retained",
                )]
            })?;
        let parsed = crate::parse(source.source(), std::path::Path::new(source_path))
            .map_err(|error| vec![error])?;
        let agent = parsed
            .agents
            .iter()
            .find(|agent| agent.stable_id == agent_id)
            .ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-G582",
                    "linked Agent identity is not declared by selected source",
                )]
            })?;
        let definition = super::compile_source_agent_declaration(agent)?;
        let linked = self.linked_agent_program(
            source_path,
            agent_id,
            definition.definition().canonical_source(),
        )?;
        crate::agent_proposal::compile_resolved_agent_proposal_schema(
            &linked.program,
            linked.source_revision,
            &definition,
        )
    }

    pub(crate) fn linked_agent_program(
        &self,
        source_path: &str,
        agent_id: &str,
        expected_definition: &str,
    ) -> Result<LinkedAgentProgram, Vec<Diagnostic>> {
        self.linked_agent_selection(source_path, agent_id, expected_definition, None)
    }

    /// Only one explicitly selected migration declaration can extend the role
    /// closure. No caller-provided HIR or collection of extra roots is accepted.
    pub(crate) fn linked_agent_migration_program(
        &self,
        source_path: &str,
        agent_id: &str,
        expected_definition: &str,
        migration_function: &str,
    ) -> Result<LinkedAgentProgram, Vec<Diagnostic>> {
        self.linked_agent_selection(
            source_path,
            agent_id,
            expected_definition,
            Some(migration_function),
        )
    }

    fn linked_agent_selection(
        &self,
        source_path: &str,
        agent_id: &str,
        expected_definition: &str,
        migration_function: Option<&str>,
    ) -> Result<LinkedAgentProgram, Vec<Diagnostic>> {
        let fail = |detail| vec![Diagnostic::io("SPX-G582", detail)];
        let source = self
            .sources()
            .iter()
            .find(|source| source.path() == source_path)
            .ok_or_else(|| fail("linked Agent source is not retained by Project"))?;
        let parsed = crate::parse(source.source(), std::path::Path::new(source_path))
            .map_err(|error| vec![error])?;
        let agent = parsed
            .agents
            .iter()
            .find(|agent| agent.stable_id == agent_id)
            .ok_or_else(|| fail("linked Agent identity is not declared by selected source"))?;
        let definition = super::compile_source_agent_declaration(agent)?;
        if definition.definition().canonical_source() != expected_definition
            || !self
                .agent_definitions()
                .iter()
                .any(|retained| retained.definition().canonical_source() == expected_definition)
        {
            return Err(fail(
                "linked Agent definition differs from retained Project",
            ));
        }
        let mut roots = Vec::new();
        for role in ["initialize", "observe", "authorize", "reduce"] {
            let (id, kind) = definition
                .definition()
                .operation(role)
                .ok_or_else(|| fail("linked Agent deterministic role missing"))?;
            if kind != "deterministic" {
                return Err(fail("linked Agent role kind differs"));
            }
            roots.push(id.to_owned());
        }
        let role_roots = roots.clone();
        let migration_source = if let Some(migration_id) = migration_function {
            let local = parsed
                .functions
                .iter()
                .any(|function| function.stable_id == migration_id && function.explicit_id);
            let import = parsed.module_uses.iter().find(|item| {
                item.kind == crate::ast::ModuleUseKind::Function
                    && item.persistent_id == migration_id
            });
            let path = if local {
                source_path.to_owned()
            } else if let Some(import) = import {
                let mut found = None;
                for candidate in self.sources() {
                    let provider =
                        crate::parse(candidate.source(), std::path::Path::new(candidate.path()))
                            .map_err(|error| vec![error])?;
                    if provider.module == import.target_module
                        && provider.functions.iter().any(|function| {
                            function.stable_id == migration_id && function.explicit_id
                        })
                        && found.replace(candidate.path().to_owned()).is_some()
                    {
                        return Err(fail("linked Agent migration declaration is ambiguous"));
                    }
                }
                found.ok_or_else(|| {
                    fail("linked Agent migration import has no retained declaration")
                })?
            } else {
                return Err(fail("linked Agent migration function is not declared or explicitly imported by selected source"));
            };
            if !roots.iter().any(|id| id == migration_id) {
                roots.push(migration_id.to_owned());
            }
            Some(path)
        } else {
            None
        };
        // Replay only retained owned bytes under the ordinary bounded Phase-A
        // verifier. This creates no shared cache publication or path authority.
        let paths = self
            .sources()
            .iter()
            .map(|source| source.path().to_owned())
            .collect::<Vec<_>>();
        let path_set = semantic_workspace::render_path_set(&paths)?;
        let sources = self
            .sources()
            .iter()
            .map(|source| SemanticWorkspaceSource {
                path: source.path().to_owned(),
                source: source.source().to_owned(),
            })
            .collect();
        let preflight = semantic_workspace::preflight_owned(&path_set, sources)?;
        let (_, manifest, revision, graph) = preflight.into_snapshot_parts();
        if revision != self.workspace_revision() || manifest != self.workspace_manifest() {
            return Err(fail(
                "linked Agent workspace replay differs from retained Project",
            ));
        }
        // This is an internal role closure, not an exported owned signature.
        // The existing linker and stage checker independently admit every call.
        let types = agent
            .types
            .iter()
            .map(|role| hir::DeclarationId::new(&role.stable_id))
            .collect::<Vec<_>>();
        let program = graph.linked_agent_role_program(&parsed.module, &roots, &types)?;
        hir::validate(&program).map_err(|error| vec![error])?;
        for id in &role_roots {
            let function = program
                .functions
                .iter()
                .find(|function| function.id.as_str() == id)
                .ok_or_else(|| fail("linked Agent role is absent from checked closure"))?;
            if !function.effects.is_empty() {
                return Err(fail("linked Agent deterministic role carries effects"));
            }
        }
        let mut facts = serde_json::json!({
            "project_revision": self.project_revision(),
            "workspace_revision": self.workspace_revision(),
            "source_path": source_path,
            "source_revision": source.source_revision(),
            "agent_id": agent_id,
            "definition_digest": definition.definition().digest(),
            "roles": role_roots,
            "functions": program.functions.iter().map(|function| function.id.as_str()).collect::<Vec<_>>(),
        });
        let schema = if let Some(migration_id) = migration_function {
            facts["migration_function"] = serde_json::json!(migration_id);
            facts["migration_source_path"] = serde_json::json!(migration_source);
            "semaprax.agent-linked-migration-source.v1"
        } else {
            "semaprax.agent-linked-source.v1"
        };
        let association = crate::execution_revision::root(schema, facts);
        Ok(LinkedAgentProgram {
            program,
            source_revision: source.source_revision().to_owned(),
            revision: association.digest().to_owned(),
            association: association.canonical_json().to_owned(),
        })
    }
}
