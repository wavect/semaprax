//! Public, bounded Native Rust SDK v1 package construction.

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
const SOURCE_DOMAIN: &[u8] = b"semaprax.native-rust-interop.source-revision.v1\0";
const DESCRIPTOR_DOMAIN: &[u8] = b"semaprax.native-rust-interop.descriptor-digest.v1\0";
const INNER_BUNDLE_DOMAIN: &[u8] = b"semaprax.native-rust-interop.bundle-digest.v1\0";
const SDK_MANIFEST_DOMAIN: &[u8] = b"semaprax.native-rust-sdk.manifest.v1\0";
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
#[derive(Clone, Copy, Default)]
struct TestBuildState {
    point: Option<TestBuildPoint>,
    archive_attempts: usize,
    publish_calls: usize,
}

#[cfg(test)]
thread_local! {
    static TEST_BUILD_STATE: std::cell::Cell<TestBuildState> = const {
        std::cell::Cell::new(TestBuildState { point: None, archive_attempts: 0, publish_calls: 0 })
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

fn canonical_spec(
    module: &str,
    source_revision: &str,
    target: &str,
    options: &NativeRustSdkOptions,
) -> Result<String, Diagnostic> {
    let mut output = String::with_capacity(8192);
    output.push_str("{\"schema\":");
    json_string(&mut output, SPEC_SCHEMA);
    output.push_str(",\"module\":");
    json_string(&mut output, module);
    output.push_str(",\"source_revision\":");
    json_string(&mut output, source_revision);
    output.push_str(",\"target\":{\"triple\":");
    json_string(&mut output, target);
    output.push_str(",\"pointer_width\":64,\"endian\":\"little\",\"panic_strategy\":\"unwind\",\"thread_policy\":\"same_thread\"},\"exports\":");
    string_array(&mut output, &options.exports);
    output.push_str(",\"imports\":");
    string_array(&mut output, &options.imports);
    output.push_str(",\"capabilities\":");
    string_array(&mut output, &options.capabilities);
    output.push_str(",\"limits\":");
    output.push_str(LIMITS_JSON);
    output.push_str(",\"nonclaims\":[");
    for (index, value) in INNER_NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        json_string(&mut output, value);
    }
    output.push_str("]}\n");
    if output.len() > MAX_SPEC_BYTES {
        return Err(sdk_error(
            "Native Rust SDK canonical intent exceeds its bound",
        ));
    }
    Ok(output)
}

fn descriptor_object<'a>(
    value: &'a Value,
    key: &str,
) -> Result<&'a Map<String, Value>, Diagnostic> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))
}

fn descriptor_scalar(value: &Value, allow_unit: bool) -> Result<Scalar, Diagnostic> {
    let row = value
        .as_object()
        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
    if row.len() != 2 || row.get("out_slot").and_then(Value::as_bool).is_none() {
        return Err(sdk_error("Native Rust SDK descriptor replay failed"));
    }
    let scalar = match row.get("type").and_then(Value::as_str) {
        Some("unit") if allow_unit => Scalar::Unit,
        Some("i64") => Scalar::I64,
        Some("bool") => Scalar::Bool,
        _ => return Err(sdk_error("Native Rust SDK descriptor replay failed")),
    };
    if row.get("out_slot").and_then(Value::as_bool) != Some(scalar != Scalar::Unit) {
        return Err(sdk_error("Native Rust SDK descriptor replay failed"));
    }
    Ok(scalar)
}

fn descriptor_parameters(value: &Value) -> Result<Vec<Parameter>, Diagnostic> {
    let rows = value
        .as_array()
        .filter(|rows| rows.len() <= 8)
        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
    rows.iter()
        .map(|row| {
            let row = row
                .as_object()
                .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
            if row.len() != 3 || row.get("mode").and_then(Value::as_str) != Some("value") {
                return Err(sdk_error("Native Rust SDK descriptor replay failed"));
            }
            let name = row
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty() && name.len() <= MAX_IDENTIFIER_BYTES)
                .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
            let ty = match row.get("type").and_then(Value::as_str) {
                Some("i64") => Scalar::I64,
                Some("bool") => Scalar::Bool,
                _ => return Err(sdk_error("Native Rust SDK descriptor replay failed")),
            };
            Ok(Parameter {
                name: name.to_owned(),
                ty,
            })
        })
        .collect()
}

