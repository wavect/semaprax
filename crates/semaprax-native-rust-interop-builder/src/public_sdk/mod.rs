//! Public, bounded Native Rust SDK v1 package construction.
//!
//! Trust flow: descriptor replay -> deterministic package rendering -> held-stage
//! authority -> one authenticated no-clobber publication.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

const SPEC_SCHEMA: &str = "semaprax.native-rust-interop-spec.v1";
const DESCRIPTOR_SCHEMA: &str = "semaprax.native-rust-interop-descriptor.v1";
const SDK_SCHEMA: &str = "semaprax.native-rust-sdk.v1";
pub const PROJECT_NATIVE_RUST_SUBJECT_SCHEMA: &str = "semaprax.project-native-rust-subject.v1";
pub const PROJECT_NATIVE_RUST_SDK_SCHEMA: &str = "semaprax.project-native-rust-sdk.v1";
const PROJECT_DESCRIPTOR_SCHEMA: &str = "semaprax.project-native-rust-interop-descriptor.v1";
const SOURCE_DOMAIN: &[u8] = b"semaprax.native-rust-interop.source-revision.v1\0";
const DESCRIPTOR_DOMAIN: &[u8] = b"semaprax.native-rust-interop.descriptor-digest.v1\0";
const INNER_BUNDLE_DOMAIN: &[u8] = b"semaprax.native-rust-interop.bundle-digest.v1\0";
const PROJECT_DESCRIPTOR_DOMAIN: &[u8] =
    b"semaprax.project-native-rust-interop.descriptor-digest.v1\0";
const PROJECT_INNER_BUNDLE_DOMAIN: &[u8] =
    b"semaprax.project-native-rust-interop.bundle-digest.v1\0";
const SDK_MANIFEST_DOMAIN: &[u8] = b"semaprax.native-rust-sdk.manifest.v1\0";
const PROJECT_SUBJECT_DOMAIN: &[u8] = b"semaprax.project-native-rust-interop.subject.v1\0";
const PROJECT_SDK_MANIFEST_DOMAIN: &[u8] = b"semaprax.project-native-rust-sdk.manifest.v1\0";
const MAX_SOURCE_BYTES: usize = 16_777_216;
const MAX_SPEC_BYTES: usize = 1_048_576;
const MAX_DESCRIPTOR_BYTES: usize = 1_048_576;
const MAX_INNER_MANIFEST_BYTES: usize = 1_048_576;
const MAX_SDK_MANIFEST_BYTES: usize = 1_048_576;
const MAX_OBJECT_BYTES: usize = 4_194_304;
const MAX_ARCHIVE_BYTES: usize = 8_388_608;
const _: () = assert!(crate::platform::SDK_ARCHIVE_MAX_BYTES == MAX_ARCHIVE_BYTES as u64);
const MAX_GENERATED_RUST_BYTES: usize = 4_194_304;
const MAX_EXPORTS: usize = 32;
const MAX_IMPORTS: usize = 32;
const MAX_EFFECTS: usize = 64;
const MAX_IDENTIFIER_BYTES: usize = 128;
const CRATE_NAME: &str = "semaprax-generated-native-rust-sdk";
const CRATE_VERSION: &str = "0.1.0";
const SDK_LIMITS_JSON: &str = "{\"max_exports\":32,\"max_imports\":32,\"max_capabilities\":64,\"max_source_bytes\":16777216,\"max_descriptor_bytes\":1048576,\"max_inner_manifest_bytes\":1048576,\"max_generated_rust_bytes\":4194304,\"max_archive_bytes\":8388608,\"max_sdk_manifest_bytes\":1048576,\"exact_package_files\":9}";

const LIMITS_JSON: &str = "{\"max_exports\":32,\"max_imports\":32,\"max_parameters\":8,\"max_closure_functions\":256,\"max_status_domains\":64,\"max_effects\":64,\"max_identifier_bytes\":128,\"max_source_bytes\":16777216,\"max_spec_bytes\":1048576,\"max_descriptor_bytes\":1048576,\"max_generated_c_bytes\":4194304,\"max_generated_header_bytes\":1048576,\"max_generated_rust_bytes\":4194304,\"max_manifest_bytes\":1048576,\"max_builder_bytes\":33554432,\"max_json_depth\":8,\"max_semantic_expression_depth\":512,\"max_call_depth\":32,\"max_calls_per_bridge\":4096,\"max_unexpected_inventory_entries\":0}";

