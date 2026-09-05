mod native_adapter_abi;
#[cfg(test)]
pub(crate) mod native_aggregate;
mod native_byte_data;
mod native_bytes;
mod native_callable_abi;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
mod native_callable_abi_v3;
mod native_callable_bundle;
mod native_callable_execution;
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "callable provider remains gated by SPX-B104")
)]
mod native_callable_provider;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
mod native_callable_provider_v3;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
mod native_callable_settlement_proof;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
mod native_callable_wire_v3;
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
mod native_capability_authority;
mod native_capability_token;
mod native_cleanup;
mod native_cleanup_emit;
mod native_command;
mod native_command_io;
#[cfg(test)]
mod native_conformance;
#[cfg(test)]
mod native_conformance_materialize;
#[cfg(test)]
mod native_conformance_wire;
mod native_host_contract;
mod native_host_output;
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
mod native_module_lease;
mod native_owned_data_provider;
mod native_resource;
mod native_runtime;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
mod native_settlement_derivation;
mod native_trace;
mod native_trace_runtime;
mod native_value;

use std::collections::HashMap;
use std::path::Path;

use sha2::{Digest as _, Sha256};

pub use native_callable_bundle::{
    build_native_callable_bundle, preflight_native_callable_bundle, NativeCallableBundle,
    NativeCallableBundlePreflight,
};

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, DeclarationId, ExpressionId, ResolvedProgram};

macro_rules! format {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

mod native_emit;

#[cfg(test)]
use native_emit::{
    c_function_symbol, c_record_symbol, c_string, emit_function_prototypes, emit_native_prelude,
    function_index, preflight_resource_lowering,
};
use native_emit::{emit_hir_c_with_labels, NativeOutputProfile};

pub use native_owned_data_provider::{
    emit_native_owned_data_provider, emit_project_v10_native_owned_utf8_provider,
    emit_project_v11_native_nested_owned_record_provider,
    emit_project_v8_native_owned_data_provider, emit_project_v9_native_flat_owned_record_provider,
    NativeOwnedDataProviderArtifact,
};
#[doc(hidden)]
pub fn native_owned_data_provider_symbol(rust_method: &str) -> String {
    native_owned_data_provider::public_provider_call_symbol(rust_method)
}

pub(super) trait COutput: std::fmt::Write {
    fn push_str(&mut self, value: &str);
    fn push(&mut self, value: char);
}

impl COutput for String {
    fn push_str(&mut self, value: &str) {
        String::push_str(self, value);
    }

    fn push(&mut self, value: char) {
        String::push(self, value);
    }
}

impl COutput for crate::bounded_output::CappedString {
    fn push_str(&mut self, value: &str) {
        self.push_str(value);
    }

    fn push(&mut self, value: char) {
        self.push(value);
    }
}

/// Resolve a parsed program fail-closed, then emit its checked native bootstrap IR.
pub fn emit_c(program: &Program) -> Result<String, Diagnostic> {
    if program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| import.native_rust)
    {
        return Err(backend_error(
            "native Rust imports are unavailable for the ordinary native target",
        ));
    }
    let resolved = hir::resolve(program).map_err(first_backend_diagnostic)?;
    emit_resolved_c_with_source(program, &resolved)
}

/// Resolve source and emit the bounded native stdout-transcript profile.
pub fn emit_c_with_stdout_transcript(program: &Program) -> Result<String, Diagnostic> {
    let resolved = hir::resolve(program).map_err(first_backend_diagnostic)?;
    crate::host_io_ops::validate_stdout_profile_authority(&resolved)?;
    let labels = contract_labels(program, &resolved);
    emit_hir_c_with_labels(
        &resolved,
        &labels,
        NativeOutputProfile::StdoutTranscript,
        None,
    )
}

/// Resolve source and emit the closed native Useful Data Command process.
///
/// The selected stable ID is authenticated by the shared target-neutral
/// command plan before C generation. The resulting translation unit contains
/// the memory-only transcript runner and one fixed platform process adapter;
/// it contains neither the legacy no-argument `main` nor its public failure
/// reporter.
pub fn emit_c_with_native_command(
    program: &Program,
    command_id: &str,
) -> Result<String, Diagnostic> {
    let resolved = hir::resolve(program).map_err(first_backend_diagnostic)?;
    let plan = crate::command_profile::CommandProfilePlan::prepare(&resolved, command_id)?;
    require_native_command_capacity(&plan)?;
    reject_native_rust_for_native(&resolved)?;
    let labels = contract_labels(program, &resolved);
    emit_hir_c_with_labels(
        &resolved,
        &labels,
        NativeOutputProfile::UsefulDataCommand,
        Some(plan.function_id()),
    )
}

/// Resolve source and emit Bounded Language Command I/O v1 for one selected
/// zero-argument boolean command. Process authority remains confined to the
/// generated adapter; semantic functions receive only the injected context.
pub fn emit_c_with_language_command_io(
    program: &Program,
    command_id: &str,
) -> Result<String, Diagnostic> {
    let resolved = hir::resolve(program).map_err(first_backend_diagnostic)?;
    emit_hir_c_with_language_command_io(&resolved, command_id)
}

/// Emit the exact production C11 projection from an already resolved source.
///
/// The parsed source remains necessary only for canonical contract labels. This
/// crate-private boundary lets semantic evidence reuse one checked HIR without
/// changing the bytes returned by [`emit_c`].
pub(crate) fn emit_resolved_c_with_source(
    source: &Program,
    resolved: &ResolvedProgram,
) -> Result<String, Diagnostic> {
    reject_native_rust_for_native(resolved)?;
    let labels = contract_labels(source, resolved);
    emit_hir_c_with_labels(resolved, &labels, NativeOutputProfile::Legacy, None)
}

/// Emit C11 from resolved HIR.
///
/// This entry point exists so backend tests and future compiler stages can prove
/// that code generation consumes semantic identities and centralized type facts,
/// rather than reconstructing either from source names.
pub fn emit_hir_c(program: &ResolvedProgram) -> Result<String, Diagnostic> {
    reject_native_rust_for_native(program)?;
    emit_hir_c_with_labels(program, &HashMap::new(), NativeOutputProfile::Legacy, None)
}

