mod native_adapter_abi;
#[cfg(test)]
pub(crate) mod native_aggregate;
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
#[cfg(test)]
mod native_conformance;
#[cfg(test)]
mod native_conformance_materialize;
#[cfg(test)]
mod native_conformance_wire;
mod native_host_contract;
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
mod native_module_lease;
mod native_resource;
mod native_runtime;
#[cfg(any(test, feature = "unstable-native-host-internal"))]
mod native_settlement_derivation;
mod native_trace;
mod native_trace_runtime;
mod native_value;

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest as _, Sha256};

use crate::bounded_output::BudgetedJoin as _;

pub use native_callable_bundle::{
    build_native_callable_bundle, preflight_native_callable_bundle, NativeCallableBundle,
    NativeCallableBundlePreflight,
};

use crate::aggregate_layout::{AggregateLayout, AggregateLayoutCache, AggregateTarget};
use crate::ast::{BinaryOp, Program, UnaryOp};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, DeclarationKind, ExpressionId, FunctionExecutionId, PlaceProjection,
    ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedProgram, ResolvedStatement,
    ResolvedType, ResolvedTypeDeclarationKind, ValueId,
};
use crate::variant_layout::{VariantLayout, VariantLayoutCache, VariantTarget};

macro_rules! format {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
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
    emit_hir_c_with_labels(resolved, &labels)
}

/// Emit C11 from resolved HIR.
///
/// This entry point exists so backend tests and future compiler stages can prove
/// that code generation consumes semantic identities and centralized type facts,
/// rather than reconstructing either from source names.
pub fn emit_hir_c(program: &ResolvedProgram) -> Result<String, Diagnostic> {
    reject_native_rust_for_native(program)?;
    emit_hir_c_with_labels(program, &HashMap::new())
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

fn emit_hir_c_with_labels(
    program: &ResolvedProgram,
    contract_labels: &HashMap<ExpressionId, String>,
) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    if program.types.iter().any(|declaration| {
        matches!(
            declaration.kind,
            ResolvedTypeDeclarationKind::Resource { .. }
        )
    }) {
        return Err(resource_lowering_gate());
    }
    let resource_abi = native_resource::build_resource_abi(program)?;
    let record_layouts = AggregateLayoutCache::build(program, AggregateTarget::Native64)?;
    let variant_layouts = VariantLayoutCache::build(program, VariantTarget::Native64)?;
    let functions = function_index(program)?;
    debug_assert!(resource_abi.resources.is_empty());
    let mut output = crate::bounded_output::CappedString::new();
    emit_native_prelude(&mut output, &resource_abi, program);
    emit_aggregate_declarations(
        &mut output,
        program,
        &resource_abi,
        &record_layouts,
        &variant_layouts,
    )?;
    emit_function_prototypes(&mut output, program, &functions, &resource_abi)?;

    let emission = NativeEmissionContext {
        program,
        resource_abi: &resource_abi,
        functions: &functions,
        contract_labels,
        record_layouts: &record_layouts,
        variant_layouts: &variant_layouts,
    };
    for function in &program.functions {
        emit_function(
            &mut output,
            function,
            &FunctionExecutionId::Monomorphic(function.id.clone()),
            &emission,
        )?;
    }
    for instance in &program.function_instances {
        emit_function(
            &mut output,
            &instance.function,
            &FunctionExecutionId::Generic(instance.id.clone()),
            &emission,
        )?;
    }

    let main = program
        .functions
        .iter()
        .find(|function| function.id == program.entrypoint)
        .ok_or_else(|| backend_error("resolved native entry point is not indexed"))?;
    if !main.params.is_empty() || main.return_type != ResolvedType::I64 {
        return Err(backend_error(
            "resolved native entry point must have type `fn main() -> i64`",
        ));
    }
    let symbol = &functions
        .get(&FunctionExecutionId::Monomorphic(main.id.clone()))
        .ok_or_else(|| backend_error("native entry point is not indexed"))?
        .symbol;
    write!(
        output,
        "#ifndef SPX_NO_ENTRY_WRAPPER\n\
         int main(void) {{\n\
             struct spx_status_entry spx_status_entries[UINT32_C(1)];\n\
             struct spx_context spx_ctx = {{0}};\n\
             if (!spx_context_init(&spx_ctx, UINT64_C(1), spx_status_entries, UINT32_C(1), NULL, NULL, NULL)) {{\n\
                 fputs(\"SEMAPRAX native runtime invariant failure: context initialization\\n\", stderr);\n\
                 return 72;\n\
             }}\n\
             int64_t result;\n\
             spx_status_token status = {symbol}(&spx_ctx, &result);\n\
             if (status != SPX_STATUS_SUCCESS) return spx_public_failure(&spx_ctx, status);\n\
             printf(\"%lld\\n\", (long long)result);\n\
             return 0;\n\
         }}\n\
         #endif\n"
    )
    .expect("writing to a string cannot fail");
    Ok(output.into_string())
}

fn emit_native_prelude(
    output: &mut impl COutput,
    resource_abi: &native_resource::NativeResourceAbi,
    program: &ResolvedProgram,
) {
    native_runtime::emit_status_runtime(output);
    output.push_str(&resource_abi.declarations);
    output.push_str("#include <stdio.h>\n\n");
    output.push_str(NATIVE_SCALAR_RUNTIME_C);
    if program_uses_u8_arithmetic(program) {
        // Checked u8 helpers stay out of programs that cannot reach them, so
        // existing projections keep their exact committed bytes.
        output.push_str(NATIVE_U8_RUNTIME_C);
    }
    if program_uses_strings(program) {
        output.push_str(NATIVE_STRING_RUNTIME_C);
    }
    if program_uses_string_ops(program) {
        // String operation helpers stay out of programs that cannot reach
        // them, so existing projections keep their exact committed bytes.
        output.push_str(NATIVE_STRING_OPS_RUNTIME_C);
    }
    if program_uses_string_ops_v2(program) {
        // Breadth-v2 string operation helpers gate as their own group so
        // first-wave programs keep their exact committed bytes.
        output.push_str(NATIVE_STRING_OPS_V2_RUNTIME_C);
    }
}

/// Whether any resolved signature, body, or contract admits an owned string
/// value that lowers through the string runtime helpers.
fn program_uses_strings(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        if matches!(function.return_type, ResolvedType::String)
            || function
                .params
                .iter()
                .any(|param| matches!(param.ty, ResolvedType::String))
        {
            return true;
        }
        pending.push(&function.body);
        for contract in function.requires.iter().chain(&function.ensures) {
            pending.push(contract);
        }
    }
    while let Some(expression) = pending.pop() {
        if matches!(expression.ty, ResolvedType::String)
            || matches!(expression.kind, ResolvedExprKind::String(_))
        {
            return true;
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether any resolved function body or contract calls a compiler-owned
/// string operation intrinsic.
fn program_uses_string_ops(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        for contract in function.requires.iter().chain(&function.ensures) {
            pending.push(contract);
        }
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str()).is_some() {
                return true;
            }
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether any resolved function body or contract calls a breadth-v2
/// compiler-owned string operation intrinsic.
fn program_uses_string_ops_v2(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        for contract in function.requires.iter().chain(&function.ensures) {
            pending.push(contract);
        }
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str())
                .is_some_and(crate::string_ops::StringOp::is_breadth_v2)
            {
                return true;
            }
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Whether any resolved function body or contract contains checked u8
/// arithmetic that lowers through the u8 runtime helpers.
fn program_uses_u8_arithmetic(program: &ResolvedProgram) -> bool {
    let mut pending: Vec<&ResolvedExpr> = Vec::new();
    for function in &program.functions {
        pending.push(&function.body);
        for contract in function.requires.iter().chain(&function.ensures) {
            pending.push(contract);
        }
    }
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Binary { op, left, right } = &expression.kind {
            if matches!(left.ty, ResolvedType::U8)
                && matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                )
            {
                return true;
            }
            pending.push(left);
            pending.push(right);
            continue;
        }
        pending.extend(resolved_expr_children(expression));
    }
    false
}

/// Every direct resolved child of an expression.
fn resolved_expr_children<'a>(
    expression: &'a ResolvedExpr,
) -> Box<dyn Iterator<Item = &'a ResolvedExpr> + 'a> {
    match &expression.kind {
        ResolvedExprKind::Binary { left, right, .. } => {
            Box::new([left.as_ref(), right.as_ref()].into_iter())
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => Box::new(std::iter::once(value.as_ref())),
        ResolvedExprKind::Block { statements, tail } => Box::new(
            statements
                .iter()
                .flat_map(|statement| {
                    (0..statement.child_count()).filter_map(move |index| statement.child(index))
                })
                .chain(std::iter::once(tail.as_ref())),
        ),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => Box::new(
            [
                condition.as_ref(),
                then_branch.as_ref(),
                else_branch.as_ref(),
            ]
            .into_iter(),
        ),
        ResolvedExprKind::Call { args, .. } => Box::new(args.iter()),
        ResolvedExprKind::NativeRustImportCall(call) => Box::new(call.args.iter()),
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            Box::new(fields.iter().map(|field| &field.value))
        }
        ResolvedExprKind::Match { scrutinee, arms } => Box::new(
            std::iter::once(scrutinee.as_ref()).chain(
                arms.iter()
                    .filter_map(|arm| arm.guard.as_deref())
                    .chain(arms.iter().map(|arm| &arm.value)),
            ),
        ),
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            Box::new(std::iter::once(base.as_ref()).chain(fields.iter().map(|field| &field.value)))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_) => Box::new(std::iter::empty()),
    }
}

fn emit_aggregate_declarations(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    record_layouts: &AggregateLayoutCache,
    variant_layouts: &VariantLayoutCache,
) -> Result<(), Diagnostic> {
    let records = record_layouts.layouts().collect::<Vec<_>>();
    let variants = variant_layouts.layouts().collect::<Vec<_>>();
    if records.is_empty() && variants.is_empty() {
        return Ok(());
    }

    output.push_str("#include <stddef.h>\n\n");
    for record in &records {
        writeln!(output, "struct {};", c_record_symbol(&record.instance))
            .expect("writing to a string cannot fail");
    }
    for variant in &variants {
        writeln!(output, "struct {};", c_variant_symbol(&variant.instance))
            .expect("writing to a string cannot fail");
    }
    output.push('\n');

    let mut visiting = BTreeSet::new();
    let mut emitted = BTreeSet::new();
    for record in &records {
        emit_record_declaration(
            output,
            program,
            resource_abi,
            record_layouts,
            &record.instance,
            &mut visiting,
            &mut emitted,
        )?;
    }
    for variant in &variants {
        emit_variant_declaration(output, program, resource_abi, variant)?;
    }
    Ok(())
}

