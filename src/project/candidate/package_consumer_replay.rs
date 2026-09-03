//! Explicit package-consumer replay against one exact candidate source.
//! Caller-owned package evidence is independently authenticated and retained
//! only for the duration of this read-only query.

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;
use crate::package_lock_v2::Coordinate;
use crate::package_resolver::{ResolutionInput, ResolutionOptions};
use crate::package_semantic_graph::{
    PackageSemanticGraph, PACKAGE_SEMANTIC_CONSUMERS_SCHEMA, PACKAGE_SEMANTIC_SUMMARY_SCHEMA,
};
use crate::package_source_capsule::{PackageSource, SourceCapsuleOptions};
use crate::project::MAX_PATH_BYTES;

use super::ProjectCandidate;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_CANDIDATE_PACKAGE_CONSUMER_REPLAY_SCHEMA: &str =
    "semaprax.project-candidate-package-consumer-replay.v1";
pub const MAX_PROJECT_CANDIDATE_PACKAGE_CONSUMER_REPLAY_BYTES: usize = 2 * 1024 * 1024;
pub const PROJECT_CANDIDATE_PACKAGE_SIGNATURE_CONFLICTS_SCHEMA: &str =
    "semaprax.project-candidate-package-signature-conflicts.v1";
pub const MAX_PROJECT_CANDIDATE_PACKAGE_SIGNATURE_CONFLICTS_BYTES: usize = 2 * 1024 * 1024;
pub const PROJECT_CANDIDATE_PACKAGE_CONSUMER_MIGRATION_SCHEMA: &str =
    "semaprax.project-candidate-package-consumer-migration.v1";
pub const MAX_PROJECT_CANDIDATE_PACKAGE_CONSUMER_MIGRATION_BYTES: usize = 16 * 1024 * 1024;

const NONCLAIMS: [&str; 8] = [
    "no_ambient_consumer_discovery_or_completeness",
    "candidate_association_covers_only_the_selected_provider_source",
    "not_api_abi_or_behavioral_compatibility",
    "imports_do_not_prove_calls",
    "calls_are_static_authenticated_source_sites_not_runtime_execution",
    "no_test_build_artifact_or_deployment_evidence",
    "no_filesystem_network_registry_or_dependency_acquisition_authority",
    "no_source_mutation_candidate_or_graph_retention_or_publication_authority",
];

const SIGNATURE_CONFLICT_NONCLAIMS: [&str; 10] = [
    "no_ambient_installed_registry_workspace_or_deployment_consumer_discovery",
    "known_consumers_cover_only_the_explicit_authenticated_baseline_capsule",
    "signature_conflict_is_not_a_claim_that_every_affected_call_fails_to_compile",
    "no_automatic_consumer_migration_or_candidate_era_consumer_acceptance",
    "not_runtime_abi_behavioral_or_deployment_compatibility",
    "imports_do_not_prove_calls",
    "calls_are_static_authenticated_baseline_source_sites_not_runtime_execution",
    "no_test_build_artifact_or_external_api_evidence",
    "no_filesystem_network_registry_or_dependency_acquisition_authority",
    "no_source_mutation_candidate_or_graph_retention_or_publication_authority",
];

/// Exact caller-owned evidence required to replay one explicit package corpus.
/// The provider report and capsule must be from the candidate-era source; a
/// baseline package graph or report is intentionally insufficient.
pub struct CandidatePackageConsumerReplayInput<'a> {
    pub provider: &'a Coordinate,
    pub provider_source_path: &'a str,
    pub target: &'a str,
    pub capsule: &'a str,
    pub sources: &'a [PackageSource],
    pub resolution_evidence: &'a str,
    pub resolution_input: &'a ResolutionInput,
    pub resolution_options: &'a ResolutionOptions,
    pub capsule_options: &'a SourceCapsuleOptions,
}