fn emit_hir_c_for_owned_data_provider(program: &ResolvedProgram) -> Result<String, Diagnostic> {
    reject_native_rust_for_native(program)?;
    emit_hir_c_with_labels(
        program,
        &HashMap::new(),
        NativeOutputProfile::OwnedDataProvider,
        None,
    )
}

fn emit_hir_c_for_owned_utf8_provider(program: &ResolvedProgram) -> Result<String, Diagnostic> {
    reject_native_rust_for_native(program)?;
    emit_hir_c_with_labels(
        program,
        &HashMap::new(),
        NativeOutputProfile::OwnedUtf8Provider,
        None,
    )
}

/// Emit the bounded native stdout-transcript profile from validated HIR.
///
/// The generated C exposes `spx_stdout_transcript_run_v1` and performs no
/// stdout write. Its caller-owned result remains canonically empty on failure.
pub fn emit_hir_c_with_stdout_transcript(program: &ResolvedProgram) -> Result<String, Diagnostic> {
    crate::host_io_ops::validate_stdout_profile_authority(program)?;
    reject_native_rust_for_native(program)?;
    emit_hir_c_with_labels(
        program,
        &HashMap::new(),
        NativeOutputProfile::StdoutTranscript,
        None,
    )
}

/// Emit the closed native Useful Data Command process from validated HIR.
pub fn emit_hir_c_with_native_command(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<String, Diagnostic> {
    let plan = crate::command_profile::CommandProfilePlan::prepare(program, command_id)?;
    require_native_command_capacity(&plan)?;
    reject_native_rust_for_native(program)?;
    emit_hir_c_with_labels(
        program,
        &HashMap::new(),
        NativeOutputProfile::UsefulDataCommand,
        Some(plan.function_id()),
    )
}

/// Emit Bounded Language Command I/O v1 from validated HIR.
pub fn emit_hir_c_with_language_command_io(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    reject_native_rust_for_native(program)?;
    let required_permits = [
        crate::command_io_ops::ARGS_READ_EFFECT,
        crate::command_io_ops::STDERR_WRITE_EFFECT,
        crate::command_io_ops::STDIN_READ_EFFECT,
        crate::host_io_ops::STDOUT_WRITE_EFFECT,
    ];
    if program.permits.as_slice() != required_permits {
        return Err(backend_error(
            "language command requires the exact canonical command-I/O permit inventory",
        ));
    }
    let command = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == command_id)
        .ok_or_else(|| {
            backend_error(format!(
                "selected language command `{command_id}` is absent"
            ))
        })?;
    if program
        .declarations
        .declaration(&command.id)
        .is_none_or(|declaration| {
            declaration.identity_origin != crate::hir::IdentityOrigin::Explicit
        })
        || !command.params.is_empty()
        || command.return_type != crate::hir::ResolvedType::Bool
    {
        return Err(backend_error(
            "selected language command must be an explicit stable-ID `fn () -> bool`",
        ));
    }
    crate::command_io_ops::validate_operation_profile(
        program,
        &command.id,
        crate::command_io_ops::CommandOperationProfile::LanguageV1,
    )?;
    emit_hir_c_with_labels(
        program,
        &HashMap::new(),
        NativeOutputProfile::LanguageCommandIo,
        Some(&command.id),
    )
}

/// Emit Line Command I/O v1 from validated HIR. This additive profile keeps
/// the v1-v6 native command projection unchanged while admitting authenticated
/// byte ranges and cumulative fallible output appends.
pub fn emit_hir_c_with_line_command_io(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    reject_native_rust_for_native(program)?;
    let required_permits = [
        crate::command_io_ops::ARGS_READ_EFFECT,
        crate::command_io_ops::STDERR_WRITE_EFFECT,
        crate::command_io_ops::STDIN_READ_EFFECT,
        crate::host_io_ops::STDOUT_WRITE_EFFECT,
    ];
    if program.permits.as_slice() != required_permits {
        return Err(backend_error(
            "line command requires the exact canonical command-I/O permit inventory",
        ));
    }
    let command = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == command_id)
        .ok_or_else(|| backend_error(format!("selected line command `{command_id}` is absent")))?;
    if program
        .declarations
        .declaration(&command.id)
        .is_none_or(|declaration| {
            declaration.identity_origin != crate::hir::IdentityOrigin::Explicit
        })
        || !command.params.is_empty()
        || command.return_type != crate::hir::ResolvedType::Bool
    {
        return Err(backend_error(
            "selected line command must be an explicit stable-ID `fn () -> bool`",
        ));
    }
    crate::command_io_ops::validate_operation_profile(
        program,
        &command.id,
        crate::command_io_ops::CommandOperationProfile::LineV1,
    )?;
    emit_hir_c_with_labels(
        program,
        &HashMap::new(),
        NativeOutputProfile::LineCommandIo,
        Some(&command.id),
    )
}

fn require_native_command_capacity(
    plan: &crate::command_profile::CommandProfilePlan,
) -> Result<(), Diagnostic> {
    if plan.stdout_capacity() != crate::host_io_ops::MAX_STDOUT_TRANSCRIPT_BYTES {
        return Err(backend_error(
            "native command transcript capacity disagrees with target-neutral admission",
        ));
    }
    Ok(())
}

fn reject_native_rust_for_native(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    if program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| import.native_rust)
    {
        return Err(backend_error(
            "native Rust imports are unavailable for the ordinary native target",
        ));
    }
    Ok(())
}

/// Doc-hidden public descriptor/provider artifact for one already validated
/// resource function.
///
/// Rust has no workspace-only visibility, so the unpublished native host uses
/// this deliberately narrow public facade instead of exposing the underlying
/// planner, descriptor, or ownership internals. This artifact contains no
/// callable SEMAPRAX entry point and does not alter the public
/// resource-lowering `SPX-B104` gate.
#[doc(hidden)]
pub struct NativeAdapterAdmissionArtifact {
    descriptor: Vec<u8>,
    getter_symbol: String,
    header: String,
    provider_source: String,
}

#[doc(hidden)]
impl NativeAdapterAdmissionArtifact {
    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }

    pub fn getter_symbol(&self) -> &str {
        &self.getter_symbol
    }

    pub fn header(&self) -> &str {
        &self.header
    }

    pub fn provider_source(&self) -> &str {
        &self.provider_source
    }
}