fn emit_record_declaration(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    layouts: &AggregateLayoutCache,
    instance: &ResolvedType,
    visiting: &mut BTreeSet<ResolvedType>,
    emitted: &mut BTreeSet<ResolvedType>,
) -> Result<(), Diagnostic> {
    if emitted.contains(instance) {
        return Ok(());
    }
    if !visiting.insert(instance.clone()) {
        return Err(backend_error(format!(
            "native aggregate instance `{}` is recursively embedded",
            instance.identity_key()
        )));
    }
    let layout = layouts.layout(instance)?.clone();
    layout.validate(program)?;
    for field in &layout.fields {
        if record_declaration_id(program, &field.ty)?.is_some() {
            emit_record_declaration(
                output,
                program,
                resource_abi,
                layouts,
                &field.ty,
                visiting,
                emitted,
            )?;
        }
    }

    let symbol = c_record_symbol(instance);
    writeln!(output, "struct {symbol} {{").expect("writing to a string cannot fail");
    if layout.fields.is_empty() {
        output.push_str("    uint8_t spx_empty_record_padding;\n");
    } else {
        for field in &layout.fields {
            writeln!(
                output,
                "    {} {};",
                c_value_type(program, resource_abi, &field.ty)?,
                c_field_symbol(&field.field)
            )
            .expect("writing to a string cannot fail");
        }
    }
    output.push_str("};\n");
    writeln!(
        output,
        "_Static_assert(sizeof(struct {symbol}) == UINT32_C({}), \"SEMAPRAX native aggregate size\");",
        layout.size
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "_Static_assert(_Alignof(struct {symbol}) == UINT32_C({}), \"SEMAPRAX native aggregate alignment\");",
        layout.align
    )
    .expect("writing to a string cannot fail");
    for field in &layout.fields {
        writeln!(
            output,
            "_Static_assert(offsetof(struct {symbol}, {}) == UINT32_C({}), \"SEMAPRAX native aggregate field offset\");",
            c_field_symbol(&field.field),
            field.offset
        )
        .expect("writing to a string cannot fail");
    }
    output.push('\n');
    visiting.remove(instance);
    emitted.insert(instance.clone());
    Ok(())
}

fn emit_variant_declaration(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    layout: &VariantLayout,
) -> Result<(), Diagnostic> {
    layout.validate(program)?;
    let symbol = c_variant_symbol(&layout.instance);
    writeln!(output, "struct {symbol} {{").expect("writing to a string cannot fail");
    output.push_str("    uint32_t spx_tag;\n    union {\n");
    for case in &layout.cases {
        output.push_str("        struct {\n");
        if case.fields.is_empty() {
            output.push_str("            uint8_t spx_empty_variant_payload;\n");
        } else {
            for field in &case.fields {
                writeln!(
                    output,
                    "            {} {};",
                    c_value_type(program, resource_abi, &field.ty)?,
                    c_field_symbol(&field.field)
                )
                .expect("writing to a string cannot fail");
            }
        }
        writeln!(output, "        }} {};", c_case_symbol(&case.case))
            .expect("writing to a string cannot fail");
    }
    output.push_str("    } spx_payload;\n};\n");
    writeln!(
        output,
        "_Static_assert(sizeof(struct {symbol}) == UINT32_C({}), \"SEMAPRAX native variant size\");",
        layout.size
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "_Static_assert(_Alignof(struct {symbol}) == UINT32_C({}), \"SEMAPRAX native variant alignment\");",
        layout.align
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "_Static_assert(offsetof(struct {symbol}, spx_tag) == UINT32_C(0), \"SEMAPRAX native variant tag offset\");"
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "_Static_assert(offsetof(struct {symbol}, spx_payload) == UINT32_C({}), \"SEMAPRAX native variant payload offset\");",
        layout.payload_offset
    )
    .expect("writing to a string cannot fail");
    for case in &layout.cases {
        let case_symbol = c_case_symbol(&case.case);
        for field in &case.fields {
            let absolute_offset = layout
                .payload_offset
                .checked_add(field.offset)
                .ok_or_else(|| backend_error("native variant field offset overflows u32"))?;
            writeln!(
                output,
                "_Static_assert(offsetof(struct {symbol}, spx_payload.{case_symbol}.{}) == UINT32_C({}), \"SEMAPRAX native variant field offset\");",
                c_field_symbol(&field.field),
                absolute_offset
            )
            .expect("writing to a string cannot fail");
        }
    }
    output.push('\n');
    Ok(())
}

fn c_value_type(
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    ty: &ResolvedType,
) -> Result<String, Diagnostic> {
    if record_declaration_id(program, ty)?.is_some() {
        Ok(format!("struct {}", c_record_symbol(ty)))
    } else if variant_declaration_id(program, ty)?.is_some() {
        Ok(format!("struct {}", c_variant_symbol(ty)))
    } else {
        resource_abi.c_type(program, ty).map(str::to_owned)
    }
}

fn is_aggregate_type(program: &ResolvedProgram, ty: &ResolvedType) -> Result<bool, Diagnostic> {
    Ok(record_declaration_id(program, ty)?.is_some()
        || variant_declaration_id(program, ty)?.is_some())
}

fn record_declaration_id<'a>(
    program: &ResolvedProgram,
    ty: &'a ResolvedType,
) -> Result<Option<&'a DeclarationId>, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| backend_error(format!("unknown native type `{declaration}`")))?;
    if !matches!(
        item.kind,
        ResolvedTypeDeclarationKind::Record { .. } | ResolvedTypeDeclarationKind::Class { .. }
    ) {
        return Ok(None);
    }
    if arguments.len() != item.type_parameters.len()
        || arguments
            .iter()
            .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
    {
        return Err(backend_error(format!(
            "native record representation requires exact concrete i64/bool arguments for `{}`",
            ty.identity_key()
        )));
    }
    Ok(Some(declaration))
}

fn variant_declaration_id<'a>(
    program: &ResolvedProgram,
    ty: &'a ResolvedType,
) -> Result<Option<&'a DeclarationId>, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| backend_error(format!("unknown native type `{declaration}`")))?;
    if !matches!(item.kind, ResolvedTypeDeclarationKind::Variant { .. }) {
        return Ok(None);
    }
    if arguments.len() != item.type_parameters.len()
        || arguments
            .iter()
            .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
    {
        return Err(backend_error(format!(
            "native variant representation requires exact concrete i64/bool arguments for `{}`",
            ty.identity_key()
        )));
    }
    Ok(Some(declaration))
}

fn c_record_symbol(ty: &ResolvedType) -> String {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        unreachable!("record C symbols require nominal types");
    };
    let mut symbol = stable_c_symbol("spx_record_", declaration);
    if !arguments.is_empty() {
        let mut digest = Sha256::new();
        digest.update(b"semaprax.native-record-instance.v1\0");
        digest.update(ty.identity_key().as_bytes());
        symbol.push_str("_inst_");
        for byte in digest.finalize() {
            write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
        }
    }
    symbol
}

fn c_variant_symbol(ty: &ResolvedType) -> String {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        unreachable!("variant C symbols require nominal types");
    };
    let mut symbol = stable_c_symbol("spx_variant_", declaration);
    if !arguments.is_empty() {
        let mut digest = Sha256::new();
        digest.update(b"semaprax.native-variant-instance.v1\0");
        digest.update(ty.identity_key().as_bytes());
        symbol.push_str("_inst_");
        for byte in digest.finalize() {
            write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
        }
    }
    symbol
}

fn c_case_symbol(id: &DeclarationId) -> String {
    stable_c_symbol("spx_case_", id)
}

fn c_field_symbol(id: &DeclarationId) -> String {
    stable_c_symbol("spx_field_", id)
}

fn stable_c_symbol(prefix: &str, id: &DeclarationId) -> String {
    let mut symbol = crate::bounded_output::CappedString::new();
    symbol.push_str(prefix);
    for byte in id.as_str().bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol.into_string()
}

fn emit_function_prototypes(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    functions: &HashMap<FunctionExecutionId, CFunction>,
    resource_abi: &native_resource::NativeResourceAbi,
) -> Result<(), Diagnostic> {
    for (function, execution) in program
        .functions
        .iter()
        .map(|function| {
            (
                function,
                FunctionExecutionId::Monomorphic(function.id.clone()),
            )
        })
        .chain(program.function_instances.iter().map(|instance| {
            (
                &instance.function,
                FunctionExecutionId::Generic(instance.id.clone()),
            )
        }))
    {
        let metadata = functions
            .get(&execution)
            .ok_or_else(|| backend_error(format!("function `{}` is not indexed", function.id)))?;
        write!(
            output,
            "static __attribute__((unused)) spx_status_token {}(struct spx_context *spx_ctx",
            metadata.symbol,
        )
        .expect("writing to a string cannot fail");
        for param in &function.params {
            let ty = c_value_type(program, resource_abi, &param.ty)?;
            if is_aggregate_type(program, &param.ty)? {
                write!(output, ", const {ty} *").expect("writing to a string cannot fail");
            } else {
                write!(output, ", {ty}").expect("writing to a string cannot fail");
            }
        }
        writeln!(
            output,
            ", {} *spx_result_out);",
            c_value_type(program, resource_abi, &function.return_type)?
        )
        .expect("writing to a string cannot fail");
    }
    output.push('\n');
    Ok(())
}

