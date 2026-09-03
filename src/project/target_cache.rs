//! Invocation-owned reuse of one exact, already admitted target product.
//!
//! The cache accepts only an immutable [`ProjectRevision`], whose private
//! construction completed source verification, HIR validation, workspace
//! linking, and profile admission. It stores no HIR and grants no source,
//! filesystem, process, execution, or publication authority.

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;

use super::{ProjectRevision, ProjectWebBuild, PROJECT_WEB_BUILD_SCHEMA};

pub const PROJECT_TARGET_CACHE_SCHEMA: &str = "semaprax.project-target-cache-work.v1";
pub const PROJECT_TARGET_CACHE_COMPATIBILITY: &str = "semaprax.project-scalar-web-target-work.v1";
pub const MAX_PROJECT_TARGET_CACHE_REPORT_BYTES: usize = 32 * 1024;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScalarWebKey {
    compiler_package: &'static str,
    compiler_version: &'static str,
    compatibility: &'static str,
    canonical_manifest: String,
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    entry_module: String,
    web_exports: Vec<String>,
    max_bytes: usize,
}

impl ScalarWebKey {
    fn derive(revision: &ProjectRevision, max_bytes: usize) -> Self {
        Self {
            compiler_package: env!("CARGO_PKG_NAME"),
            compiler_version: env!("CARGO_PKG_VERSION"),
            compatibility: PROJECT_TARGET_CACHE_COMPATIBILITY,
            canonical_manifest: revision.manifest().to_canonical_toml(),
            project_revision: revision.project_revision().to_owned(),
            workspace_revision: revision.workspace_revision().to_owned(),
            project_graph_digest: revision.semantic_graph_digest().to_owned(),
            entry_module: revision.manifest().entry().to_owned(),
            web_exports: revision.manifest().web_exports().to_vec(),
            max_bytes,
        }
    }
}

struct ScalarWebEntry {
    key: ScalarWebKey,
    build: ProjectWebBuild,
}

/// One target result and its deterministic work report.
#[derive(Debug)]
pub struct ProjectTargetBuild {
    build: ProjectWebBuild,
    reused: bool,
    report: String,
}

impl ProjectTargetBuild {
    pub fn build(&self) -> &ProjectWebBuild {
        &self.build
    }

    pub fn into_build(self) -> ProjectWebBuild {
        self.build
    }

    pub fn reused(&self) -> bool {
        self.reused
    }

    pub fn to_json(&self) -> &str {
        &self.report
    }
}

/// A caller-owned, single-entry scalar-Web target cache.
///
/// Every requested revision has already passed the complete Project builder.
/// An exact hit skips only deterministic target emission and carrier assembly;
/// it independently replays the retained carrier before returning it. A miss
/// replaces the prior entry only after emission, replay, subject matching, and
/// report rendering all succeed.
#[derive(Default)]
pub struct ProjectTargetCache {
    scalar_web: Option<ScalarWebEntry>,
}

impl ProjectTargetCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build_scalar_web(
        &mut self,
        revision: &ProjectRevision,
        max_bytes: usize,
    ) -> Result<ProjectTargetBuild> {
        // ProjectRevision has no public constructor; this records the retained
        // admission invariant at the target boundary without creating a weaker
        // source/HIR/link/profile route.
        revision.check()?;
        let key = ScalarWebKey::derive(revision, max_bytes);
        let retained = self
            .scalar_web
            .as_ref()
            .filter(|entry| entry.key == key)
            .map(|entry| entry.build.clone());
        let (build, reused) = if let Some(build) = retained {
            verify_exact_subject(&build, revision, max_bytes)?;
            (build, true)
        } else {
            let build = revision.build_web_inline(max_bytes)?;
            verify_exact_subject(&build, revision, max_bytes)?;
            (build, false)
        };
        let report = render_report(&key, &build, reused)?;
        if !reused {
            self.scalar_web = Some(ScalarWebEntry {
                key,
                build: build.clone(),
            });
        }
        Ok(ProjectTargetBuild {
            build,
            reused,
            report,
        })
    }

    pub fn clear(&mut self) {
        self.scalar_web = None;
    }
}