fn parse_descriptor(
    bytes: &[u8],
    expected_module: &str,
    expected_revision: &str,
    expected_target: &str,
    options: &NativeRustSdkOptions,
) -> Result<DescriptorFacts, Diagnostic> {
    if bytes.len() > MAX_DESCRIPTOR_BYTES || !bytes.ends_with(b"\n") {
        return Err(sdk_error("Native Rust SDK descriptor replay failed"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| sdk_error("Native Rust SDK descriptor replay failed"))?;
    let root = value
        .as_object()
        .filter(|root| root.len() == 11)
        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
    let target = descriptor_object(&value, "target")?;
    if root.get("schema").and_then(Value::as_str) != Some(DESCRIPTOR_SCHEMA)
        || root.get("module").and_then(Value::as_str) != Some(expected_module)
        || root.get("source_revision").and_then(Value::as_str) != Some(expected_revision)
        || target.len() != 5
        || target.get("triple").and_then(Value::as_str) != Some(expected_target)
        || target.get("pointer_width").and_then(Value::as_u64) != Some(64)
        || target.get("endian").and_then(Value::as_str) != Some("little")
        || target.get("panic_strategy").and_then(Value::as_str) != Some("unwind")
        || target.get("thread_policy").and_then(Value::as_str) != Some("same_thread")
    {
        return Err(sdk_error("Native Rust SDK descriptor replay failed"));
    }
    let exports = root
        .get("exports")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == options.exports.len())
        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
    let imports = root
        .get("imports")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == options.imports.len())
        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;

    let exports = exports
        .iter()
        .zip(&options.exports)
        .map(|(row, expected)| {
            let row = row
                .as_object()
                .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
            let id = row.get("id").and_then(Value::as_str).unwrap_or_default();
            let inner = row
                .get("rust_method")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if id != expected || inner != format!("export_{}", full_hash(id)) {
                return Err(sdk_error("Native Rust SDK descriptor replay failed"));
            }
            Ok(Export {
                id: id.to_owned(),
                public_method: encode_stable_id(id)?,
                inner_method: inner.to_owned(),
                parameters: descriptor_parameters(
                    row.get("parameters")
                        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?,
                )?,
                result: descriptor_scalar(
                    row.get("result")
                        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?,
                    false,
                )?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let imports = imports
        .iter()
        .zip(&options.imports)
        .map(|(row, expected)| {
            let row = row
                .as_object()
                .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
            let id = row.get("id").and_then(Value::as_str).unwrap_or_default();
            let inner = row
                .get("rust_method")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if id != expected || inner != format!("import_{}", full_hash(id)) {
                return Err(sdk_error("Native Rust SDK descriptor replay failed"));
            }
            Ok(Import {
                id: id.to_owned(),
                public_method: encode_stable_id(id)?,
                inner_method: inner.to_owned(),
                parameters: descriptor_parameters(
                    row.get("parameters")
                        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?,
                )?,
                result: descriptor_scalar(
                    row.get("result")
                        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?,
                    true,
                )?,
                failure_domain: {
                    let failure = row
                        .get("failure")
                        .and_then(Value::as_object)
                        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
                    match failure.get("kind").and_then(Value::as_str) {
                        Some("infallible") if failure.len() == 1 => None,
                        Some("status") if failure.len() == 2 => Some(
                            failure
                                .get("domain_id")
                                .and_then(Value::as_str)
                                .ok_or_else(|| {
                                    sdk_error("Native Rust SDK descriptor replay failed")
                                })?
                                .to_owned(),
                        ),
                        _ => {
                            return Err(sdk_error("Native Rust SDK descriptor replay failed"));
                        }
                    }
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut public_names = BTreeSet::new();
    if exports
        .iter()
        .map(|fact| &fact.public_method)
        .chain(imports.iter().map(|fact| &fact.public_method))
        .any(|name| !public_names.insert(name.clone()))
    {
        return Err(sdk_error("Native Rust SDK stable method encoding collided"));
    }
    Ok(DescriptorFacts {
        module: expected_module.to_owned(),
        source_revision: expected_revision.to_owned(),
        target: expected_target.to_owned(),
        exports,
        imports,
    })
}

fn parameters(parameters: &[Parameter]) -> String {
    parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| format!("arg_{index}: {}", parameter.ty.rust()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn arguments(parameters: &[Parameter]) -> String {
    (0..parameters.len())
        .map(|index| format!("arg_{index}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_lib(facts: &DescriptorFacts, capabilities: &[String]) -> String {
    let mut output = String::with_capacity(65_536);
    output.push_str("#[path=\"semaprax_native_rust_interop.rs\"]mod inner;\nmod public_api{#![forbid(unsafe_code)]\nuse super::inner;\nuse core::num::NonZeroU32;\n#[repr(u8)]#[derive(Clone,Copy,Debug,Eq,PartialEq)]pub enum NativeRustSdkStatusClass{Semantic=1,Contract=2,Import=3,Adapter=4}\n#[derive(Debug,Eq,PartialEq)]pub enum NativeRustSdkImportResult<T>{Success(T),Status{code:NonZeroU32,class:NativeRustSdkStatusClass,retryable:bool},HostFailure}\n#[derive(Debug,Eq,PartialEq)]pub enum NativeRustSdkCallError{Semantic{domain_id:&'static str,code:NonZeroU32,class:NativeRustSdkStatusClass,retryable:bool},HostFailed,HostPanicked,AdapterRejected}\n#[derive(Clone,Copy,Debug,Eq,PartialEq)]pub struct NativeRustSdkAdmissionError;\n");
    output.push_str("pub trait NativeRustSdkImports{");
    for import in &facts.imports {
        write!(
            output,
            "fn {}(&mut self{}{})->NativeRustSdkImportResult<{}>;",
            import.public_method,
            if import.parameters.is_empty() {
                ""
            } else {
                ","
            },
            parameters(&import.parameters),
            import.result.rust(),
        )
        .expect("writing generated Rust cannot fail");
    }
    output.push_str("}\nstruct HostAdapter<H>(H);\nimpl<H:NativeRustSdkImports>inner::NativeRustImports for HostAdapter<H>{");
    for import in &facts.imports {
        let args = arguments(&import.parameters);
        write!(
            output,
            "fn {}(&mut self{}{})->inner::NativeRustImportResult<{}>{{match self.0.{}({}){{NativeRustSdkImportResult::Success(value)=>inner::NativeRustImportResult::Success(value),NativeRustSdkImportResult::Status{{code,class,retryable}}=>inner::NativeRustImportResult::Status{{code,class:match class{{NativeRustSdkStatusClass::Semantic=>inner::NativeRustStatusClass::Semantic,NativeRustSdkStatusClass::Contract=>inner::NativeRustStatusClass::Contract,NativeRustSdkStatusClass::Import=>inner::NativeRustStatusClass::Import,NativeRustSdkStatusClass::Adapter=>inner::NativeRustStatusClass::Adapter}},retryable}},NativeRustSdkImportResult::HostFailure=>inner::NativeRustImportResult::HostFailure}}}}",
            import.inner_method,
            if import.parameters.is_empty() { "" } else { "," },
            parameters(&import.parameters),
            import.result.rust(),
            import.public_method,
            args,
        )
        .expect("writing generated Rust cannot fail");
    }
    output.push_str("}\npub struct NativeRustSdk<H:NativeRustSdkImports>{bridge:inner::NativeRustBridge<HostAdapter<H>>}\nimpl<H:NativeRustSdkImports>NativeRustSdk<H>{pub fn new(host:H,capabilities:&[&str])->Result<Self,NativeRustSdkAdmissionError>{let capabilities=inner::NativeRustCapabilities::new(capabilities).map_err(|_|NativeRustSdkAdmissionError)?;Ok(Self{bridge:inner::NativeRustBridge::new(HostAdapter(host),capabilities)})}\n");
    for export in &facts.exports {
        let args = arguments(&export.parameters);
        write!(
            output,
            "pub fn {}(&mut self{}{})->Result<{},NativeRustSdkCallError>{{self.bridge.{}({}).map_err(|error|match error{{inner::NativeRustCallError::Semantic{{domain_id,code,class,retryable}}=>NativeRustSdkCallError::Semantic{{domain_id,code,class:match class{{inner::NativeRustStatusClass::Semantic=>NativeRustSdkStatusClass::Semantic,inner::NativeRustStatusClass::Contract=>NativeRustSdkStatusClass::Contract,inner::NativeRustStatusClass::Import=>NativeRustSdkStatusClass::Import,inner::NativeRustStatusClass::Adapter=>NativeRustSdkStatusClass::Adapter}},retryable}},inner::NativeRustCallError::HostFailed=>NativeRustSdkCallError::HostFailed,inner::NativeRustCallError::HostPanicked=>NativeRustSdkCallError::HostPanicked,inner::NativeRustCallError::AdapterRejected=>NativeRustSdkCallError::AdapterRejected}})}}",
            export.public_method,
            if export.parameters.is_empty() { "" } else { "," },
            parameters(&export.parameters),
            export.result.rust(),
            export.inner_method,
            args,
        )
        .expect("writing generated Rust cannot fail");
    }
    output.push_str("}\n");
    output.push_str("pub const SEMAPRAX_NATIVE_RUST_SDK_CAPABILITIES:&[&str]=&[");
    for (index, capability) in capabilities.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        json_string(&mut output, capability);
    }
    output.push_str("];\n}\npub use public_api::*;\n");
    output
}

fn render_package_sources(facts: &DescriptorFacts, capabilities: &[String]) -> PackageSources {
    let cargo_toml = format!(
        "[package]\nname = \"{CRATE_NAME}\"\nversion = \"{CRATE_VERSION}\"\nedition = \"2021\"\nrust-version = \"1.85\"\npublish = false\nbuild = \"build.rs\"\n\n[lib]\npath = \"src/lib.rs\"\n\n[workspace]\n"
    );
    let archive = if cfg!(windows) {
        "semaprax_native_rust_sdk.lib"
    } else {
        "libsemaprax_native_rust_sdk.a"
    };
    let build_rs = format!(
        "#![forbid(unsafe_code)]\nfn main(){{let target=std::env::var(\"TARGET\").unwrap_or_default();if target!={:?}{{panic!(\"generated SEMAPRAX SDK target mismatch\")}}let root=std::env::var_os(\"CARGO_MANIFEST_DIR\").expect(\"Cargo must set CARGO_MANIFEST_DIR\");let native=std::path::PathBuf::from(root).join(\"native\");let native=native.to_str().filter(|path|!path.contains(['\\r','\\n'])).expect(\"generated SDK package path must be Unicode without CR/LF\");println!(\"cargo:rerun-if-changed=native/{archive}\");println!(\"cargo:rustc-link-search=native={{native}}\");println!(\"cargo:rustc-link-lib=static=semaprax_native_rust_sdk\");}}\n",
        facts.target,
    );
    PackageSources {
        cargo_toml,
        build_rs,
        lib_rs: render_lib(facts, capabilities),
    }
}

fn file_row(output: &mut String, path: &str, bytes: &[u8]) {
    output.push_str("{\"path\":");
    json_string(output, path);
    write!(output, ",\"bytes\":{},\"sha256\":", bytes.len()).expect("writing manifest cannot fail");
    json_string(output, &raw_digest(bytes));
    output.push('}');
}

fn signature(output: &mut String, parameters: &[Parameter], result: Scalar) {
    output.push_str("\"parameters\":[");
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"name\":");
        json_string(output, &parameter.name);
        output.push_str(",\"type\":");
        json_string(output, parameter.ty.wire());
        output.push('}');
    }
    output.push_str("],\"result\":");
    json_string(output, result.wire());
}

#[allow(clippy::too_many_arguments)]
fn render_sdk_manifest(
    facts: &DescriptorFacts,
    options: &NativeRustSdkOptions,
    descriptor: &[u8],
    inner_manifest: &[u8],
    sources: &PackageSources,
    safe_inner: &[u8],
    ffi_inner: &[u8],
    archive: &[u8],
) -> Result<String, Diagnostic> {
    let archive_path = if cfg!(windows) {
        "native/semaprax_native_rust_sdk.lib"
    } else {
        "native/libsemaprax_native_rust_sdk.a"
    };
    let mut files = vec![
        ("Cargo.toml", sources.cargo_toml.as_bytes()),
        ("build.rs", sources.build_rs.as_bytes()),
        ("native/descriptor.json", descriptor),
        (archive_path, archive),
        ("native/semaprax.native-rust-interop.json", inner_manifest),
        ("src/lib.rs", sources.lib_rs.as_bytes()),
        ("src/semaprax_native_rust_interop.rs", safe_inner),
        ("src/semaprax_native_rust_interop_ffi.rs", ffi_inner),
    ];
    files.sort_by_key(|(path, _)| path.as_bytes());
    let mut output = String::with_capacity(65_536);
    output.push_str("{\"schema\":");
    json_string(&mut output, SDK_SCHEMA);
    output.push_str(",\"crate\":{\"name\":");
    json_string(&mut output, CRATE_NAME);
    output.push_str(",\"version\":");
    json_string(&mut output, CRATE_VERSION);
    output.push_str(",\"target\":");
    json_string(&mut output, &facts.target);
    output.push_str("},\"source\":{\"module\":");
    json_string(&mut output, &facts.module);
    output.push_str(",\"revision\":");
    json_string(&mut output, &facts.source_revision);
    output.push_str("},\"inner\":{\"descriptor_digest\":");
    json_string(&mut output, &domain_digest(DESCRIPTOR_DOMAIN, descriptor));
    output.push_str(",\"bundle_digest\":");
    json_string(
        &mut output,
        &domain_digest(INNER_BUNDLE_DOMAIN, inner_manifest),
    );
    output.push_str("},\"files\":[");
    for (index, (path, bytes)) in files.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        file_row(&mut output, path, bytes);
    }
    output.push_str("],\"exports\":[");
    for (index, export) in facts.exports.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":");
        json_string(&mut output, &export.id);
        output.push_str(",\"method\":");
        json_string(&mut output, &export.public_method);
        output.push_str(",\"inner_method\":");
        json_string(&mut output, &export.inner_method);
        output.push(',');
        signature(&mut output, &export.parameters, export.result);
        output.push('}');
    }
    output.push_str("],\"imports\":[");
    for (index, import) in facts.imports.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"id\":");
        json_string(&mut output, &import.id);
        output.push_str(",\"method\":");
        json_string(&mut output, &import.public_method);
        output.push_str(",\"inner_method\":");
        json_string(&mut output, &import.inner_method);
        output.push(',');
        signature(&mut output, &import.parameters, import.result);
        output.push_str(",\"failure\":{\"kind\":");
        if let Some(domain) = &import.failure_domain {
            json_string(&mut output, "status");
            output.push_str(",\"domain_id\":");
            json_string(&mut output, domain);
        } else {
            json_string(&mut output, "infallible");
        }
        output.push('}');
        output.push('}');
    }
    output.push_str("],\"capabilities\":");
    string_array(&mut output, &options.capabilities);
    output.push_str(",\"limits\":");
    output.push_str(SDK_LIMITS_JSON);
    output.push_str(",\"nonclaims\":[");
    for (index, value) in SDK_NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        json_string(&mut output, value);
    }
    output.push_str("]}\n");
    if output.len() > MAX_SDK_MANIFEST_BYTES {
        return Err(sdk_error("Native Rust SDK manifest exceeds its bound"));
    }
    Ok(output)
}

struct ManifestReplay<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ManifestReplay<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn text(&mut self, expected: &str) -> Result<(), Diagnostic> {
        let end = self
            .offset
            .checked_add(expected.len())
            .ok_or_else(|| sdk_error("Native Rust SDK manifest replay failed"))?;
        if self.bytes.get(self.offset..end) != Some(expected.as_bytes()) {
            return Err(sdk_error("Native Rust SDK manifest replay failed"));
        }
        self.offset = end;
        Ok(())
    }

    fn json(&mut self, expected: &str) -> Result<(), Diagnostic> {
        let encoded = serde_json::to_string(expected)
            .map_err(|_| sdk_error("Native Rust SDK manifest replay failed"))?;
        self.text(&encoded)
    }

    fn number(&mut self, value: usize) -> Result<(), Diagnostic> {
        self.text(&value.to_string())
    }

    fn finish(self) -> Result<(), Diagnostic> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(sdk_error("Native Rust SDK manifest replay failed"))
        }
    }
}

fn replay_file(
    replay: &mut ManifestReplay<'_>,
    path: &str,
    bytes: &[u8],
) -> Result<(), Diagnostic> {
    replay.text("{\"path\":")?;
    replay.json(path)?;
    replay.text(",\"bytes\":")?;
    replay.number(bytes.len())?;
    replay.text(",\"sha256\":")?;
    replay.json(&raw_digest(bytes))?;
    replay.text("}")
}

fn replay_signature(
    replay: &mut ManifestReplay<'_>,
    parameters: &[Parameter],
    result: Scalar,
) -> Result<(), Diagnostic> {
    replay.text("\"parameters\":[")?;
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            replay.text(",")?;
        }
        replay.text("{\"name\":")?;
        replay.json(&parameter.name)?;
        replay.text(",\"type\":")?;
        replay.json(parameter.ty.wire())?;
        replay.text("}")?;
    }
    replay.text("],\"result\":")?;
    replay.json(result.wire())
}