/// Derive one descriptor-only physical admission artifact from validated HIR.
///
/// No loader, capability authority, owner, callable symbol, or runtime payload
/// is created by this compiler-side operation.
#[doc(hidden)]
pub fn emit_native_adapter_admission(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    header_name: &str,
) -> Result<NativeAdapterAdmissionArtifact, Diagnostic> {
    hir::validate(program)?;
    let resource_abi = native_resource::build_resource_abi(program)?;
    let function = program
        .functions
        .iter()
        .find(|candidate| &candidate.id == function_id)
        .ok_or_else(|| backend_error(format!("function `{function_id}` is not in the program")))?;
    let cleanup = native_cleanup::classify(program, function)?;
    let values = native_value::plan(program, function, &cleanup, &resource_abi, &HashMap::new())?;
    let template = native_host_contract::derive_from_admitted(
        program,
        function_id,
        &resource_abi,
        &cleanup,
        &values,
    )?;
    let descriptor = native_adapter_abi::derive(&template)?;
    let header = native_adapter_abi::emit_header(&descriptor);
    let provider_source = native_adapter_abi::emit_source(&descriptor, header_name)?;
    Ok(NativeAdapterAdmissionArtifact {
        descriptor: descriptor.bytes,
        getter_symbol: descriptor.getter_symbol,
        header,
        provider_source,
    })
}

/// Compiler-private callable-v2 admission metadata derived only from validated
/// HIR and exact compiler-emitted runtime/cleanup/dictionary artifacts.
///
/// The artifact contains the complete private C11 provider translation unit,
/// including its generated callable and immutable descriptor getter. It still
/// cannot open the public native resource gate by itself: loading and adopting
/// physical resources remain quarantined behind the native host and
/// `SPX-B104`.
struct NativeCallableAdmissionCore {
    descriptor: Vec<u8>,
    getter_symbol: String,
    callable_symbol: String,
    call_contract: [u8; 32],
    max_request_bytes: u32,
    max_response_bytes: u32,
    provider_source: String,
    #[cfg(any(test, feature = "unstable-native-host-internal"))]
    semantic_event_dictionary: crate::semantic_trace::SemanticEventDictionary,
    trace_path_certificate: crate::trace_path_certificate::TracePathCertificate,
    event_dictionary: String,
    #[cfg(any(test, feature = "unstable-native-host-internal"))]
    codec_profile_fingerprint: [u8; 32],
    #[cfg(any(test, feature = "unstable-native-host-internal"))]
    normalized_execution_projection: String,
}

impl NativeCallableAdmissionCore {
    fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }

    fn getter_symbol(&self) -> &str {
        &self.getter_symbol
    }

    fn callable_symbol(&self) -> &str {
        &self.callable_symbol
    }

    fn call_contract(&self) -> [u8; 32] {
        self.call_contract
    }

    fn max_request_bytes(&self) -> u32 {
        self.max_request_bytes
    }

    fn max_response_bytes(&self) -> u32 {
        self.max_response_bytes
    }

    fn provider_source(&self) -> &str {
        &self.provider_source
    }

    #[cfg(any(test, feature = "unstable-native-host-internal"))]
    fn semantic_event_dictionary(&self) -> &crate::semantic_trace::SemanticEventDictionary {
        &self.semantic_event_dictionary
    }

    fn trace_path_certificate(&self) -> &crate::trace_path_certificate::TracePathCertificate {
        &self.trace_path_certificate
    }

    fn event_dictionary(&self) -> &str {
        &self.event_dictionary
    }

    #[cfg(any(test, feature = "unstable-native-host-internal"))]
    fn codec_profile_fingerprint(&self) -> [u8; 32] {
        self.codec_profile_fingerprint
    }

    #[cfg(any(test, feature = "unstable-native-host-internal"))]
    fn normalized_execution_projection(&self) -> &str {
        &self.normalized_execution_projection
    }
}