/// Exact caller-owned baseline evidence for checking a changed provider
/// signature against consumers that were admitted with the base source. This
/// input is deliberately distinct from candidate-era replay evidence.
pub struct CandidatePackageSignatureConflictInput<'a> {
    pub provider: &'a Coordinate,
    pub provider_source_path: &'a str,
    pub target: &'a str,
    pub baseline_capsule: &'a str,
    pub baseline_sources: &'a [PackageSource],
    pub baseline_resolution_evidence: &'a str,
    pub baseline_resolution_input: &'a ResolutionInput,
    pub baseline_resolution_options: &'a ResolutionOptions,
    pub baseline_capsule_options: &'a SourceCapsuleOptions,
}

/// Exact baseline corpus plus caller-supplied candidate-era package evidence.
/// The migration is emitted only after the rebuilt candidate-era capsule and
/// package graph independently admit every proposed canonical source.
pub struct CandidatePackageConsumerMigrationInput<'a> {
    pub baseline: CandidatePackageSignatureConflictInput<'a>,
    pub candidate_provider_report: &'a str,
    pub candidate_resolution_evidence: &'a str,
    pub candidate_resolution_input: &'a ResolutionInput,
    pub candidate_resolution_options: &'a ResolutionOptions,
    pub candidate_capsule_options: &'a SourceCapsuleOptions,
}

impl ProjectCandidate {
    /// Propose one closed scalar/Copy append-parameter migration for consumers
    /// in an exact authenticated baseline package corpus. No source is written.
    pub fn package_consumer_migration(
        &self,
        expected_candidate: &str,
        input: &CandidatePackageConsumerMigrationInput<'_>,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let change = if self.changes.len() == 1 {
            &self.changes[0].intent
        } else {
            return Err(migration_refusal(
                "consumer migration requires exactly one candidate change",
            ));
        };
        let target = change
            .get("target")
            .and_then(Value::as_str)
            .ok_or_else(|| migration_refusal("consumer migration change has no exact target"))?;
        if change.get("kind").and_then(Value::as_str) != Some("change_function_signature")
            || target != input.baseline.target
            || change
                .as_object()
                .is_none_or(|object| object.len() != 3 || !object.contains_key("append_parameters"))
        {
            return Err(migration_refusal(
                "consumer migration admits only one scalar Copy append-parameter change",
            ));
        }
        let additions = change["append_parameters"].as_array().ok_or_else(|| {
            migration_refusal("consumer migration append-parameter table is absent")
        })?;
        if additions.is_empty()
            || additions.len() > 8
            || additions.iter().any(|addition| {
                !matches!(
                    addition.get("type").and_then(Value::as_str),
                    Some("i64" | "i32" | "char" | "u8" | "usize" | "f32" | "f64" | "bool")
                )
            })
        {
            return Err(migration_refusal(
                "consumer migration append parameters must be one through eight Copy scalars",
            ));
        }

        let conflicts: Value = serde_json::from_str(
            &self.package_signature_consumer_conflicts(expected_candidate, &input.baseline)?,
        )
        .map_err(|_| migration_binding("authenticated signature conflict report is not JSON"))?;
        let affected_calls = array(&conflicts, "affected_calls")?;
        if affected_calls.is_empty() {
            return Err(migration_refusal(
                "consumer migration requires at least one authenticated affected call",
            ));
        }

