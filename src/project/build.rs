//! Authority-neutral complete Project v1 construction from owned source bytes.

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::semantic_workspace::{self, SemanticWorkspaceSource};

use super::{admission, semantic, ProjectManifest, ProjectSource, PublicApiSubject};

pub(super) struct BuiltProject {
    pub(super) sources: Vec<ProjectSource>,
    pub(super) law_modules: Vec<crate::assurance_manifest::law_set::LawModule>,
    pub(super) workspace_manifest: String,
    pub(super) workspace_revision: String,
    pub(super) project_revision: String,
    pub(super) entry_program: crate::hir::ResolvedProgram,
    pub(super) public_api_program: crate::hir::ResolvedProgram,
    pub(super) test_program: crate::hir::ResolvedProgram,
    pub(super) semantic: semantic::ProjectSemanticState,
    pub(super) profile_admission: admission::PreparedProjectAdmission,
    pub(super) source_agents: Vec<super::ResolvedSourceAgent>,
    pub(super) agent_definitions: Vec<crate::agent_definition::CompiledAgentDefinition>,
    pub(super) agent_interaction_contract_facts: Option<super::AgentInteractionContractFacts>,
}

/// Build and validate the complete manifest-owned Project from already-owned
/// bytes. This helper has no path, handle, read, write, or commit authority.
pub(super) fn build_owned(
    manifest: &ProjectManifest,
    mut sources: Vec<SemanticWorkspaceSource>,
) -> Result<BuiltProject, Vec<Diagnostic>> {
    let (mut sources, law_sources, law_source_facts) = partition_law_sources(manifest, sources)?;
    super::std_collections::authenticate_no_export_package(manifest, &sources)?;
    super::std_mem::authenticate_no_export_package(manifest, &sources)?;
    super::standard_dependencies::extend_sources(manifest, &mut sources)?;
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    let paths = sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();
    let path_set = semantic_workspace::render_path_set(&paths)?;
    let preflight = semantic_workspace::preflight_owned(&path_set, sources)?;
    finish_build(manifest, preflight, None, law_sources, law_source_facts)
}

pub(super) fn build_owned_with_frontend(
    manifest: &ProjectManifest,
    mut sources: Vec<SemanticWorkspaceSource>,
    frontend: &mut super::incremental::FrontendPass,
) -> Result<BuiltProject, Vec<Diagnostic>> {
    let (mut sources, law_sources, law_source_facts) = partition_law_sources(manifest, sources)?;
    super::std_collections::authenticate_no_export_package(manifest, &sources)?;
    super::std_mem::authenticate_no_export_package(manifest, &sources)?;
    super::standard_dependencies::extend_sources(manifest, &mut sources)?;
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    let paths = sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();
    let path_set = semantic_workspace::render_path_set(&paths)?;
    let preflight =
        semantic_workspace::preflight_owned_with_frontend(&path_set, sources, frontend)?;
    finish_build(
        manifest,
        preflight,
        Some(frontend),
        law_sources,
        law_source_facts,
    )
}

fn partition_law_sources(
    manifest: &ProjectManifest,
    sources: Vec<SemanticWorkspaceSource>,
) -> Result<
    (
        Vec<SemanticWorkspaceSource>,
        Vec<crate::assurance_manifest::law_set::LawModule>,
        Vec<ProjectSource>,
    ),
    Vec<Diagnostic>,