/// Derive strict callable-descriptor-v2 admission metadata for one validated
/// direct-resource function.
///
/// The execution/cleanup fingerprint is computed over the exact deterministic
/// resource ABI, status/trace/scalar runtimes, declarations, cleanup scaffold,
/// pinned call codec profile, and canonicalized wrapper/direct-hook template.
/// Dictionary facts and maximum trace capacity are computed internally;
/// callers cannot assert these security-critical facts.
fn emit_native_callable_admission_core(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
) -> Result<NativeCallableAdmissionCore, Diagnostic> {
    if program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| import.native_rust)
    {
        return Err(resource_lowering_gate());
    }
    hir::validate(program)?;
    if !program.function_templates.is_empty() || !program.function_instances.is_empty() {
        return Err(backend_error(
            "native callable admission does not accept generic function templates or instances",
        ));
    }
    let resource_abi = native_resource::build_resource_abi(program)?;
    let function = program
        .functions
        .iter()
        .find(|candidate| &candidate.id == function_id)
        .ok_or_else(|| backend_error(format!("function `{function_id}` is not in the program")))?;
    let cleanup = native_cleanup::classify(program, function)?;
    let mut values =
        native_value::plan(program, function, &cleanup, &resource_abi, &HashMap::new())?;
    let dictionary = crate::semantic_trace::build_semantic_event_dictionary(program, function_id)?;
    let trace_path_certificate = crate::trace_path_certificate::build_trace_path_certificate(
        program,
        function,
        &dictionary,
    )?;
    values.cleanup_bindings.semantic_events = Some(dictionary.clone());
    let declarations = native_value::emit_declarations(&values);
    let cleanup_body = native_cleanup_emit::emit_with_block_prologues(
        &cleanup,
        &values.cleanup_bindings,
        |block, output| {
            output.push_str(&native_value::emit_block_prologue(&values, block));
            Ok(())
        },
    )?;
    let mut status_runtime = String::new();
    native_runtime::emit_status_runtime(&mut status_runtime);
    let mut trace_runtime = String::new();
    native_trace_runtime::emit_trace_runtime(&mut trace_runtime);
    let execution = native_callable_execution::plan(
        program,
        function,
        &cleanup,
        &values,
        &resource_abi,
        &dictionary,
        declarations.clone(),
        cleanup_body.clone(),
    )?;
    let (normalized_execution_projection, codec_profile_fingerprint) =
        execution.normalized_projection()?;
    let execution_cleanup_fingerprint = native_callable_execution_cleanup_fingerprint(&[
        resource_abi.declarations.as_bytes(),
        status_runtime.as_bytes(),
        trace_runtime.as_bytes(),
        NATIVE_SCALAR_RUNTIME_C.as_bytes(),
        declarations.as_bytes(),
        cleanup_body.as_bytes(),
        &codec_profile_fingerprint,
        normalized_execution_projection.as_bytes(),
    ]);
    let event_dictionary = dictionary.canonical_json();
    let semantics = native_callable_abi::NativeCallableSemantics::new(
        execution_cleanup_fingerprint,
        dictionary.fingerprint(),
        trace_path_certificate.fingerprint(),
        event_dictionary.len(),
        dictionary.entries().len(),
        usize::try_from(values.required_event_capacity)
            .map_err(|_| backend_error("native event capacity does not fit usize"))?,
    )?;
    let template = native_host_contract::derive_from_admitted(
        program,
        function_id,
        &resource_abi,
        &cleanup,
        &values,
    )?;
    let descriptor = native_callable_abi::derive(&template, &semantics)?;
    let concrete = execution.emit_concrete(&descriptor, &normalized_execution_projection)?;
    if concrete.codec_profile_fingerprint != codec_profile_fingerprint
        || concrete.normalized_projection != normalized_execution_projection
    {
        return Err(backend_error(
            "concrete callable provider changed its authenticated codec or execution projection",
        ));
    }
    let mut provider_source = String::new();
    provider_source.push_str(&status_runtime);
    provider_source.push_str(&resource_abi.declarations);
    provider_source.push_str("#include <stdio.h>\n");
    provider_source.push_str(NATIVE_SCALAR_RUNTIME_C);
    provider_source.push_str(&trace_runtime);
    provider_source.push_str(&concrete.source);
    provider_source.push_str(&native_callable_abi::emit_getter_source(&descriptor));
    Ok(NativeCallableAdmissionCore {
        descriptor: descriptor.bytes,
        getter_symbol: descriptor.getter_symbol,
        callable_symbol: descriptor.callable_symbol,
        call_contract: descriptor.call_contract,
        max_request_bytes: descriptor.max_request_bytes,
        max_response_bytes: descriptor.max_response_bytes,
        provider_source,
        #[cfg(any(test, feature = "unstable-native-host-internal"))]
        semantic_event_dictionary: dictionary,
        trace_path_certificate,
        event_dictionary,
        #[cfg(any(test, feature = "unstable-native-host-internal"))]
        codec_profile_fingerprint,
        #[cfg(any(test, feature = "unstable-native-host-internal"))]
        normalized_execution_projection,
    })
}

/// Feature-gated callable-v2 admission artifact for the unpublished native
/// host. The default public compiler surface exposes only the build-only
/// bundle API, not these host-facing semantic internals.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub struct NativeCallableAdmissionArtifact(NativeCallableAdmissionCore);

#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
impl NativeCallableAdmissionArtifact {
    pub fn descriptor(&self) -> &[u8] {
        self.0.descriptor()
    }

    pub fn getter_symbol(&self) -> &str {
        self.0.getter_symbol()
    }

    pub fn callable_symbol(&self) -> &str {
        self.0.callable_symbol()
    }

    pub fn call_contract(&self) -> [u8; 32] {
        self.0.call_contract()
    }

    pub fn max_request_bytes(&self) -> u32 {
        self.0.max_request_bytes()
    }

    pub fn max_response_bytes(&self) -> u32 {
        self.0.max_response_bytes()
    }

    pub fn provider_source(&self) -> &str {
        self.0.provider_source()
    }

    pub fn semantic_event_dictionary(&self) -> &crate::semantic_trace::SemanticEventDictionary {
        self.0.semantic_event_dictionary()
    }

    pub fn trace_path_certificate(&self) -> &crate::trace_path_certificate::TracePathCertificate {
        self.0.trace_path_certificate()
    }

    pub fn event_dictionary(&self) -> &str {
        self.0.event_dictionary()
    }

    pub fn codec_profile_fingerprint(&self) -> [u8; 32] {
        self.0.codec_profile_fingerprint()
    }

    pub fn normalized_execution_projection(&self) -> &str {
        self.0.normalized_execution_projection()
    }
}

/// Feature-gated compiler facade used by the unpublished native host tests.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub fn emit_native_callable_admission(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
) -> Result<NativeCallableAdmissionArtifact, Diagnostic> {
    emit_native_callable_admission_core(program, function_id).map(NativeCallableAdmissionArtifact)
}

/// Feature-gated, authority-free callable-v2 settlement proof used only by the
/// unpublished native host's independent decoder tests. This remains distinct
/// from the metadata-only callable-v3 descriptor and provides no provider or
/// execution path.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub struct NativeCallableSettlementProofArtifact(
    native_callable_settlement_proof::NativeCallableSettlementProof,
);

#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
impl NativeCallableSettlementProofArtifact {
    pub fn bytes(&self) -> &[u8] {
        self.0.bytes()
    }
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub fn emit_native_callable_settlement_proof(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
) -> Result<NativeCallableSettlementProofArtifact, Diagnostic> {
    native_callable_settlement_proof::derive(program, function_id)
        .map(NativeCallableSettlementProofArtifact)
}

/// Feature-gated, metadata-only callable-v3 descriptor artifact.
///
/// This exposes no provider, loader, native function pointer, finalizer, or
/// host authority. The unpublished host uses the bytes and exact symbol names
/// solely to exercise its independent parser and version-confusion gates.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub struct NativeCallableV3DescriptorArtifact(native_callable_abi_v3::NativeCallableV3Descriptor);

#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
impl NativeCallableV3DescriptorArtifact {
    pub fn bytes(&self) -> &[u8] {
        &self.0.bytes
    }

    pub fn getter_symbol(&self) -> &str {
        &self.0.getter_symbol
    }

    pub fn execute_symbol(&self) -> &str {
        &self.0.execute_symbol
    }

    pub fn settle_symbol(&self) -> &str {
        &self.0.settle_symbol
    }

    pub fn call_contract(&self) -> [u8; 32] {
        self.0.call_contract
    }
}

/// Derive metadata-only callable-v3 bytes from validated compiler facts.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub fn emit_native_callable_v3_descriptor(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
) -> Result<NativeCallableV3DescriptorArtifact, Diagnostic> {
    native_callable_abi_v3::derive(program, function_id).map(NativeCallableV3DescriptorArtifact)
}