        let mut programs = Vec::with_capacity(input.baseline.baseline_sources.len());
        for source in input.baseline.baseline_sources {
            let program = crate::parse(&source.source, format!("package:{}", source.package))
                .map_err(|error| vec![error])?;
            if crate::format::canonical(&program) != source.source {
                return Err(migration_binding(
                    "baseline consumer corpus source is not exact canonical source",
                ));
            }
            programs.push(program);
        }
        let mut owner = None;
        for (program_index, program) in programs.iter().enumerate() {
            for (function_index, function) in program.functions.iter().enumerate() {
                if function.stable_id == target
                    && function.explicit_id
                    && owner.replace((program_index, function_index)).is_some()
                {
                    return Err(migration_binding(
                        "baseline provider function identity is ambiguous",
                    ));
                }
            }
        }
        let (owner, function) = owner.ok_or_else(|| {
            migration_binding("baseline provider function is absent from the exact corpus")
        })?;
        let migrated_calls =
            super::intent::apply_detached_signature(&mut programs, change, owner, function)?;
        if migrated_calls != affected_calls.len() {
            return Err(migration_refusal(
                "consumer migration excludes provider-local or unauthenticated call inventory",
            ));
        }

        let candidate_provider = unique_source(
            self.revision.sources(),
            input.baseline.provider_source_path,
            "candidate provider source is absent or duplicated",
        )?;
        let mut migrated_sources = Vec::with_capacity(programs.len());
        let mut proposals = Vec::new();
        for ((baseline, program), index) in input
            .baseline
            .baseline_sources
            .iter()
            .zip(programs)
            .zip(0usize..)
        {
            let source = crate::format::canonical(&program);
            let provider = baseline.package == input.baseline.provider.package;
            if provider && source != candidate_provider.source() {
                return Err(migration_refusal(
                    "reconstructed provider does not exactly equal candidate source",
                ));
            }
            if !provider && source != baseline.source {
                proposals.push(json!({
                    "package": baseline.package,
                    "source_index": index,
                    "baseline_source_digest": super::wire::digest(b"semaprax.package-consumer-migration.source.v1\0", baseline.source.as_bytes()),
                    "proposed_source_digest": super::wire::digest(b"semaprax.package-consumer-migration.source.v1\0", source.as_bytes()),
                    "proposed_source": source,
                }));
            }
            migrated_sources.push(PackageSource {
                package: baseline.package.clone(),
                report: if provider {
                    input.candidate_provider_report.to_owned()
                } else {
                    baseline.report.clone()
                },
                source,
            });
        }
        if proposals.is_empty() {
            return Err(migration_refusal(
                "authenticated affected calls produced no consumer source proposal",
            ));
        }
        let capsule = crate::package_source_capsule::generate(
            &migrated_sources,
            input.candidate_resolution_evidence,
            input.candidate_resolution_input,
            input.candidate_resolution_options,
            input.candidate_capsule_options,
        )?;
        let replay = self.package_consumer_replay(
            expected_candidate,
            &CandidatePackageConsumerReplayInput {
                provider: input.baseline.provider,
                provider_source_path: input.baseline.provider_source_path,
                target,
                capsule: &capsule,
                sources: &migrated_sources,
                resolution_evidence: input.candidate_resolution_evidence,
                resolution_input: input.candidate_resolution_input,
                resolution_options: input.candidate_resolution_options,
                capsule_options: input.candidate_capsule_options,
            },
        )?;
        let replay: Value = serde_json::from_str(&replay)
            .map_err(|_| migration_binding("candidate-era package replay is not JSON"))?;
        if replay["counts"]["calls"] != affected_calls.len() {
            return Err(migration_refusal(
                "candidate-era replay call inventory differs from the migrated baseline inventory",
            ));
        }
        render_migration(json!({
            "schema": PROJECT_CANDIDATE_PACKAGE_CONSUMER_MIGRATION_SCHEMA,
            "candidate_revision": self.candidate_digest(),
            "base_project_revision": self.base.project_revision(),
            "candidate_project_revision": self.revision.project_revision(),
            "provider": {"package":input.baseline.provider.package,"version":input.baseline.provider.version},
            "target": target,
            "change": change,
            "baseline_conflict_report_digest": super::wire::digest(b"semaprax.package-consumer-migration.conflicts.v1\0", serde_json::to_vec(&conflicts).map_err(|_| migration_binding("conflict report serialization failed"))?.as_slice()),
            "candidate_source_capsule": {
                "digest": replay["source_capsule_digest"],
                "bytes": capsule.len(),
                "source_set_digest": replay["source_set_digest"],
                "link_digest": replay["link_digest"],
                "complete_bytes_embedded": false,
            },
            "candidate_replay": replay,
            "proposals": proposals,
            "counts":{"affected_calls":affected_calls.len(),"migrated_calls":migrated_calls,"changed_consumer_sources":proposals.len()},
            "status":"proposed_sources_independently_admitted_by_candidate_era_capsule_replay",
            "generated_artifact_provenance":"not_supplied_or_inferred",
            "source_authority":false,"execution":false,"publication_authority":false,
            "compatibility":"not_assessed","tests":"not_run",
            "nonclaims":["explicit_baseline_capsule_only_no_consumer_discovery_or_completeness","scalar_Copy_append_defaults_only_not_general_signature_migration","canonical_source_proposal_not_write_patch_commit_or_publication_authority","candidate_era_source_capsule_replay_not_runtime_behavior_or_compatibility","no_installed_generated_or_deployed_consumer_claim","generated_artifact_provenance_not_inferred","no_filesystem_network_registry_model_tool_or_target_execution_authority"]
        }))
    }

    /// Independently replay explicit package consumers against one source from
    /// this exact candidate. This is static source admission, not compatibility.
    pub fn package_consumer_replay(
        &self,
        expected_candidate: &str,
        input: &CandidatePackageConsumerReplayInput<'_>,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let manifest = self.revision.manifest();
        if manifest.name() != input.provider.package
            || manifest.package_version() != Some(input.provider.version.as_str())
        {
            return Err(association(
                "candidate manifest name and version disagree with the provider coordinate",
            ));
        }
        if input.provider_source_path.is_empty()
            || input.provider_source_path.len() > MAX_PATH_BYTES
            || input.provider_source_path.contains('\0')
        {
            return Err(association(
                "candidate provider source path is outside its exact logical-path bound",
            ));
        }
        let candidate_source = unique_source(
            self.revision.sources(),
            input.provider_source_path,
            "candidate provider source is absent or duplicated",
        )?;
        let base_source = unique_source(
            self.base.sources(),
            input.provider_source_path,
            "base provider source is absent or duplicated",
        )?;
        let mut provider_sources = input
            .sources
            .iter()
            .filter(|source| source.package == input.provider.package);
        let provider_source = provider_sources.next().ok_or_else(|| {
            association("explicit package corpus has no candidate provider source")
        })?;
        if provider_sources.next().is_some() || provider_source.source != candidate_source.source()
        {
            return Err(association(
                "explicit package corpus provider source does not exactly equal the candidate source",
            ));
        }

        let graph = PackageSemanticGraph::derive(
            input.capsule,
            input.sources,
            input.resolution_evidence,
            input.resolution_input,
            input.resolution_options,
            input.capsule_options,
        )?;
        let graph_revision = graph.graph_digest().to_owned();
        let summary = parse_report(
            &graph.summary(&graph_revision)?,
            PACKAGE_SEMANTIC_SUMMARY_SCHEMA,
        )?;
        let consumers = parse_report(
            &graph.consumers(&graph_revision, input.provider, input.target)?,
            PACKAGE_SEMANTIC_CONSUMERS_SCHEMA,
        )?;
        let coordinate = json!({
            "package": input.provider.package,
            "version": input.provider.version,
        });
        let provider_rows = summary["packages"]
            .as_array()
            .ok_or_else(|| association("verified package graph package inventory is absent"))?
            .iter()
            .filter(|row| row["coordinate"] == coordinate)
            .collect::<Vec<_>>();
        let provider_fact = if provider_rows.len() == 1 {
            provider_rows[0]
        } else {
            return Err(association(
                "verified package graph provider inventory is absent or ambiguous",
            ));
        };
        let package_source_digest =
            crate::package_source_capsule::semantic_graph_source_digest(candidate_source.source());
        if summary["graph_revision"] != graph_revision
            || consumers["graph_revision"] != graph_revision
            || consumers["provider"] != coordinate
            || consumers["target"] != input.target
            || consumers["provider_source_revision"] != candidate_source.source_revision()
            || consumers["provider_source_digest"] != package_source_digest
            || provider_fact["source_revision"] != candidate_source.source_revision()
            || provider_fact["source_digest"] != package_source_digest
            || provider_fact["source_bytes"] != candidate_source.source().len()
            || provider_fact["interface_source_revision"] != candidate_source.source_revision()
            || consumers["project_association"] != "none"
        {
            return Err(association(
                "verified package graph consumers disagree with candidate provider bindings",
            ));
        }
        for report in [&summary, &consumers] {
            if report["source_authority"] != false
                || report["execution"] != false
                || report["publication_authority"] != false
            {
                return Err(association(
                    "verified package graph report claims unsupported authority",
                ));
            }
        }
        let imports = array(&consumers, "imports")?;
        let calls = array(&consumers, "calls")?;
        let packages = summary["counts"]["packages"]
            .as_u64()
            .ok_or_else(|| association("verified package graph package count is absent"))?;
        render(json!({
            "schema": PROJECT_CANDIDATE_PACKAGE_CONSUMER_REPLAY_SCHEMA,
            "candidate_revision": self.candidate_digest(),
            "base_project_revision": self.base.project_revision(),
            "candidate_project_revision": self.revision.project_revision(),
            "candidate_workspace_revision": self.revision.workspace_revision(),
            "candidate_project_graph_digest": self.revision.semantic_graph_digest(),
            "provider": coordinate,
            "provider_source": {
                "path": candidate_source.path(),
                "provider_interface_source_revision": provider_fact["interface_source_revision"],
                "provider_package_source_digest": provider_fact["source_digest"],
                "base_source_revision": base_source.source_revision(),
                "base_source_digest": base_source.source_digest(),
                "candidate_source_revision": candidate_source.source_revision(),
                "candidate_source_digest": candidate_source.source_digest(),
                "source_bytes": candidate_source.source().len(),
                "changed_from_base": base_source.source_revision() != candidate_source.source_revision(),
            },
            "target": input.target,
            "package_graph_revision": graph_revision,
            "source_capsule_digest": summary["source_capsule_digest"],
            "source_set_digest": summary["source_set_digest"],
            "link_digest": summary["link_digest"],
            "imports": imports,
            "calls": calls,
            "counts": {"packages": packages, "imports": imports.len(), "calls": calls.len()},
            "association": "exact_candidate_source_projection_in_explicit_package_replay",
            "validation": "verified_package_source_capsule_and_candidate_source_identity",
            "tests": "not_run",
            "project_association": "candidate_provider_source_projection_only",
            "source_authority": false,
            "execution": false,
            "publication_authority": false,
            "candidate_retained": false,
            "graph_retained": false,
            "nonclaims": NONCLAIMS,
        }))
    }

    /// Compare one exact base/candidate provider signature and expose only the
    /// consumers authenticated by an explicit baseline package corpus. A
    /// changed signature conservatively conflicts with those retained source
    /// sites until a separate candidate-era package replay accepts them.
    pub fn package_signature_consumer_conflicts(
        &self,
        expected_candidate: &str,
        input: &CandidatePackageSignatureConflictInput<'_>,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let manifest = self.revision.manifest();
        if manifest.name() != input.provider.package
            || manifest.package_version() != Some(input.provider.version.as_str())
        {
            return Err(association(
                "candidate manifest name and version disagree with the provider coordinate",
            ));
        }
        if input.provider_source_path.is_empty()
            || input.provider_source_path.len() > MAX_PATH_BYTES
            || input.provider_source_path.contains('\0')
        {
            return Err(association(
                "candidate provider source path is outside its exact logical-path bound",
            ));
        }
        let base_source = unique_source(
            self.base.sources(),
            input.provider_source_path,
            "base provider source is absent or duplicated",
        )?;
        let candidate_source = unique_source(
            self.revision.sources(),
            input.provider_source_path,
            "candidate provider source is absent or duplicated",
        )?;
        let mut provider_sources = input
            .baseline_sources
            .iter()
            .filter(|source| source.package == input.provider.package);
        let provider_source = provider_sources.next().ok_or_else(|| {
            association("explicit package corpus has no baseline provider source")
        })?;
        if provider_sources.next().is_some() || provider_source.source != base_source.source() {
            return Err(association(
                "explicit package corpus provider source does not exactly equal the base source",
            ));
        }

        let graph = PackageSemanticGraph::derive(
            input.baseline_capsule,
            input.baseline_sources,
            input.baseline_resolution_evidence,
            input.baseline_resolution_input,
            input.baseline_resolution_options,
            input.baseline_capsule_options,
        )?;
        let graph_revision = graph.graph_digest().to_owned();
        let summary = parse_report(
            &graph.summary(&graph_revision)?,
            PACKAGE_SEMANTIC_SUMMARY_SCHEMA,
        )?;
        let consumers = parse_report(
            &graph.consumers(&graph_revision, input.provider, input.target)?,
            PACKAGE_SEMANTIC_CONSUMERS_SCHEMA,
        )?;
        let coordinate = json!({
            "package": input.provider.package,
            "version": input.provider.version,
        });
        let provider_rows = summary["packages"]
            .as_array()
            .ok_or_else(|| association("verified package graph package inventory is absent"))?
            .iter()
            .filter(|row| row["coordinate"] == coordinate)
            .collect::<Vec<_>>();
        let provider_fact = if provider_rows.len() == 1 {
            provider_rows[0]
        } else {
            return Err(association(
                "verified package graph provider inventory is absent or ambiguous",
            ));
        };
        let package_source_digest =
            crate::package_source_capsule::semantic_graph_source_digest(base_source.source());
        if summary["graph_revision"] != graph_revision
            || consumers["graph_revision"] != graph_revision
            || consumers["provider"] != coordinate
            || consumers["target"] != input.target
            || consumers["provider_source_revision"] != base_source.source_revision()
            || consumers["provider_source_digest"] != package_source_digest
            || provider_fact["source_revision"] != base_source.source_revision()
            || provider_fact["source_digest"] != package_source_digest
            || provider_fact["source_bytes"] != base_source.source().len()
            || provider_fact["interface_source_revision"] != base_source.source_revision()
            || consumers["project_association"] != "none"
        {
            return Err(association(
                "verified package graph consumers disagree with base provider bindings",
            ));
        }
        for report in [&summary, &consumers] {
            if report["source_authority"] != false
                || report["execution"] != false
                || report["publication_authority"] != false
            {
                return Err(association(
                    "verified package graph report claims unsupported authority",
                ));
            }
        }

        let base_signature =
            selected_signature(&self.base, input.provider_source_path, input.target)?;
        let candidate_signature =
            selected_signature(&self.revision, input.provider_source_path, input.target)?;
        let changed_facets = changed_signature_facets(&base_signature, &candidate_signature);
        let changed = !changed_facets.is_empty();
        let imports = array(&consumers, "imports")?;
        let calls = array(&consumers, "calls")?;
        let affected_imports = if changed { imports.as_slice() } else { &[] };
        let affected_calls = if changed { calls.as_slice() } else { &[] };
        let status = if !changed {
            "selected_signature_unchanged"
        } else if !affected_calls.is_empty() {
            "known_source_consumers_require_candidate_era_replay"
        } else if !affected_imports.is_empty() {
            "known_importers_require_candidate_era_review"
        } else {
            "signature_changed_without_known_consumers_in_explicit_corpus"
        };
        let packages = summary["counts"]["packages"]
            .as_u64()
            .ok_or_else(|| association("verified package graph package count is absent"))?;
        render_signature_conflicts(json!({
            "schema": PROJECT_CANDIDATE_PACKAGE_SIGNATURE_CONFLICTS_SCHEMA,
            "candidate_revision": self.candidate_digest(),
            "base_project_revision": self.base.project_revision(),
            "candidate_project_revision": self.revision.project_revision(),
            "candidate_workspace_revision": self.revision.workspace_revision(),
            "candidate_project_graph_digest": self.revision.semantic_graph_digest(),
            "provider": coordinate,
            "provider_source": {
                "path": input.provider_source_path,
                "baseline_interface_source_revision": provider_fact["interface_source_revision"],
                "baseline_package_source_digest": provider_fact["source_digest"],
                "base_source_revision": base_source.source_revision(),
                "base_source_digest": base_source.source_digest(),
                "candidate_source_revision": candidate_source.source_revision(),
                "candidate_source_digest": candidate_source.source_digest(),
                "changed_from_base": base_source.source_revision() != candidate_source.source_revision(),
            },
            "target": input.target,
            "base_signature": base_signature,
            "candidate_signature": candidate_signature,
            "signature_changed": changed,
            "changed_facets": changed_facets,
            "compatibility_status": status,
            "conflict_basis": "authenticated_baseline_consumers_and_exact_retained_provider_signatures",
            "required_next_step": "construct_candidate_era_consumer_sources_and_replay_the_complete_package_capsule",
            "baseline_package_graph_revision": graph_revision,
            "baseline_source_capsule_digest": summary["source_capsule_digest"],
            "baseline_source_set_digest": summary["source_set_digest"],
            "baseline_link_digest": summary["link_digest"],
            "affected_imports": affected_imports,
            "affected_calls": affected_calls,
            "counts": {
                "packages": packages,
                "known_imports": imports.len(),
                "known_calls": calls.len(),
                "affected_imports": affected_imports.len(),
                "affected_calls": affected_calls.len(),
            },
            "association": "exact_base_provider_source_and_explicit_baseline_package_consumers",
            "validation": "verified_baseline_package_source_capsule_and_exact_base_candidate_signature_comparison",
            "tests": "not_run",
            "source_authority": false,
            "execution": false,
            "publication_authority": false,
            "candidate_retained": false,
            "graph_retained": false,
            "nonclaims": SIGNATURE_CONFLICT_NONCLAIMS,
        }))
    }
}