#[allow(clippy::too_many_arguments)]
fn verify_sdk_manifest(
    manifest: &[u8],
    facts: &DescriptorFacts,
    options: &NativeRustSdkOptions,
    descriptor: &[u8],
    inner_manifest: &[u8],
    sources: &PackageSources,
    safe_inner: &[u8],
    ffi_inner: &[u8],
    archive: &[u8],
) -> Result<(), Diagnostic> {
    if manifest.len() > MAX_SDK_MANIFEST_BYTES || !manifest.ends_with(b"\n") {
        return Err(sdk_error("Native Rust SDK manifest replay failed"));
    }
    let archive_path = if cfg!(windows) {
        "native/semaprax_native_rust_sdk.lib"
    } else {
        "native/libsemaprax_native_rust_sdk.a"
    };
    let mut files = vec![
        ("Cargo.toml", sources.cargo_toml.as_bytes()),
        ("build.rs", sources.build_rs.as_bytes()),
        ("native/descriptor.json", descriptor),
        (archive_path, archive),
        ("native/semaprax.native-rust-interop.json", inner_manifest),
        ("src/lib.rs", sources.lib_rs.as_bytes()),
        ("src/semaprax_native_rust_interop.rs", safe_inner),
        ("src/semaprax_native_rust_interop_ffi.rs", ffi_inner),
    ];
    files.sort_by_key(|(path, _)| path.as_bytes());
    let mut replay = ManifestReplay::new(manifest);
    replay.text("{\"schema\":")?;
    replay.json(SDK_SCHEMA)?;
    replay.text(",\"crate\":{\"name\":")?;
    replay.json(CRATE_NAME)?;
    replay.text(",\"version\":")?;
    replay.json(CRATE_VERSION)?;
    replay.text(",\"target\":")?;
    replay.json(&facts.target)?;
    replay.text("},\"source\":{\"module\":")?;
    replay.json(&facts.module)?;
    replay.text(",\"revision\":")?;
    replay.json(&facts.source_revision)?;
    replay.text("},\"inner\":{\"descriptor_digest\":")?;
    replay.json(&domain_digest(DESCRIPTOR_DOMAIN, descriptor))?;
    replay.text(",\"bundle_digest\":")?;
    replay.json(&domain_digest(INNER_BUNDLE_DOMAIN, inner_manifest))?;
    replay.text("},\"files\":[")?;
    for (index, (path, bytes)) in files.iter().enumerate() {
        if index != 0 {
            replay.text(",")?;
        }
        replay_file(&mut replay, path, bytes)?;
    }
    replay.text("],\"exports\":[")?;
    for (index, export) in facts.exports.iter().enumerate() {
        if index != 0 {
            replay.text(",")?;
        }
        replay.text("{\"id\":")?;
        replay.json(&export.id)?;
        replay.text(",\"method\":")?;
        replay.json(&export.public_method)?;
        replay.text(",\"inner_method\":")?;
        replay.json(&export.inner_method)?;
        replay.text(",")?;
        replay_signature(&mut replay, &export.parameters, export.result)?;
        replay.text("}")?;
    }
    replay.text("],\"imports\":[")?;
    for (index, import) in facts.imports.iter().enumerate() {
        if index != 0 {
            replay.text(",")?;
        }
        replay.text("{\"id\":")?;
        replay.json(&import.id)?;
        replay.text(",\"method\":")?;
        replay.json(&import.public_method)?;
        replay.text(",\"inner_method\":")?;
        replay.json(&import.inner_method)?;
        replay.text(",")?;
        replay_signature(&mut replay, &import.parameters, import.result)?;
        replay.text(",\"failure\":{\"kind\":")?;
        if let Some(domain) = &import.failure_domain {
            replay.json("status")?;
            replay.text(",\"domain_id\":")?;
            replay.json(domain)?;
        } else {
            replay.json("infallible")?;
        }
        replay.text("}")?;
        replay.text("}")?;
    }
    replay.text("],\"capabilities\":[")?;
    for (index, capability) in options.capabilities.iter().enumerate() {
        if index != 0 {
            replay.text(",")?;
        }
        replay.json(capability)?;
    }
    replay.text("],\"limits\":")?;
    replay.text(SDK_LIMITS_JSON)?;
    replay.text(",\"nonclaims\":[")?;
    for (index, nonclaim) in SDK_NONCLAIMS.iter().enumerate() {
        if index != 0 {
            replay.text(",")?;
        }
        replay.json(nonclaim)?;
    }
    replay.text("]}\n")?;
    replay.finish()
}

fn simple_output_name(output: &Path) -> Result<&OsStr, Diagnostic> {
    use std::path::Component;
    let parent = output.parent().ok_or_else(publication_error)?;
    let name = output.file_name().ok_or_else(publication_error)?;
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || output.strip_prefix(parent).ok() != Some(Path::new(name))
    {
        return Err(publication_error());
    }
    Ok(name)
}

fn planned_child(
    parent_path: &Path,
    parent: &crate::platform::HeldDirectory,
    purpose: &str,
) -> Result<(String, PathBuf, crate::platform::PreparedStageName), Diagnostic> {
    for _ in 0..32 {
        let nonce = STAGE_NONCE.fetch_add(1, Ordering::Relaxed);
        let name = format!(
            ".semaprax-native-rust-sdk-{}-{nonce}-{purpose}",
            std::process::id()
        );
        let path = parent_path.join(&name);
        let probe = crate::platform::prepare_child_name(OsStr::new(&name))
            .map_err(|_| publication_error())?;
        if crate::platform::child_absent_prepared(parent, &probe)
            .map_err(|_| publication_error())?
        {
            let stage = crate::platform::prepare_stage_name(OsStr::new(&name))
                .map_err(|_| publication_error())?;
            return Ok((name, path, stage));
        }
    }
    Err(publication_error())
}

fn authenticate_inventory<const N: usize>(
    scan: &mut crate::platform::PreparedInventoryExact<N>,
    directory: &crate::platform::HeldDirectory,
    inventory: &crate::platform::PreparedDiscardInventory<N>,
) -> Result<(), Diagnostic> {
    crate::platform::inventory_exact_prepared(scan, directory, inventory)
        .map_err(|_| publication_error())?;
    crate::platform::recheck_directory(directory).map_err(|_| publication_error())?;
    Ok(())
}

struct InnerBundle {
    directory: crate::platform::HeldDirectory,
    name: crate::platform::PreparedStageName,
    inventory: crate::platform::PreparedDiscardInventory<7>,
}

fn authenticate_inner_bundle(
    parent: &crate::platform::HeldDirectory,
    prepared_name: crate::platform::PreparedStageName,
    path: &Path,
    object_name: &'static str,
    inventory: crate::platform::PreparedDiscardInventory<7>,
    mut scan: crate::platform::PreparedInventoryExact<7>,
) -> Result<InnerBundle, Diagnostic> {
    crate::platform::recheck_directory(parent).map_err(|_| publication_error())?;
    let directory = crate::platform::hold_directory(path).map_err(|_| publication_error())?;
    let mut inner = InnerBundle {
        directory,
        name: prepared_name,
        inventory,
    };
    let authentication = (|| -> Result<(), Diagnostic> {
        if !crate::platform::same_directory_path(&inner.directory, path)
            .map_err(|_| publication_error())?
        {
            return Err(publication_error());
        }
        for name in [
            "descriptor.json",
            "module.c",
            object_name,
            "semaprax_native_rust_interop.h",
            "semaprax_native_rust_interop.rs",
            "semaprax_native_rust_interop_ffi.rs",
            "semaprax.native-rust-interop.json",
        ] {
            let file = crate::platform::hold_regular_file(&inner.directory, OsStr::new(name))
                .map_err(|_| publication_error())?;
            inner
                .inventory
                .attach(name, file)
                .map_err(|_| publication_error())?;
        }
        authenticate_inventory(&mut scan, &inner.directory, &inner.inventory)
    })();
    if let Err(error) = authentication {
        let _ = discard_inner_bundle(parent, &inner);
        return Err(error);
    }
    Ok(inner)
}