/// Closed cross-target selector used only by the unpublished iOS-static host
/// admission evidence. It emits metadata and grants no registration or call
/// authority.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateNativeCallableV3IosTarget {
    DeviceArm64,
    SimulatorArm64,
    SimulatorX86_64,
    MacCatalystArm64,
    MacCatalystX86_64,
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
impl PrivateNativeCallableV3IosTarget {
    fn canonical_tag(self) -> &'static str {
        match self {
            Self::DeviceArm64 => "aarch64-ios-device-apple-macho-ptr64-little-callable-v3",
            Self::SimulatorArm64 => "aarch64-ios-simulator-apple-macho-ptr64-little-callable-v3",
            Self::SimulatorX86_64 => "x86_64-ios-simulator-apple-macho-ptr64-little-callable-v3",
            Self::MacCatalystArm64 => "aarch64-ios-catalyst-apple-macho-ptr64-little-callable-v3",
            Self::MacCatalystX86_64 => "x86_64-ios-catalyst-apple-macho-ptr64-little-callable-v3",
        }
    }

    fn provider_target(self) -> native_callable_provider::IosProviderPhysicalTarget {
        match self {
            Self::DeviceArm64 => native_callable_provider::IosProviderPhysicalTarget::DeviceArm64,
            Self::SimulatorArm64 => {
                native_callable_provider::IosProviderPhysicalTarget::SimulatorArm64
            }
            Self::SimulatorX86_64 => {
                native_callable_provider::IosProviderPhysicalTarget::SimulatorX86_64
            }
            Self::MacCatalystArm64 => {
                native_callable_provider::IosProviderPhysicalTarget::MacCatalystArm64
            }
            Self::MacCatalystX86_64 => {
                native_callable_provider::IosProviderPhysicalTarget::MacCatalystX86_64
            }
        }
    }
}

/// Closed cross-target selector used only by unpublished Android dynamic-host
/// evidence. The target is authenticated in descriptor bytes and paired with
/// exact C preprocessor guards; it grants no loader or call authority.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateNativeCallableV3AndroidTarget {
    Arm64,
    X86_64,
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
impl PrivateNativeCallableV3AndroidTarget {
    fn canonical_tag(self) -> &'static str {
        match self {
            Self::Arm64 => "aarch64-android-android-elf-ptr64-little-callable-v3",
            Self::X86_64 => "x86_64-android-android-elf-ptr64-little-callable-v3",
        }
    }

    fn provider_target(self) -> native_callable_provider::AndroidProviderPhysicalTarget {
        match self {
            Self::Arm64 => native_callable_provider::AndroidProviderPhysicalTarget::Arm64,
            Self::X86_64 => native_callable_provider::AndroidProviderPhysicalTarget::EmulatorX86_64,
        }
    }
}

/// Derive one exact iOS-static descriptor for private registration evidence.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub fn emit_private_native_callable_v3_ios_descriptor(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    target: PrivateNativeCallableV3IosTarget,
) -> Result<NativeCallableV3DescriptorArtifact, Diagnostic> {
    native_callable_abi_v3::derive_ios_static_for_target(
        program,
        function_id,
        target.canonical_tag(),
    )
    .map(NativeCallableV3DescriptorArtifact)
}

/// Derive one exact Android dynamic-image descriptor for private admission
/// evidence without opening or invoking an image.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub fn emit_private_native_callable_v3_android_descriptor(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    target: PrivateNativeCallableV3AndroidTarget,
) -> Result<NativeCallableV3DescriptorArtifact, Diagnostic> {
    native_callable_abi_v3::derive_dynamic_for_target(program, function_id, target.canonical_tag())
        .map(NativeCallableV3DescriptorArtifact)
}

/// Closed fixture selector for the first compiler/provider/loader/host v3
/// composition proof. This remains unavailable without the unpublished host
/// feature and cannot select arbitrary caller-supplied cleanup metadata.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateNativeCallableV3Fixture {
    ScalarDiscardTwo,
    OwnedIdentity,
}

/// Closed fault selector for private physical recovery evidence. These values
/// are compiler-sealed into generated provider fixtures and remain absent from
/// the default/public compiler surface.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateNativeCallableV3Fault {
    PhysicalFailure { checkpoint: u32, code: u32 },
    MalformedResponse { offset: u32 },
    MalformedFrame { offset: u32 },
    MalformedCandidate { offset: u32 },
    FinalizerInterruption { action: u32, boundary: u32 },
}

/// Exact descriptor bytes and the strict-C provider sealed around those same
/// bytes. No loader lease, receipt authority, or public execution permission
/// is carried by this compiler artifact.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub struct PrivateNativeCallableV3Artifact {
    descriptor: Vec<u8>,
    source: String,
    getter_symbol: String,
    execute_symbol: String,
    settle_symbol: String,
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
impl PrivateNativeCallableV3Artifact {
    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn getter_symbol(&self) -> &str {
        &self.getter_symbol
    }

    pub fn execute_symbol(&self) -> &str {
        &self.execute_symbol
    }