const INNER_NONCLAIMS: &[&str] = &[
    "no_resource_owned_borrow_shared_or_aggregate_abi",
    "no_pointer_reference_slice_string_trait_object_or_generic_abi",
    "no_cross_boundary_allocator_or_deallocator",
    "no_wasm_component_or_canonical_abi_detour",
    "no_dynamic_loading_symbol_lookup_unload_or_hot_reload",
    "no_public_execution_or_spx_b104_change_in_private_ab",
    "no_callable_v2_v3_proof_bundle_or_loader_wire_change",
    "no_graph_schema_api_kat_or_semantic_projection_change",
    "no_agent_runtime_economic_workspace_or_patch_wire_change",
    "no_untrusted_native_code_sandbox_or_memory_safety",
    "no_same_uid_process_signal_or_task_port_isolation",
    "no_same_uid_active_filesystem_mutation_or_namespace_race_isolation",
    "no_same_user_process_handle_or_thread_resume_isolation",
    "no_abi_compatibility_outside_exact_descriptor_target_toolchain",
    "no_cross_target_cross_toolchain_or_cross_build_bundle_reuse",
    "no_panic_or_unwind_across_ffi",
    "no_abort_oom_stack_overflow_signal_seh_or_process_crash_recovery",
    "no_power_loss_durability_or_crash_atomicity",
    "no_async_reentrant_parallel_cross_thread_or_send_sync_bridge",
    "no_host_capability_provenance_or_os_authority",
    "no_ambient_effect_capability_or_callback_discovery",
    "no_host_error_text_panic_payload_secret_or_pointer_evidence",
    "no_exactly_once_external_effect",
    "no_exception_cpp_rust_unwind_translation",
    "no_dynamic_library_code_signing_supply_chain_or_linker_provenance",
    "no_dynamic_dependency_identity_or_filesystem_race_isolation",
    "no_c_cpp_objective_c_swift_kotlin_jni_or_other_ecosystem_binding",
    "no_stable_rust_abi_claim_beyond_generated_c_abi_wrapper",
    "no_public_cli_package_registry_or_build_script_network",
    "no_general_interop_or_production_readiness",
    "no_completion_matrix_status_promotion",
];

const SDK_NONCLAIMS: &[&str] = &[
    "no_dynamic_loading_or_runtime_symbol_lookup",
    "no_cross_target_or_cross_toolchain_reuse",
    "no_resource_aggregate_pointer_string_or_borrowed_abi",
    "no_async_reentrant_parallel_cross_thread_or_send_sync_bridge",
    "no_untrusted_native_code_sandbox_or_memory_safety",
    "no_same_uid_active_filesystem_mutation_isolation",
    "no_abort_oom_signal_process_crash_or_power_loss_recovery",
    "no_registry_network_dependency_or_build_time_tool_execution",
    "no_phase_c_pre_reserved_cumulative_memory_or_allocation_failure_recovery",
    "no_independent_linker_index_semantic_reconstruction",
    "no_stable_rust_abi_beyond_the_generated_c_wrapper",
    "no_general_native_interop_or_production_readiness",
];

/// Owned, effect-free build intent for one bounded generated Native Rust SDK.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRustSdkOptions {
    pub exports: Vec<String>,
    pub imports: Vec<String>,
    pub capabilities: Vec<String>,
}

/// Immutable facts returned only after the complete SDK package is published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRustSdkBundle {
    output_directory: PathBuf,
    manifest_path: PathBuf,
    manifest_digest: String,
    crate_name: String,
    target_triple: String,
}

/// Immutable facts returned only after one authenticated Project SDK package
/// and the retained Project inputs have both been rechecked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNativeRustSdkBundle {
    sdk: NativeRustSdkBundle,
    project_revision: String,
    workspace_revision: String,
    subject_digest: String,
}

impl ProjectNativeRustSdkBundle {
    pub fn sdk(&self) -> &NativeRustSdkBundle {
        &self.sdk
    }

    pub fn output_directory(&self) -> &Path {
        self.sdk.output_directory()
    }

    pub fn manifest_path(&self) -> &Path {
        self.sdk.manifest_path()
    }