fn read_inner<const N: usize>(
    inventory: &crate::platform::PreparedDiscardInventory<N>,
    name: &str,
    maximum: usize,
) -> Result<Vec<u8>, Diagnostic> {
    crate::platform::read_exact(
        inventory.file(name).map_err(|_| publication_error())?,
        maximum,
    )
    .map_err(|_| publication_error())
}

// Private B has already canonically replayed its own manifest before returning.
// Phase C composes with that trusted result by authenticating the returned
// digest and binding every exact payload row; it does not redefine B's wire.
fn verify_inner_payload_bindings(
    manifest: &[u8],
    artifacts: &InnerArtifacts<'_>,
    expected_digest: &str,
) -> Result<(), Diagnostic> {
    if manifest.len() > MAX_INNER_MANIFEST_BYTES
        || !manifest.ends_with(b"\n")
        || domain_digest(INNER_BUNDLE_DOMAIN, manifest) != expected_digest
    {
        return Err(sdk_error("Native Rust SDK inner payload binding failed"));
    }
    let value: Value = serde_json::from_slice(manifest)
        .map_err(|_| sdk_error("Native Rust SDK inner payload binding failed"))?;
    let root = value
        .as_object()
        .filter(|root| root.len() == 6)
        .ok_or_else(|| sdk_error("Native Rust SDK inner payload binding failed"))?;
    if root.get("schema").and_then(Value::as_str) != Some("semaprax.native-rust-interop-bundle.v1")
    {
        return Err(sdk_error("Native Rust SDK inner payload binding failed"));
    }
    let descriptor_row = root
        .get("descriptor")
        .and_then(Value::as_object)
        .filter(|row| row.len() == 3)
        .ok_or_else(|| sdk_error("Native Rust SDK inner payload binding failed"))?;
    if descriptor_row.get("schema").and_then(Value::as_str) != Some(DESCRIPTOR_SCHEMA)
        || descriptor_row.get("digest").and_then(Value::as_str)
            != Some(domain_digest(DESCRIPTOR_DOMAIN, artifacts.descriptor).as_str())
        || descriptor_row.get("bytes").and_then(Value::as_u64)
            != u64::try_from(artifacts.descriptor.len()).ok()
    {
        return Err(sdk_error("Native Rust SDK inner payload binding failed"));
    }
    let rows = root
        .get("files")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == 6)
        .ok_or_else(|| sdk_error("Native Rust SDK inner payload binding failed"))?;
    let known = [
        ("descriptor.json", artifacts.descriptor),
        ("module.c", artifacts.generated_c),
        (artifacts.object_name, artifacts.object),
        ("semaprax_native_rust_interop.h", artifacts.generated_header),
        ("semaprax_native_rust_interop.rs", artifacts.safe_rust),
        ("semaprax_native_rust_interop_ffi.rs", artifacts.ffi_rust),
    ];
    for (path, bytes) in known {
        let row = rows
            .iter()
            .find(|row| row.get("path").and_then(Value::as_str) == Some(path))
            .and_then(Value::as_object)
            .filter(|row| row.len() == 3)
            .ok_or_else(|| sdk_error("Native Rust SDK inner payload binding failed"))?;
        if row.get("bytes").and_then(Value::as_u64) != u64::try_from(bytes.len()).ok()
            || row.get("sha256").and_then(Value::as_str) != Some(raw_digest(bytes).as_str())
        {
            return Err(sdk_error("Native Rust SDK inner payload binding failed"));
        }
    }
    Ok(())
}

fn discard_inner_bundle(
    parent: &crate::platform::HeldDirectory,
    inner: &InnerBundle,
) -> Result<(), Diagnostic> {
    crate::platform::discard_owned_stage_prepared(
        parent,
        &inner.directory,
        &inner.name,
        &inner.inventory,
    )
    .map_err(|_| publication_error())
}

fn fail_after_inner(
    parent: &crate::platform::HeldDirectory,
    inner: &InnerBundle,
    primary: Diagnostic,
) -> PublicBuildError {
    if discard_inner_bundle(parent, inner).is_err() {
        PublicBuildError::Many(vec![primary, publication_error()])
    } else {
        PublicBuildError::One(primary)
    }
}

struct ArchiveStage {
    directory: crate::platform::HeldDirectory,
    name: crate::platform::PreparedStageName,
    inventory: crate::platform::PreparedDiscardInventory<2>,
}

fn create_archive_stage(
    parent: &crate::platform::HeldDirectory,
    prepared_name: crate::platform::PreparedStageName,
    path: &Path,
    object_name: &'static str,
    object: &[u8],
    inventory: crate::platform::PreparedDiscardInventory<2>,
) -> Result<ArchiveStage, StageCreationError> {
    let directory = crate::platform::create_directory_new_prepared(parent, &prepared_name, 0o700)
        .map_err(|_| StageCreationError::certain(publication_error()))?;
    let mut stage = ArchiveStage {
        directory,
        name: prepared_name,
        inventory,
    };
    let result = (|| -> Result<(), Diagnostic> {
        #[cfg(test)]
        if test_hook(TestBuildPoint::ArchiveCreationCleanupUncertainty) {
            return Err(injected_error());
        }
        if !crate::platform::same_directory_path(&stage.directory, path)
            .map_err(|_| publication_error())?
        {
            return Err(publication_error());
        }
        crate::platform::write_file_new_prepared(
            &stage.directory,
            &mut stage.inventory,
            object_name,
            object,
            0o600,
        )
        .map_err(|_| publication_error())
    })();
    if let Err(error) = result {
        let cleanup = discard_archive_stage(parent, &stage);
        return Err(if cleanup.is_err() {
            StageCreationError::uncertain(error)
        } else {
            StageCreationError::certain(error)
        });
    }
    Ok(stage)
}

fn discard_archive_stage(
    parent: &crate::platform::HeldDirectory,
    stage: &ArchiveStage,
) -> Result<(), Diagnostic> {
    crate::platform::discard_owned_stage_prepared(
        parent,
        &stage.directory,
        &stage.name,
        &stage.inventory,
    )
    .map_err(|_| publication_error())
}

fn fail_after_archive(
    parent: &crate::platform::HeldDirectory,
    archive: &ArchiveStage,
    inner: &InnerBundle,
    primary: Diagnostic,
) -> PublicBuildError {
    if discard_archive_stage(parent, archive).is_err() {
        return PublicBuildError::Many(vec![primary, publication_error()]);
    }
    if discard_inner_bundle(parent, inner).is_err() {
        return PublicBuildError::Many(vec![primary, publication_error()]);
    }
    PublicBuildError::One(primary)
}

struct OuterStage {
    directory: crate::platform::HeldDirectory,
    src: crate::platform::HeldDirectory,
    native: crate::platform::HeldDirectory,
    name: crate::platform::PreparedStageName,
    src_name: crate::platform::PreparedStageName,
    native_name: crate::platform::PreparedStageName,
    root_files: crate::platform::PreparedDiscardInventory<3>,
    src_files: crate::platform::PreparedDiscardInventory<3>,
    native_files: crate::platform::PreparedDiscardInventory<3>,
}

struct OuterStagePlan {
    name: crate::platform::PreparedStageName,
    src_name: crate::platform::PreparedStageName,
    native_name: crate::platform::PreparedStageName,
    root_files: crate::platform::PreparedDiscardInventory<3>,
    src_files: crate::platform::PreparedDiscardInventory<3>,
    native_files: crate::platform::PreparedDiscardInventory<3>,
}

impl OuterStage {
    fn recheck_all(
        &self,
        root_scan: &mut crate::platform::PreparedInventoryEntriesExact<5>,
        src_scan: &mut crate::platform::PreparedInventoryExact<3>,
        native_scan: &mut crate::platform::PreparedInventoryExact<3>,
    ) -> Result<(), Diagnostic> {
        crate::platform::recheck_directory(&self.directory).map_err(|_| publication_error())?;
        crate::platform::recheck_directory(&self.src).map_err(|_| publication_error())?;
        crate::platform::recheck_directory(&self.native).map_err(|_| publication_error())?;
        authenticate_inventory(src_scan, &self.src, &self.src_files)?;
        authenticate_inventory(native_scan, &self.native, &self.native_files)?;
        for name in ["Cargo.toml", "build.rs", "semaprax.native-rust-sdk.json"] {
            crate::platform::recheck_regular_file(
                self.root_files
                    .file(name)
                    .map_err(|_| publication_error())?,
            )
            .map_err(|_| publication_error())?;
        }
        crate::platform::inventory_entries_exact_prepared(
            root_scan,
            &self.directory,
            [
                self.root_files
                    .file("Cargo.toml")
                    .map_err(|_| publication_error())?,
                self.root_files
                    .file("build.rs")
                    .map_err(|_| publication_error())?,
                self.root_files
                    .file("semaprax.native-rust-sdk.json")
                    .map_err(|_| publication_error())?,
            ],
            [&self.src, &self.native],
        )
        .map_err(|_| publication_error())?;
        Ok(())
    }
}