fn selected_signature(
    revision: &crate::project::ProjectRevision,
    path: &str,
    target: &str,
) -> Result<Value> {
    let graph: Value = serde_json::from_str(revision.semantic_graph())
        .map_err(|_| association("retained Project graph is not JSON"))?;
    let declarations = graph["declarations"]
        .as_array()
        .ok_or_else(|| association("retained Project declaration inventory is absent"))?;
    let selected = declarations
        .iter()
        .filter(|row| row["id"] == target)
        .collect::<Vec<_>>();
    if selected.len() != 1
        || selected[0]["kind"] != "function"
        || selected[0]["identity_origin"] != "explicit"
        || selected[0]["path"] != path
    {
        return Err(association(
            "selected provider target is not one exact explicit function in the provider source",
        ));
    }
    let mut signatures = revision
        .entry_program()
        .functions
        .iter()
        .chain(revision.test_program().functions.iter())
        .filter(|function| function.id.as_str() == target)
        .map(|function| signature_value(revision.manifest().name(), function))
        .collect::<Result<Vec<_>>>()?;
    signatures.dedup();
    if signatures.len() != 1 {
        return Err(association(
            "selected provider target has no unique retained monomorphic signature",
        ));
    }
    Ok(signatures.pop().expect("one checked signature"))
}