    pub fn settle_symbol(&self) -> &str {
        &self.settle_symbol
    }
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
fn private_native_callable_v3_artifact(
    descriptor: native_callable_abi_v3::NativeCallableV3Descriptor,
    provider: native_callable_provider_v3::NativeCallableProviderV3,
) -> PrivateNativeCallableV3Artifact {
    PrivateNativeCallableV3Artifact {
        getter_symbol: descriptor.getter_symbol,
        execute_symbol: descriptor.execute_symbol,
        settle_symbol: descriptor.settle_symbol,
        descriptor: descriptor.bytes,
        source: provider.source,
    }
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub fn emit_private_native_callable_v3_fixture(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    fixture: PrivateNativeCallableV3Fixture,
) -> Result<PrivateNativeCallableV3Artifact, Diagnostic> {
    let descriptor = native_callable_abi_v3::derive(program, function_id)?;
    let plan = private_native_callable_v3_plan(fixture);
    let spec =
        native_callable_provider_v3::NativeCallableProviderV3Spec::new(descriptor.clone(), plan)?;
    let provider = native_callable_provider_v3::emit(&spec)?;
    Ok(private_native_callable_v3_artifact(descriptor, provider))
}

/// Emit one exact statically linked iOS-family callable-v3 fixture. Descriptor
/// target/linkage bytes and C physical-target guards are selected by the same
/// closed enum and cross-checked before source emission.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub fn emit_private_native_callable_v3_ios_fixture(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    fixture: PrivateNativeCallableV3Fixture,
    target: PrivateNativeCallableV3IosTarget,
) -> Result<PrivateNativeCallableV3Artifact, Diagnostic> {
    let descriptor = native_callable_abi_v3::derive_ios_static_for_target(
        program,
        function_id,
        target.canonical_tag(),
    )?;
    let plan = private_native_callable_v3_plan(fixture);
    let spec = native_callable_provider_v3::NativeCallableProviderV3Spec::new_ios_static(
        descriptor.clone(),
        plan,
        target.provider_target(),
    )?;
    let provider = native_callable_provider_v3::emit(&spec)?;
    Ok(private_native_callable_v3_artifact(descriptor, provider))
}

/// Emit one exact dynamically loaded Android callable-v3 fixture. Descriptor
/// target/linkage bytes and C physical-target guards are selected by the same
/// closed enum and cross-checked before source emission.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub fn emit_private_native_callable_v3_android_fixture(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    fixture: PrivateNativeCallableV3Fixture,
    target: PrivateNativeCallableV3AndroidTarget,
) -> Result<PrivateNativeCallableV3Artifact, Diagnostic> {
    let descriptor = native_callable_abi_v3::derive_dynamic_for_target(
        program,
        function_id,
        target.canonical_tag(),
    )?;
    let plan = private_native_callable_v3_plan(fixture);
    let spec = native_callable_provider_v3::NativeCallableProviderV3Spec::new_android_dynamic(
        descriptor.clone(),
        plan,
        target.provider_target(),
    )?;
    let provider = native_callable_provider_v3::emit(&spec)?;
    Ok(private_native_callable_v3_artifact(descriptor, provider))
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub fn emit_private_native_callable_v3_fault_fixture(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    fixture: PrivateNativeCallableV3Fixture,
    fault: PrivateNativeCallableV3Fault,
) -> Result<PrivateNativeCallableV3Artifact, Diagnostic> {
    let descriptor = native_callable_abi_v3::derive(program, function_id)?;
    let plan = private_native_callable_v3_plan(fixture);
    let fault = match fault {
        PrivateNativeCallableV3Fault::PhysicalFailure { checkpoint, code } => {
            native_callable_provider_v3::ProviderV3TestFault::PhysicalFailure { checkpoint, code }
        }
        PrivateNativeCallableV3Fault::MalformedResponse { offset } => {
            native_callable_provider_v3::ProviderV3TestFault::MalformedResponse { offset }
        }
        PrivateNativeCallableV3Fault::MalformedFrame { offset } => {
            native_callable_provider_v3::ProviderV3TestFault::MalformedFrame { offset }
        }
        PrivateNativeCallableV3Fault::MalformedCandidate { offset } => {
            native_callable_provider_v3::ProviderV3TestFault::MalformedCandidate { offset }
        }
        PrivateNativeCallableV3Fault::FinalizerInterruption { action, boundary } => {
            native_callable_provider_v3::ProviderV3TestFault::FinalizerInterruption {
                action,
                boundary,
            }
        }
    };
    let spec =
        native_callable_provider_v3::NativeCallableProviderV3Spec::new(descriptor.clone(), plan)?
            .with_test_fault(fault)?;
    let provider = native_callable_provider_v3::emit(&spec)?;
    Ok(private_native_callable_v3_artifact(descriptor, provider))
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
fn private_native_callable_v3_plan(
    fixture: PrivateNativeCallableV3Fixture,
) -> native_callable_provider_v3::ProviderV3Plan {
    match fixture {
        PrivateNativeCallableV3Fixture::ScalarDiscardTwo => {
            native_callable_provider_v3::ProviderV3Plan::ScalarDiscard {
                scalar_result: 0,
                finalizer_order: vec![1, 0],
                completed_checkpoints: vec![2, 3],
                semantic_ordinals: vec![1, 2, 3, 4, 5],
            }
        }
        PrivateNativeCallableV3Fixture::OwnedIdentity => {
            native_callable_provider_v3::ProviderV3Plan::OwnedIdentity {
                owner_ordinal: 0,
                staged_checkpoint: 2,
                semantic_ordinals: vec![1, 2, 3],
            }
        }
    }
}

/// Hidden composition fixture derived from one canonical owned-resource
/// corpus execution. The stable function ID and scenario inputs are explicit;
/// no test label selects behavior, and the authenticated descriptor graph is
/// still the sole cleanup authority.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub fn emit_private_native_callable_v3_corpus_fixture(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    arguments: &[crate::owned_resource_corpus::OwnedResourceCorpusArgument],
    expected_owned_result_ordinal: Option<usize>,
    reference: &crate::conformance::ConformanceTrace,
) -> Result<PrivateNativeCallableV3Artifact, Diagnostic> {
    let descriptor = native_callable_abi_v3::derive(program, function_id)?;
    let plan = native_callable_provider_v3::corpus_witness_plan(
        program,
        function_id,
        arguments,
        expected_owned_result_ordinal,
        reference,
    )?;
    let spec =
        native_callable_provider_v3::NativeCallableProviderV3Spec::new(descriptor.clone(), plan)?;
    let provider = native_callable_provider_v3::emit(&spec)?;
    Ok(private_native_callable_v3_artifact(descriptor, provider))
}

/// Hidden Android-dynamic composition fixture derived from one canonical
/// owned-resource corpus case, including semantic-failure witnesses such as
/// `requires-false`. The exact target is authenticated in the descriptor and
/// paired with exact C preprocessor guards; no loader lease, receipt
/// authority, or public execution permission is carried by this artifact.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub fn emit_private_native_callable_v3_android_corpus_fixture(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    arguments: &[crate::owned_resource_corpus::OwnedResourceCorpusArgument],
    expected_owned_result_ordinal: Option<usize>,
    reference: &crate::conformance::ConformanceTrace,
    target: PrivateNativeCallableV3AndroidTarget,
) -> Result<PrivateNativeCallableV3Artifact, Diagnostic> {
    let descriptor = native_callable_abi_v3::derive_dynamic_for_target(
        program,
        function_id,
        target.canonical_tag(),
    )?;
    let plan = native_callable_provider_v3::corpus_witness_plan(
        program,
        function_id,
        arguments,
        expected_owned_result_ordinal,
        reference,
    )?;
    let spec = native_callable_provider_v3::NativeCallableProviderV3Spec::new_android_dynamic(
        descriptor.clone(),
        plan,
        target.provider_target(),
    )?;
    let provider = native_callable_provider_v3::emit(&spec)?;
    Ok(private_native_callable_v3_artifact(descriptor, provider))
}