> {
    let mut ordinary = Vec::with_capacity(sources.len());
    let mut laws = Vec::new();
    let mut law_facts = Vec::new();
    for source in sources {
        if manifest.law_sources().contains(&source.path) {
            let module = crate::native_law_source::parse(&source.source, &source.path)
                .map_err(|error| vec![error])?;
            let canonical = crate::native_law_source::canonical(&module);
            let source_digest = crate::review::source_digest(source.source.as_bytes());
            let source_revision = format!(
                "sha256:{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(canonical.as_bytes()))
            );
            law_facts.push(ProjectSource {
                path: source.path.clone(),
                source_graph_schema: "semaprax.native-law.v1".to_owned(),
                source_revision,
                source_digest,
                source: source.source,
            });
            laws.push(module.law_module());
        } else {
            ordinary.push(source);
        }
    }
    if laws.len() != manifest.law_sources().len() {
        return Err(vec![Diagnostic::io(
            "SPX-LW110",
            "an explicitly selected native law source is absent from the Project source inventory",
        )]);
    }
    Ok((ordinary, laws, law_facts))
}

fn finish_build(
    manifest: &ProjectManifest,
    preflight: semantic_workspace::SemanticWorkspacePreflight,
    frontend: Option<&super::incremental::FrontendPass>,
    law_modules: Vec<crate::assurance_manifest::law_set::LawModule>,
    mut law_source_facts: Vec<ProjectSource>,
) -> Result<BuiltProject, Vec<Diagnostic>> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _workflow_span = crate::workflow_profile::span(crate::workflow_profile::Stage::ProjectLink);
    let (files, ordinary_workspace_manifest, ordinary_workspace_revision, graph) =
        preflight.into_snapshot_parts();
    let (workspace_manifest, workspace_revision) = if law_source_facts.is_empty() {
        (ordinary_workspace_manifest, ordinary_workspace_revision)
    } else {
        let mut facts = files
            .iter()
            .map(|file| {
                (
                    file.path(),
                    file.source_graph_schema(),
                    file.source_revision(),
                    file.source_digest(),
                    file.source().len(),
                )
            })
            .collect::<Vec<_>>();
        facts.extend(law_source_facts.iter().map(|source| {
            (
                source.path.as_str(),
                source.source_graph_schema.as_str(),
                source.source_revision.as_str(),
                source.source_digest.as_str(),
                source.source.len(),
            )
        }));
        facts.sort_by(|left, right| left.0.cmp(right.0));
        let manifest = semantic_workspace::render_manifest_facts(&facts)?;
        let revision = semantic_workspace::semantic_workspace_revision(&manifest);
        (manifest, revision)
    };
    // The frontend pass already owns exact, authenticated source ASTs. Reuse
    // those for Agent extraction instead of reparsing outside its work counters.
    let programs = if frontend.is_none() {
        files
            .iter()
            .map(|file| crate::parse(file.source(), file.path()).map_err(|error| vec![error]))
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?
    } else {
        Vec::new()
    };
    let program_refs = if let Some(frontend) = frontend {
        files
            .iter()
            .map(|file| frontend.retained_source_program(file.path(), file.source()))
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?
    } else {
        programs.iter().collect::<Vec<_>>()
    };
    validate_native_laws(&law_source_facts, &program_refs)?;
    let (source_agents, agent_definitions) =
        super::compile_source_project_agents(&program_refs)?.into_parts();
    let canonical_manifest = manifest.to_canonical_toml();
    let project_revision = project_revision(manifest, &canonical_manifest, &workspace_revision);
    let graph_source_facts = files
        .iter()
        .map(|file| crate::workspace_graph::ProjectGraphSourceFact {
            path: file.path().to_owned(),
            source_graph_schema: file.source_graph_schema().to_owned(),
            source_revision: file.source_revision().to_owned(),
            source_digest: file.source_digest().to_owned(),
        })
        .collect();
    let filesystem_roots = manifest
        .command()
        .map(|id| vec![id.to_owned()])
        .unwrap_or_default();
    let provider_agent_schemas = super::agent_contract_facts::prepare_provider_agent_schemas(
        &graph,
        manifest.entry(),
        &files,
        &program_refs,
        &agent_definitions,
    )?;
    // The closed indexed Future profile has two independent retained products:
    // its one source-local Future root and the exact Regex/Url Web exports
    // whose selected Rust paths feed the generated M1 packages.  Retain their
    // authenticated union so the native SDK subject cannot silently lose the
    // indexed import facts while rendering either package.
    let mut indexed_future_roots = Vec::new();
    if manifest.project_profile() == super::ProjectProfile::SourceLocalFutureIndexedRustV1 {
        indexed_future_roots.extend_from_slice(manifest.web_exports());
        indexed_future_roots.extend_from_slice(manifest.rust_async_exports());
        indexed_future_roots.sort();
        indexed_future_roots.dedup();
    }
    let semantic_parts = graph.into_project_semantic_parts(
        &workspace_revision,
        graph_source_facts,
        canonical_manifest.len(),
        manifest.entry(),
        manifest.test_module(),
        crate::workspace_graph::ProjectWebRoots {
            stable_ids: if manifest.project_profile()
                == super::ProjectProfile::SourceLocalFutureIndexedRustV1
            {
                &indexed_future_roots
            } else if manifest.project_profile().is_source_local_future() {
                manifest.rust_async_exports()
            } else if manifest.project_profile().is_filesystem()
                || manifest.project_profile() == super::ProjectProfile::EnvironmentIoV1
                || manifest.project_profile() == super::ProjectProfile::ProcessIoV1
                || manifest.project_profile() == super::ProjectProfile::SourceCommandV1
            {
                &filesystem_roots
            } else {
                manifest.web_exports()
            },
            profile: manifest.project_profile(),
            dependency_anchors: !manifest.dependency_sources().is_empty(),
        },
    )?;
    let selected_source_agents = source_agents.iter().filter(|agent| {
        !agent.has_execution_metadata()
            || agent.execution_functions_present(&semantic_parts.entry_program.functions)
    });
    if !selected_source_agents.eq(semantic_parts.entry_program.agents.iter()) {
        return Err(vec![Diagnostic::io(
            "SPX-G559",
            "source Agent lowering disagrees with the retained linked HIR inventory",
        )]);
    }
    let source_agents = semantic_parts.entry_program.agents.clone();
    let semantic = semantic::ProjectSemanticState::new(
        semantic_parts.projection,
        manifest.schema(),
        manifest.name(),
        &project_revision,
        manifest.test_module(),
        &law_modules,
    )?;
    // This is the complete public target admission gate used by ordinary
    // Project loading. Candidate planning must not validate a weaker profile,
    // and every additive schema must pass this one exhaustive dispatcher.
    let profile_admission = {
        #[cfg(feature = "unstable-workflow-profiling")]
        let _workflow_span =
            crate::workflow_profile::span(crate::workflow_profile::Stage::TargetAdmission);
        admission::prepare(
            manifest,
            &semantic_parts.web_program,
            PublicApiSubject {
                project_schema: manifest.schema(),
                project_revision: &project_revision,
                workspace_revision: &workspace_revision,
                project_graph_digest: semantic.graph_digest(),
            },
        )
        .map_err(|error| vec![error])?
    };
    let agent_interaction_contract_facts = if agent_definitions.is_empty() {
        None
    } else {
        Some(super::AgentInteractionContractFacts::derive(
            &project_revision,
            &workspace_revision,
            semantic.graph_digest(),
            &files,
            &program_refs,
            &agent_definitions,
            provider_agent_schemas,
        )?)
    };
    // Keep execution bound to the entry-only closure while retaining the
    // independently admitted entry-plus-export closure for public targets.
    // Conflating these programs changes cleanup plans and executable ABIs;
    // discarding the latter loses selected exports that `main` does not call.
    let entry_program = semantic_parts.entry_program;
    let public_api_program = semantic_parts.web_program;
    let test_program = semantic_parts.test_program;
    let mut sources: Vec<ProjectSource> = files
        .into_iter()
        .map(|file| {
            let (path, source_graph_schema, source_revision, source_digest, source) =
                file.into_parts();
            ProjectSource {
                path,
                source_graph_schema,
                source_revision,
                source_digest,
                source,
            }
        })
        .collect();
    sources.append(&mut law_source_facts);
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(BuiltProject {
        sources,
        law_modules,
        workspace_manifest,
        workspace_revision,
        project_revision,
        entry_program,
        public_api_program,
        test_program,
        semantic,
        profile_admission,
        source_agents,
        agent_definitions,
        agent_interaction_contract_facts,
    })
}

