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
    struct spx_status_detail detail = {NULL, NULL, NULL, operation};
    if (!spx_status_attach_detail(spx_ctx, token, detail)) {
        spx_runtime_invariant_failure("arithmetic status detail attachment");
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
    spx_status_token token = SPX_STATUS_SUCCESS;
    bool recorded = code == SPX_STATUS_CONTRACT_REQUIRES_FALSE
        ? spx_status_record_requires_false(spx_ctx, &token)
        : code == SPX_STATUS_CONTRACT_ENSURES_FALSE
            ? spx_status_record_ensures_false(spx_ctx, &token)
            : false;
    if (!recorded) spx_runtime_invariant_failure("status arena exhaustion");
    struct spx_status_detail detail = {kind, function, expression, NULL};
    if (!spx_status_attach_detail(spx_ctx, token, detail)) {
        spx_runtime_invariant_failure("contract status detail attachment");
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
            "SEMAPRAX contract failure: %s in %s: %s\n",
            detail->failure_kind,
            detail->failure_function,
            detail->failure_expression
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
mod tests {
    use std::fmt::Write as _;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::hir::ResolvedType;
    use crate::{hir, parse};
    use sha2::{Digest, Sha256};

    use super::*;

    static NEXT_CALLABLE_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct CallableFixture(PathBuf);

    impl CallableFixture {
        fn create() -> Self {
            let ordinal = NEXT_CALLABLE_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "semaprax-integrated-callable-{}-{ordinal}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for CallableFixture {
        fn drop(&mut self) {
            let Ok(metadata) = fs::symlink_metadata(&self.0) else {
                return;
            };
            if metadata.file_type().is_symlink() || metadata.is_file() {
                let _ = fs::remove_file(&self.0);
            } else if metadata.is_dir() {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn generic_record_c_symbols_bind_the_full_concrete_instance() {
        let declaration = DeclarationId::new("test.phantom");
        let i64_instance = ResolvedType::Nominal {
            declaration: declaration.clone(),
            arguments: vec![ResolvedType::I64],
        };
        let bool_instance = ResolvedType::Nominal {
            declaration: declaration.clone(),
            arguments: vec![ResolvedType::Bool],
        };
        let i64_symbol = c_record_symbol(&i64_instance);
        let bool_symbol = c_record_symbol(&bool_instance);
        assert_ne!(i64_symbol, bool_symbol);
        assert!(i64_symbol.starts_with("spx_record_746573742e7068616e746f6d_"));
        assert!(bool_symbol.starts_with("spx_record_746573742e7068616e746f6d_"));
        assert_eq!(
            c_record_symbol(&ResolvedType::Nominal {
                declaration,
                arguments: Vec::new(),
            }),
            "spx_record_746573742e7068616e746f6d"
        );
    }

    const RESOURCE_SOURCE: &str = r#"module test.native_resource_types;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

    const CALLABLE_EXECUTION_SOURCE: &str = r#"module test.native_callable_execution;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.discard")
fn discard(value: own Token) -> i64 { 0 }

@id("token.discard-two")
fn discard_two(first: own Token, second: own Token) -> i64 { 0 }

@id("token.requires")
fn requires_guard(value: own Token, allowed: bool) -> i64
    requires allowed
{
    0
}

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.checked")
fn checked(value: own Token, number: i64) -> i64
    requires number >= 0
{
    number + 1
}

@id("token.choose-second")
fn choose_second(first: own Token, count: i64, second: own Token) -> Token
    requires count >= 0
{
    second
}

@id("token.ensures-false")
fn ensures_false(value: own Token) -> Token
    ensures false
{
    value
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

    enum CallableArgument {
        I64(i64),
        Bool(bool),
        Owned { ordinal: u32, payload: u64 },
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn callable_request(
        artifact: &NativeCallableAdmissionArtifact,
        invocation: u64,
        arguments: &[CallableArgument],
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"SPXNREQ1");
        push_u32(&mut bytes, 1);
        push_u32(&mut bytes, 20);
        push_u32(&mut bytes, 0);
        bytes.extend_from_slice(&artifact.call_contract());
        push_u64(&mut bytes, invocation);
        push_u32(&mut bytes, arguments.len() as u32);
        for (index, argument) in arguments.iter().enumerate() {
            match argument {
                CallableArgument::I64(value) => {
                    push_u32(&mut bytes, 1);
                    push_u32(&mut bytes, index as u32);
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                CallableArgument::Bool(value) => {
                    push_u32(&mut bytes, 1);
                    push_u32(&mut bytes, index as u32);
                    push_u32(&mut bytes, u32::from(*value));
                }
                CallableArgument::Owned { ordinal, payload } => {
                    push_u32(&mut bytes, 2);
                    push_u32(&mut bytes, index as u32);
                    push_u32(&mut bytes, *ordinal);
                    push_u64(&mut bytes, *payload);
                }
            }
        }
        let length = u32::try_from(bytes.len()).unwrap();
        bytes[16..20].copy_from_slice(&length.to_le_bytes());
        assert_eq!(length, artifact.max_request_bytes());
        bytes
    }

    fn c_byte_array(name: &str, bytes: &[u8]) -> String {
        let mut output = format!("static const uint8_t {name}[] = {{");
        for byte in bytes {
            write!(output, "0x{byte:02x},").unwrap();
        }
        output.push_str("};\n");
        output
    }

    fn provider_harness(
        artifact: &NativeCallableAdmissionArtifact,
        requests: &[Vec<u8>],
    ) -> String {
        let mut source = artifact.provider_source().to_owned();
        source.push_str("#if defined(_WIN32)\n#include <fcntl.h>\n#include <io.h>\n#endif\n");
        for (index, request) in requests.iter().enumerate() {
            source.push_str(&c_byte_array(&format!("spx_request_{index}"), request));
        }
        source.push_str("int main(void) {\n");
        source.push_str(
            "#if defined(_WIN32)\n    if (_setmode(_fileno(stdout), _O_BINARY) == -1) return 89;\n#endif\n",
        );
        writeln!(
            source,
            "    if (memcmp({}(), \"SPXNABI2\", UINT32_C(8)) != 0) return 90;",
            artifact.getter_symbol()
        )
        .unwrap();
        writeln!(
            source,
            "    uint8_t response[UINT32_C({})];",
            artifact.max_response_bytes()
        )
        .unwrap();
        for index in 0..requests.len() {
            source.push_str("    memset(response, 0xa5, sizeof(response));\n");
            writeln!(source, "    if ({}(spx_request_{index}, (uint32_t)sizeof(spx_request_{index}), response, (uint32_t)sizeof(response)) != SPX_CALL_COMPLETE) return {};", artifact.callable_symbol(), 100 + index).unwrap();
            writeln!(source, "    uint32_t declared_{index} = spx_load_u32(response + UINT32_C(16)); if (declared_{index} > (uint32_t)sizeof(response) || fwrite(response, UINT32_C(1), declared_{index}, stdout) != declared_{index}) return {};", 120 + index).unwrap();
        }
        source.push_str("    return 0;\n}\n");
        source
    }

    fn compile_and_run_provider(
        artifact: &NativeCallableAdmissionArtifact,
        requests: &[Vec<u8>],
        optimization: &str,
        sanitizers: bool,
    ) -> Vec<u8> {
        let fixture = CallableFixture::create();
        let source_path = fixture.0.join("provider.c");
        let executable = fixture.0.join("provider");
        fs::write(&source_path, provider_harness(artifact, requests)).unwrap();
        let mut command = Command::new("clang");
        command.args([
            "-std=c11",
            "-pedantic-errors",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-fvisibility=hidden",
            optimization,
        ]);
        if sanitizers {
            command.args([
                "-fsanitize=address,undefined",
                "-fno-omit-frame-pointer",
                "-fno-sanitize-recover=all",
            ]);
        }
        let compiled = command
            .arg(&source_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "integrated provider compilation ({optimization}, sanitizers={sanitizers}) failed:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let run = Command::new(&executable)
            .env("ASAN_OPTIONS", "detect_leaks=0:halt_on_error=1")
            .env("UBSAN_OPTIONS", "halt_on_error=1")
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "integrated provider execution failed with {}:\n{}",
            run.status,
            String::from_utf8_lossy(&run.stderr)
        );
        run.stdout
    }

    fn split_responses(bytes: &[u8]) -> Vec<&[u8]> {
        let mut responses = Vec::new();
        let mut offset = 0_usize;
        while offset < bytes.len() {
            assert!(bytes.len() - offset >= 20);
            let declared =
                u32::from_le_bytes(bytes[offset + 16..offset + 20].try_into().unwrap()) as usize;
            assert!(declared >= 68 && declared <= bytes.len() - offset);
            responses.push(&bytes[offset..offset + declared]);
            offset += declared;
        }
        responses
    }

    fn response_word(response: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(response[offset..offset + 4].try_into().unwrap())
    }

    fn resolved_resource_program() -> ResolvedProgram {
        let parsed = parse(
            RESOURCE_SOURCE,
            Path::new("native-resource-type-selection.spx"),
        )
        .unwrap();
        hir::resolve(&parsed).unwrap()
    }

    #[test]
    fn private_ios_static_fixture_facade_binds_descriptor_source_and_symbols() {
        let corpus = crate::owned_resource_corpus::build_owned_resource_corpus_v1().unwrap();
        let function = DeclarationId::new("token.discard-two");
        let targets = [
            PrivateNativeCallableV3IosTarget::DeviceArm64,
            PrivateNativeCallableV3IosTarget::SimulatorArm64,
            PrivateNativeCallableV3IosTarget::SimulatorX86_64,
            PrivateNativeCallableV3IosTarget::MacCatalystArm64,
            PrivateNativeCallableV3IosTarget::MacCatalystX86_64,
        ];
        let mut artifacts = Vec::new();
        for target in targets {
            let artifact = emit_private_native_callable_v3_ios_fixture(
                &corpus.program,
                &function,
                PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
                target,
            )
            .unwrap();
            let metadata =
                emit_private_native_callable_v3_ios_descriptor(&corpus.program, &function, target)
                    .unwrap();
            assert_eq!(artifact.descriptor(), metadata.bytes());
            assert_eq!(artifact.getter_symbol(), metadata.getter_symbol());
            assert_eq!(artifact.execute_symbol(), metadata.execute_symbol());
            assert_eq!(artifact.settle_symbol(), metadata.settle_symbol());
            assert!(artifact.source().contains(artifact.getter_symbol()));
            assert!(artifact.source().contains(artifact.execute_symbol()));
            assert!(artifact.source().contains(artifact.settle_symbol()));
            assert!(artifacts
                .iter()
                .all(|prior: &Vec<u8>| prior.as_slice() != artifact.descriptor()));
            artifacts.push(artifact.descriptor().to_vec());
        }
    }

    #[test]
    fn private_android_dynamic_fixture_facade_binds_descriptor_source_and_symbols() {
        let corpus = crate::owned_resource_corpus::build_owned_resource_corpus_v1().unwrap();
        let function = DeclarationId::new("token.discard-two");
        let targets = [
            PrivateNativeCallableV3AndroidTarget::Arm64,
            PrivateNativeCallableV3AndroidTarget::X86_64,
        ];
        let mut artifacts = Vec::new();
        for target in targets {
            let first = emit_private_native_callable_v3_android_fixture(
                &corpus.program,
                &function,
                PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
                target,
            )
            .unwrap();
            let second = emit_private_native_callable_v3_android_fixture(
                &corpus.program,
                &function,
                PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
                target,
            )
            .unwrap();
            let metadata = emit_private_native_callable_v3_android_descriptor(
                &corpus.program,
                &function,
                target,
            )
            .unwrap();
            assert_eq!(first.descriptor(), metadata.bytes());
            assert_eq!(first.getter_symbol(), metadata.getter_symbol());
            assert_eq!(first.execute_symbol(), metadata.execute_symbol());
            assert_eq!(first.settle_symbol(), metadata.settle_symbol());
            assert_eq!(first.descriptor(), second.descriptor());
            assert_eq!(first.source(), second.source());
            assert!(first.source().contains(first.getter_symbol()));
            assert!(first.source().contains(first.execute_symbol()));
            assert!(first.source().contains(first.settle_symbol()));
            assert!(artifacts
                .iter()
                .all(|prior: &Vec<u8>| prior.as_slice() != first.descriptor()));
            artifacts.push(first.descriptor().to_vec());
        }
    }

    #[test]
    fn android_corpus_failure_fixture_is_deterministic_and_target_bound() {
        let corpus = crate::owned_resource_corpus::build_owned_resource_corpus_v1().unwrap();
        let case = corpus
            .cases
            .iter()
            .find(|case| case.scenario_id == "requires-false")
            .expect("requires-false corpus case");
        let function = DeclarationId::new(case.function_id);
        let first = emit_private_native_callable_v3_android_corpus_fixture(
            &corpus.program,
            &function,
            &case.arguments,
            case.expected_owned_result_ordinal,
            &case.reference,
            PrivateNativeCallableV3AndroidTarget::X86_64,
        )
        .unwrap();
        let second = emit_private_native_callable_v3_android_corpus_fixture(
            &corpus.program,
            &function,
            &case.arguments,
            case.expected_owned_result_ordinal,
            &case.reference,
            PrivateNativeCallableV3AndroidTarget::X86_64,
        )
        .unwrap();
        assert_eq!(first.descriptor(), second.descriptor());
        assert_eq!(first.source(), second.source());
        assert!(first.source().contains(first.getter_symbol()));
        assert!(first.source().contains(first.execute_symbol()));
        assert!(first.source().contains(first.settle_symbol()));
        let arm64 = emit_private_native_callable_v3_android_corpus_fixture(
            &corpus.program,
            &function,
            &case.arguments,
            case.expected_owned_result_ordinal,
            &case.reference,
            PrivateNativeCallableV3AndroidTarget::Arm64,
        )
        .unwrap();
        assert_ne!(first.descriptor(), arm64.descriptor());
    }

    #[test]
    fn ios_corpus_failure_fixture_is_deterministic_and_target_bound() {
        let corpus = crate::owned_resource_corpus::build_owned_resource_corpus_v1().unwrap();
        let case = corpus
            .cases
            .iter()
            .find(|case| case.scenario_id == "requires-false")
            .expect("requires-false corpus case");
        let function = DeclarationId::new(case.function_id);
        let first = emit_private_native_callable_v3_ios_corpus_fixture(
            &corpus.program,
            &function,
            &case.arguments,
            case.expected_owned_result_ordinal,
            &case.reference,
            PrivateNativeCallableV3IosTarget::SimulatorArm64,
        )
        .unwrap();
        let second = emit_private_native_callable_v3_ios_corpus_fixture(
            &corpus.program,
            &function,
            &case.arguments,
            case.expected_owned_result_ordinal,
            &case.reference,
            PrivateNativeCallableV3IosTarget::SimulatorArm64,
        )
        .unwrap();
        assert_eq!(first.descriptor(), second.descriptor());
        assert_eq!(first.source(), second.source());
        assert!(first.source().contains(first.getter_symbol()));
        assert!(first.source().contains(first.execute_symbol()));
        assert!(first.source().contains(first.settle_symbol()));
        let device = emit_private_native_callable_v3_ios_corpus_fixture(
            &corpus.program,
            &function,
            &case.arguments,
            case.expected_owned_result_ordinal,
            &case.reference,
            PrivateNativeCallableV3IosTarget::DeviceArm64,
        )
        .unwrap();
        assert_ne!(first.descriptor(), device.descriptor());
    }

    #[test]
    fn scalar_c_output_matches_the_committed_pre_resource_template_baseline() {
        let parsed = parse(
            r#"module test.native_scalar_baseline;

@id("math.increment")
fn increment(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64 { increment(41) }
"#,
            Path::new("native-scalar-baseline.spx"),
        )
        .unwrap();
        let generated = emit_c(&parsed).unwrap();
        let digest = format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(generated.as_bytes()))
        );
        assert_eq!(
            digest,
            "0bd8d71ca3927dc35cdbd803e8b86d54a5abc72f2c65d4e68cf41fdb06fc143c"
        );
    }

    #[test]
    fn direct_resource_parameters_and_results_use_the_stable_wrapper_type() {
        let program = resolved_resource_program();
        let resource_abi = native_resource::build_resource_abi(&program).unwrap();
        let functions = function_index(&program).unwrap();
        let wrapper = &resource_abi.resources[0].c_type;
        let mut output = String::new();
        emit_native_prelude(&mut output, &resource_abi, &program);
        emit_function_prototypes(&mut output, &program, &functions, &resource_abi).unwrap();

        let identity_symbol = c_function_symbol(&DeclarationId::new("token.identity"));
        let prototype = output
            .lines()
            .find(|line| line.contains(&identity_symbol))
            .unwrap();
        assert!(prototype.contains(&format!(
            "{identity_symbol}(struct spx_context *spx_ctx, {wrapper}, {wrapper} *spx_result_out);"
        )));
        assert!(!prototype.contains("void *"));
        assert!(
            output
                .find("/* semaprax.native-resource-abi.v1 */")
                .unwrap()
                < output.find(&identity_symbol).unwrap()
        );

        let mut second = String::new();
        emit_native_prelude(&mut second, &resource_abi, &program);
        emit_function_prototypes(&mut second, &program, &functions, &resource_abi).unwrap();
        assert_eq!(output, second);
    }

    #[test]
    fn public_resource_emission_runs_preflight_but_remains_b104_gated() {
        let parsed = parse(
            RESOURCE_SOURCE,
            Path::new("native-resource-public-gate.spx"),
        )
        .unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let resource_abi = native_resource::build_resource_abi(&program).unwrap();
        let functions = function_index(&program).unwrap();
        preflight_resource_lowering(&program, &functions, &resource_abi, &HashMap::new()).unwrap();

        let diagnostic = emit_c(&parsed).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert_eq!(
            diagnostic.message,
            "native resource lowering requires lifecycle declarations and the verified cleanup ABI"
        );
    }

    #[test]
    fn callable_v2_admission_is_deterministic_and_does_not_open_b104() {
        let program = resolved_resource_program();
        let function = DeclarationId::new("token.identity");
        let first = emit_native_callable_admission(&program, &function).unwrap();
        let second = emit_native_callable_admission(&program, &function).unwrap();

        assert_eq!(first.descriptor(), second.descriptor());
        assert_eq!(first.getter_symbol(), second.getter_symbol());
        assert_eq!(first.callable_symbol(), second.callable_symbol());
        assert_ne!(first.getter_symbol(), first.callable_symbol());
        assert_eq!(&first.descriptor()[..8], b"SPXNABI2");
        assert_eq!(first.max_request_bytes(), 84);
        assert!(first.max_response_bytes() > 68);
        assert!(first.event_dictionary().contains("token.identity"));

        let parsed = parse(
            RESOURCE_SOURCE,
            Path::new("native-callable-v2-public-gate.spx"),
        )
        .unwrap();
        let public = emit_c(&parsed).unwrap_err();
        assert_eq!(public.code, "SPX-B104");
    }

    #[test]
    fn callable_v2_integrated_provider_is_strict_c11() {
        if Command::new("clang").arg("--version").output().is_err() {
            return;
        }
        let program = resolved_resource_program();
        let artifact =
            emit_native_callable_admission(&program, &DeclarationId::new("token.identity"))
                .unwrap();
        let fixture = CallableFixture::create();
        let source = fixture.0.join("provider.c");
        let object = fixture.0.join("provider.o");
        fs::write(&source, artifact.provider_source()).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                "-pedantic-errors",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-O2",
                "-fvisibility=hidden",
                "-c",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "integrated provider C11 compilation failed:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
    }

    #[test]
    fn callable_v2_production_provider_is_strict_cc_at_o0_and_o2() {
        #[cfg(windows)]
        let compiler_name = "clang";
        #[cfg(not(windows))]
        let compiler_name = "cc";
        if Command::new(compiler_name)
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let parsed = parse(
            CALLABLE_EXECUTION_SOURCE,
            Path::new("native-callable-strict-cc.spx"),
        )
        .unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let artifact =
            emit_native_callable_admission(&program, &DeclarationId::new("token.checked")).unwrap();
        let fixture = CallableFixture::create();
        let source = fixture.0.join("provider.c");
        fs::write(&source, artifact.provider_source()).unwrap();

        for optimization in ["-O0", "-O2"] {
            let object = fixture.0.join(format!(
                "provider-{}.o",
                optimization.trim_start_matches('-').to_ascii_lowercase()
            ));
            let compiled = Command::new(compiler_name)
                .args([
                    "-std=c11",
                    "-pedantic-errors",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    optimization,
                    "-fvisibility=hidden",
                    "-c",
                ])
                .arg(&source)
                .arg("-o")
                .arg(&object)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "production provider strict {compiler_name} compilation failed at {optimization}:\n{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
        }
    }

    #[test]
    fn callable_v2_wrong_physical_owned_result_fails_without_touching_response() {
        if Command::new("clang").arg("--version").output().is_err() {
            return;
        }
        let parsed = parse(
            CALLABLE_EXECUTION_SOURCE,
            Path::new("native-callable-physical-result-integrity.spx"),
        )
        .unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let artifact =
            emit_native_callable_admission(&program, &DeclarationId::new("token.identity"))
                .unwrap();
        let request = callable_request(
            &artifact,
            201,
            &[CallableArgument::Owned {
                ordinal: 0,
                payload: u64::MAX,
            }],
        );

        // Establish that the unmodified provider emits the authenticated owned
        // result-commit for this exact request.
        let baseline =
            compile_and_run_provider(&artifact, std::slice::from_ref(&request), "-O2", false);
        let baseline = split_responses(&baseline)[0];
        assert_eq!(response_word(baseline, 60), 1);
        assert_eq!(response_word(baseline, 68), 2);
        assert_eq!(response_word(baseline, 72), 0);

        // Model a lowering defect after the verified body has already emitted
        // its valid semantic trace: corrupt only the physical result payload.
        // The generated hook must detect the disagreement before the wrapper
        // writes even one response byte.
        let marker = "    if (spx_trace.length == UINT32_C(0) ||";
        assert_eq!(artifact.provider_source().matches(marker).count(), 1);
        let mut hostile = artifact.provider_source().replacen(
            marker,
            "    spx_result.payload ^= (uintptr_t)UINT32_C(1);\n    if (spx_trace.length == UINT32_C(0) ||",
            1,
        );
        hostile.push_str(&c_byte_array("spx_physical_mismatch_request", &request));
        writeln!(
            hostile,
            "static int spx_response_unchanged(const uint8_t *response, size_t length) {{ for (size_t i = 0; i < length; ++i) if (response[i] != UINT8_C(0xa5)) return 0; return 1; }}\nint main(void) {{ uint8_t response[UINT32_C({})]; memset(response, 0xa5, sizeof(response)); uint32_t physical = {}(spx_physical_mismatch_request, (uint32_t)sizeof(spx_physical_mismatch_request), response, (uint32_t)sizeof(response)); if (physical != SPX_CALL_INTERNAL_FAILURE || !spx_response_unchanged(response, sizeof(response))) return 1; return 0; }}",
            artifact.max_response_bytes(),
            artifact.callable_symbol()
        )
        .unwrap();

        for optimization in ["-O0", "-O2"] {
            let fixture = CallableFixture::create();
            let source = fixture.0.join("provider.c");
            let executable = fixture.0.join("provider");
            fs::write(&source, &hostile).unwrap();
            let compiled = Command::new("clang")
                .args([
                    "-std=c11",
                    "-pedantic-errors",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-fvisibility=hidden",
                    optimization,
                ])
                .arg(&source)
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "physical-result mismatch fixture compilation ({optimization}) failed:\n{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let run = Command::new(&executable).output().unwrap();
            assert!(
                run.status.success(),
                "physical-result mismatch fixture ({optimization}) failed with {}:\n{}",
                run.status,
                String::from_utf8_lossy(&run.stderr)
            );
        }
    }

    #[test]
    fn callable_v2_integrated_scalar_owned_success_failure_are_o0_o2_exact() {
        let sanitizers_required =
            std::env::var("SEMAPRAX_REQUIRE_NATIVE_SANITIZERS").is_ok_and(|value| value == "1");
        if Command::new("clang").arg("--version").output().is_err() {
            assert!(
                !sanitizers_required,
                "clang is required for sanitizer evidence"
            );
            return;
        }
        let parsed = parse(
            CALLABLE_EXECUTION_SOURCE,
            Path::new("native-callable-execution.spx"),
        )
        .unwrap();
        let program = hir::resolve(&parsed).unwrap();

        let requires =
            emit_native_callable_admission(&program, &DeclarationId::new("token.requires"))
                .unwrap();
        let requires_again =
            emit_native_callable_admission(&program, &DeclarationId::new("token.requires"))
                .unwrap();
        assert_eq!(requires.provider_source(), requires_again.provider_source());
        assert_eq!(
            requires.semantic_event_dictionary().canonical_json(),
            requires.event_dictionary()
        );
        let requires_requests = vec![
            callable_request(
                &requires,
                101,
                &[
                    CallableArgument::Owned {
                        ordinal: 0,
                        payload: u64::MAX,
                    },
                    CallableArgument::Bool(true),
                ],
            ),
            callable_request(
                &requires,
                102,
                &[
                    CallableArgument::Owned {
                        ordinal: 0,
                        payload: 0,
                    },
                    CallableArgument::Bool(false),
                ],
            ),
        ];
        let requires_o0 = compile_and_run_provider(&requires, &requires_requests, "-O0", false);
        let requires_o2 = compile_and_run_provider(&requires, &requires_requests, "-O2", false);
        assert_eq!(requires_o0, requires_o2);
        let responses = split_responses(&requires_o2);
        assert_eq!(responses.len(), 2);
        assert_eq!(&responses[0][..8], b"SPXNRSP1");
        assert_eq!(response_word(responses[0], 60), 1);
        assert_eq!(response_word(responses[1], 60), 2);
        assert_eq!(
            u64::from_le_bytes(responses[0][52..60].try_into().unwrap()),
            101
        );
        assert_eq!(
            u64::from_le_bytes(responses[1][52..60].try_into().unwrap()),
            102
        );
        let selected = response_word(responses[1], 68);
        let failure_events = (0..response_word(responses[1], 64) as usize)
            .map(|index| response_word(responses[1], 72 + index * 4))
            .collect::<Vec<_>>();
        assert!(failure_events.contains(&selected));

        let checked =
            emit_native_callable_admission(&program, &DeclarationId::new("token.checked")).unwrap();
        let checked_requests = vec![
            callable_request(
                &checked,
                105,
                &[
                    CallableArgument::Owned {
                        ordinal: 0,
                        payload: 0,
                    },
                    CallableArgument::I64(41),
                ],
            ),
            callable_request(
                &checked,
                106,
                &[
                    CallableArgument::Owned {
                        ordinal: 0,
                        payload: u64::MAX,
                    },
                    CallableArgument::I64(i64::MAX),
                ],
            ),
        ];
        let checked_o0 = compile_and_run_provider(&checked, &checked_requests, "-O0", false);
        let checked_o2 = compile_and_run_provider(&checked, &checked_requests, "-O2", false);
        assert_eq!(checked_o0, checked_o2);
        let checked_responses = split_responses(&checked_o2);
        assert_eq!(response_word(checked_responses[0], 60), 1);
        assert_eq!(
            i64::from_le_bytes(checked_responses[0][72..80].try_into().unwrap()),
            42
        );
        assert_eq!(response_word(checked_responses[1], 60), 2);

        let identity =
            emit_native_callable_admission(&program, &DeclarationId::new("token.identity"))
                .unwrap();
        let identity_requests = vec![callable_request(
            &identity,
            103,
            &[CallableArgument::Owned {
                ordinal: 0,
                payload: u64::MAX,
            }],
        )];
        let identity_o0 = compile_and_run_provider(&identity, &identity_requests, "-O0", false);
        let identity_o2 = compile_and_run_provider(&identity, &identity_requests, "-O2", false);
        assert_eq!(identity_o0, identity_o2);
        let response = split_responses(&identity_o2)[0];
        assert_eq!(response_word(response, 60), 1);
        assert_eq!(response_word(response, 68), 2);
        assert_eq!(response_word(response, 72), 0);

        let ensures =
            emit_native_callable_admission(&program, &DeclarationId::new("token.ensures-false"))
                .unwrap();
        let ensures_requests = vec![callable_request(
            &ensures,
            104,
            &[CallableArgument::Owned {
                ordinal: 0,
                payload: u64::MAX,
            }],
        )];
        let ensures_o0 = compile_and_run_provider(&ensures, &ensures_requests, "-O0", false);
        let ensures_o2 = compile_and_run_provider(&ensures, &ensures_requests, "-O2", false);
        assert_eq!(ensures_o0, ensures_o2);
        assert_eq!(response_word(split_responses(&ensures_o2)[0], 60), 2);

        if sanitizers_required {
            assert_eq!(
                compile_and_run_provider(&requires, &requires_requests, "-O1", true),
                requires_o2
            );
            assert_eq!(
                compile_and_run_provider(&identity, &identity_requests, "-O1", true),
                identity_o2
            );
            assert_eq!(
                compile_and_run_provider(&checked, &checked_requests, "-O1", true),
                checked_o2
            );
            assert_eq!(
                compile_and_run_provider(&ensures, &ensures_requests, "-O1", true),
                ensures_o2
            );
        }

        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(
                    requires.normalized_execution_projection().as_bytes()
                ))
            ),
            "e5802548830ebc278bfd727a91fecebd763c81d5729374d85a4bded1e0dbf83c"
        );
    }

    #[test]
    fn callable_v2_shared_library_exports_only_getter_and_callable() {
        if Command::new("clang").arg("--version").output().is_err()
            || Command::new("nm").arg("--version").output().is_err()
        {
            return;
        }
        if !cfg!(any(target_os = "linux", target_os = "macos")) {
            return;
        }
        let parsed = parse(
            CALLABLE_EXECUTION_SOURCE,
            Path::new("native-callable-exports.spx"),
        )
        .unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let artifact =
            emit_native_callable_admission(&program, &DeclarationId::new("token.identity"))
                .unwrap();
        let fixture = CallableFixture::create();
        let source = fixture.0.join("provider.c");
        let library = fixture.0.join(if cfg!(target_os = "macos") {
            "provider.dylib"
        } else {
            "provider.so"
        });
        fs::write(&source, artifact.provider_source()).unwrap();
        let mut compile = Command::new("clang");
        compile.args([
            "-std=c11",
            "-pedantic-errors",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-O2",
            "-fvisibility=hidden",
        ]);
        if cfg!(target_os = "macos") {
            compile.arg("-dynamiclib");
        } else {
            compile.args(["-shared", "-fPIC"]);
        }
        let compiled = compile
            .arg(&source)
            .arg("-o")
            .arg(&library)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "callable shared-library compilation failed:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let symbols = if cfg!(target_os = "macos") {
            Command::new("nm").args(["-gU"]).arg(&library).output()
        } else {
            Command::new("nm")
                .args(["-D", "--defined-only"])
                .arg(&library)
                .output()
        }
        .unwrap();
        assert!(symbols.status.success());
        let mut actual = String::from_utf8(symbols.stdout)
            .unwrap()
            .lines()
            .filter_map(|line| line.split_whitespace().last())
            .map(|symbol| {
                if cfg!(target_os = "macos") {
                    symbol.strip_prefix('_').unwrap_or(symbol).to_owned()
                } else {
                    symbol.to_owned()
                }
            })
            .collect::<Vec<_>>();
        actual.sort();
        let mut expected = vec![
            artifact.callable_symbol().to_owned(),
            artifact.getter_symbol().to_owned(),
        ];
        expected.sort();
        assert_eq!(actual, expected);
    }

    #[test]
    fn callable_v2_authoritative_fourteen_case_corpus_has_production_providers() {
        let parsed = parse(
            CALLABLE_EXECUTION_SOURCE,
            Path::new("native-callable-authoritative-corpus.spx"),
        )
        .unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let scenario_functions = [
            ("discard-zero", "token.discard"),
            ("discard-max", "token.discard"),
            ("discard-two-reverse", "token.discard-two"),
            ("requires-false", "token.requires"),
            ("requires-true", "token.requires"),
            ("checked-success", "token.checked"),
            ("checked-add-overflow", "token.checked"),
            ("checked-precondition-false", "token.checked"),
            ("identity-zero", "token.identity"),
            ("identity-max", "token.identity"),
            ("choose-second-zero-max", "token.choose-second"),
            ("choose-second-zero-zero", "token.choose-second"),
            ("choose-second-requires-false", "token.choose-second"),
            ("ensures-false", "token.ensures-false"),
        ];
        assert_eq!(
            scenario_functions.map(|(scenario, _)| scenario),
            crate::semantic_trace::OWNED_RESOURCE_CORPUS_V1_SCENARIOS
        );
        for (_, function) in scenario_functions {
            let artifact =
                emit_native_callable_admission(&program, &DeclarationId::new(function)).unwrap();
            assert!(artifact
                .provider_source()
                .contains(artifact.callable_symbol()));
            assert!(artifact
                .provider_source()
                .contains(artifact.getter_symbol()));
            assert_eq!(
                artifact.semantic_event_dictionary().canonical_json(),
                artifact.event_dictionary()
            );
        }
    }

    #[test]
    fn callable_v2_admission_changes_with_checked_semantics() {
        let baseline = resolved_resource_program();
        let changed_source = RESOURCE_SOURCE.replace(
            "fn identity(value: own Token) -> Token { value }",
            "fn identity(value: own Token) -> Token ensures false { value }",
        );
        let changed = parse(
            &changed_source,
            Path::new("native-callable-v2-semantic-delta.spx"),
        )
        .unwrap();
        let changed = hir::resolve(&changed).unwrap();
        let function = DeclarationId::new("token.identity");
        let baseline = emit_native_callable_admission(&baseline, &function).unwrap();
        let changed = emit_native_callable_admission(&changed, &function).unwrap();

        assert_ne!(baseline.descriptor(), changed.descriptor());
        assert_ne!(baseline.getter_symbol(), changed.getter_symbol());
        assert_ne!(baseline.callable_symbol(), changed.callable_symbol());
        assert_ne!(baseline.event_dictionary(), changed.event_dictionary());
    }

    #[test]
    fn resource_value_preflight_rejects_unstaged_borrow_without_changing_public_gate() {
        let source = RESOURCE_SOURCE.replace(
            "@id(\"token.identity\")\nfn identity(value: own Token) -> Token { value }",
            "@id(\"token.observe\")\nfn observe(value: borrow Token) -> i64 { 0 }",
        );
        let parsed = parse(
            &source,
            Path::new("native-resource-borrow-value-preflight.spx"),
        )
        .unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let resource_abi = native_resource::build_resource_abi(&program).unwrap();
        let functions = function_index(&program).unwrap();
        let preflight =
            preflight_resource_lowering(&program, &functions, &resource_abi, &HashMap::new())
                .unwrap_err();
        assert_eq!(preflight.code, "SPX-B104");
        assert!(preflight.message.contains("resource parameter"));

        let public = emit_c(&parsed).unwrap_err();
        let gate = resource_lowering_gate();
        assert_eq!(public.code, gate.code);
        assert_eq!(public.message, gate.message);
    }

    #[test]
    fn c_literals_preserve_utf8_bytes_without_exposing_trigraphs() {
        static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

        if Command::new("clang").arg("--version").output().is_err() {
            return;
        }

        let escaped = c_string("??/λ\n\r\t\\\"\u{7f}");
        assert_eq!(escaped, "\\077\\077/\\316\\273\\n\\r\\t\\\\\\\"\\177");
        assert!(!escaped.contains("??"));
        assert!(escaped.is_ascii());
        assert_eq!(
            c_i64(i64::MIN),
            "(-INT64_C(9223372036854775807) - INT64_C(1))"
        );
        assert_eq!(c_i64(-42), "-INT64_C(42)");
        assert_eq!(c_i64(42), "INT64_C(42)");

        let source = format!(
            "#include <stddef.h>\n\
             #include <stdint.h>\n\
             static const unsigned char value[] = \"{escaped}\";\n\
             static const unsigned char expected[] = {{0x3f, 0x3f, 0x2f, 0xce, 0xbb, 0x0a, 0x0d, 0x09, 0x5c, 0x22, 0x7f, 0x00}};\n\
             static const int64_t minimum = {};\n\
             static const int64_t negative = {};\n\
             int main(void) {{\n\
                 if (sizeof(value) != sizeof(expected)) return 1;\n\
                 for (size_t index = 0; index < sizeof(expected); ++index) {{\n\
                     if (value[index] != expected[index]) return 2;\n\
                 }}\n\
                 if (minimum != INT64_MIN || negative != -INT64_C(42)) return 3;\n\
                 return 0;\n\
             }}\n",
            c_i64(i64::MIN),
            c_i64(-42),
        );
        assert!(!source.contains("??"));

        let suffix = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let c_path = std::env::temp_dir().join(format!(
            "semaprax-c-string-{}-{suffix}.c",
            std::process::id()
        ));
        let binary = std::env::temp_dir().join(format!(
            "semaprax-c-string-{}-{suffix}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        std::fs::write(&c_path, source).expect("write strict C string fixture");
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                "-O2",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-Wtrigraphs",
                "-Wimplicitly-unsigned-literal",
            ])
            .arg(&c_path)
            .arg("-o")
            .arg(&binary)
            .output()
            .expect("clang was available during the version probe");
        let _ = std::fs::remove_file(&c_path);
        assert!(
            compiled.status.success(),
            "strict C11 compilation failed:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let run = Command::new(&binary)
            .output()
            .expect("run C string fixture");
        let _ = std::fs::remove_file(&binary);
        assert!(
            run.status.success(),
            "C string fixture exited {}",
            run.status
        );
    }
}