    pub fn manifest_digest(&self) -> &str {
        self.sdk.manifest_digest()
    }

    pub fn crate_name(&self) -> &str {
        self.sdk.crate_name()
    }

    pub fn target_triple(&self) -> &str {
        self.sdk.target_triple()
    }

    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }

    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    pub fn subject_digest(&self) -> &str {
        &self.subject_digest
    }
}

impl NativeRustSdkBundle {
    pub fn output_directory(&self) -> &Path {
        &self.output_directory
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    pub fn target_triple(&self) -> &str {
        &self.target_triple
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scalar {
    Unit,
    I64,
    Bool,
}

impl Scalar {
    const fn rust(self) -> &'static str {
        match self {
            Self::Unit => "()",
            Self::I64 => "i64",
            Self::Bool => "bool",
        }
    }

    const fn wire(self) -> &'static str {
        match self {
            Self::Unit => "unit",
            Self::I64 => "i64",
            Self::Bool => "bool",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Parameter {
    name: String,
    ty: Scalar,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Export {
    id: String,
    public_method: String,
    inner_method: String,
    parameters: Vec<Parameter>,
    result: Scalar,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Import {
    id: String,
    public_method: String,
    inner_method: String,
    parameters: Vec<Parameter>,
    result: Scalar,
    failure_domain: Option<String>,
}

struct DescriptorFacts {
    module: String,
    source_revision: String,
    target: String,
    exports: Vec<Export>,
    imports: Vec<Import>,
}

struct PackageSources {
    cargo_toml: String,
    build_rs: String,
    lib_rs: String,
}

struct InnerArtifacts<'a> {
    descriptor: &'a [u8],
    generated_c: &'a [u8],
    generated_header: &'a [u8],
    safe_rust: &'a [u8],
    ffi_rust: &'a [u8],
    object: &'a [u8],
    object_name: &'a str,
}

struct PublishedPackage<'a> {
    manifest: &'a str,
    archive_name: &'a str,
    sources: &'a PackageSources,
    descriptor: &'a [u8],
    inner_manifest: &'a [u8],
    safe_inner: &'a [u8],
    ffi_inner: &'a [u8],
    archive: &'a [u8],
}

static STAGE_NONCE: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestBuildPoint {
    ArchiveCreationCleanupUncertainty,
    BeforeArchive,
    ArchiveOutputMutation,
    AfterFirstOuterWrite,
    ScratchCleanupUncertainty,
    BeforePublish,
    PostPivotAuthenticationFailure,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
enum TestBuildLastStage {
    #[default]
    Start,
    InnerAuthenticated,
    InnerPayloadVerified,
    ArchiveStageCreated,
    ArchiveToolReturned,
    ArchiveAttached,
    ArchiveInventoryAuthenticated,
    ArchiveRead,
    OuterStageCreated,
    OuterStageWritten,
    OuterInventoryAuthenticated,
    ArchiveScratchDiscarded,
    InnerScratchDiscarded,
    PrePublishAuthenticated,
    PublishReturned,
    PublishedPackageAuthenticated,
    PublishedAuthenticated,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TestBuildSnapshot {
    last_stage: TestBuildLastStage,
    archive_attempts: usize,
    publish_calls: usize,
}

#[cfg(test)]
#[derive(Clone, Copy, Default)]
struct TestBuildState {
    point: Option<TestBuildPoint>,
    last_stage: TestBuildLastStage,
    archive_attempts: usize,
    publish_calls: usize,
}

#[cfg(test)]
thread_local! {
    static TEST_BUILD_STATE: std::cell::Cell<TestBuildState> = const {
        std::cell::Cell::new(TestBuildState {
            point: None,
            last_stage: TestBuildLastStage::Start,
            archive_attempts: 0,
            publish_calls: 0,
        })
    };
}

#[cfg(test)]
fn test_hook(point: TestBuildPoint) -> bool {
    TEST_BUILD_STATE.with(|state| state.get().point == Some(point))
}

#[cfg(test)]
fn injected_error() -> Diagnostic {
    sdk_error("injected Native Rust SDK boundary failure")
}

#[cfg(test)]
fn record_archive_attempt() {
    TEST_BUILD_STATE.with(|state| {
        let mut value = state.get();
        value.archive_attempts += 1;
        state.set(value);
    });
}

#[cfg(test)]
fn record_publish_call() {
    TEST_BUILD_STATE.with(|state| {
        let mut value = state.get();
        value.publish_calls += 1;
        state.set(value);
    });
}

#[cfg(test)]
fn record_test_build_stage(last_stage: TestBuildLastStage) {
    TEST_BUILD_STATE.with(|state| {
        let mut value = state.get();
        assert!(last_stage > value.last_stage);
        value.last_stage = last_stage;
        state.set(value);
    });
}

#[cfg(test)]
fn test_build_snapshot() -> TestBuildSnapshot {
    TEST_BUILD_STATE.with(|state| {
        let state = state.get();
        TestBuildSnapshot {
            last_stage: state.last_stage,
            archive_attempts: state.archive_attempts,
            publish_calls: state.publish_calls,
        }
    })
}

fn sdk_error(message: &'static str) -> Diagnostic {
    Diagnostic::io("SPX-B112", message)
}

fn publication_error() -> Diagnostic {
    Diagnostic::io("SPX-I233", "Native Rust SDK publication failed")
}

enum PublicBuildError {
    One(Diagnostic),
    Many(Vec<Diagnostic>),
}

struct StageCreationError {
    primary: Diagnostic,
    settlement_uncertain: bool,
}

impl StageCreationError {
    fn certain(primary: Diagnostic) -> Self {
        Self {
            primary,
            settlement_uncertain: false,
        }
    }

    fn uncertain(primary: Diagnostic) -> Self {
        Self {
            primary,
            settlement_uncertain: true,
        }
    }

    fn stop(self) -> PublicBuildError {
        PublicBuildError::Many(vec![self.primary, publication_error()])
    }
}

impl From<Diagnostic> for PublicBuildError {
    fn from(error: Diagnostic) -> Self {
        Self::One(error)
    }
}

impl PublicBuildError {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::One(error) => vec![error],
            Self::Many(errors) if errors.is_empty() => vec![publication_error()],
            Self::Many(errors) => errors,
        }
    }
}

fn raw_digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn json_string(output: &mut String, value: &str) {
    output.push_str(&serde_json::to_string(value).expect("string serialization cannot fail"));
}

fn string_array(output: &mut String, values: &[String]) {
    output.push('[');
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        json_string(output, value);
    }
    output.push(']');
}

fn target_triple() -> Option<&'static str> {
    if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        Some("x86_64-unknown-linux-gnu")
    } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
        Some("aarch64-unknown-linux-gnu")
    } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
        Some("x86_64-apple-darwin")
    } else if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
        Some("aarch64-apple-darwin")
    } else if cfg!(all(target_arch = "x86_64", target_os = "windows")) {
        Some("x86_64-pc-windows-msvc")
    } else {
        None
    }
}