#[cfg(test)]
fn preflight_resource_lowering(
    program: &ResolvedProgram,
    functions: &HashMap<FunctionExecutionId, CFunction>,
    resource_abi: &native_resource::NativeResourceAbi,
    contract_labels: &HashMap<ExpressionId, String>,
) -> Result<(), Diagnostic> {
    let mut first_failure = None;
    for function in &program.functions {
        match native_cleanup::classify(program, function) {
            Ok(cleanup) => {
                match native_value::plan(program, function, &cleanup, resource_abi, contract_labels)
                {
                    Ok(mut values) => {
                        match crate::semantic_trace::build_semantic_event_dictionary(
                            program,
                            &function.id,
                        ) {
                            Ok(dictionary) => {
                                values.cleanup_bindings.semantic_events = Some(dictionary);
                            }
                            Err(diagnostic) => {
                                first_failure.get_or_insert(diagnostic);
                                continue;
                            }
                        }
                        match native_host_contract::derive_from_admitted(
                            program,
                            &function.id,
                            resource_abi,
                            &cleanup,
                            &values,
                        ) {
                            Ok(host_template) => {
                                match native_adapter_abi::derive(&host_template) {
                                    Ok(descriptor) => {
                                        let _descriptor_header =
                                            native_adapter_abi::emit_header(&descriptor);
                                        if let Err(diagnostic) = native_adapter_abi::emit_source(
                                            &descriptor,
                                            "semaprax_adapter_descriptor.h",
                                        ) {
                                            first_failure.get_or_insert(diagnostic);
                                        }
                                    }
                                    Err(diagnostic) => {
                                        first_failure.get_or_insert(diagnostic);
                                    }
                                }
                                let _declarations = native_value::emit_declarations(&values);
                                match native_cleanup_emit::emit_with_block_prologues(
                                    &cleanup,
                                    &values.cleanup_bindings,
                                    |block, output| {
                                        output.push_str(&native_value::emit_block_prologue(
                                            &values, block,
                                        ));
                                        Ok(())
                                    },
                                ) {
                                    Ok(_cleanup_body) => {}
                                    Err(diagnostic) => {
                                        first_failure.get_or_insert(diagnostic);
                                    }
                                }
                            }
                            Err(diagnostic) => {
                                first_failure.get_or_insert(diagnostic);
                            }
                        }
                    }
                    Err(diagnostic) => {
                        first_failure.get_or_insert(diagnostic);
                    }
                }
            }
            Err(diagnostic) => {
                first_failure.get_or_insert(diagnostic);
            }
        }
        if let Err(diagnostic) = native_trace::required_event_capacity(program, function) {
            first_failure.get_or_insert(diagnostic);
        }
    }

    // Exercise the same ABI declaration/type/prototype order that the eventual
    // resource emitter will use, then discard it. The loop above separately
    // constructs the gated value/cleanup bodies; the exact conformance harness
    // composes those inside strict C functions. No resource artifact may escape
    // until a public host ownership boundary is defined and proven.
    let mut staged_output = crate::bounded_output::CappedString::new();
    emit_native_prelude(&mut staged_output, resource_abi, program);
    if let Err(diagnostic) =
        emit_function_prototypes(&mut staged_output, program, functions, resource_abi)
    {
        first_failure.get_or_insert(diagnostic);
    }
    first_failure.map_or(Ok(()), Err)
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

const NATIVE_STRING_RUNTIME_C: &str = r#"#include <stdlib.h>
#include <string.h>

static __attribute__((unused)) char *spx_string_from_literal(
    const char *spx_data, uint64_t spx_len
) {
    char *spx_copy = (char *)malloc((size_t)spx_len + 1u);
    if (spx_copy == NULL) spx_runtime_invariant_failure("string allocation failed");
    memcpy(spx_copy, spx_data, (size_t)spx_len);
    spx_copy[spx_len] = '\0';
    return spx_copy;
}

static __attribute__((unused)) char *spx_string_clone(const char *spx_source) {
    return spx_string_from_literal(spx_source, (uint64_t)strlen(spx_source));
}

static __attribute__((unused)) bool spx_string_eq(const char *a, const char *b) {
    return strcmp(a, b) == 0;
}

static __attribute__((unused)) void spx_string_drop(char *spx_value) {
    free(spx_value);
}
"#;

const NATIVE_STRING_OPS_RUNTIME_C: &str = r#"static __attribute__((unused)) int64_t spx_string_len(const char *spx_value) {
    return (int64_t)strlen(spx_value);
}

static __attribute__((unused)) bool spx_string_is_empty(const char *spx_value) {
    return spx_value[0] == '\0';
}

static __attribute__((unused)) char *spx_string_concat(
    char *spx_left, char *spx_right
) {
    uint64_t spx_left_len = (uint64_t)strlen(spx_left);
    uint64_t spx_right_len = (uint64_t)strlen(spx_right);
    char *spx_joined = (char *)malloc((size_t)(spx_left_len + spx_right_len) + 1u);
    if (spx_joined == NULL) spx_runtime_invariant_failure("string allocation failed");
    memcpy(spx_joined, spx_left, (size_t)spx_left_len);
    memcpy(spx_joined + spx_left_len, spx_right, (size_t)spx_right_len);
    spx_joined[spx_left_len + spx_right_len] = '\0';
    return spx_joined;
}
"#;

const NATIVE_STRING_OPS_V2_RUNTIME_C: &str = r#"static __attribute__((unused)) bool spx_string_starts_with(
    const char *spx_value, const char *spx_prefix
) {
    return strncmp(spx_value, spx_prefix, strlen(spx_prefix)) == 0;
}

static __attribute__((unused)) bool spx_string_contains(
    const char *spx_value, const char *spx_needle
) {
    return strstr(spx_value, spx_needle) != NULL;
}

static __attribute__((unused)) int64_t spx_string_len_chars(const char *spx_value) {
    int64_t spx_count = 0;
    for (const unsigned char *spx_cursor = (const unsigned char *)spx_value;
         *spx_cursor != UINT8_C(0);
         ++spx_cursor) {
        if ((*spx_cursor & UINT8_C(0xC0)) != UINT8_C(0x80)) ++spx_count;
    }
    return spx_count;
}

static __attribute__((unused)) char *spx_string_from_char(uint32_t spx_scalar) {
    char spx_encoded[4];
    uint64_t spx_length;
    if (spx_scalar < UINT32_C(0x80)) {
        spx_encoded[0] = (char)(uint8_t)spx_scalar;
        spx_length = UINT64_C(1);
    } else if (spx_scalar < UINT32_C(0x800)) {
        spx_encoded[0] = (char)(uint8_t)(UINT8_C(0xC0) | (uint8_t)(spx_scalar >> 6));
        spx_encoded[1] = (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)(spx_scalar & UINT32_C(0x3F)));
        spx_length = UINT64_C(2);
    } else if (spx_scalar < UINT32_C(0x10000)) {
        spx_encoded[0] = (char)(uint8_t)(UINT8_C(0xE0) | (uint8_t)(spx_scalar >> 12));
        spx_encoded[1] =
            (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)((spx_scalar >> 6) & UINT32_C(0x3F)));
        spx_encoded[2] = (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)(spx_scalar & UINT32_C(0x3F)));
        spx_length = UINT64_C(3);
    } else {
        spx_encoded[0] = (char)(uint8_t)(UINT8_C(0xF0) | (uint8_t)(spx_scalar >> 18));
        spx_encoded[1] =
            (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)((spx_scalar >> 12) & UINT32_C(0x3F)));
        spx_encoded[2] =
            (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)((spx_scalar >> 6) & UINT32_C(0x3F)));
        spx_encoded[3] = (char)(uint8_t)(UINT8_C(0x80) | (uint8_t)(spx_scalar & UINT32_C(0x3F)));
        spx_length = UINT64_C(4);
    }
    return spx_string_from_literal(spx_encoded, spx_length);
}
"#;

const NATIVE_U8_RUNTIME_C: &str = r#"static __attribute__((unused)) spx_status_token spx_rt_u8_add(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    int64_t result = (int64_t)a + (int64_t)b;
    if (result < 0 || result > UINT8_MAX) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, "addition overflow"
        );
    }
    *result_out = (uint8_t)result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_u8_sub(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    int64_t result = (int64_t)a - (int64_t)b;
    if (result < 0 || result > UINT8_MAX) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, "subtraction overflow"
        );
    }
    *result_out = (uint8_t)result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_u8_mul(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    int64_t result = (int64_t)a * (int64_t)b;
    if (result < 0 || result > UINT8_MAX) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, "multiplication overflow"
        );
    }
    *result_out = (uint8_t)result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_u8_div(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, "invalid division"
        );
    }
    *result_out = (uint8_t)((int64_t)a / (int64_t)b);
    return SPX_STATUS_SUCCESS;
}
"#;

struct NativeEmissionContext<'a> {
    program: &'a ResolvedProgram,
    resource_abi: &'a native_resource::NativeResourceAbi,
    functions: &'a HashMap<FunctionExecutionId, CFunction>,
    contract_labels: &'a HashMap<ExpressionId, String>,
    record_layouts: &'a AggregateLayoutCache,
    variant_layouts: &'a VariantLayoutCache,
}

fn emit_function(
    output: &mut impl COutput,
    function: &ResolvedFunction,
    execution: &FunctionExecutionId,
    emission: &NativeEmissionContext<'_>,
) -> Result<(), Diagnostic> {
    let program = emission.program;
    let resource_abi = emission.resource_abi;
    let functions = emission.functions;
    let contract_labels = emission.contract_labels;
    let has_try = expression_has_try(&function.body);
    let metadata = functions
        .get(execution)
        .ok_or_else(|| backend_error(format!("function `{}` is not indexed", function.id)))?;
    write!(
        output,
        "static __attribute__((unused)) spx_status_token {}(struct spx_context *spx_ctx",
        metadata.symbol
    )
    .expect("writing to a string cannot fail");
    for (index, param) in function.params.iter().enumerate() {
        let ty = c_value_type(program, resource_abi, &param.ty)?;
        if is_aggregate_type(program, &param.ty)? {
            write!(output, ", const {ty} *spx_param_{index}")
                .expect("writing to a string cannot fail");
        } else {
            write!(output, ", {ty} spx_param_{index}").expect("writing to a string cannot fail");
        }
    }
    writeln!(
        output,
        ", {} *spx_result_out) {{",
        c_value_type(program, resource_abi, &function.return_type)?
    )
    .expect("writing to a string cannot fail");

    let mut variables = HashMap::new();
    for (index, param) in function.params.iter().enumerate() {
        let name = if is_aggregate_type(program, &param.ty)? {
            format!("(*spx_param_{index})")
        } else {
            format!("spx_param_{index}")
        };
        variables.insert(
            param.id.clone(),
            CBinding {
                name,
                ty: param.ty.clone(),
            },
        );
    }
    let mut emitter = CEmitter::new(output, variables, &function.return_type, emission);
    emitter.line("spx_status_token spx_status = SPX_STATUS_SUCCESS;");
    if has_try {
        emitter.line("bool spx_result_staged = false;");
    }
    emitter.line("(void)spx_ctx;");
    emitter.line(&format!(
        "{} spx_result = {{0}};",
        c_value_type(program, resource_abi, &function.return_type)?
    ));
    for index in 0..function.params.len() {
        emitter.line(&format!("(void)spx_param_{index};"));
    }
    for contract in &function.requires {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_type(&condition.ty, &ResolvedType::Bool, "precondition")?;
        emitter.line(&format!("if (!({})) {{", condition.code));
        emitter.indent += 1;
        emitter.line(&format!(
            "spx_status = spx_rt_contract(spx_ctx, SPX_STATUS_CONTRACT_REQUIRES_FALSE, \"requires\", \"{}\", \"{}\");",
            c_string(&function.name),
            c_string(contract_label(contract, contract_labels))
        ));
        emitter.line("goto spx_epilogue;");
        emitter.indent -= 1;
        emitter.line("}");
    }
    emitter.try_target_enabled = true;
    let body = emitter.emit_expr(&function.body)?;
    emitter.try_target_enabled = false;
    emitter.require_type(&body.ty, &function.return_type, "function body")?;
    emitter.line(&format!("spx_result = {};", body.code));
    if has_try {
        emitter.line("spx_result_staged = true;");
        emitter.label("spx_postconditions");
    }

    emitter.variables.insert(
        function.result_id.clone(),
        CBinding {
            name: "spx_result".to_owned(),
            ty: function.return_type.clone(),
        },
    );
    for contract in &function.ensures {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_type(&condition.ty, &ResolvedType::Bool, "postcondition")?;
        emitter.line(&format!("if (!({})) {{", condition.code));
        emitter.indent += 1;
        emitter.line(&format!(
            "spx_status = spx_rt_contract(spx_ctx, SPX_STATUS_CONTRACT_ENSURES_FALSE, \"ensures\", \"{}\", \"{}\");",
            c_string(&function.name),
            c_string(contract_label(contract, contract_labels))
        ));
        emitter.line("goto spx_epilogue;");
        emitter.indent -= 1;
        emitter.line("}");
    }
    emitter.line("goto spx_epilogue;");
    drop(emitter);
    output.push_str("spx_epilogue:\n");
    // Callee-owned string parameters free their buffers on every exit path;
    // the staged result is handed to the caller instead.
    for (index, param) in function.params.iter().enumerate() {
        if matches!(param.ty, ResolvedType::String) {
            output.push_str(&format!("    spx_string_drop(spx_param_{index});\n"));
        }
    }
    if has_try {
        output.push_str("    if (spx_status == SPX_STATUS_SUCCESS && !spx_result_staged) spx_runtime_invariant_failure(\"unstaged function result\");\n");
    }
    output.push_str("    if (spx_status != SPX_STATUS_SUCCESS) return spx_status;\n");
    output.push_str("    *spx_result_out = spx_result;\n");
    output.push_str("    return SPX_STATUS_SUCCESS;\n");
    output.push_str("}\n\n");
    Ok(())
}

