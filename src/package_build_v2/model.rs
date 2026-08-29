use crate::diagnostic::Diagnostic;
use crate::package_lock_v2::Coordinate;

pub const MANIFEST_SCHEMA: &str = "semaprax.offline-linked-scalar-wasm-package-build.v2";
pub const EVIDENCE_SCHEMA: &str = "semaprax.offline-linked-scalar-wasm-package-build-evidence.v2";
pub const PROFILE: &str = "linked-effect-free-core-wasm-scalar.v2";
pub const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_EVIDENCE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_EVIDENCE_RENDER_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MIN_LIMIT_BYTES: usize = 4 * 1024;
pub(crate) const MAX_EXPORTS: usize = 32;
pub(crate) const MAX_STABLE_ID_BYTES: usize = 128;

pub(crate) const NONCLAIMS: [&str; 9] = [
    "offline_caller_owned_capsule_source_and_resolution_replay_only",
    "linked_effect_free_scalar_core_wasm_not_target_execution_or_conformance",
    "no_registry_network_fetch_cache_or_dependency_discovery",
    "no_build_scripts_external_compiler_linker_or_tool_execution",
    "no_native_artifact_component_model_wasi_or_dynamic_linking",
    "no_cross_platform_hermetic_sandbox_claim",
    "capabilities_and_effects_must_be_empty_not_runtime_enforced",
    "no_signature_publisher_identity_provenance_license_or_sbom",
    "capsule_evidence_and_receipts_are_not_publication_authority",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkedOfflinePackageBuildOptions {
    pub root_package: String,
    pub exports: Vec<String>,
    pub max_artifact_bytes: usize,
    pub max_evidence_bytes: usize,
}

impl LinkedOfflinePackageBuildOptions {
    pub fn new(
        root_package: String,
        exports: Vec<String>,
        max_artifact_bytes: usize,
        max_evidence_bytes: usize,
    ) -> Result<Self, Diagnostic> {
        let value = Self {
            root_package,
            exports,
            max_artifact_bytes,
            max_evidence_bytes,
        };
        super::admission::validate_options(&value)?;
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkedOfflinePackageBuild {
    pub module_wasm: Vec<u8>,
    pub manifest_json: String,
    pub evidence_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedLinkedOfflinePackageBuild {
    pub root_package: String,
    pub packages: Vec<Coordinate>,
    pub capsule_digest: String,
    pub wasm_sha256: String,
    pub artifact_bytes: usize,
}

pub(crate) struct BuildFacts {
    pub(crate) root: Coordinate,
    pub(crate) packages: Vec<Coordinate>,
    pub(crate) capsule_digest: String,
    pub(crate) capsule_schema: String,
    pub(crate) capsule_bytes: usize,
    pub(crate) source_set_digest: String,
    pub(crate) link_digest: String,
    pub(crate) source_bytes: usize,
    pub(crate) exports: Vec<crate::wasm::PackageScalarExportFact>,
}