fn create_outer_stage(
    parent: &crate::platform::HeldDirectory,
    path: &Path,
    plan: OuterStagePlan,
) -> Result<OuterStage, StageCreationError> {
    let OuterStagePlan {
        name: prepared_name,
        src_name,
        native_name,
        root_files,
        src_files,
        native_files,
    } = plan;
    let directory = crate::platform::create_directory_new_prepared(parent, &prepared_name, 0o700)
        .map_err(|_| StageCreationError::certain(publication_error()))?;
    let same_root = crate::platform::same_directory_path(&directory, path)
        .map_err(|_| publication_error())
        .unwrap_or(false);
    if !same_root {
        let cleanup = crate::platform::discard_owned_stage_prepared(
            parent,
            &directory,
            &prepared_name,
            &root_files,
        );
        return Err(if cleanup.is_err() {
            StageCreationError::uncertain(publication_error())
        } else {
            StageCreationError::certain(publication_error())
        });
    }
    let src = match crate::platform::create_directory_new_prepared(&directory, &src_name, 0o700) {
        Ok(src) => src,
        Err(_) => {
            let cleanup = crate::platform::discard_owned_stage_prepared(
                parent,
                &directory,
                &prepared_name,
                &root_files,
            );
            return Err(if cleanup.is_err() {
                StageCreationError::uncertain(publication_error())
            } else {
                StageCreationError::certain(publication_error())
            });
        }
    };
    let native =
        match crate::platform::create_directory_new_prepared(&directory, &native_name, 0o700) {
            Ok(native) => native,
            Err(_) => {
                if crate::platform::discard_owned_stage_prepared(
                    &directory, &src, &src_name, &src_files,
                )
                .is_err()
                {
                    return Err(StageCreationError::uncertain(publication_error()));
                }
                if crate::platform::discard_owned_stage_prepared(
                    parent,
                    &directory,
                    &prepared_name,
                    &root_files,
                )
                .is_err()
                {
                    return Err(StageCreationError::uncertain(publication_error()));
                }
                return Err(StageCreationError::certain(publication_error()));
            }
        };
    let same_src = crate::platform::same_directory_path(&src, &path.join("src"))
        .map_err(|_| publication_error())
        .unwrap_or(false);
    let same_native = crate::platform::same_directory_path(&native, &path.join("native"))
        .map_err(|_| publication_error())
        .unwrap_or(false);
    if !same_src || !same_native {
        if crate::platform::discard_owned_stage_prepared(&directory, &src, &src_name, &src_files)
            .is_err()
        {
            return Err(StageCreationError::uncertain(publication_error()));
        }
        if crate::platform::discard_owned_stage_prepared(
            &directory,
            &native,
            &native_name,
            &native_files,
        )
        .is_err()
        {
            return Err(StageCreationError::uncertain(publication_error()));
        }
        if crate::platform::discard_owned_stage_prepared(
            parent,
            &directory,
            &prepared_name,
            &root_files,
        )
        .is_err()
        {
            return Err(StageCreationError::uncertain(publication_error()));
        }
        return Err(StageCreationError::certain(publication_error()));
    }
    Ok(OuterStage {
        directory,
        src,
        native,
        name: prepared_name,
        src_name,
        native_name,
        root_files,
        src_files,
        native_files,
    })
}

#[allow(clippy::too_many_arguments)]
fn populate_outer_stage(
    stage: &mut OuterStage,
    sources: &PackageSources,
    descriptor: &[u8],
    inner_manifest: &[u8],
    safe_inner: &[u8],
    ffi_inner: &[u8],
    archive: &[u8],
    manifest: &str,
    archive_name: &str,
) -> Result<(), Diagnostic> {
    for (name, bytes) in [
        ("Cargo.toml", sources.cargo_toml.as_bytes()),
        ("build.rs", sources.build_rs.as_bytes()),
        ("semaprax.native-rust-sdk.json", manifest.as_bytes()),
    ] {
        crate::platform::write_file_new_prepared(
            &stage.directory,
            &mut stage.root_files,
            name,
            bytes,
            0o600,
        )
        .map_err(|_| publication_error())?;
        #[cfg(test)]
        if name == "Cargo.toml" && test_hook(TestBuildPoint::AfterFirstOuterWrite) {
            return Err(injected_error());
        }
    }
    for (name, bytes) in [
        ("lib.rs", sources.lib_rs.as_bytes()),
        ("semaprax_native_rust_interop.rs", safe_inner),
        ("semaprax_native_rust_interop_ffi.rs", ffi_inner),
    ] {
        crate::platform::write_file_new_prepared(
            &stage.src,
            &mut stage.src_files,
            name,
            bytes,
            0o600,
        )
        .map_err(|_| publication_error())?;
    }
    for (name, bytes) in [
        ("descriptor.json", descriptor),
        (archive_name, archive),
        ("semaprax.native-rust-interop.json", inner_manifest),
    ] {
        crate::platform::write_file_new_prepared(
            &stage.native,
            &mut stage.native_files,
            name,
            bytes,
            0o600,
        )
        .map_err(|_| publication_error())?;
    }
    Ok(())
}

fn discard_outer_stage(
    parent: &crate::platform::HeldDirectory,
    stage: &OuterStage,
) -> Result<(), Diagnostic> {
    let src = crate::platform::discard_owned_stage_prepared(
        &stage.directory,
        &stage.src,
        &stage.src_name,
        &stage.src_files,
    );
    if src.is_err() {
        return Err(publication_error());
    }
    let native = crate::platform::discard_owned_stage_prepared(
        &stage.directory,
        &stage.native,
        &stage.native_name,
        &stage.native_files,
    );
    if native.is_err() {
        return Err(publication_error());
    }
    crate::platform::discard_owned_stage_prepared(
        parent,
        &stage.directory,
        &stage.name,
        &stage.root_files,
    )
    .map_err(|_| publication_error())
}

fn fail_before_publish(
    parent: &crate::platform::HeldDirectory,
    outer: &OuterStage,
    archive: &ArchiveStage,
    inner: &InnerBundle,
    primary: Diagnostic,
) -> PublicBuildError {
    if discard_outer_stage(parent, outer).is_err() {
        return PublicBuildError::Many(vec![primary, publication_error()]);
    }
    if discard_archive_stage(parent, archive).is_err() {
        return PublicBuildError::Many(vec![primary, publication_error()]);
    }
    if discard_inner_bundle(parent, inner).is_err() {
        return PublicBuildError::Many(vec![primary, publication_error()]);
    }
    PublicBuildError::One(primary)
}

fn verify_published_package(
    output: &Path,
    package: &PublishedPackage<'_>,
) -> Result<Vec<u8>, Diagnostic> {
    let root = crate::platform::hold_directory(output).map_err(|_| publication_error())?;
    let src =
        crate::platform::hold_directory(&output.join("src")).map_err(|_| publication_error())?;
    let native =
        crate::platform::hold_directory(&output.join("native")).map_err(|_| publication_error())?;
    let manifest =
        crate::platform::hold_regular_file(&root, OsStr::new("semaprax.native-rust-sdk.json"))
            .map_err(|_| publication_error())?;
    let bytes = crate::platform::read_exact(&manifest, MAX_SDK_MANIFEST_BYTES)
        .map_err(|_| publication_error())?;
    if bytes != package.manifest.as_bytes() {
        return Err(publication_error());
    }
    let cargo = hold_matching(
        &root,
        "Cargo.toml",
        package.sources.cargo_toml.as_bytes(),
        MAX_SDK_MANIFEST_BYTES,
    )?;
    let build = hold_matching(
        &root,
        "build.rs",
        package.sources.build_rs.as_bytes(),
        MAX_SDK_MANIFEST_BYTES,
    )?;
    let lib = hold_matching(
        &src,
        "lib.rs",
        package.sources.lib_rs.as_bytes(),
        MAX_GENERATED_RUST_BYTES,
    )?;
    let safe = hold_matching(
        &src,
        "semaprax_native_rust_interop.rs",
        package.safe_inner,
        MAX_GENERATED_RUST_BYTES,
    )?;
    let ffi = hold_matching(
        &src,
        "semaprax_native_rust_interop_ffi.rs",
        package.ffi_inner,
        MAX_GENERATED_RUST_BYTES,
    )?;
    let descriptor_file = hold_matching(
        &native,
        "descriptor.json",
        package.descriptor,
        MAX_DESCRIPTOR_BYTES,
    )?;
    let archive_file = hold_matching(
        &native,
        package.archive_name,
        package.archive,
        MAX_ARCHIVE_BYTES,
    )?;
    let inner_manifest_file = hold_matching(
        &native,
        "semaprax.native-rust-interop.json",
        package.inner_manifest,
        MAX_INNER_MANIFEST_BYTES,
    )?;

    let mut src_scan = crate::platform::prepare_inventory_entries_exact(
        [
            OsStr::new("lib.rs"),
            OsStr::new("semaprax_native_rust_interop.rs"),
            OsStr::new("semaprax_native_rust_interop_ffi.rs"),
        ],
        3,
    )
    .map_err(|_| publication_error())?;
    crate::platform::inventory_entries_exact_prepared(&mut src_scan, &src, [&lib, &safe, &ffi], [])
        .map_err(|_| publication_error())?;
    let mut native_scan = crate::platform::prepare_inventory_entries_exact(
        [
            OsStr::new("descriptor.json"),
            OsStr::new(package.archive_name),
            OsStr::new("semaprax.native-rust-interop.json"),
        ],
        3,
    )
    .map_err(|_| publication_error())?;
    crate::platform::inventory_entries_exact_prepared(
        &mut native_scan,
        &native,
        [&descriptor_file, &archive_file, &inner_manifest_file],
        [],
    )
    .map_err(|_| publication_error())?;
    let mut root_scan = crate::platform::prepare_inventory_entries_exact(
        [
            OsStr::new("Cargo.toml"),
            OsStr::new("build.rs"),
            OsStr::new("semaprax.native-rust-sdk.json"),
            OsStr::new("src"),
            OsStr::new("native"),
        ],
        3,
    )
    .map_err(|_| publication_error())?;
    crate::platform::inventory_entries_exact_prepared(
        &mut root_scan,
        &root,
        [&cargo, &build, &manifest],
        [&src, &native],
    )
    .map_err(|_| publication_error())?;
    crate::platform::recheck_directory(&root).map_err(|_| publication_error())?;
    crate::platform::recheck_directory(&src).map_err(|_| publication_error())?;
    crate::platform::recheck_directory(&native).map_err(|_| publication_error())?;
    Ok(bytes)
}

