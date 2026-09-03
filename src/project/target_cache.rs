//! Invocation-owned reuse of one exact, already admitted target product.
//!
//! The cache accepts only an immutable [`ProjectRevision`], whose private
//! construction completed source verification, HIR validation, workspace
//! linking, and profile admission. It stores no HIR and grants no source,
//! filesystem, process, execution, or publication authority.

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;

use super::{ProjectNpmBuild, ProjectRevision, ProjectWebBuild, PROJECT_WEB_BUILD_SCHEMA};

pub const PROJECT_TARGET_CACHE_SCHEMA: &str = "semaprax.project-target-cache-work.v1";
pub const PROJECT_TARGET_CACHE_COMPATIBILITY: &str = "semaprax.project-scalar-web-target-work.v1";
pub const PROJECT_C_TARGET_CACHE_COMPATIBILITY: &str = "semaprax.project-native-c11-target-work.v1";
pub const PROJECT_NPM_TARGET_CACHE_COMPATIBILITY: &str =
    "semaprax.project-pathless-npm-target-work.v1";
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct NativeCKey {
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

impl NativeCKey {
    fn derive(revision: &ProjectRevision, max_bytes: usize) -> Self {
        Self {
            compiler_package: env!("CARGO_PKG_NAME"),
            compiler_version: env!("CARGO_PKG_VERSION"),
            compatibility: PROJECT_C_TARGET_CACHE_COMPATIBILITY,
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

struct NativeCEntry {
    key: NativeCKey,
    envelope: String,
    payload_digest: String,
    artifact_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NpmKey {
    compiler_package: &'static str,
    compiler_version: &'static str,
    compatibility: &'static str,
    canonical_manifest: String,
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    entry_module: String,
    web_exports: Vec<String>,
    source_bindings: Vec<NpmSourceBinding>,
    max_bytes: usize,
}

impl NpmKey {
    fn derive(revision: &ProjectRevision, max_bytes: usize) -> Self {
        Self {
            compiler_package: env!("CARGO_PKG_NAME"),
            compiler_version: env!("CARGO_PKG_VERSION"),
            compatibility: PROJECT_NPM_TARGET_CACHE_COMPATIBILITY,
            canonical_manifest: revision.manifest().to_canonical_toml(),
            project_revision: revision.project_revision().to_owned(),
            workspace_revision: revision.workspace_revision().to_owned(),
            project_graph_digest: revision.semantic_graph_digest().to_owned(),
            entry_module: revision.manifest().entry().to_owned(),
            web_exports: revision.manifest().web_exports().to_vec(),
            source_bindings: revision
                .sources()
                .iter()
                .map(|source| NpmSourceBinding {
                    path: source.path().to_owned(),
                    source_revision: source.source_revision().to_owned(),
                    source_digest: source.source_digest().to_owned(),
                })
                .collect(),
            max_bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NpmSourceBinding {
    path: String,
    source_revision: String,
    source_digest: String,
}

struct NpmEntry {
    key: NpmKey,
    build: ProjectNpmBuild,
    payload_digest: String,
    artifact_bytes: usize,
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

/// One canonical pathless native-C source carrier and its deterministic work
/// report. The decoded artifacts remain inspection data and gain no compiler,
/// filesystem, linkage, execution, or publication authority.
#[derive(Debug)]
pub struct ProjectCTargetBuild {
    envelope: String,
    payload_digest: String,
    artifact_bytes: usize,
    reused: bool,
    report: String,
}

impl ProjectCTargetBuild {
    pub fn envelope(&self) -> &str {
        &self.envelope
    }

    pub fn into_envelope(self) -> String {
        self.envelope
    }

    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }

    pub fn artifact_bytes(&self) -> usize {
        self.artifact_bytes
    }

    pub fn reused(&self) -> bool {
        self.reused
    }

    pub fn to_json(&self) -> &str {
        &self.report
    }
}

/// One verified pathless npm package carrier and its deterministic work
/// report. This value grants no package-manager, installation, runtime,
/// filesystem, persistence, or publication authority.
#[derive(Debug)]
pub struct ProjectNpmTargetBuild {
    build: ProjectNpmBuild,
    reused: bool,
    report: String,
}

impl ProjectNpmTargetBuild {
    pub fn build(&self) -> &ProjectNpmBuild {
        &self.build
    }

    pub fn into_build(self) -> ProjectNpmBuild {
        self.build
    }

    pub fn reused(&self) -> bool {
        self.reused
    }

    pub fn to_json(&self) -> &str {
        &self.report
    }
}

/// A caller-owned cache with one exact entry per admitted target carrier.
///
/// Every requested revision has already passed the complete Project builder.
/// An exact hit skips only deterministic target emission and carrier assembly;
/// it independently replays the retained carrier before returning it. A miss
/// replaces the prior entry only after emission, replay, subject matching, and
/// report rendering all succeed.
#[derive(Default)]
pub struct ProjectTargetCache {
    scalar_web: Option<ScalarWebEntry>,
    native_c: Option<NativeCEntry>,
    npm: Option<NpmEntry>,
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

    /// Reuse one exact compiler-produced pathless native-C carrier. An exact
    /// hit independently replays all subject, source, export, artifact, and
    /// embedded-header bindings without invoking either target emitter.
    pub fn build_native_c(
        &mut self,
        revision: &ProjectRevision,
        max_bytes: usize,
    ) -> Result<ProjectCTargetBuild> {
        revision.check()?;
        let key = NativeCKey::derive(revision, max_bytes);
        let retained = self
            .native_c
            .as_ref()
            .filter(|entry| entry.key == key)
            .map(|entry| {
                (
                    entry.envelope.clone(),
                    entry.payload_digest.clone(),
                    entry.artifact_bytes,
                )
            });
        let (envelope, retained_facts) = if let Some((envelope, digest, bytes)) = retained {
            (envelope, Some((digest, bytes)))
        } else {
            (revision.build_c_inline(max_bytes)?, None)
        };
        let replay = super::image_targets::c::replay_carrier(revision, &envelope, max_bytes)?;
        if retained_facts.as_ref().is_some_and(|(digest, bytes)| {
            replay.payload_digest != *digest || replay.artifact_bytes != *bytes
        }) {
            return Err(target_error(
                "retained native C carrier disagrees with its committed cache facts",
            ));
        }
        let reused = retained_facts.is_some();
        let report = render_c_report(&key, &replay, reused)?;
        if !reused {
            self.native_c = Some(NativeCEntry {
                key,
                envelope: envelope.clone(),
                payload_digest: replay.payload_digest.clone(),
                artifact_bytes: replay.artifact_bytes,
            });
        }
        Ok(ProjectCTargetBuild {
            envelope,
            payload_digest: replay.payload_digest,
            artifact_bytes: replay.artifact_bytes,
            reused,
            report,
        })
    }

    /// Reuse one exact compiler-produced pathless npm carrier. An exact hit
    /// invokes no generator and replays the carrier's closed schema, semantic
    /// recipe, file inventory, bytes, digests, and retained Project identity.
    pub fn build_npm(
        &mut self,
        revision: &ProjectRevision,
        max_bytes: usize,
    ) -> Result<ProjectNpmTargetBuild> {
        revision.check()?;
        let key = NpmKey::derive(revision, max_bytes);
        let retained = self
            .npm
            .as_ref()
            .filter(|entry| entry.key == key)
            .map(|entry| {
                (
                    entry.build.clone(),
                    entry.payload_digest.clone(),
                    entry.artifact_bytes,
                )
            });
        let (build, retained_facts) = if let Some((build, digest, bytes)) = retained {
            (build, Some((digest, bytes)))
        } else {
            (revision.build_npm_inline(max_bytes)?, None)
        };
        verify_exact_npm_subject(&build, revision, max_bytes)?;
        if retained_facts.as_ref().is_some_and(|(digest, bytes)| {
            build.payload_digest() != digest || build.artifact_bytes() != *bytes
        }) {
            return Err(target_error(
                "retained npm carrier disagrees with its committed cache facts",
            ));
        }
        let reused = retained_facts.is_some();
        let report = render_npm_report(&key, &build, reused)?;
        if !reused {
            self.npm = Some(NpmEntry {
                key,
                payload_digest: build.payload_digest().to_owned(),
                artifact_bytes: build.artifact_bytes(),
                build: build.clone(),
            });
        }
        Ok(ProjectNpmTargetBuild {
            build,
            reused,
            report,
        })
    }

    pub fn clear(&mut self) {
        self.scalar_web = None;
        self.native_c = None;
        self.npm = None;
    }
}

fn verify_exact_npm_subject(
    build: &ProjectNpmBuild,
    revision: &ProjectRevision,
    max_bytes: usize,
) -> Result<()> {
    build.verify().map_err(|diagnostic| vec![diagnostic])?;
    let carrier: Value = serde_json::from_str(build.envelope())
        .map_err(|_| target_error("retained npm carrier is invalid JSON"))?;
    if carrier["project_schema"] != revision.manifest().schema()
        || carrier["package"] != revision.manifest().name()
        || carrier["version"] != revision.manifest().package_version().unwrap_or("")
        || carrier["project_revision"] != revision.project_revision()
        || carrier["workspace_revision"] != revision.workspace_revision()
        || carrier["project_graph_digest"] != revision.semantic_graph_digest()
        || carrier["payload_digest"] != build.payload_digest()
        || carrier["artifact_bytes"].as_u64() != u64::try_from(build.artifact_bytes()).ok()
        || build.max_bytes() != max_bytes
    {
        return Err(target_error(
            "retained npm carrier does not exactly bind the admitted revision",
        ));
    }
    Ok(())
}

fn render_npm_report(key: &NpmKey, build: &ProjectNpmBuild, reused: bool) -> Result<String> {
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
                "kind": "npm_pathless",
                "canonical_manifest": key.canonical_manifest,
                "project_revision": key.project_revision,
                "workspace_revision": key.workspace_revision,
                "project_graph_digest": key.project_graph_digest,
                "entry_module": key.entry_module,
                "web_exports": key.web_exports,
                "source_bindings": key.source_bindings.iter().map(|source| json!({
                    "path": source.path,
                    "source_revision": source.source_revision,
                    "source_digest": source.source_digest,
                })).collect::<Vec<_>>(),
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
                "exact_manifest_source_export_subject_matched": true,
                "closed_package_files_and_digests_replayed": true,
            },
            "retained": {"entries": 1, "strategy": "single_exact_entry_for_npm"},
            "nonclaims": [
                "no_source_HIR_link_or_profile_validation_bypass",
                "no_package_manager_or_installation",
                "no_target_runtime_execution",
                "no_filesystem_materialization_or_publication_authority",
                "no_implicit_persistence_or_cross_process_reuse",
                "no_untrusted_target_deserialization",
                "no_target_admission_widening",
                "no_cross_target_reuse",
                "not_allocator_RSS_or_wall_clock_accounting",
            ],
        }),
        true,
        MAX_PROJECT_TARGET_CACHE_REPORT_BYTES,
    )
}

fn render_c_report(
    key: &NativeCKey,
    replay: &super::image_targets::c::ReplayedCarrier,
    reused: bool,
) -> Result<String> {
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
                "kind": "native_c11_pathless",
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
                "carrier_payload_digest": replay.payload_digest,
                "carrier_artifact_bytes": replay.artifact_bytes,
            },
            "validation": {
                "admitted_project_revision_required": true,
                "full_source_verification": "completed_before_revision_construction",
                "full_HIR_validation": "completed_before_revision_construction",
                "full_cross_file_linking": "completed_before_revision_construction",
                "full_profile_admission": "completed_before_revision_construction",
                "exact_target_subject_replayed": true,
                "embedded_header_envelopes_replayed": true,
            },
            "retained": {"entries": 1, "strategy": "single_exact_entry_for_native_c11"},
            "nonclaims": [
                "no_source_HIR_link_or_profile_validation_bypass",
                "no_implicit_persistence_or_cross_process_reuse",
                "no_untrusted_target_deserialization",
                "no_C_compilation_linking_or_runtime_execution",
                "no_filesystem_artifact_materialization_or_publication_authority",
                "no_target_admission_widening",
                "no_cross_target_reuse",
                "not_allocator_RSS_or_wall_clock_accounting",
            ],
        }),
        true,
        MAX_PROJECT_TARGET_CACHE_REPORT_BYTES,
    )
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
    use crate::project::image_targets::MAX_IMAGE_ARTIFACT_BUILD_BYTES;
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

    #[test]
    fn exact_admitted_native_c_target_reuses_only_after_complete_carrier_replay() {
        let revision = revision("calculator-project");
        let mut cache = ProjectTargetCache::new();
        let cold = cache
            .build_native_c(&revision, MAX_IMAGE_ARTIFACT_BUILD_BYTES)
            .unwrap();
        assert!(!cold.reused());
        let envelope = cold.envelope().to_owned();
        let digest = cold.payload_digest().to_owned();
        let bytes = cold.artifact_bytes();

        let warm = cache
            .build_native_c(&revision, MAX_IMAGE_ARTIFACT_BUILD_BYTES)
            .unwrap();
        assert!(warm.reused());
        assert_eq!(warm.envelope(), envelope);
        assert_eq!(warm.payload_digest(), digest);
        assert_eq!(warm.artifact_bytes(), bytes);
        let report: Value = serde_json::from_str(warm.to_json()).unwrap();
        assert_eq!(report["target"]["kind"], "native_c11_pathless");
        assert_eq!(report["work"]["target_emitter_calls"], 0);
        assert_eq!(report["work"]["carrier_replay_calls"], 1);
        assert_eq!(
            report["validation"]["embedded_header_envelopes_replayed"],
            true
        );
        assert!(report["nonclaims"]
            .as_array()
            .unwrap()
            .contains(&json!("no_C_compilation_linking_or_runtime_execution")));
    }

    #[test]
    fn target_lanes_are_independent_and_max_bytes_is_part_of_the_c_key() {
        let revision = revision("calculator-project");
        let mut cache = ProjectTargetCache::new();
        cache
            .build_scalar_web(&revision, MAX_PROJECT_WEB_BUILD_BYTES)
            .unwrap();
        cache
            .build_native_c(&revision, MAX_IMAGE_ARTIFACT_BUILD_BYTES)
            .unwrap();
        assert!(cache
            .build_scalar_web(&revision, MAX_PROJECT_WEB_BUILD_BYTES)
            .unwrap()
            .reused());
        assert!(cache
            .build_native_c(&revision, MAX_IMAGE_ARTIFACT_BUILD_BYTES)
            .unwrap()
            .reused());

        assert!(!cache
            .build_native_c(&revision, MAX_IMAGE_ARTIFACT_BUILD_BYTES - 1)
            .unwrap()
            .reused());
        assert!(cache
            .build_scalar_web(&revision, MAX_PROJECT_WEB_BUILD_BYTES)
            .unwrap()
            .reused());
    }

    #[test]
    fn corrupted_retained_c_carrier_fails_closed_without_emitter_recovery() {
        let revision = revision("calculator-project");
        let mut cache = ProjectTargetCache::new();
        cache
            .build_native_c(&revision, MAX_IMAGE_ARTIFACT_BUILD_BYTES)
            .unwrap();
        let exact = cache.native_c.as_ref().unwrap().envelope.clone();
        cache.native_c.as_mut().unwrap().envelope = exact.replacen("sha256:", "sha257:", 1);

        let diagnostics = cache
            .build_native_c(&revision, MAX_IMAGE_ARTIFACT_BUILD_BYTES)
            .unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-G292");
        assert!(cache
            .native_c
            .as_ref()
            .unwrap()
            .envelope
            .contains("sha257:"));

        cache.native_c.as_mut().unwrap().envelope = exact;
        assert!(cache
            .build_native_c(&revision, MAX_IMAGE_ARTIFACT_BUILD_BYTES)
            .unwrap()
            .reused());
    }

    #[test]
    fn exact_admitted_npm_target_reuses_only_after_closed_carrier_replay() {
        let revision = revision("config-validator-project");
        let mut cache = ProjectTargetCache::new();
        let cold = cache
            .build_npm(&revision, crate::project::MAX_PROJECT_NPM_BUILD_BYTES)
            .unwrap();
        assert!(!cold.reused());
        let expected = cold.build().clone();

        let warm = cache
            .build_npm(&revision, crate::project::MAX_PROJECT_NPM_BUILD_BYTES)
            .unwrap();
        assert!(warm.reused());
        assert_eq!(warm.build(), &expected);
        let report: Value = serde_json::from_str(warm.to_json()).unwrap();
        assert_eq!(report["target"]["kind"], "npm_pathless");
        assert_eq!(report["work"]["target_emitter_calls"], 0);
        assert_eq!(report["work"]["carrier_replay_calls"], 1);
        assert_eq!(
            report["validation"]["closed_package_files_and_digests_replayed"],
            true
        );
        assert!(report["nonclaims"]
            .as_array()
            .unwrap()
            .contains(&json!("no_package_manager_or_installation")));
    }

    #[test]
    fn npm_and_native_c_lanes_are_independent_and_npm_limit_is_exact() {
        let revision = revision("config-validator-project");
        let mut cache = ProjectTargetCache::new();
        cache
            .build_native_c(&revision, MAX_IMAGE_ARTIFACT_BUILD_BYTES)
            .unwrap();
        cache
            .build_npm(&revision, crate::project::MAX_PROJECT_NPM_BUILD_BYTES)
            .unwrap();
        assert!(cache
            .build_native_c(&revision, MAX_IMAGE_ARTIFACT_BUILD_BYTES)
            .unwrap()
            .reused());
        assert!(cache
            .build_npm(&revision, crate::project::MAX_PROJECT_NPM_BUILD_BYTES)
            .unwrap()
            .reused());

        assert!(!cache
            .build_npm(&revision, crate::project::MAX_PROJECT_NPM_BUILD_BYTES - 1,)
            .unwrap()
            .reused());
        assert!(cache
            .build_native_c(&revision, MAX_IMAGE_ARTIFACT_BUILD_BYTES)
            .unwrap()
            .reused());
    }

    #[test]
    fn altered_npm_cache_facts_fail_closed_without_generator_recovery() {
        let revision = revision("config-validator-project");
        let mut cache = ProjectTargetCache::new();
        cache
            .build_npm(&revision, crate::project::MAX_PROJECT_NPM_BUILD_BYTES)
            .unwrap();
        let exact = cache.npm.as_ref().unwrap().payload_digest.clone();
        cache.npm.as_mut().unwrap().payload_digest = "sha256:00".to_owned();

        let diagnostics = cache
            .build_npm(&revision, crate::project::MAX_PROJECT_NPM_BUILD_BYTES)
            .unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-W117");
        assert_eq!(cache.npm.as_ref().unwrap().payload_digest, "sha256:00");

        cache.npm.as_mut().unwrap().payload_digest = exact;
        assert!(cache
            .build_npm(&revision, crate::project::MAX_PROJECT_NPM_BUILD_BYTES)
            .unwrap()
            .reused());
    }
}