fn signature_value(package: &str, function: &crate::hir::ResolvedFunction) -> Result<Value> {
    let interface = crate::package_report_v2::scalar_interface_from_resolved(package, &[function])
        .map_err(|error| vec![error])?;
    let signature = interface
        .functions
        .first()
        .ok_or_else(|| association("selected provider signature projection is absent"))?;
    let parameters = signature
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let ty: Value = serde_json::from_str(&parameter.ty)
                .map_err(|_| association("selected provider parameter type is not JSON"))?;
            Ok(json!({"index":index,"type":ty,"ownership":parameter.ownership}))
        })
        .collect::<Result<Vec<_>>>()?;
    let result_type: Value = serde_json::from_str(&signature.result_type)
        .map_err(|_| association("selected provider result type is not JSON"))?;
    let parse_contracts = |facts: &[String]| -> Result<Vec<Value>> {
        facts
            .iter()
            .map(|fact| {
                serde_json::from_str(fact)
                    .map_err(|_| association("selected provider contract fact is not JSON"))
            })
            .collect()
    };
    Ok(json!({
        "stable_id": signature.stable_id,
        "interface_digest": interface.digest,
        "parameters": parameters,
        "result": {"type":result_type,"ownership":signature.result_ownership},
        "effects": signature.effects,
        "requires": parse_contracts(&signature.requires)?,
        "ensures": parse_contracts(&signature.ensures)?,
    }))
}