fn hold_matching(
    directory: &crate::platform::HeldDirectory,
    name: &str,
    expected: &[u8],
    maximum: usize,
) -> Result<crate::platform::HeldRegularFile, Diagnostic> {
    let held = crate::platform::hold_regular_file(directory, OsStr::new(name))
        .map_err(|_| publication_error())?;
    let actual = crate::platform::read_exact(&held, maximum).map_err(|_| publication_error())?;
    if actual != expected || raw_digest(&actual) != raw_digest(expected) {
        return Err(publication_error());
    }
    Ok(held)
}

/// Builds and publishes one fresh, current-host Native Rust SDK package.
pub fn build_native_rust_sdk(
    source: &str,
    source_path: &Path,
    options: NativeRustSdkOptions,
    output: &Path,
) -> Result<NativeRustSdkBundle, Vec<Diagnostic>> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![sdk_error("Native Rust SDK source exceeds its bound")]);
    }
    let program = semaprax::check(source, source_path)?;
    build_native_rust_sdk_inner(&program, options, output)
        .map_err(PublicBuildError::into_diagnostics)
}

fn build_native_rust_sdk_inner(
    program: &crate::ast::Program,
    options: NativeRustSdkOptions,
    output: &Path,
) -> Result<NativeRustSdkBundle, PublicBuildError> {
    let options = NativeRustSdkOptions {
        exports: canonical_values(options.exports, MAX_EXPORTS)?,
        imports: canonical_values(options.imports, MAX_IMPORTS)?,
        capabilities: canonical_values(options.capabilities, MAX_EFFECTS)?,
    };
    if options.exports.is_empty()
        || options
            .exports
            .iter()
            .any(|id| options.imports.binary_search(id).is_ok())
    {
        return Err(sdk_error("Native Rust SDK export and import selections are invalid").into());
    }
    let canonical_source = semaprax::format::canonical(program);
    let source_revision = domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes());
    let target = target_triple()
        .ok_or_else(|| sdk_error("Native Rust SDK current target is unsupported"))?;
    let spec = canonical_spec(&program.module, &source_revision, target, &options)?;
    if output
        .to_str()
        .filter(|path| !path.contains(['\r', '\n']))
        .is_none()
    {
        return Err(publication_error().into());
    }
    let output_name = simple_output_name(output)?;
    let parent_path = output.parent().ok_or_else(publication_error)?;
    if !output.is_absolute() || !parent_path.is_absolute() {
        return Err(publication_error().into());
    }
    let parent = crate::platform::hold_directory(parent_path).map_err(|_| publication_error())?;
    crate::platform::recheck_directory(&parent).map_err(|_| publication_error())?;
    let output_probe =
        crate::platform::prepare_child_name(output_name).map_err(|_| publication_error())?;
    if !crate::platform::child_absent_prepared(&parent, &output_probe)
        .map_err(|_| publication_error())?
    {
        return Err(publication_error().into());
    }

    // All Phase-C process and publication plans are fixed before A+B starts.
    let configured_archiver = std::env::var_os("SEMAPRAX_ARCHIVER")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(publication_error)?;
    #[cfg(windows)]
    let vctools = std::env::var_os("SEMAPRAX_VCTOOLS")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(publication_error)?;
    #[cfg(not(windows))]
    let vctools: Option<PathBuf> = None;
    let archiver =
        crate::platform::hold_configured_archiver(configured_archiver, vctools.as_deref())
            .map_err(|_| publication_error())?;
    let process_plan =
        crate::platform::prepare_process_arena_plan(1).map_err(|_| publication_error())?;
    let mut process_arena = crate::platform::materialize_process_arena(process_plan)
        .map_err(|_| publication_error())?;
    let object_name = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    let archive_name = if cfg!(windows) {
        "semaprax_native_rust_sdk.lib"
    } else {
        "libsemaprax_native_rust_sdk.a"
    };
    let (_inner_text, inner_path, inner_stage_name) = planned_child(parent_path, &parent, "inner")?;
    let (_archive_text, archive_stage_path, archive_stage_name) =
        planned_child(parent_path, &parent, "archive")?;
    let (_outer_text, outer_path, outer_stage_name) =
        planned_child(parent_path, &parent, "package")?;
    let inner_inventory = crate::platform::prepare_discard_inventory([
        OsStr::new("descriptor.json"),
        OsStr::new("module.c"),
        OsStr::new(object_name),
        OsStr::new("semaprax_native_rust_interop.h"),
        OsStr::new("semaprax_native_rust_interop.rs"),
        OsStr::new("semaprax_native_rust_interop_ffi.rs"),
        OsStr::new("semaprax.native-rust-interop.json"),
    ])
    .map_err(|_| publication_error())?;
    let archive_inventory = crate::platform::prepare_discard_inventory([
        OsStr::new(object_name),
        OsStr::new(archive_name),
    ])
    .map_err(|_| publication_error())?;
    #[cfg(test)]
    let mut archive_inventory = archive_inventory;
    #[cfg(test)]
    if test_hook(TestBuildPoint::ArchiveCreationCleanupUncertainty) {
        archive_inventory.inject_discard_failure_after_delete(Some(0));
    }
    let root_inventory = crate::platform::prepare_discard_inventory([
        OsStr::new("Cargo.toml"),
        OsStr::new("build.rs"),
        OsStr::new("semaprax.native-rust-sdk.json"),
    ])
    .map_err(|_| publication_error())?;
    let src_inventory = crate::platform::prepare_discard_inventory([
        OsStr::new("lib.rs"),
        OsStr::new("semaprax_native_rust_interop.rs"),
        OsStr::new("semaprax_native_rust_interop_ffi.rs"),
    ])
    .map_err(|_| publication_error())?;
    let native_inventory = crate::platform::prepare_discard_inventory([
        OsStr::new("descriptor.json"),
        OsStr::new(archive_name),
        OsStr::new("semaprax.native-rust-interop.json"),
    ])
    .map_err(|_| publication_error())?;
    let src_stage_name =
        crate::platform::prepare_stage_name(OsStr::new("src")).map_err(|_| publication_error())?;
    let native_stage_name = crate::platform::prepare_stage_name(OsStr::new("native"))
        .map_err(|_| publication_error())?;
    let inner_scan = crate::platform::prepare_inventory_exact(&inner_inventory)
        .map_err(|_| publication_error())?;
    let mut archive_scan = crate::platform::prepare_inventory_exact(&archive_inventory)
        .map_err(|_| publication_error())?;
    let mut src_stage_scan = crate::platform::prepare_inventory_exact(&src_inventory)
        .map_err(|_| publication_error())?;
    let mut native_stage_scan = crate::platform::prepare_inventory_exact(&native_inventory)
        .map_err(|_| publication_error())?;
    let mut src_publish_scan = crate::platform::prepare_inventory_exact(&src_inventory)
        .map_err(|_| publication_error())?;
    let mut native_publish_scan = crate::platform::prepare_inventory_exact(&native_inventory)
        .map_err(|_| publication_error())?;
    let root_entry_names = [
        OsStr::new("Cargo.toml"),
        OsStr::new("build.rs"),
        OsStr::new("semaprax.native-rust-sdk.json"),
        OsStr::new("src"),
        OsStr::new("native"),
    ];
    let mut root_stage_scan = crate::platform::prepare_inventory_entries_exact(root_entry_names, 3)
        .map_err(|_| publication_error())?;
    let mut root_publish_scan =
        crate::platform::prepare_inventory_entries_exact(root_entry_names, 3)
            .map_err(|_| publication_error())?;
    let archive_invocation = crate::platform::prepare_archive_invocation(
        OsStr::new(object_name),
        OsStr::new(archive_name),
    )
    .map_err(|_| publication_error())?;
    let mut final_publish =
        crate::platform::prepare_publish_directory(output_name).map_err(|_| publication_error())?;

    // Private B remains byte-for-byte unchanged and publishes into an owned
    // sibling scratch directory that Phase C authenticates independently.
    let inner_facts = match crate::implementation::build_native_rust_interop_bundle(
        program,
        spec.as_bytes(),
        &inner_path,
    ) {
        Ok(facts) => facts,
        Err(errors) => return Err(PublicBuildError::Many(errors)),
    };
    let inner = authenticate_inner_bundle(
        &parent,
        inner_stage_name,
        &inner_path,
        object_name,
        inner_inventory,
        inner_scan,
    )?;
    let inner_prepared = (|| -> Result<_, Diagnostic> {
        if inner_facts.output_directory() != inner_path.as_path()
            || inner_facts.descriptor_path() != inner_path.join("descriptor.json")
            || inner_facts.manifest_path() != inner_path.join("semaprax.native-rust-interop.json")
        {
            return Err(publication_error());
        }
        let descriptor = read_inner(&inner.inventory, "descriptor.json", MAX_DESCRIPTOR_BYTES)?;
        let inner_manifest = read_inner(
            &inner.inventory,
            "semaprax.native-rust-interop.json",
            MAX_INNER_MANIFEST_BYTES,
        )?;
        let safe_inner = read_inner(
            &inner.inventory,
            "semaprax_native_rust_interop.rs",
            MAX_GENERATED_RUST_BYTES,
        )?;
        let ffi_inner = read_inner(
            &inner.inventory,
            "semaprax_native_rust_interop_ffi.rs",
            MAX_GENERATED_RUST_BYTES,
        )?;
        let object = read_inner(&inner.inventory, object_name, MAX_OBJECT_BYTES)?;
        let generated_c = read_inner(&inner.inventory, "module.c", MAX_OBJECT_BYTES)?;
        let generated_header = read_inner(
            &inner.inventory,
            "semaprax_native_rust_interop.h",
            MAX_DESCRIPTOR_BYTES,
        )?;
        verify_inner_payload_bindings(
            &inner_manifest,
            &InnerArtifacts {
                descriptor: &descriptor,
                generated_c: &generated_c,
                generated_header: &generated_header,
                safe_rust: &safe_inner,
                ffi_rust: &ffi_inner,
                object: &object,
                object_name,
            },
            inner_facts.manifest_digest(),
        )?;
        let descriptor_facts = parse_descriptor(
            &descriptor,
            &program.module,
            &source_revision,
            target,
            &options,
        )?;
        let sources = render_package_sources(&descriptor_facts, &options.capabilities);
        Ok((
            descriptor,
            inner_manifest,
            safe_inner,
            ffi_inner,
            object,
            descriptor_facts,
            sources,
        ))
    })();
    let (descriptor, inner_manifest, safe_inner, ffi_inner, object, descriptor_facts, sources) =
        match inner_prepared {
            Ok(prepared) => prepared,
            Err(error) => return Err(fail_after_inner(&parent, &inner, error)),
        };

    // The archiver sees a private held run stage and one exact held object.
    let mut archive_stage = match create_archive_stage(
        &parent,
        archive_stage_name,
        &archive_stage_path,
        object_name,
        &object,
        archive_inventory,
    ) {
        Ok(stage) => stage,
        Err(error) if error.settlement_uncertain => return Err(error.stop()),
        Err(error) => return Err(fail_after_inner(&parent, &inner, error.primary)),
    };
    let archive_result = (|| -> Result<Vec<u8>, Diagnostic> {
        #[cfg(test)]
        if test_hook(TestBuildPoint::BeforeArchive) {
            return Err(injected_error());
        }
        #[cfg(test)]
        if test_hook(TestBuildPoint::ArchiveOutputMutation) {
            std::fs::write(archive_stage_path.join(archive_name), b"foreign")
                .map_err(|_| publication_error())?;
        }
        #[cfg(test)]
        record_archive_attempt();
        let archive_file = crate::platform::archive_tool_prepared(
            &archiver,
            &archive_stage.directory,
            archive_stage
                .inventory
                .file(object_name)
                .map_err(|_| publication_error())?,
            archive_invocation,
            &mut process_arena,
        )
        .map_err(|_| publication_error())?;
        archive_stage
            .inventory
            .attach(archive_name, archive_file)
            .map_err(|_| publication_error())?;
        authenticate_inventory(
            &mut archive_scan,
            &archive_stage.directory,
            &archive_stage.inventory,
        )?;
        crate::platform::read_exact(
            archive_stage
                .inventory
                .file(archive_name)
                .map_err(|_| publication_error())?,
            MAX_ARCHIVE_BYTES,
        )
        .map_err(|_| publication_error())
    })();
    let archive = match archive_result {
        Ok(archive) => archive,
        Err(error) => {
            return Err(fail_after_archive(&parent, &archive_stage, &inner, error));
        }
    };

    let mut outer = match create_outer_stage(
        &parent,
        &outer_path,
        OuterStagePlan {
            name: outer_stage_name,
            src_name: src_stage_name,
            native_name: native_stage_name,
            root_files: root_inventory,
            src_files: src_inventory,
            native_files: native_inventory,
        },
    ) {
        Ok(stage) => stage,
        Err(error) if error.settlement_uncertain => return Err(error.stop()),
        Err(error) => {
            return Err(fail_after_archive(
                &parent,
                &archive_stage,
                &inner,
                error.primary,
            ));
        }
    };
    let outer_result = (|| -> Result<String, Diagnostic> {
        let manifest = render_sdk_manifest(
            &descriptor_facts,
            &options,
            &descriptor,
            &inner_manifest,
            &sources,
            &safe_inner,
            &ffi_inner,
            &archive,
        )?;
        verify_sdk_manifest(
            manifest.as_bytes(),
            &descriptor_facts,
            &options,
            &descriptor,
            &inner_manifest,
            &sources,
            &safe_inner,
            &ffi_inner,
            &archive,
        )?;
        populate_outer_stage(
            &mut outer,
            &sources,
            &descriptor,
            &inner_manifest,
            &safe_inner,
            &ffi_inner,
            &archive,
            &manifest,
            archive_name,
        )?;
        outer.recheck_all(
            &mut root_stage_scan,
            &mut src_stage_scan,
            &mut native_stage_scan,
        )?;
        Ok(manifest)
    })();
    let manifest = match outer_result {
        Ok(manifest) => manifest,
        Err(error) => {
            return Err(fail_before_publish(
                &parent,
                &outer,
                &archive_stage,
                &inner,
                error,
            ));
        }
    };

    // Scratch settlement is mandatory before the public pivot. Any uncertain
    // discard is sticky and prevents later publication.
    #[cfg(test)]
    if test_hook(TestBuildPoint::ScratchCleanupUncertainty) {
        std::fs::write(archive_stage_path.join("foreign"), b"foreign")
            .map_err(|_| publication_error())?;
    }
    if discard_archive_stage(&parent, &archive_stage).is_err() {
        return Err(publication_error().into());
    }
    if discard_inner_bundle(&parent, &inner).is_err() {
        return Err(publication_error().into());
    }
    let publication = (|| -> Result<(), Diagnostic> {
        #[cfg(test)]
        #[cfg(debug_assertions)]
        if test_hook(TestBuildPoint::BeforePublish) {
            crate::platform::inject_publish_directory_failure(&mut final_publish, 4)
                .map_err(|_| publication_error())?;
        }
        crate::platform::recheck_directory(&parent).map_err(|_| publication_error())?;
        outer.recheck_all(
            &mut root_publish_scan,
            &mut src_publish_scan,
            &mut native_publish_scan,
        )?;
        #[cfg(test)]
        record_publish_call();
        crate::platform::publish_directory_new_prepared(
            &mut final_publish,
            &parent,
            &outer.directory,
            &outer.name,
            output_name,
        )
        .map_err(|_| publication_error())
    })();
    if let Err(error) = publication {
        let cleanup = discard_outer_stage(&parent, &outer);
        return Err(if cleanup.is_err() {
            publication_error().into()
        } else {
            error.into()
        });
    }

    // Post-publication replay is read-only. Failure leaves the complete,
    // digest-bound package for caller reconciliation; it is never deleted.
    #[cfg(test)]
    if test_hook(TestBuildPoint::PostPivotAuthenticationFailure) {
        return Err(publication_error().into());
    }
    let published_manifest = verify_published_package(
        output,
        &PublishedPackage {
            manifest: &manifest,
            archive_name,
            sources: &sources,
            descriptor: &descriptor,
            inner_manifest: &inner_manifest,
            safe_inner: &safe_inner,
            ffi_inner: &ffi_inner,
            archive: &archive,
        },
    )?;
    verify_sdk_manifest(
        &published_manifest,
        &descriptor_facts,
        &options,
        &descriptor,
        &inner_manifest,
        &sources,
        &safe_inner,
        &ffi_inner,
        &archive,
    )?;
    let manifest_digest = domain_digest(SDK_MANIFEST_DOMAIN, manifest.as_bytes());
    Ok(NativeRustSdkBundle {
        output_directory: output.to_path_buf(),
        manifest_path: output.join("semaprax.native-rust-sdk.json"),
        manifest_digest,
        crate_name: CRATE_NAME.to_owned(),
        target_triple: target.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_SOURCE: &str = r#"module sdk.path_fixture;

@id("sdk.value")
fn value() -> i64 { 1 }

@id("sdk.main")
fn main() -> i64 { 0 }
"#;

    const BOUNDARY_SOURCE: &str = r#"module interop.fixture;

permit { host.math }

@id("host.math")
interface HostMath permits { host.math } {
    @id("host.add")
    import rust fn host_add(left: i64, right: i64) -> i64
        effects { host.math }
        failure status "host.math.v1";
}

@id("interop.add")
fn add(left: i64, right: i64) -> i64 uses { host.math } {
    host_add(left, right) + right
}

@id("interop.main")
fn main() -> i64 { 0 }
"#;

    fn minimal_options() -> NativeRustSdkOptions {
        NativeRustSdkOptions {
            exports: vec!["sdk.value".into()],
            imports: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    fn boundary_options() -> NativeRustSdkOptions {
        NativeRustSdkOptions {
            exports: vec!["interop.add".into()],
            imports: vec!["host.add".into()],
            capabilities: vec!["host.math".into()],
        }
    }

    #[test]
    fn stable_id_method_encoding_is_injective_for_the_public_grammar() {
        assert_eq!(
            encode_stable_id("calculator.add").unwrap(),
            "spx_calculator_dot_add"
        );
        assert_eq!(
            encode_stable_id("a_b-c.d").unwrap(),
            "spx_a_underscore_b_hyphen_c_dot_d"
        );
        let ids = ["a.b", "a_b", "a-b", "ab", "a0"];
        let encoded = ids
            .iter()
            .map(|id| encode_stable_id(id).unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(encoded.len(), ids.len());
    }

    #[test]
    fn canonical_options_reject_duplicates_and_nonportable_ids() {
        assert!(canonical_values(vec!["a.b".into(), "a.b".into()], 32).is_err());
        assert!(canonical_values(vec!["Upper".into()], 32).is_err());
        assert_eq!(
            canonical_values(vec!["z".into(), "a".into()], 32).unwrap(),
            ["a", "z"]
        );
    }

    #[test]
    fn generated_cargo_package_has_no_dependency_or_repository_escape() {
        let facts = DescriptorFacts {
            module: "calculator".into(),
            source_revision: "sha256:00".into(),
            target: target_triple().unwrap().into(),
            exports: Vec::new(),
            imports: Vec::new(),
        };
        let sources = render_package_sources(&facts, &[]);
        assert!(sources.cargo_toml.contains("publish = false"));
        assert!(!sources.cargo_toml.contains("dependencies"));
        assert!(!sources.cargo_toml.contains("path = \"../"));
        assert!(!sources.build_rs.contains("Command"));
        assert!(!sources.build_rs.contains("cargo:rustc-env"));
        assert!(sources.build_rs.contains("var_os(\"CARGO_MANIFEST_DIR\")"));
        assert!(sources.build_rs.contains("path.contains(['\\r','\\n'])"));
        assert!(!sources
            .build_rs
            .contains("cargo:rustc-link-search=native=native"));
        assert!(!sources.build_rs.contains("eprintln!"));
    }

    #[test]
    fn hostile_output_paths_are_rejected_before_tool_configuration() {
        let newline = std::env::temp_dir().join("semaprax-sdk-hostile\noutput");
        let error = build_native_rust_sdk(
            MINIMAL_SOURCE,
            Path::new("path-fixture.spx"),
            minimal_options(),
            &newline,
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I233");
        assert!(std::fs::symlink_metadata(&newline).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt as _;
            let non_unicode = std::env::temp_dir().join(std::ffi::OsString::from_vec(vec![0xff]));
            let error = build_native_rust_sdk(
                MINIMAL_SOURCE,
                Path::new("path-fixture.spx"),
                minimal_options(),
                &non_unicode,
            )
            .unwrap_err();
            assert_eq!(error[0].code, "SPX-I233");
            assert!(std::fs::symlink_metadata(&non_unicode).is_err());
        }
    }

    #[test]
    fn effect_boundaries_fail_stop_and_preserve_sticky_status() {
        if std::env::var_os("RUSTC").is_none()
            || std::env::var_os("CLANG").is_none()
            || std::env::var_os("SEMAPRAX_ARCHIVER").is_none()
        {
            return;
        }
        let program = semaprax::check(BOUNDARY_SOURCE, Path::new("boundary-fixture.spx")).unwrap();
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-sdk-boundaries-{}-{}",
                std::process::id(),
                STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir(&root).unwrap();

        let run = |point, name: &str| {
            TEST_BUILD_STATE.with(|state| {
                state.set(TestBuildState {
                    point: Some(point),
                    archive_attempts: 0,
                    publish_calls: 0,
                });
            });
            let output = root.join(name);
            let result = build_native_rust_sdk_inner(&program, boundary_options(), &output);
            let state = TEST_BUILD_STATE.with(std::cell::Cell::get);
            TEST_BUILD_STATE.with(|slot| slot.set(TestBuildState::default()));
            (output, result, state)
        };

        let (output, error, state) = run(TestBuildPoint::BeforeArchive, "before-archive");
        let diagnostics = error.err().unwrap().into_diagnostics();
        assert_eq!(
            diagnostics[0].code, "SPX-B112",
            "{}",
            diagnostics[0].message
        );
        assert_eq!((state.archive_attempts, state.publish_calls), (0, 0));
        assert!(!output.exists());

        let entries_before = std::fs::read_dir(&root).unwrap().count();
        let (output, error, state) = run(
            TestBuildPoint::ArchiveCreationCleanupUncertainty,
            "archive-creation-cleanup",
        );
        let diagnostics = error.err().unwrap().into_diagnostics();
        assert_eq!(diagnostics[0].code, "SPX-B112");
        assert_eq!(diagnostics[1].code, "SPX-I233");
        assert_eq!((state.archive_attempts, state.publish_calls), (0, 0));
        assert!(!output.exists());
        assert!(std::fs::read_dir(&root).unwrap().count() >= entries_before + 2);

        let (output, error, state) = run(TestBuildPoint::ArchiveOutputMutation, "archive-mutation");
        let diagnostics = error.err().unwrap().into_diagnostics();
        assert_eq!(diagnostics[0].code, "SPX-I233");
        assert_eq!((state.archive_attempts, state.publish_calls), (1, 0));
        assert!(!output.exists());
        assert!(std::fs::read_dir(&root).unwrap().any(|entry| {
            entry.ok().is_some_and(|entry| {
                std::fs::read(entry.path().join(if cfg!(windows) {
                    "semaprax_native_rust_sdk.lib"
                } else {
                    "libsemaprax_native_rust_sdk.a"
                }))
                .ok()
                .as_deref()
                    == Some(b"foreign")
            })
        }));
        let (output, error, state) = run(TestBuildPoint::AfterFirstOuterWrite, "partial-outer");
        let diagnostics = error.err().unwrap().into_diagnostics();
        assert_eq!(diagnostics[0].code, "SPX-B112");
        assert_eq!((state.archive_attempts, state.publish_calls), (1, 0));
        assert!(!output.exists());

        let (output, error, state) = run(TestBuildPoint::BeforePublish, "before-publish");
        let diagnostics = error.err().unwrap().into_diagnostics();
        assert_eq!(diagnostics[0].code, "SPX-I233");
        assert_eq!((state.archive_attempts, state.publish_calls), (1, 1));
        assert!(!output.exists());

        let (output, error, state) = run(
            TestBuildPoint::ScratchCleanupUncertainty,
            "cleanup-uncertainty",
        );
        let diagnostics = error.err().unwrap().into_diagnostics();
        assert_eq!(diagnostics[0].code, "SPX-I233");
        assert_eq!((state.archive_attempts, state.publish_calls), (1, 0));
        assert!(!output.exists());
        assert!(std::fs::read_dir(&root).unwrap().any(|entry| {
            entry
                .ok()
                .is_some_and(|entry| entry.path().join("foreign").is_file())
        }));

        let (output, error, state) = run(
            TestBuildPoint::PostPivotAuthenticationFailure,
            "post-pivot-authentication",
        );
        let diagnostics = error.err().unwrap().into_diagnostics();
        assert_eq!(diagnostics[0].code, "SPX-I233");
        assert_eq!((state.archive_attempts, state.publish_calls), (1, 1));
        assert!(output.is_dir());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn effectful_no_import_sdk_builds_the_exact_public_inventory() {
        if std::env::var_os("RUSTC").is_none()
            || std::env::var_os("CLANG").is_none()
            || std::env::var_os("SEMAPRAX_ARCHIVER").is_none()
        {
            return;
        }
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-sdk-no-import-{}-{}",
                std::process::id(),
                STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir(&root).unwrap();
        let output = root.join("generated");
        let bundle = build_native_rust_sdk(
            MINIMAL_SOURCE,
            Path::new("no-import-fixture.spx"),
            minimal_options(),
            &output,
        )
        .unwrap();
        assert_eq!(bundle.output_directory(), output);
        let root_entries = std::fs::read_dir(&output).unwrap().count();
        let src_entries = std::fs::read_dir(output.join("src")).unwrap().count();
        let native_entries = std::fs::read_dir(output.join("native")).unwrap().count();
        assert_eq!((root_entries, src_entries, native_entries), (5, 3, 3));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn public_intent_and_facade_replay_the_unchanged_private_artifacts() {
        const SOURCE: &str = r#"module sdk.fixture;

permit { host.math }

@id("host.math")
interface HostMath permits { host.math } {
    @id("host.add")
    import rust fn host_add(left: i64, right: i64) -> i64
        effects { host.math }
        failure status "host.math.v1";
}

@id("sdk.add")
fn add(left: i64, right: i64) -> i64 uses { host.math } {
    host_add(left, right)
}

@id("sdk.main")
fn main() -> i64 { 0 }
"#;
        let program = semaprax::check(SOURCE, Path::new("sdk-fixture.spx")).unwrap();
        let options = NativeRustSdkOptions {
            exports: vec!["sdk.add".into()],
            imports: vec!["host.add".into()],
            capabilities: vec!["host.math".into()],
        };
        let canonical = semaprax::format::canonical(&program);
        let revision = domain_digest(SOURCE_DOMAIN, canonical.as_bytes());
        let target = target_triple().unwrap();
        let spec = canonical_spec(&program.module, &revision, target, &options).unwrap();
        let prepared =
            crate::implementation::prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
        assert_eq!(prepared.canonical_spec(), spec);
        let facts = parse_descriptor(
            prepared.descriptor().as_bytes(),
            &program.module,
            &revision,
            target,
            &options,
        )
        .unwrap();
        assert_eq!(facts.exports[0].public_method, "spx_sdk_dot_add");
        assert_eq!(facts.imports[0].public_method, "spx_host_dot_add");
        let sources = render_package_sources(&facts, &options.capabilities);
        assert!(!sources.lib_rs.starts_with("#![forbid(unsafe_code)]"));
        assert!(sources
            .lib_rs
            .contains("mod public_api{#![forbid(unsafe_code)]"));
        assert!(sources.lib_rs.contains("pub fn spx_sdk_dot_add"));
        assert!(sources.lib_rs.contains("fn spx_host_dot_add"));
        let inner_manifest = b"inner\n";
        let archive = b"archive";
        let manifest = render_sdk_manifest(
            &facts,
            &options,
            prepared.descriptor().as_bytes(),
            inner_manifest,
            &sources,
            prepared.generated_rust().as_bytes(),
            prepared.private_ffi_source().as_bytes(),
            archive,
        )
        .unwrap();
        verify_sdk_manifest(
            manifest.as_bytes(),
            &facts,
            &options,
            prepared.descriptor().as_bytes(),
            inner_manifest,
            &sources,
            prepared.generated_rust().as_bytes(),
            prepared.private_ffi_source().as_bytes(),
            archive,
        )
        .unwrap();
        let manifest = manifest.into_bytes();
        let rejects = |bytes: &[u8]| {
            verify_sdk_manifest(
                bytes,
                &facts,
                &options,
                prepared.descriptor().as_bytes(),
                inner_manifest,
                &sources,
                prepared.generated_rust().as_bytes(),
                prepared.private_ffi_source().as_bytes(),
                archive,
            )
            .is_err()
        };
        for index in 0..manifest.len() {
            let mut substituted = manifest.clone();
            substituted[index] ^= 1;
            assert!(rejects(&substituted), "substitution {index}");

            let mut deleted = manifest.clone();
            deleted.remove(index);
            assert!(rejects(&deleted), "deletion {index}");
        }
        for index in 0..=manifest.len() {
            let mut inserted = manifest.clone();
            inserted.insert(index, b'x');
            assert!(rejects(&inserted), "insertion {index}");
        }
        for length in 0..manifest.len() {
            assert!(rejects(&manifest[..length]), "truncation {length}");
        }
    }
}