fn contract_label<'a>(
    expression: &'a ResolvedExpr,
    labels: &'a HashMap<ExpressionId, String>,
) -> &'a str {
    labels
        .get(&expression.id)
        .map_or_else(|| expression.id.as_str(), String::as_str)
}

fn expression_has_try(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => true,
        ResolvedExprKind::Call { args, .. } => args.iter().any(expression_has_try),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.iter().any(expression_has_try),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => expression_has_try(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            expression_has_try(left) || expression_has_try(right)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| {
                (0..statement.child_count())
                    .any(|index| statement.child(index).is_some_and(expression_has_try))
            }) || expression_has_try(tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_has_try(condition)
                || expression_has_try(then_branch)
                || expression_has_try(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.iter().any(|field| expression_has_try(&field.value))
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            expression_has_try(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| expression_has_try(guard))
                        || expression_has_try(&arm.value)
                })
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            expression_has_try(base) || fields.iter().any(|field| expression_has_try(&field.value))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_) => false,
    }
}

pub fn build(program: &Program, output: &Path) -> Result<(), Diagnostic> {
    write_and_compile_c(&emit_c(program)?, output)
}

/// Compile one already-emitted C11 projection into a native executable.
///
/// Project v1 publishes its linked entry closure through this boundary so the
/// public native lane consumes exactly the authenticated linked HIR that Web
/// publication and the internal lowering-equivalence evidence consume. This
/// performs no parsing, HIR resolution, or source projection.
pub fn compile_native_executable(c_source: &str, output: &Path) -> Result<(), Diagnostic> {
    write_and_compile_c(c_source, output)
}

fn write_and_compile_c(c_source: &str, output: &Path) -> Result<(), Diagnostic> {
    static BUILD_ID: AtomicU64 = AtomicU64::new(0);
    let build_id = BUILD_ID.fetch_add(1, Ordering::Relaxed);
    let c_path = std::env::temp_dir().join(format!(
        "semaprax-codegen-{}-{build_id}.c",
        std::process::id()
    ));
    std::fs::write(&c_path, c_source).map_err(|error| {
        Diagnostic::io(
            "SPX-I101",
            format!(
                "cannot write temporary C source {}: {error}",
                c_path.display()
            ),
        )
    })?;
    let result = Command::new("clang")
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(&c_path)
        .arg("-o")
        .arg(output)
        .output();
    let _ = std::fs::remove_file(&c_path);
    let result = result.map_err(|error| {
        Diagnostic::io(
            "SPX-B101",
            format!("failed to start clang; install a C11 toolchain: {error}"),
        )
    })?;
    if !result.status.success() {
        return Err(Diagnostic::io(
            "SPX-B102",
            format!(
                "native backend failed:\n{}",
                String::from_utf8_lossy(&result.stderr)
            ),
        ));
    }
    Ok(())
}

#[derive(Clone)]
struct CFunction {
    symbol: String,
    params: Vec<ResolvedType>,
    return_type: ResolvedType,
}

fn function_index(
    program: &ResolvedProgram,
) -> Result<HashMap<FunctionExecutionId, CFunction>, Diagnostic> {
    let mut functions = HashMap::new();
    for function in &program.functions {
        let declaration = program
            .declarations
            .declaration(&function.id)
            .ok_or_else(|| {
                backend_error(format!(
                    "resolved function `{}` has no declaration",
                    function.id
                ))
            })?;
        if declaration.kind != DeclarationKind::Function {
            return Err(backend_error(format!(
                "resolved callable `{}` does not refer to a function declaration",
                function.id
            )));
        }
        let metadata = CFunction {
            symbol: c_function_symbol(&function.id),
            params: function
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect(),
            return_type: function.return_type.clone(),
        };
        if functions
            .insert(
                FunctionExecutionId::Monomorphic(function.id.clone()),
                metadata,
            )
            .is_some()
        {
            return Err(backend_error(format!(
                "duplicate resolved function identity `{}`",
                function.id
            )));
        }
    }
    for instance in &program.function_instances {
        let function = &instance.function;
        let execution = FunctionExecutionId::Generic(instance.id.clone());
        let metadata = CFunction {
            symbol: c_function_execution_symbol(&execution),
            params: function
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect(),
            return_type: function.return_type.clone(),
        };
        if functions.insert(execution, metadata).is_some() {
            return Err(backend_error(format!(
                "duplicate resolved function instance `{}`",
                instance.id
            )));
        }
    }
    Ok(functions)
}

fn c_function_execution_symbol(id: &FunctionExecutionId) -> String {
    let identity = match id {
        FunctionExecutionId::Monomorphic(declaration) => format!(
            "semaprax.function-execution.v1:monomorphic:{}:{}",
            declaration.as_str().len(),
            declaration
        ),
        FunctionExecutionId::Generic(instance) => format!(
            "semaprax.function-execution.v1:generic:{}:{}",
            instance.as_str().len(),
            instance
        ),
    };
    let mut symbol = crate::bounded_output::CappedString::new();
    symbol.push_str("spx_exec_");
    for byte in identity.bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol.into_string()
}

fn c_function_symbol(id: &DeclarationId) -> String {
    let mut symbol = crate::bounded_output::CappedString::new();
    symbol.push_str("spx_decl_");
    for byte in id.as_str().bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol.into_string()
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

/// Refutable Match v1: exact C spelling of a literal pattern, using the same
/// conventions as the matching expression literals.
fn c_pattern_literal(value: hir::PatternValue) -> String {
    match value {
        hir::PatternValue::Int(value) => c_i64(value),
        hir::PatternValue::Int32(value) => format!("INT32_C({value})"),
        hir::PatternValue::Uint8(value) => format!("UINT8_C({value})"),
        hir::PatternValue::Char(value) => format!("UINT32_C(0x{value:x})"),
        hir::PatternValue::Bool(value) => value.to_string(),
    }
}

#[derive(Clone)]
struct CBinding {
    name: String,
    ty: ResolvedType,
}

struct CValue {
    code: String,
    ty: ResolvedType,
}

struct CEmitter<'a, O: COutput> {
    output: &'a mut O,
    program: &'a ResolvedProgram,
    resource_abi: &'a native_resource::NativeResourceAbi,
    variables: HashMap<ValueId, CBinding>,
    functions: &'a HashMap<FunctionExecutionId, CFunction>,
    record_layouts: &'a AggregateLayoutCache,
    variant_layouts: &'a VariantLayoutCache,
    return_type: &'a ResolvedType,
    try_target_enabled: bool,
    next_local: usize,
    indent: usize,
}

impl<'a, O: COutput> CEmitter<'a, O> {
    fn new(
        output: &'a mut O,
        variables: HashMap<ValueId, CBinding>,
        return_type: &'a ResolvedType,
        emission: &'a NativeEmissionContext<'a>,
    ) -> Self {
        Self {
            output,
            program: emission.program,
            resource_abi: emission.resource_abi,
            variables,
            functions: emission.functions,
            record_layouts: emission.record_layouts,
            variant_layouts: emission.variant_layouts,
            return_type,
            try_target_enabled: false,
            next_local: 0,
            indent: 1,
        }
    }