fn verify_exact_subject(
    build: &ProjectWebBuild,
    revision: &ProjectRevision,
    max_bytes: usize,
) -> Result<()> {
    build.verify().map_err(|error| vec![error])?;
    let carrier: Value = serde_json::from_str(build.envelope())
        .map_err(|_| target_error("retained target carrier is invalid JSON"))?;
    if carrier["schema"] != PROJECT_WEB_BUILD_SCHEMA
        || carrier["project_schema"] != revision.manifest().schema()
        || carrier["project"] != revision.manifest().name()
        || carrier["project_revision"] != revision.project_revision()
        || carrier["workspace_revision"] != revision.workspace_revision()
        || carrier["project_graph_digest"] != revision.semantic_graph_digest()
        || carrier["entry_module"] != revision.manifest().entry()
        || carrier["limits"]["max_bytes"].as_u64() != u64::try_from(max_bytes).ok()
        || build.max_bytes() != max_bytes
    {
        return Err(target_error(
            "retained target carrier does not exactly bind the admitted revision",
        ));
    }
    Ok(())
}

fn render_report(key: &ScalarWebKey, build: &ProjectWebBuild, reused: bool) -> Result<String> {
    super::image::render(
        json!({
            "schema": PROJECT_TARGET_CACHE_SCHEMA,
            "compiler": {
                "package": key.compiler_package,
                "version": key.compiler_version,
                "compatibility": key.compatibility,
                "binary_identity_claimed": false,
            },
            "target": {
                "kind": "scalar_web_inline",
                "canonical_manifest": key.canonical_manifest,
                "project_revision": key.project_revision,
                "workspace_revision": key.workspace_revision,
                "project_graph_digest": key.project_graph_digest,
                "entry_module": key.entry_module,
                "web_exports": key.web_exports,
                "max_bytes": key.max_bytes,
            },
            "work": {
                "target_emission_reused": reused,
                "target_emitter_calls": usize::from(!reused),
                "carrier_replay_calls": 1,
                "carrier_payload_digest": build.payload_digest(),
                "carrier_artifact_bytes": build.artifact_bytes(),
            },
            "validation": {
                "admitted_project_revision_required": true,
                "full_source_verification": "completed_before_revision_construction",
                "full_HIR_validation": "completed_before_revision_construction",
                "full_cross_file_linking": "completed_before_revision_construction",
                "full_profile_admission": "completed_before_revision_construction",
                "exact_target_subject_replayed": true,
            },
            "retained": {"entries": 1, "strategy": "single_exact_entry"},
            "nonclaims": [
                "no_source_HIR_link_or_profile_validation_bypass",
                "no_implicit_persistence_or_cross_process_reuse",
                "no_untrusted_target_deserialization",
                "no_filesystem_process_execution_or_publication_authority",
                "no_native_npm_or_non_scalar_target_reuse",
                "not_allocator_RSS_or_wall_clock_accounting",
            ],
        }),
        true,
        MAX_PROJECT_TARGET_CACHE_REPORT_BYTES,
    )
}

fn target_error(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-W117", message)]
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::project::MAX_PROJECT_WEB_BUILD_BYTES;

    fn revision(example: &str) -> std::sync::Arc<ProjectRevision> {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(example)
            .join("semaprax.toml");
        super::super::load_snapshot(&manifest)
            .unwrap()
            .retain_revision()
    }

    #[test]
    fn exact_admitted_scalar_target_reuses_only_after_carrier_replay() {
        let revision = revision("calculator-project");
        let mut cache = ProjectTargetCache::new();
        let cold = cache
            .build_scalar_web(&revision, MAX_PROJECT_WEB_BUILD_BYTES)
            .unwrap();
        assert!(!cold.reused());
        let expected = cold.build().clone();
        let warm = cache
            .build_scalar_web(&revision, MAX_PROJECT_WEB_BUILD_BYTES)
            .unwrap();
        assert!(warm.reused());
        assert_eq!(warm.build(), &expected);
        let report: Value = serde_json::from_str(warm.to_json()).unwrap();
        assert_eq!(report["schema"], PROJECT_TARGET_CACHE_SCHEMA);
        assert_eq!(report["work"]["target_emitter_calls"], 0);
        assert_eq!(report["work"]["carrier_replay_calls"], 1);
        assert_eq!(
            report["target"]["project_revision"],
            revision.project_revision()
        );
    }

    #[test]
    fn incompatible_profile_fails_without_replacing_the_exact_entry() {
        let scalar = revision("calculator-project");
        let incompatible = revision("config-validator-project");
        let mut cache = ProjectTargetCache::new();
        cache
            .build_scalar_web(&scalar, MAX_PROJECT_WEB_BUILD_BYTES)
            .unwrap();
        let diagnostics = cache
            .build_scalar_web(&incompatible, MAX_PROJECT_WEB_BUILD_BYTES)
            .unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-W120");
        assert!(cache
            .build_scalar_web(&scalar, MAX_PROJECT_WEB_BUILD_BYTES)
            .unwrap()
            .reused());
    }
}