fn validate_native_laws(
    law_sources: &[ProjectSource],
    programs: &[&crate::ast::Program],
) -> Result<(), Vec<Diagnostic>> {
    use crate::assurance_manifest::law_set::ContractKind;
    use crate::native_law_source::NativeLawSubject;
    let mut identities = std::collections::BTreeSet::new();
    for source in law_sources {
        let module = crate::native_law_source::parse(&source.source, &source.path)
            .map_err(|error| vec![error])?;
        for law in module.laws {
            if !identities.insert(law.law_id.clone()) {
                return Err(vec![Diagnostic::io(
                    "SPX-LW110",
                    "duplicate native law stable ID across selected modules",
                )]);
            }
            let NativeLawSubject::Contract { subject_id, clause } = &law.subject else {
                // The parser has already checked the independent relation's
                // typed binders and pure scalar proposition. It has no
                // function-contract owner to resolve here.
                continue;
            };
            let mut subjects = programs
                .iter()
                .flat_map(|program| {
                    program
                        .functions
                        .iter()
                        .chain(program.types.iter().flat_map(|ty| match &ty.kind {
                            crate::ast::TypeDeclarationKind::Class { methods, .. } => {
                                methods.iter()
                            }
                            _ => [].iter(),
                        }))
                })
                .filter(|function| function.stable_id == *subject_id);
            let Some(function) = subjects.next() else {
                return Err(vec![Diagnostic::io(
                    "SPX-LW110",
                    format!(
                        "native law `{}` has an unresolved contract subject `{}`",
                        law.law_id, subject_id,
                    ),
                )]);
            };
            if subjects.next().is_some() || !function.explicit_id {
                return Err(vec![Diagnostic::io(
                    "SPX-LW110",
                    "native law contract subject is ambiguous or lacks a persistent @id",
                )]);
            }
            for binder in &law.binders {
                let parameter = function.params.iter().any(|param| {
                    param.name == binder.name && param.ty.to_string() == binder.ty.source()
                });
                let result = *clause == ContractKind::Postcondition
                    && binder.name == "result"
                    && function.return_type.to_string() == binder.ty.source();
                if !parameter && !result {
                    return Err(vec![Diagnostic::io(
                        "SPX-LW110",
                        format!(
                            "native law `{}` binder `{}` does not match a typed subject parameter or postcondition result",
                            law.law_id, binder.name,
                        ),
                    )]);
                }
            }
            let clauses = match clause {
                ContractKind::Precondition => &function.requires,
                ContractKind::Postcondition => &function.ensures,
            };
            if clauses
                .iter()
                .filter(|expr| crate::format::expr(expr, 0) == law.proposition)
                .count()
                != 1
            {
                return Err(vec![Diagnostic::io(
                    "SPX-LW110",
                    format!(
                        "native law `{}` must select exactly one subject contract clause",
                        law.law_id,
                    ),
                )]);
            }
        }
    }
    Ok(())
}

fn project_revision(manifest: &ProjectManifest, bytes: &str, workspace_revision: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(
        if manifest.manifest_schema() == super::PACKAGE_MANIFEST_SCHEMA_V2 {
            b"semaprax.project-revision.v2\0".as_slice()
        } else {
            b"semaprax.project-revision.v1\0".as_slice()
        },
    );
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes.as_bytes());
    digest.update((workspace_revision.len() as u64).to_le_bytes());
    digest.update(workspace_revision.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

#[cfg(test)]
mod tests;