/// Hidden iOS-static composition fixture derived from one canonical
/// owned-resource corpus case, including semantic-failure witnesses such as
/// `requires-false`. The exact target is authenticated in the descriptor;
/// this artifact carries no loader lease, receipt authority, or public
/// execution permission.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[doc(hidden)]
pub fn emit_private_native_callable_v3_ios_corpus_fixture(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    arguments: &[crate::owned_resource_corpus::OwnedResourceCorpusArgument],
    expected_owned_result_ordinal: Option<usize>,
    reference: &crate::conformance::ConformanceTrace,
    target: PrivateNativeCallableV3IosTarget,
) -> Result<PrivateNativeCallableV3Artifact, Diagnostic> {
    let descriptor = native_callable_abi_v3::derive_ios_static_for_target(
        program,
        function_id,
        target.canonical_tag(),
    )?;
    let plan = native_callable_provider_v3::corpus_witness_plan(
        program,
        function_id,
        arguments,
        expected_owned_result_ordinal,
        reference,
    )?;
    let spec = native_callable_provider_v3::NativeCallableProviderV3Spec::new_ios_static(
        descriptor.clone(),
        plan,
        target.provider_target(),
    )?;
    let provider = native_callable_provider_v3::emit(&spec)?;
    Ok(private_native_callable_v3_artifact(descriptor, provider))
}

fn native_callable_execution_cleanup_fingerprint(components: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.native-callable-execution-cleanup.v2\0");
    hasher.update(b"resource-abi;status-runtime;trace-runtime;scalar-runtime;value-declarations;value-cleanup-scaffold;provider-codec-profile;canonical-wrapper-direct-hook-projection");
    hasher.update((components.len() as u64).to_be_bytes());
    for fragment in components {
        hasher.update((fragment.len() as u64).to_be_bytes());
        hasher.update(fragment);
    }
    hasher.finalize().into()
}

fn first_backend_diagnostic(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity.is_error())
        .cloned()
        .or_else(|| diagnostics.into_iter().next())
        .unwrap_or_else(|| backend_error("HIR resolution failed without a diagnostic"))
}

fn contract_labels(program: &Program, resolved: &ResolvedProgram) -> HashMap<ExpressionId, String> {
    let mut labels = HashMap::new();
    for function in &resolved.functions {
        let Some(source) = program
            .functions
            .iter()
            .find(|candidate| candidate.stable_id == function.id.as_str())
        else {
            continue;
        };
        for (expression, source) in function.requires.iter().zip(&source.requires) {
            labels.insert(expression.id.clone(), crate::format::expr(source, 0));
        }
        for (expression, source) in function.ensures.iter().zip(&source.ensures) {
            labels.insert(expression.id.clone(), crate::format::expr(source, 0));
        }
    }
    for instance in &resolved.function_instances {
        let Some(source) = program
            .functions
            .iter()
            .find(|candidate| candidate.stable_id == instance.template.as_str())
        else {
            continue;
        };
        for (expression, source) in instance.function.requires.iter().zip(&source.requires) {
            labels.insert(expression.id.clone(), crate::format::expr(source, 0));
        }
        for (expression, source) in instance.function.ensures.iter().zip(&source.ensures) {
            labels.insert(expression.id.clone(), crate::format::expr(source, 0));
        }
    }
    labels
}

fn resource_lowering_gate() -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        "native resource lowering requires lifecycle declarations and the verified cleanup ABI",
    )
}

const NATIVE_SCALAR_RUNTIME_C: &str = r#"#include <stdlib.h>

static __attribute__((noreturn, unused)) void spx_runtime_invariant_failure(
    const char *message
) {
    fprintf(stderr, "SEMAPRAX native runtime invariant failure: %s\n", message);
    abort();
}

