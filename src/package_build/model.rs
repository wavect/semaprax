use crate::diagnostic::Diagnostic;
use crate::package_lock_v2::Coordinate;

pub const MANIFEST_SCHEMA: &str = "semaprax.offline-effect-free-wasm-package-build.v1";
pub const EVIDENCE_SCHEMA: &str = "semaprax.offline-effect-free-wasm-package-build-evidence.v1";
pub const PROFILE: &str = "effect-free-core-wasm-scalar.v1";
pub const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_EVIDENCE_BYTES: usize = 16 * 1024 * 1024;
pub const MIN_LIMIT_BYTES: usize = 4 * 1024;
pub const MAX_EXPORTS: usize = 32;
pub const MAX_STABLE_ID_BYTES: usize = 128;

pub(crate) const RUNTIME_IMPORTS: [&str; 7] = [
    "spx_add",
    "spx_sub",
    "spx_mul",
    "spx_div",
    "spx_rem",
    "spx_neg",
    "spx_contract_fail",
];

pub(crate) const NONCLAIMS: [&str; 9] = [
    "offline_caller_owned_catalog_and_source_replay_only",
    "effect_free_source_with_fixed_runtime_imports_not_target_execution_or_conformance",
    "no_registry_network_fetch_cache_or_dependency_discovery",
    "no_build_scripts_external_compiler_linker_or_tool_execution",
    "no_native_artifact_or_cross_platform_hermetic_sandbox_claim",
    "capabilities_and_effects_must_be_empty_not_runtime_enforced",
    "no_signature_publisher_identity_provenance_license_or_sbom",
    "no_multi_package_source_linking_component_model_wasi_dynamic_linking_or_runtime_instantiation",
    "evidence_is_not_publication_authority",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflinePackageBuildOptions {
    pub root_package: String,
    pub exports: Vec<String>,
    pub max_artifact_bytes: usize,
    pub max_evidence_bytes: usize,
}

impl OfflinePackageBuildOptions {
    pub fn new(
        root_package: String,
        exports: Vec<String>,
        max_artifact_bytes: usize,
        max_evidence_bytes: usize,
    ) -> Result<Self, Diagnostic> {
        let options = Self {
            root_package,
            exports,
            max_artifact_bytes,
            max_evidence_bytes,
        };
        super::admission::validate_options(&options)?;
        Ok(options)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflinePackageBuild {
    pub module_wasm: Vec<u8>,
    pub manifest_json: String,
    pub evidence_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedOfflinePackageBuild {
    pub root_package: String,
    pub packages: Vec<Coordinate>,
    pub wasm_sha256: String,
    pub artifact_bytes: usize,
}

#[derive(Clone)]
pub(crate) struct BuildFacts {
    pub(crate) coordinate: Coordinate,
    pub(crate) subject_digest: String,
    pub(crate) subject_bytes: usize,
    pub(crate) report_digest: String,
    pub(crate) source_revision: String,
    pub(crate) source_bytes: usize,
    pub(crate) source_set_digest: String,
    pub(crate) link_digest: String,
    pub(crate) resolution_digest: String,
    pub(crate) resolution_bytes: usize,
    pub(crate) lock_digest: String,
    pub(crate) lock_bytes: usize,
    pub(crate) exports: Vec<crate::wasm::PackageScalarExportFact>,
}