fn validate_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.' | b'-')
        })
}

fn canonical_values(mut values: Vec<String>, maximum: usize) -> Result<Vec<String>, Diagnostic> {
    if values.len() > maximum || values.iter().any(|value| !validate_identifier(value)) {
        return Err(sdk_error(
            "Native Rust SDK selection is outside the bounded profile",
        ));
    }
    values.sort();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(sdk_error(
            "Native Rust SDK selected identities must be unique",
        ));
    }
    Ok(values)
}

fn encode_stable_id(id: &str) -> Result<String, Diagnostic> {
    if !validate_identifier(id) {
        return Err(sdk_error("Native Rust SDK stable identity is not portable"));
    }
    let mut output = String::with_capacity(id.len().saturating_mul(12).saturating_add(4));
    output.push_str("spx_");
    for byte in id.bytes() {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' => output.push(char::from(byte)),
            b'_' => output.push_str("_underscore_"),
            b'.' => output.push_str("_dot_"),
            b'-' => output.push_str("_hyphen_"),
            _ => return Err(sdk_error("Native Rust SDK stable identity is not portable")),
        }
    }
    Ok(output)
}

fn full_hash(value: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(value.as_bytes()))
    )
}

mod authentication;
mod authority;
mod build;
mod descriptor;
mod package;
mod project;

pub use build::build_native_rust_sdk;
pub use project::build_project_native_rust_sdk;

#[cfg(test)]
mod tests;