static __attribute__((unused)) spx_status_token spx_rt_arithmetic_failure(
    struct spx_context *spx_ctx,
    uint32_t code,
    const char *operation
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_arithmetic(spx_ctx, code, &token)) {
        spx_runtime_invariant_failure("status arena exhaustion");
    }
    struct spx_status_detail detail = {
        .failure_kind = NULL,
        .failure_function = NULL,
        .failure_expression = NULL,
        .failure_operation = operation,
        .failure_arguments = {0}
    };
    if (!spx_status_attach_detail(spx_ctx, token, detail)) {
        spx_runtime_invariant_failure("arithmetic status detail attachment");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_rt_contract_with_arguments(
    struct spx_context *spx_ctx,
    uint32_t code,
    const char *kind,
    const char *function,
    const char *expression,
    const char *arguments
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    bool recorded = code == SPX_STATUS_CONTRACT_REQUIRES_FALSE
        ? spx_status_record_requires_false(spx_ctx, &token)
        : code == SPX_STATUS_CONTRACT_ENSURES_FALSE
            ? spx_status_record_ensures_false(spx_ctx, &token)
            : false;
    if (!recorded) spx_runtime_invariant_failure("status arena exhaustion");
    struct spx_status_detail detail = {
        .failure_kind = kind,
        .failure_function = function,
        .failure_expression = expression,
        .failure_operation = NULL,
        .failure_arguments = {0}
    };
    int arguments_written = snprintf(
        detail.failure_arguments,
        sizeof detail.failure_arguments,
        "%s",
        arguments
    );
    if (arguments_written < 0 ||
        (size_t)arguments_written >= sizeof detail.failure_arguments) {
        spx_runtime_invariant_failure("contract argument detail overflow");
    }
    if (!spx_status_attach_detail(spx_ctx, token, detail)) {
        spx_runtime_invariant_failure("contract status detail attachment");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_rt_contract(
    struct spx_context *spx_ctx,
    uint32_t code,
    const char *kind,
    const char *function,
    const char *expression
) {
    return spx_rt_contract_with_arguments(
        spx_ctx, code, kind, function, expression, "none"
    );
}

static __attribute__((unused)) spx_status_token spx_rt_call_depth_failure(
    struct spx_context *spx_ctx
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(
        spx_ctx,
        "semaprax.runtime.v1",
        UINT32_C(1),
        SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE,
        &token
    )) {
        spx_runtime_invariant_failure("call-depth status arena exhaustion");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_rt_add(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    int64_t result;
    if (__builtin_add_overflow(a, b, &result)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, "addition overflow"
        );
    }
    *result_out = result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_sub(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    int64_t result;
    if (__builtin_sub_overflow(a, b, &result)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, "subtraction overflow"
        );
    }
    *result_out = result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_mul(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    int64_t result;
    if (__builtin_mul_overflow(a, b, &result)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, "multiplication overflow"
        );
    }
    *result_out = result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_div(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, "invalid division"
        );
    }
    if (a == INT64_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_OVERFLOW, "invalid division"
        );
    }
    *result_out = a / b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_rem(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO, "invalid remainder"
        );
    }
    if (a == INT64_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_OVERFLOW, "invalid remainder"
        );
    }
    *result_out = a % b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_add_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    int64_t wide = (int64_t)a + (int64_t)b;
    if (wide > INT32_MAX || wide < INT32_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, "addition overflow"
        );
    }
    *result_out = (int32_t)wide;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_sub_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    int64_t wide = (int64_t)a - (int64_t)b;
    if (wide > INT32_MAX || wide < INT32_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, "subtraction overflow"
        );
    }
    *result_out = (int32_t)wide;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_mul_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    int64_t wide = (int64_t)a * (int64_t)b;
    if (wide > INT32_MAX || wide < INT32_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, "multiplication overflow"
        );
    }
    *result_out = (int32_t)wide;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_div_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, "invalid division"
        );
    }
    if (a == INT32_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_OVERFLOW, "invalid division"
        );
    }
    *result_out = a / b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_rem_i32(
    struct spx_context *spx_ctx, int32_t a, int32_t b, int32_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO, "invalid remainder"
        );
    }
    if (a == INT32_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_OVERFLOW, "invalid remainder"
        );
    }
    *result_out = a % b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_neg_i32(
    struct spx_context *spx_ctx, int32_t value, int32_t *result_out
) {
    if (value == INT32_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW, "negation overflow"
        );
    }
    *result_out = -value;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_neg(
    struct spx_context *spx_ctx, int64_t value, int64_t *result_out
) {
    if (value == INT64_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW, "negation overflow"
        );
    }
    *result_out = -value;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) int spx_public_failure(
    const struct spx_context *spx_ctx,
    spx_status_token token
) {
    const struct spx_normalized_status *status = spx_status_resolve(spx_ctx, token);
    if (status == NULL) {
        fputs("SEMAPRAX native runtime invariant failure: invalid status token\n", stderr);
        return 72;
    }
    const struct spx_status_detail *detail = spx_status_resolve_detail(spx_ctx, token);
    if (status->status_class == SPX_STATUS_CLASS_CONTRACT) {
        if (detail == NULL || detail->failure_kind == NULL ||
            detail->failure_function == NULL || detail->failure_expression == NULL) {
            fputs("SEMAPRAX native runtime invariant failure: missing contract detail\n", stderr);
            return 72;
        }
        fprintf(
            stderr,
            "SEMAPRAX contract failure\n  contract: %s %s in %s\n  arguments: %s\n",
            detail->failure_kind,
            detail->failure_expression,
            detail->failure_function,
            detail->failure_arguments
        );
        return 70;
    }
    if (status->status_class == SPX_STATUS_CLASS_ARITHMETIC) {
        if (detail == NULL || detail->failure_operation == NULL) {
            fputs("SEMAPRAX native runtime invariant failure: missing arithmetic detail\n", stderr);
            return 72;
        }
        fprintf(
            stderr,
            "SEMAPRAX checked arithmetic failure: %s\n",
            detail->failure_operation
        );
        return 71;
    }
    if (strcmp(status->domain_id, "semaprax.runtime.v1") == 0 &&
        status->code == UINT32_C(1)) {
        fprintf(
            stderr,
            "SEMAPRAX runtime failure: call depth exceeded (%u frames)\n",
            (unsigned int)SPX_MAX_CALL_DEPTH
        );
        return 73;
    }
    fprintf(
        stderr,
        "SEMAPRAX operation failure: %s/%u\n",
        status->domain_id,
        status->code
    );
    return 73;
}

"#;

pub fn build(program: &Program, output: &Path) -> Result<(), Diagnostic> {
    native_emit::write_and_compile_c(&emit_c(program)?, output)
}

/// Compile one already-emitted C11 projection into a native executable.
///
/// Project v1 publishes its linked entry closure through this boundary so the
/// public native lane consumes exactly the authenticated linked HIR that Web
/// publication and the internal lowering-equivalence evidence consume. This
/// performs no parsing, HIR resolution, or source projection.
pub fn compile_native_executable(c_source: &str, output: &Path) -> Result<(), Diagnostic> {
    native_emit::write_and_compile_c_with_mode(c_source, output, false)
}

/// Compile one emitted native Useful Data Command translation unit.
///
/// This keeps the ordinary native compiler command byte-for-byte unchanged.
/// GNU-target Windows Clang alone requires `-municode` to select the `wmain`
/// CRT startup; MSVC-target Clang selects the wide console entry from the
/// translation unit without that MinGW-only option.
pub fn compile_native_command_executable(c_source: &str, output: &Path) -> Result<(), Diagnostic> {
    native_emit::write_and_compile_c_with_mode(c_source, output, true)
}

fn c_i64(value: i64) -> String {
    if value == i64::MIN {
        "(-INT64_C(9223372036854775807) - INT64_C(1))".to_owned()
    } else if value < 0 {
        format!("-INT64_C({})", value.unsigned_abs())
    } else {
        format!("INT64_C({value})")
    }
}

fn backend_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B103", message)
}

#[cfg(test)]
#[path = "codegen/tests.rs"]
mod tests;
