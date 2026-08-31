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

impl ProjectCandidate {
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

fn association(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G336", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G337", message)]
}