fn changed_signature_facets(base: &Value, candidate: &Value) -> Vec<&'static str> {
    [
        ("parameters", "parameters"),
        ("result", "result"),
        ("effects", "effects"),
        ("requires", "requires"),
        ("ensures", "ensures"),
    ]
    .into_iter()
    .filter_map(|(label, field)| (base[field] != candidate[field]).then_some(label))
    .collect()
}

fn unique_source<'a>(
    sources: &'a [crate::project::ProjectSource],
    path: &str,
    message: &'static str,
) -> Result<&'a crate::project::ProjectSource> {
    let mut matches = sources.iter().filter(|source| source.path() == path);
    let source = matches.next().ok_or_else(|| association(message))?;
    if matches.next().is_some() {
        return Err(association(message));
    }
    Ok(source)
}

fn parse_report(bytes: &str, schema: &str) -> Result<Value> {
    let value: Value = serde_json::from_str(bytes)
        .map_err(|_| association("verified package graph report is not JSON"))?;
    if value.as_object().is_none() || value["schema"] != schema {
        return Err(association(
            "verified package graph report schema is invalid",
        ));
    }
    Ok(value)
}

fn array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    value[field]
        .as_array()
        .ok_or_else(|| association("verified package consumer inventory is absent"))
}

fn render(value: Value) -> Result<String> {
    super::super::image::render(
        value,
        false,
        MAX_PROJECT_CANDIDATE_PACKAGE_CONSUMER_REPLAY_BYTES,
    )
    .map_err(|_| capacity("candidate package consumer replay exceeds its byte bound"))
}

fn render_signature_conflicts(value: Value) -> Result<String> {
    super::super::image::render(
        value,
        false,
        MAX_PROJECT_CANDIDATE_PACKAGE_SIGNATURE_CONFLICTS_BYTES,
    )
    .map_err(|_| capacity("candidate package signature conflict report exceeds its byte bound"))
}

fn association(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G336", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G337", message)]
}

fn migration_binding(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G509", message)]
}

fn migration_refusal(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G510", message)]
}

fn render_migration(value: Value) -> Result<String> {
    super::super::image::render(
        value,
        false,
        MAX_PROJECT_CANDIDATE_PACKAGE_CONSUMER_MIGRATION_BYTES,
    )
    .map_err(|_| {
        vec![Diagnostic::io(
            "SPX-G511",
            "candidate package consumer migration report exceeds its byte bound",
        )]
    })
}