    fn line(&mut self, value: &str) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        writeln!(self.output, "{value}").expect("writing to a string cannot fail");
    }

    fn label(&mut self, value: &str) {
        writeln!(self.output, "{value}: ;").expect("writing to a string cannot fail");
    }

    fn temporary(&mut self, ty: &ResolvedType) -> Result<String, Diagnostic> {
        let name = format!("spx_internal_{}", self.next_local);
        self.next_local += 1;
        self.line(&format!(
            "{} {name};",
            c_value_type(self.program, self.resource_abi, ty)?
        ));
        Ok(name)
    }

    fn require_type(
        &self,
        actual: &ResolvedType,
        expected: &ResolvedType,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if actual == expected {
            Ok(())
        } else {
            Err(backend_error(format!(
                "inconsistent HIR type for {context}: expected `{}`, found `{}`",
                expected.identity_key(),
                actual.identity_key()
            )))
        }
    }

    /// Refutable Match v1 native lowering: the scrutinee stages once, then
    /// every arm tests `!matched && (<literal equality>)` with an optional
    /// inner guard branch. `&&` short-circuits so a guard evaluates exactly
    /// once per reached arm whose pattern matched; failing guards leave
    /// `matched` false and fall through to the following arms. The resolver
    /// guarantees one trailing irrefutable guard-free catch-all, but the
    /// defensive no-arm check mirrors exhaustive matches.
    fn emit_scalar_match(
        &mut self,
        expr: &ResolvedExpr,
        scrutinee: &CValue,
        arms: &[hir::ResolvedMatchArm],
    ) -> Result<CValue, Diagnostic> {
        let staged = self.temporary(&scrutinee.ty)?;
        self.line(&format!("{staged} = {};", scrutinee.code));
        let result = self.temporary(&expr.ty)?;
        let matched = self.temporary(&ResolvedType::Bool)?;
        self.line(&format!("{matched} = false;"));
        for arm in arms {
            let saved = self.variables.clone();
            if let hir::ResolvedMatchPattern::Binding(binding) = &arm.pattern {
                self.variables.insert(
                    binding.id.clone(),
                    CBinding {
                        name: staged.clone(),
                        ty: binding.ty.clone(),
                    },
                );
            }
            let test = match &arm.pattern {
                hir::ResolvedMatchPattern::Wildcard | hir::ResolvedMatchPattern::Binding(_) => None,
                hir::ResolvedMatchPattern::Literal(value) => {
                    Some(format!("{staged} == {}", c_pattern_literal(*value)))
                }
                hir::ResolvedMatchPattern::Or(alternatives) => Some(
                    alternatives
                        .iter()
                        .map(|alternative| match alternative {
                            hir::ResolvedMatchPattern::Literal(value) => {
                                format!("{staged} == {}", c_pattern_literal(*value))
                            }
                            _ => unreachable!("or-pattern alternatives are literals"),
                        })
                        .collect::<Vec<_>>()
                        .join(" || "),
                ),
                hir::ResolvedMatchPattern::Variant { .. }
                | hir::ResolvedMatchPattern::Record { .. } => {
                    return Err(backend_error(
                        "aggregate pattern has a Copy-scalar match scrutinee",
                    ));
                }
            };
            match &test {
                Some(test) => self.line(&format!("if (!{matched} && ({test})) {{")),
                None => self.line(&format!("if (!{matched}) {{")),
            }
            self.indent += 1;
            if let Some(guard) = &arm.guard {
                // The guard evaluates once here, after the pattern matched
                // and before any part of the arm value; a false guard leaves
                // `matched` untouched and falls through to the next arm.
                let flag = self.emit_expr(guard)?;
                self.require_type(&flag.ty, &ResolvedType::Bool, "match guard")?;
                self.line(&format!("if ({}) {{", flag.code));
                self.indent += 1;
                self.line(&format!("{matched} = true;"));
                let value = self.emit_expr(&arm.value)?;
                self.require_type(&value.ty, &expr.ty, "match arm result")?;
                self.line(&format!("{result} = {};", value.code));
                self.indent -= 1;
                self.line("}");
            } else {
                self.line(&format!("{matched} = true;"));
                let value = self.emit_expr(&arm.value)?;
                self.require_type(&value.ty, &expr.ty, "match arm result")?;
                self.line(&format!("{result} = {};", value.code));
            }
            self.variables = saved;
            self.indent -= 1;
            self.line("}");
        }
        self.line(&format!(
            "if (!{matched}) spx_runtime_invariant_failure(\"refutable match selected no arm\");"
        ));
        Ok(CValue {
            code: result,
            ty: expr.ty.clone(),
        })
    }

    fn emit_string_op(
        &mut self,
        op: crate::string_ops::StringOp,
        args: &[ResolvedExpr],
        result_type: &ResolvedType,
    ) -> Result<CValue, Diagnostic> {
        // Arguments stage left-to-right; every argument evaluation yields a
        // fresh caller-owned buffer, and consuming operations free their
        // inputs exactly at the operation site like owned string equality.
        let mut arguments = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            let value = self.emit_expr(argument)?;
            self.require_type(
                &value.ty,
                &op.param_types()[index],
                "string operation argument",
            )?;
            arguments.push(value);
        }
        self.require_type(result_type, &op.return_type(), "string operation result")?;
        let temporary = self.temporary(&op.return_type())?;
        match op {
            crate::string_ops::StringOp::Len => {
                let input = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_len({input});"));
                self.line(&format!("spx_string_drop({input});"));
            }
            crate::string_ops::StringOp::IsEmpty => {
                let input = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_is_empty({input});"));
                self.line(&format!("spx_string_drop({input});"));
            }
            crate::string_ops::StringOp::Concat => {
                let left = &arguments[0].code;
                let right = &arguments[1].code;
                self.line(&format!(
                    "{temporary} = spx_string_concat({left}, {right});"
                ));
                self.line(&format!("spx_string_drop({left});"));
                self.line(&format!("spx_string_drop({right});"));
            }
            crate::string_ops::StringOp::StartsWith => {
                let value = &arguments[0].code;
                let prefix = &arguments[1].code;
                self.line(&format!(
                    "{temporary} = spx_string_starts_with({value}, {prefix});"
                ));
                self.line(&format!("spx_string_drop({value});"));
                self.line(&format!("spx_string_drop({prefix});"));
            }
            crate::string_ops::StringOp::Contains => {
                let value = &arguments[0].code;
                let needle = &arguments[1].code;
                self.line(&format!(
                    "{temporary} = spx_string_contains({value}, {needle});"
                ));
                self.line(&format!("spx_string_drop({value});"));
                self.line(&format!("spx_string_drop({needle});"));
            }
            crate::string_ops::StringOp::LenChars => {
                let input = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_len_chars({input});"));
                self.line(&format!("spx_string_drop({input});"));
            }
            crate::string_ops::StringOp::FromChar => {
                let scalar = &arguments[0].code;
                self.line(&format!("{temporary} = spx_string_from_char({scalar});"));
            }
        }
        Ok(CValue {
            code: temporary,
            ty: op.return_type(),
        })
    }

    fn emit_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::Int(value) => {
                self.require_type(&expr.ty, &ResolvedType::I64, "integer literal")?;
                CValue {
                    code: c_i64(*value),
                    ty: ResolvedType::I64,
                }
            }
            ResolvedExprKind::Int32(value) => {
                self.require_type(&expr.ty, &ResolvedType::I32, "i32 literal")?;
                CValue {
                    code: format!("INT32_C({value})"),
                    ty: ResolvedType::I32,
                }
            }
            ResolvedExprKind::Char(value) => {
                self.require_type(&expr.ty, &ResolvedType::Char, "char literal")?;
                CValue {
                    code: format!("UINT32_C(0x{value:x})"),
                    ty: ResolvedType::Char,
                }
            }
            ResolvedExprKind::Uint8(value) => {
                self.require_type(&expr.ty, &ResolvedType::U8, "u8 literal")?;
                CValue {
                    code: format!("UINT8_C({value})"),
                    ty: ResolvedType::U8,
                }
            }
            ResolvedExprKind::Float32(bits) => {
                self.require_type(&expr.ty, &ResolvedType::F32, "float literal")?;
                CValue {
                    code: format!("{}f", crate::format::canonical_f32_bits(*bits)),
                    ty: ResolvedType::F32,
                }
            }
            ResolvedExprKind::Float64(bits) => {
                self.require_type(&expr.ty, &ResolvedType::F64, "float literal")?;
                CValue {
                    code: crate::format::canonical_f64_bits(*bits),
                    ty: ResolvedType::F64,
                }
            }
            ResolvedExprKind::Bool(value) => {
                self.require_type(&expr.ty, &ResolvedType::Bool, "boolean literal")?;
                CValue {
                    code: value.to_string(),
                    ty: ResolvedType::Bool,
                }
            }
            ResolvedExprKind::String(value) => {
                self.require_type(&expr.ty, &ResolvedType::String, "string literal")?;
                let temporary = self.temporary(&ResolvedType::String)?;
                self.line(&format!(
                    "{temporary} = spx_string_from_literal(\"{}\", UINT64_C({}));",
                    c_string(value),
                    value.len()
                ));
                CValue {
                    code: temporary,
                    ty: ResolvedType::String,
                }
            }
            ResolvedExprKind::Place(place) => {
                let value = self.emit_place(place)?;
                self.require_type(&expr.ty, &value.ty, "place expression")?;
                // Every read of an owned string place yields a fresh buffer so
                // the source place keeps its unique owner.
                if matches!(value.ty, ResolvedType::String) {
                    let temporary = self.temporary(&ResolvedType::String)?;
                    self.line(&format!("{temporary} = spx_string_clone({});", value.code));
                    return Ok(CValue {
                        code: temporary,
                        ty: value.ty,
                    });
                }
                value
            }
            ResolvedExprKind::Call {
                callee,
                instance,
                args,
                ..
            } => {
                if instance.is_none() {
                    if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                        return self.emit_string_op(op, args, &expr.ty);
                    }
                }
                let execution = instance.as_ref().map_or_else(
                    || FunctionExecutionId::Monomorphic(callee.clone()),
                    |instance| FunctionExecutionId::Generic(instance.clone()),
                );
                let target = self.functions.get(&execution).ok_or_else(|| {
                    backend_error(format!("resolved callee `{callee}` is not indexed"))
                })?;
                if args.len() != target.params.len() {
                    return Err(backend_error(format!(
                        "resolved call to `{callee}` has {} arguments; expected {}",
                        args.len(),
                        target.params.len()
                    )));
                }
                let target = target.clone();
                let mut arguments = Vec::with_capacity(args.len());
                for (index, (arg, expected)) in args.iter().zip(&target.params).enumerate() {
                    let argument = self.emit_expr(arg)?;
                    self.require_type(&argument.ty, expected, &format!("call argument {index}"))?;
                    arguments.push(if is_aggregate_type(self.program, expected)? {
                        format!("&({})", argument.code)
                    } else {
                        argument.code
                    });
                }
                self.require_type(&expr.ty, &target.return_type, "call result")?;
                let temporary = self.temporary(&target.return_type)?;
                self.line(&format!(
                    "spx_status = {}(spx_ctx{}{}, &{temporary});",
                    target.symbol,
                    if arguments.is_empty() { "" } else { ", " },
                    arguments.budgeted_join(", ")
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                CValue {
                    code: temporary,
                    ty: target.return_type,
                }
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                return Err(backend_error(format!(
                    "native Rust import `{}` is unavailable in the ordinary native backend",
                    call.import
                )));
            }
            ResolvedExprKind::Unary { op, value } => {
                let value = self.emit_expr(value)?;
                let (ty, operand_type) = match op {
                    UnaryOp::Neg => match &value.ty {
                        ResolvedType::F32 => (ResolvedType::F32, ResolvedType::F32),
                        ResolvedType::F64 => (ResolvedType::F64, ResolvedType::F64),
                        ResolvedType::I32 => (ResolvedType::I32, ResolvedType::I32),
                        _ => (ResolvedType::I64, ResolvedType::I64),
                    },
                    UnaryOp::Not => (ResolvedType::Bool, ResolvedType::Bool),
                };
                self.require_type(&value.ty, &operand_type, "unary operand")?;
                self.require_type(&expr.ty, &ty, "unary result")?;
                let temporary = self.temporary(&ty)?;
                match op {
                    UnaryOp::Neg if matches!(ty, ResolvedType::F32 | ResolvedType::F64) => {
                        self.line(&format!("{temporary} = (-({}));", value.code));
                    }
                    UnaryOp::Neg if ty == ResolvedType::I32 => {
                        self.line(&format!(
                            "spx_status = spx_rt_neg_i32(spx_ctx, {}, &{temporary});",
                            value.code
                        ));
                        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                    }
                    UnaryOp::Neg => {
                        self.line(&format!(
                            "spx_status = spx_rt_neg(spx_ctx, {}, &{temporary});",
                            value.code
                        ));
                        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                    }
                    UnaryOp::Not => self.line(&format!("{temporary} = (!{});", value.code)),
                }
                CValue {
                    code: temporary,
                    ty,
                }
            }
            ResolvedExprKind::Binary { op, left, right } => {
                return self.emit_binary(*op, left, right, &expr.ty);
            }
            ResolvedExprKind::Block { statements, tail } => {
                let saved = self.variables.clone();
                for statement in statements {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            let value = self.emit_expr(value)?;
                            self.require_type(&value.ty, &binding.ty, "local binding")?;
                            let local = format!("spx_local_{}", self.next_local);
                            self.next_local += 1;
                            self.line(&format!(
                                "{} {local} = {};",
                                c_value_type(self.program, self.resource_abi, &binding.ty)?,
                                value.code
                            ));
                            if self
                                .variables
                                .insert(
                                    binding.id.clone(),
                                    CBinding {
                                        name: local,
                                        ty: binding.ty.clone(),
                                    },
                                )
                                .is_some()
                            {
                                return Err(backend_error(format!(
                                    "duplicate resolved local identity `{}`",
                                    binding.id
                                )));
                            }
                        }
                        ResolvedStatement::Assign {
                            binding,
                            field,
                            value: assigned,
                            ..
                        } => {
                            // The assigned value is emitted fully first; the
                            // store is a plain C11 assignment into the local
                            // or, for Field Mutation v1, into its one direct
                            // scalar field.
                            let value = self.emit_expr(assigned)?;
                            match field {
                                Some(field_id) => {
                                    let layout = self.record_layout(&binding.ty)?;
                                    let field =
                                        layout.field(field_id).cloned().ok_or_else(|| {
                                            backend_error(format!(
                                                "native record `{}` has no assignment field `{field_id}`",
                                                layout.record
                                            ))
                                        })?;
                                    self.require_type(&value.ty, &field.ty, "field assignment")?;
                                    if matches!(field.ty, ResolvedType::String) {
                                        return Err(backend_error(
                                            "string field assignment has no admitted native lowering",
                                        ));
                                    }
                                    let target =
                                        self.variables.get(&binding.id).ok_or_else(|| {
                                            backend_error(format!(
                                                "assignment target `{}` has no native local",
                                                binding.id
                                            ))
                                        })?;
                                    self.line(&format!(
                                        "{}.{} = {};",
                                        target.name,
                                        c_field_symbol(&field.field),
                                        value.code
                                    ));
                                }
                                None => {
                                    self.require_type(&value.ty, &binding.ty, "assignment")?;
                                    if matches!(binding.ty, ResolvedType::String) {
                                        return Err(backend_error(
                                            "string assignment has no admitted native lowering",
                                        ));
                                    }
                                    let target =
                                        self.variables.get(&binding.id).ok_or_else(|| {
                                            backend_error(format!(
                                                "assignment target `{}` has no native local",
                                                binding.id
                                            ))
                                        })?;
                                    self.line(&format!("{} = {};", target.name, value.code));
                                }
                            }
                        }
                        ResolvedStatement::Unsafe { body, .. } => {
                            // Backends treat the boundary transparently: emit
                            // exactly the ordinary block body and discard its
                            // scalar Copy result.
                            let value = self.emit_expr(body)?;
                            if matches!(value.ty, ResolvedType::String) {
                                return Err(backend_error(
                                    "discarding an owned string has no admitted native lowering",
                                ));
                            }
                            self.line(&format!("(void)({});", value.code));
                        }
                        ResolvedStatement::While {
                            condition, body, ..
                        } => {
                            // Bounded While-Loops v1 lowers to a native C11
                            // loop. Because checked sub-expressions lower to
                            // statements, the condition re-evaluates at the
                            // top of every iteration and breaks out on false;
                            // checked-arithmetic failures inside the loop jump
                            // to the shared epilogue exactly like
                            // straight-line failures.
                            self.line("for (;;) {");
                            self.indent += 1;
                            let condition = self.emit_expr(condition)?;
                            self.require_type(
                                &condition.ty,
                                &ResolvedType::Bool,
                                "while condition",
                            )?;
                            self.line(&format!("if (!({})) break;", condition.code));
                            let body_value = self.emit_expr(body)?;
                            if matches!(body_value.ty, ResolvedType::String) {
                                return Err(backend_error(
                                    "discarding an owned string has no admitted native lowering",
                                ));
                            }
                            self.line(&format!("(void)({});", body_value.code));
                            self.indent -= 1;
                            self.line("}");
                        }
                    }
                }
                let tail = self.emit_expr(tail)?;
                self.require_type(&tail.ty, &expr.ty, "block result")?;
                // Owned string locals introduced in this block free exactly
                // their own buffer when the block exits; outer bindings and
                // the tail value are untouched. The order is sorted so the
                // projection stays byte-deterministic.
                let mut introduced: Vec<String> = self
                    .variables
                    .iter()
                    .filter(|(id, binding)| {
                        matches!(binding.ty, ResolvedType::String) && !saved.contains_key(*id)
                    })
                    .map(|(_, binding)| binding.name.clone())
                    .collect();
                introduced.sort();
                for name in introduced {
                    self.line(&format!("spx_string_drop({name});"));
                }
                self.variables = saved;
                tail
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.emit_expr(condition)?;
                self.require_type(&condition.ty, &ResolvedType::Bool, "if condition")?;
                let temporary = self.temporary(&expr.ty)?;
                self.line(&format!("if ({}) {{", condition.code));
                self.indent += 1;
                let then_value = self.emit_expr(then_branch)?;
                self.require_type(&then_value.ty, &expr.ty, "then branch")?;
                self.line(&format!("{temporary} = {};", then_value.code));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                let else_value = self.emit_expr(else_branch)?;
                self.require_type(&else_value.ty, &expr.ty, "else branch")?;
                self.line(&format!("{temporary} = {};", else_value.code));
                self.indent -= 1;
                self.line("}");
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::ConstructRecord { record, fields } => {
                let layout = self.record_layout(&expr.ty)?;
                if layout.record != *record {
                    return Err(backend_error(format!(
                        "native record constructor `{record}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let temporary = self.temporary(&expr.ty)?;
                if layout.fields.is_empty() {
                    self.line(&format!(
                        "{temporary}.spx_empty_record_padding = UINT8_C(0);"
                    ));
                }
                for initializer in fields {
                    let field = layout.field(&initializer.field).cloned().ok_or_else(|| {
                        backend_error(format!(
                            "native record `{record}` has no field `{}`",
                            initializer.field
                        ))
                    })?;
                    let value = self.emit_expr(&initializer.value)?;
                    self.require_type(&value.ty, &field.ty, "record field initializer")?;
                    self.line(&format!(
                        "{temporary}.{} = {};",
                        c_field_symbol(&field.field),
                        value.code
                    ));
                }
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                let layout = self.variant_layout(&expr.ty)?;
                if layout.variant != *variant {
                    return Err(backend_error(format!(
                        "native variant constructor `{variant}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let case_layout = layout.case(case).cloned().ok_or_else(|| {
                    backend_error(format!("native variant `{variant}` has no case `{case}`"))
                })?;
                let mut values = Vec::with_capacity(fields.len());
                for initializer in fields {
                    let field =
                        case_layout
                            .field(&initializer.field)
                            .cloned()
                            .ok_or_else(|| {
                                backend_error(format!(
                                    "native variant case `{case}` has no field `{}`",
                                    initializer.field
                                ))
                            })?;
                    let value = self.emit_expr(&initializer.value)?;
                    self.require_type(&value.ty, &field.ty, "variant field initializer")?;
                    values.push((field, value));
                }
                let temporary = self.temporary(&expr.ty)?;
                self.line(&format!("memset(&{temporary}, 0, sizeof({temporary}));"));
                let case_symbol = c_case_symbol(case);
                for (field, value) in values {
                    self.line(&format!(
                        "{temporary}.spx_payload.{case_symbol}.{} = {};",
                        c_field_symbol(&field.field),
                        value.code
                    ));
                }
                self.line(&format!(
                    "{temporary}.spx_tag = UINT32_C({});",
                    case_layout.tag
                ));
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                if is_aggregate_type(self.program, &expr.ty)? {
                    return Err(backend_error("copy match arms must produce i64 or bool"));
                }
                let scrutinee = self.emit_expr(scrutinee)?;
                // Refutable Match v1: Copy-scalar scrutinees lower to the
                // literal/guard decision chain; aggregates keep the exact
                // pre-feature lowering below.
                if matches!(
                    scrutinee.ty,
                    ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::U8
                        | ResolvedType::Char
                        | ResolvedType::Bool
                ) {
                    return self.emit_scalar_match(expr, &scrutinee, arms);
                }
                if let Some(record) = record_declaration_id(self.program, &scrutinee.ty)?.cloned() {
                    let [arm] = arms.as_slice() else {
                        return Err(backend_error(
                            "irrefutable record match must have exactly one arm",
                        ));
                    };
                    let staged = self.temporary(&scrutinee.ty)?;
                    self.line(&format!("{staged} = {};", scrutinee.code));
                    let saved = self.variables.clone();
                    match &arm.pattern {
                        hir::ResolvedMatchPattern::Wildcard => {}
                        hir::ResolvedMatchPattern::Record {
                            record: pattern_record,
                            instance,
                            fields,
                        } => self.bind_record_match_pattern(
                            &staged,
                            &scrutinee.ty,
                            pattern_record,
                            instance,
                            fields,
                        )?,
                        hir::ResolvedMatchPattern::Variant { .. } => {
                            return Err(backend_error(
                                "variant pattern has a record match scrutinee",
                            ));
                        }
                        hir::ResolvedMatchPattern::Literal(_)
                        | hir::ResolvedMatchPattern::Or(_)
                        | hir::ResolvedMatchPattern::Binding(_) => {
                            return Err(backend_error(
                                "refutable pattern has an aggregate record match scrutinee",
                            ));
                        }
                    }
                    if record_declaration_id(self.program, &scrutinee.ty)? != Some(&record) {
                        return Err(backend_error(
                            "record match scrutinee identity changed during lowering",
                        ));
                    }
                    let value = self.emit_expr(&arm.value)?;
                    self.require_type(&value.ty, &expr.ty, "record match arm result")?;
                    self.variables = saved;
                    return Ok(CValue {
                        code: value.code,
                        ty: expr.ty.clone(),
                    });
                }
                let layout = self.variant_layout(&scrutinee.ty)?;
                let staged = self.temporary(&scrutinee.ty)?;
                self.line(&format!("{staged} = {};", scrutinee.code));
                self.line(&format!(
                    "if ({staged}.spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid variant tag\");",
                    layout.cases.len()
                ));
                let result = self.temporary(&expr.ty)?;
                let matched = self.temporary(&ResolvedType::Bool)?;
                self.line(&format!("{matched} = false;"));
                for arm in arms {
                    let saved = self.variables.clone();
                    match &arm.pattern {
                        hir::ResolvedMatchPattern::Variant {
                            variant,
                            case,
                            fields,
                        } => {
                            if *variant != layout.variant {
                                return Err(backend_error(format!(
                                    "match arm variant `{variant}` disagrees with `{}`",
                                    layout.variant
                                )));
                            }
                            let case_layout = layout.case(case).cloned().ok_or_else(|| {
                                backend_error(format!("match arm references unknown case `{case}`"))
                            })?;
                            self.line(&format!(
                                "if (!{matched} && {staged}.spx_tag == UINT32_C({})) {{",
                                case_layout.tag
                            ));
                            self.indent += 1;
                            self.line(&format!("{matched} = true;"));
                            let case_symbol = c_case_symbol(case);
                            for pattern_field in fields {
                                let field = case_layout
                                    .field(&pattern_field.field)
                                    .cloned()
                                    .ok_or_else(|| {
                                        backend_error(format!(
                                            "match case `{case}` has no field `{}`",
                                            pattern_field.field
                                        ))
                                    })?;
                                self.require_type(
                                    &pattern_field.binding.ty,
                                    &field.ty,
                                    "match payload binding",
                                )?;
                                self.variables.insert(
                                    pattern_field.binding.id.clone(),
                                    CBinding {
                                        name: format!(
                                            "({staged}).spx_payload.{case_symbol}.{}",
                                            c_field_symbol(&field.field)
                                        ),
                                        ty: field.ty,
                                    },
                                );
                            }
                        }
                        hir::ResolvedMatchPattern::Wildcard => {
                            self.line(&format!("if (!{matched}) {{"));
                            self.indent += 1;
                            self.line(&format!("{matched} = true;"));
                        }
                        hir::ResolvedMatchPattern::Record { .. } => {
                            return Err(backend_error(
                                "record pattern has a variant match scrutinee",
                            ));
                        }
                        hir::ResolvedMatchPattern::Literal(_)
                        | hir::ResolvedMatchPattern::Or(_)
                        | hir::ResolvedMatchPattern::Binding(_) => {
                            return Err(backend_error(
                                "refutable pattern has an aggregate variant match scrutinee",
                            ));
                        }
                    }
                    let value = self.emit_expr(&arm.value)?;
                    self.require_type(&value.ty, &expr.ty, "match arm result")?;
                    self.line(&format!("{result} = {};", value.code));
                    self.variables = saved;
                    self.indent -= 1;
                    self.line("}");
                }
                self.line(&format!(
                    "if (!{matched}) spx_runtime_invariant_failure(\"exhaustive variant match selected no arm\");"
                ));
                CValue {
                    code: result,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::Try {
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            } => {
                if !self.try_target_enabled {
                    return Err(backend_error(
                        "copy-result propagation is allowed only in a function body",
                    ));
                }
                self.require_type(
                    residual_type,
                    self.return_type,
                    "copy-result residual target",
                )?;
                let operand_layout = self.variant_layout(&operand.ty)?;
                let residual_layout = self.variant_layout(residual_type)?;
                if operand_layout.variant != *result || residual_layout.variant != *result {
                    return Err(backend_error(
                        "copy-result propagation does not reference its resolved Result declaration",
                    ));
                }
                let operand_ok = operand_layout
                    .case(ok_case)
                    .and_then(|case| case.field(ok_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-result propagation has no resolved Ok payload")
                    })?;
                let operand_err = operand_layout
                    .case(err_case)
                    .and_then(|case| case.field(err_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-result propagation has no resolved Err payload")
                    })?;
                let residual_err = residual_layout
                    .case(err_case)
                    .and_then(|case| case.field(err_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-result residual has no resolved Err payload")
                    })?;
                self.require_type(&operand_ok.1.ty, &expr.ty, "copy-result Ok payload")?;
                self.require_type(
                    &operand_err.1.ty,
                    &residual_err.1.ty,
                    "copy-result Err payload",
                )?;

                let operand_value = self.emit_expr(operand)?;
                self.require_type(&operand_value.ty, &operand.ty, "copy-result operand")?;
                let operand_stage = self.temporary(&operand.ty)?;
                self.line(&format!("{operand_stage} = {};", operand_value.code));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid variant tag\");",
                    operand_layout.cases.len()
                ));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag == UINT32_C({})) {{",
                    operand_err.0.tag
                ));
                self.indent += 1;
                self.line("memset(&spx_result, 0, sizeof(spx_result));");
                self.line(&format!(
                    "spx_result.spx_payload.{}.{} = {operand_stage}.spx_payload.{}.{};",
                    c_case_symbol(err_case),
                    c_field_symbol(err_field),
                    c_case_symbol(err_case),
                    c_field_symbol(err_field),
                ));
                self.line(&format!(
                    "spx_result.spx_tag = UINT32_C({});",
                    residual_err.0.tag
                ));
                self.line("spx_result_staged = true;");
                self.line("goto spx_postconditions;");
                self.indent -= 1;
                self.line("}");
                self.line(&format!(
                    "if ({operand_stage}.spx_tag != UINT32_C({})) spx_runtime_invariant_failure(\"invalid Result tag\");",
                    operand_ok.0.tag
                ));
                let output = self.temporary(&expr.ty)?;
                self.line(&format!(
                    "{output} = {operand_stage}.spx_payload.{}.{};",
                    c_case_symbol(ok_case),
                    c_field_symbol(ok_field),
                ));
                CValue {
                    code: output,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::TryOption {
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
            } => {
                if !self.try_target_enabled {
                    return Err(backend_error(
                        "copy-Option propagation is allowed only in a function body",
                    ));
                }
                self.require_type(
                    residual_type,
                    self.return_type,
                    "copy-Option residual target",
                )?;
                let operand_layout = self.variant_layout(&operand.ty)?;
                let residual_layout = self.variant_layout(residual_type)?;
                if operand_layout.variant != *option || residual_layout.variant != *option {
                    return Err(backend_error(
                        "copy-Option propagation does not reference its resolved Option declaration",
                    ));
                }
                let operand_some = operand_layout
                    .case(some_case)
                    .and_then(|case| case.field(some_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-Option propagation has no resolved Some payload")
                    })?;
                let operand_none = operand_layout.case(none_case).ok_or_else(|| {
                    backend_error("copy-Option propagation has no resolved None case")
                })?;
                let residual_none = residual_layout.case(none_case).ok_or_else(|| {
                    backend_error("copy-Option residual has no resolved None case")
                })?;
                if !operand_none.fields.is_empty() || !residual_none.fields.is_empty() {
                    return Err(backend_error(
                        "copy-Option None case unexpectedly has a payload",
                    ));
                }
                self.require_type(&operand_some.1.ty, &expr.ty, "copy-Option Some payload")?;

                let operand_value = self.emit_expr(operand)?;
                self.require_type(&operand_value.ty, &operand.ty, "copy-Option operand")?;
                let operand_stage = self.temporary(&operand.ty)?;
                self.line(&format!("{operand_stage} = {};", operand_value.code));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid variant tag\");",
                    operand_layout.cases.len()
                ));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag == UINT32_C({})) {{",
                    operand_none.tag
                ));
                self.indent += 1;
                self.line("memset(&spx_result, 0, sizeof(spx_result));");
                self.line(&format!(
                    "spx_result.spx_tag = UINT32_C({});",
                    residual_none.tag
                ));
                self.line("spx_result_staged = true;");
                self.line("goto spx_postconditions;");
                self.indent -= 1;
                self.line("}");
                self.line(&format!(
                    "if ({operand_stage}.spx_tag != UINT32_C({})) spx_runtime_invariant_failure(\"invalid Option tag\");",
                    operand_some.0.tag
                ));
                let output = self.temporary(&expr.ty)?;
                self.line(&format!(
                    "{output} = {operand_stage}.spx_payload.{}.{};",
                    c_case_symbol(some_case),
                    c_field_symbol(some_field),
                ));
                CValue {
                    code: output,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::Project { base, field } => {
                let base = self.emit_expr(base)?;
                let layout = self.record_layout(&base.ty)?;
                let field = layout.field(field).cloned().ok_or_else(|| {
                    backend_error(format!(
                        "native record `{}` has no projected field `{field}`",
                        layout.record
                    ))
                })?;
                self.require_type(&expr.ty, &field.ty, "record projection")?;
                CValue {
                    code: format!("({}).{}", base.code, c_field_symbol(&field.field)),
                    ty: field.ty,
                }
            }
            ResolvedExprKind::Upcast { source } => {
                // Class Inheritance v1: the ancestor prefix moves
                // field-by-field from the consumed descendant value; the
                // canonical layouts guarantee identical offsets.
                let source = self.emit_expr(source)?;
                let target_layout = self.record_layout(&expr.ty)?;
                let source_layout = self.record_layout(&source.ty)?;
                if target_layout.record == source_layout.record {
                    return Err(backend_error(format!(
                        "native upcast `{}` requires a descendant source",
                        expr.ty.identity_key()
                    )));
                }
                for field in &target_layout.fields {
                    let source_field =
                        source_layout.field(&field.field).cloned().ok_or_else(|| {
                            backend_error(format!(
                                "native upcast source `{}` has no inherited field `{}`",
                                source_layout.record, field.field
                            ))
                        })?;
                    if (source_field.offset, source_field.size, source_field.align)
                        != (field.offset, field.size, field.align)
                    {
                        return Err(backend_error(format!(
                            "native upcast field `{}` disagrees with the ancestor prefix layout",
                            field.field
                        )));
                    }
                }
                let temporary = self.temporary(&expr.ty)?;
                if target_layout.fields.is_empty() {
                    self.line(&format!(
                        "{temporary}.spx_empty_record_padding = UINT8_C(0);"
                    ));
                }
                for field in &target_layout.fields {
                    self.line(&format!(
                        "{temporary}.{} = ({}).{};",
                        c_field_symbol(&field.field),
                        source.code,
                        c_field_symbol(&field.field)
                    ));
                }
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                let base = self.emit_expr(base)?;
                self.require_type(&base.ty, &expr.ty, "record update base")?;
                let layout = self.record_layout(&expr.ty)?;
                if layout.record != *record {
                    return Err(backend_error(format!(
                        "native record update `{record}` has result type `{}`",
                        expr.ty.identity_key()
                    )));
                }
                let temporary = self.temporary(&expr.ty)?;
                self.line(&format!("{temporary} = {};", base.code));
                for replacement in fields {
                    let field = layout.field(&replacement.field).cloned().ok_or_else(|| {
                        backend_error(format!(
                            "native record `{record}` has no update field `{}`",
                            replacement.field
                        ))
                    })?;
                    let value = self.emit_expr(&replacement.value)?;
                    self.require_type(&value.ty, &field.ty, "record update field")?;
                    self.line(&format!(
                        "{temporary}.{} = {};",
                        c_field_symbol(&field.field),
                        value.code
                    ));
                }
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    fn emit_place(&self, place: &hir::Place) -> Result<CValue, Diagnostic> {
        let binding = self.variables.get(&place.root).cloned().ok_or_else(|| {
            backend_error(format!("resolved value `{}` is not in scope", place.root))
        })?;
        let mut code = binding.name;
        let mut ty = binding.ty;
        for projection in &place.projections {
            let PlaceProjection::Field(field) = projection else {
                return Err(backend_error(
                    "native variant-field projection is outside executable records v1",
                ));
            };
            let layout = self.record_layout(&ty)?;
            let field = layout.field(field).cloned().ok_or_else(|| {
                backend_error(format!(
                    "native record `{}` has no place field `{field}`",
                    layout.record
                ))
            })?;
            code = format!("({code}).{}", c_field_symbol(&field.field));
            ty = field.ty;
        }
        Ok(CValue { code, ty })
    }

    fn record_layout(&self, ty: &ResolvedType) -> Result<AggregateLayout, Diagnostic> {
        record_declaration_id(self.program, ty)?.ok_or_else(|| {
            backend_error(format!(
                "native aggregate operation requires a record, found `{}`",
                ty.identity_key()
            ))
        })?;
        let layout = self.record_layouts.layout(ty)?.clone();
        layout.validate(self.program)?;
        Ok(layout)
    }

    fn variant_layout(&self, ty: &ResolvedType) -> Result<VariantLayout, Diagnostic> {
        variant_declaration_id(self.program, ty)?.ok_or_else(|| {
            backend_error(format!(
                "native variant operation requires a variant, found `{}`",
                ty.identity_key()
            ))
        })?;
        let layout = self.variant_layouts.layout(ty)?.clone();
        layout.validate(self.program)?;
        Ok(layout)
    }

    fn bind_record_match_pattern(
        &mut self,
        base: &str,
        expected: &ResolvedType,
        record: &DeclarationId,
        instance: &ResolvedType,
        fields: &[hir::ResolvedRecordMatchPatternField],
    ) -> Result<(), Diagnostic> {
        self.require_type(instance, expected, "record pattern instance")?;
        let layout = self.record_layout(expected)?;
        if layout.record != *record || fields.len() != layout.fields.len() {
            return Err(backend_error(
                "record pattern disagrees with its exact aggregate layout",
            ));
        }
        let mut seen = BTreeSet::new();
        for field in fields {
            let layout_field = layout.field(&field.field).cloned().ok_or_else(|| {
                backend_error(format!(
                    "record pattern `{record}` has unknown field `{}`",
                    field.field
                ))
            })?;
            if !seen.insert(field.field.clone()) {
                return Err(backend_error(format!(
                    "record pattern `{record}` repeats field `{}`",
                    field.field
                )));
            }
            let field_code = format!("({base}).{}", c_field_symbol(&layout_field.field));
            match &field.pattern {
                hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                    self.require_type(&binding.ty, &layout_field.ty, "record pattern binding")?;
                    if binding.ownership != hir::OwnershipMode::Value
                        || self
                            .variables
                            .insert(
                                binding.id.clone(),
                                CBinding {
                                    name: field_code,
                                    ty: layout_field.ty,
                                },
                            )
                            .is_some()
                    {
                        return Err(backend_error(
                            "record pattern binding is not a fresh Copy value",
                        ));
                    }
                }
                hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
                hir::ResolvedRecordMatchFieldPattern::Record {
                    record,
                    instance,
                    fields,
                } => self.bind_record_match_pattern(
                    &field_code,
                    &layout_field.ty,
                    record,
                    instance,
                    fields,
                )?,
            }
        }
        Ok(())
    }

    fn emit_binary(
        &mut self,
        op: BinaryOp,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
        result_type: &ResolvedType,
    ) -> Result<CValue, Diagnostic> {
        let left = self.emit_expr(left)?;
        if matches!(op, BinaryOp::Eq | BinaryOp::Ne) && is_aggregate_type(self.program, &left.ty)? {
            return Err(backend_error(
                "aggregate equality is outside executable copy variants v1",
            ));
        }
        // Owned strings compare by UTF-8 contents; both operand buffers stay
        // owned by this expression and are freed right after the comparison.
        if matches!(op, BinaryOp::Eq | BinaryOp::Ne) && matches!(left.ty, ResolvedType::String) {
            let right = self.emit_expr(right)?;
            self.require_type(&right.ty, &ResolvedType::String, "binary right operand")?;
            self.require_type(result_type, &ResolvedType::Bool, "binary result")?;
            let temporary = self.temporary(&ResolvedType::Bool)?;
            let comparison = if op == BinaryOp::Eq {
                format!("spx_string_eq({}, {})", left.code, right.code)
            } else {
                format!("!spx_string_eq({}, {})", left.code, right.code)
            };
            self.line(&format!("{temporary} = {comparison};"));
            self.line(&format!("spx_string_drop({});", left.code));
            self.line(&format!("spx_string_drop({});", right.code));
            return Ok(CValue {
                code: temporary,
                ty: ResolvedType::Bool,
            });
        }
        if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) && matches!(left.ty, ResolvedType::String) {
            return Err(backend_error(
                "string operands only support equality comparison",
            ));
        }
        let float_operand = matches!(left.ty, ResolvedType::F32 | ResolvedType::F64);
        // Chars compare by Unicode scalar value; C unsigned comparison on
        // uint32_t matches the verified ordering exactly.
        let char_operand = matches!(left.ty, ResolvedType::Char);
        let int32_operand = matches!(left.ty, ResolvedType::I32);
        let narrow_operand = matches!(left.ty, ResolvedType::U8);
        let operand_type = match op {
            BinaryOp::And | BinaryOp::Or => ResolvedType::Bool,
            BinaryOp::Eq | BinaryOp::Ne => left.ty.clone(),
            _ if float_operand || char_operand || int32_operand || narrow_operand => {
                left.ty.clone()
            }
            _ => ResolvedType::I64,
        };
        self.require_type(&left.ty, &operand_type, "binary left operand")?;
        let expected_result = match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                if float_operand || int32_operand =>
            {
                left.ty.clone()
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                if narrow_operand =>
            {
                ResolvedType::U8
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                ResolvedType::I64
            }
            _ => ResolvedType::Bool,
        };
        self.require_type(result_type, &expected_result, "binary result")?;
        if float_operand && op == BinaryOp::Rem {
            return Err(backend_error(
                "floating-point remainder has no admitted native lowering",
            ));
        }
        if narrow_operand && op == BinaryOp::Rem {
            return Err(backend_error(
                "u8 remainder has no admitted native lowering",
            ));
        }
        if char_operand
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            )
        {
            return Err(backend_error(
                "char arithmetic has no admitted native lowering",
            ));
        }
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            let temporary = self.temporary(&ResolvedType::Bool)?;
            if op == BinaryOp::And {
                self.line(&format!("if ({}) {{", left.code));
                self.indent += 1;
                let right = self.emit_expr(right)?;
                self.require_type(&right.ty, &ResolvedType::Bool, "lazy right operand")?;
                self.line(&format!("{temporary} = {};", right.code));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                self.line(&format!("{temporary} = false;"));
            } else {
                self.line(&format!("if ({}) {{", left.code));
                self.indent += 1;
                self.line(&format!("{temporary} = true;"));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                let right = self.emit_expr(right)?;
                self.require_type(&right.ty, &ResolvedType::Bool, "lazy right operand")?;
                self.line(&format!("{temporary} = {};", right.code));
            }
            self.indent -= 1;
            self.line("}");
            return Ok(CValue {
                code: temporary,
                ty: ResolvedType::Bool,
            });
        }
        let right = self.emit_expr(right)?;
        self.require_type(&right.ty, &operand_type, "binary right operand")?;
        let temporary = self.temporary(&expected_result)?;
        if float_operand
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
            )
        {
            // IEEE-754 semantics are total: overflow, signed zero, and
            // division by zero follow the hardware rules and never select a
            // failure status.
            self.line(&format!(
                "{temporary} = ({} {} {});",
                left.code,
                op.text(),
                right.code
            ));
        } else if narrow_operand
            && matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
            )
        {
            // Checked u8 arithmetic computes in int64_t and range-checks the
            // 0..=255 result before narrowing to the uint8_t temporary.
            let helper = match op {
                BinaryOp::Add => "spx_rt_u8_add",
                BinaryOp::Sub => "spx_rt_u8_sub",
                BinaryOp::Mul => "spx_rt_u8_mul",
                BinaryOp::Div => "spx_rt_u8_div",
                _ => unreachable!("u8 arithmetic operation was matched above"),
            };
            self.line(&format!(
                "spx_status = {helper}(spx_ctx, {}, {}, &{temporary});",
                left.code, right.code
            ));
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        } else if matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
        ) {
            let helper = match op {
                BinaryOp::Add if int32_operand => "spx_rt_add_i32",
                BinaryOp::Sub if int32_operand => "spx_rt_sub_i32",
                BinaryOp::Mul if int32_operand => "spx_rt_mul_i32",
                BinaryOp::Div if int32_operand => "spx_rt_div_i32",
                BinaryOp::Rem if int32_operand => "spx_rt_rem_i32",
                BinaryOp::Add => "spx_rt_add",
                BinaryOp::Sub => "spx_rt_sub",
                BinaryOp::Mul => "spx_rt_mul",
                BinaryOp::Div => "spx_rt_div",
                BinaryOp::Rem => "spx_rt_rem",
                _ => unreachable!("checked arithmetic operation was matched above"),
            };
            self.line(&format!(
                "spx_status = {helper}(spx_ctx, {}, {}, &{temporary});",
                left.code, right.code
            ));
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        } else {
            self.line(&format!(
                "{temporary} = ({} {} {});",
                left.code,
                op.text(),
                right.code
            ));
        }
        Ok(CValue {
            code: temporary,
            ty: expected_result,
        })
    }
}

fn backend_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B103", message)
}

fn c_string(value: &str) -> String {
    let mut escaped = crate::bounded_output::CappedString::new();
    for byte in value.as_bytes() {
        match *byte {
            b'\\' => escaped.push_str("\\\\"),
            b'"' => escaped.push_str("\\\""),
            b'\n' => escaped.push_str("\\n"),
            b'\r' => escaped.push_str("\\r"),
            b'\t' => escaped.push_str("\\t"),
            b'?' | 0x00..=0x1f | 0x7f..=0xff => {
                write!(escaped, "\\{byte:03o}").expect("writing to a string cannot fail");
            }
            value => escaped.push(char::from(value)),
        }
    }
    escaped.into_string()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

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
            "45c15e9cafe21bb7bb2a94036ba7eff70f406ff1ef65426c5fe295cb2c0d366d"
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
