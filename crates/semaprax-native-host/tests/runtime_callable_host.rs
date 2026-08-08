#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::codegen::emit_native_callable_admission;
use semaprax::conformance::TraceEventKind;
use semaprax::hir::{self, DeclarationId};
use semaprax::semantic_trace::{build_semantic_event_dictionary, SemanticEventDictionary};
use semaprax::trace_path_certificate::TracePathCertificate;
use semaprax_native_host::{
    AdmissionError, CallRejection, NativeCallableExecution, NativeCallableHost, NativeOwner,
    RejectedCall, ScalarValue,
};

const SOURCE: &str = r#"module test.callable_host;

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.checked-discard")
fn checked_discard(value: own Token, count: i64) -> i64
requires count >= 0
{
    7
}

@id("test.main")
fn main() -> i64 { 0 }
"#;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy)]
enum ProviderBehavior {
    Success,
    SemanticFailure,
    PhysicalFailure(u32),
    MalformedResponse,
}

struct ProviderOrdinals {
    result_commit: Option<u32>,
    selected_failure: Option<u32>,
    transfers: Vec<u32>,
    finalizers: Vec<u32>,
}

struct Fixture {
    directory: PathBuf,
    library: PathBuf,
    #[cfg(unix)]
    unload_marker: PathBuf,
    descriptor: Vec<u8>,
    getter_symbol: String,
    callable_symbol: String,
    dictionary: SemanticEventDictionary,
    trace_path_certificate: TracePathCertificate,
}

impl Fixture {
    fn build(function: &str, behavior: ProviderBehavior) -> Self {
        let parsed = semaprax::parse(SOURCE, Path::new("runtime-callable-host.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        let function_id = DeclarationId::new(function);
        let artifact = emit_native_callable_admission(&resolved, &function_id).unwrap();
        let dictionary = build_semantic_event_dictionary(&resolved, &function_id).unwrap();
        let result_commit = dictionary
            .entries()
            .iter()
            .find(|entry| matches!(entry.event, TraceEventKind::ResultCommit { .. }))
            .map(|entry| entry.ordinal);
        let selected_failure = dictionary
            .entries()
            .iter()
            .find(|entry| matches!(entry.event, TraceEventKind::SelectFailure { .. }))
            .map(|entry| entry.ordinal);
        let transfer_ordinals = dictionary
            .entries()
            .iter()
            .filter(|entry| matches!(entry.event, TraceEventKind::Transfer { .. }))
            .map(|entry| entry.ordinal)
            .collect::<Vec<_>>();
        let finalizer_ordinals = dictionary
            .entries()
            .iter()
            .filter(|entry| {
                matches!(
                    entry.event,
                    TraceEventKind::FinalizeBegin { .. } | TraceEventKind::FinalizeEnd { .. }
                )
            })
            .map(|entry| entry.ordinal)
            .collect::<Vec<_>>();
        let ordinals = ProviderOrdinals {
            result_commit,
            selected_failure,
            transfers: transfer_ordinals,
            finalizers: finalizer_ordinals,
        };
        let provider = provider_source(
            artifact.descriptor(),
            artifact.getter_symbol(),
            artifact.callable_symbol(),
            function,
            behavior,
            &ordinals,
        );
        Self::compile(
            artifact.descriptor().to_vec(),
            artifact.getter_symbol().to_owned(),
            artifact.callable_symbol().to_owned(),
            dictionary,
            artifact.trace_path_certificate().clone(),
            provider,
        )
    }

    fn build_generated(function: &str) -> Self {
        let parsed = semaprax::parse(SOURCE, Path::new("runtime-callable-generated.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        let artifact =
            emit_native_callable_admission(&resolved, &DeclarationId::new(function)).unwrap();
        Self::compile(
            artifact.descriptor().to_vec(),
            artifact.getter_symbol().to_owned(),
            artifact.callable_symbol().to_owned(),
            artifact.semantic_event_dictionary().clone(),
            artifact.trace_path_certificate().clone(),
            artifact.provider_source().to_owned(),
        )
    }

    fn compile(
        descriptor: Vec<u8>,
        getter_symbol: String,
        callable_symbol: String,
        dictionary: SemanticEventDictionary,
        trace_path_certificate: TracePathCertificate,
        provider: String,
    ) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-callable-host-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create isolated callable fixture directory");
        let directory = fs::canonicalize(directory).expect("canonical callable fixture directory");
        let source = directory.join("provider.c");
        let library = directory.join(library_filename());
        #[cfg(unix)]
        let unload_marker = directory.join("unloaded.marker");
        #[cfg(unix)]
        let provider = {
            let mut provider = provider;
            provider.push_str(&format!(
                "\n#include <stdio.h>\n__attribute__((destructor)) static void spx_callable_host_unload(void) {{ FILE *marker = fopen({}, \"wb\"); if (marker != NULL) {{ fputs(\"unloaded\", marker); fclose(marker); }} }}\n",
                c_string_literal(&unload_marker)
            ));
            provider
        };
        fs::write(&source, provider).expect("write callable provider fixture");

        let compiler_name = std::env::var_os("CC").unwrap_or_else(|| {
            if cfg!(windows) {
                "clang".into()
            } else {
                "cc".into()
            }
        });
        let mut compiler = Command::new(compiler_name);
        #[cfg(target_os = "macos")]
        compiler.args(["-dynamiclib", "-fPIC"]);
        #[cfg(target_os = "linux")]
        compiler.args(["-shared", "-fPIC"]);
        #[cfg(target_os = "windows")]
        compiler.arg("-shared");
        let output = compiler
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
            .arg(&source)
            .arg("-o")
            .arg(&library)
            .output()
            .expect("compile callable host fixture");
        assert!(
            output.status.success(),
            "callable fixture compiler failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let library = fs::canonicalize(library).expect("canonical callable fixture library");
        Self {
            directory,
            library,
            #[cfg(unix)]
            unload_marker,
            descriptor,
            getter_symbol,
            callable_symbol,
            dictionary,
            trace_path_certificate,
        }
    }

    unsafe fn open(&self) -> Result<NativeCallableHost, AdmissionError> {
        // SAFETY: The fixture defines only the exact immutable descriptor getter
        // and synchronous bounded byte callable in a private canonical image.
        unsafe {
            NativeCallableHost::open_admitted_callable_exact(
                &self.library,
                self.getter_symbol.as_bytes(),
                self.callable_symbol.as_bytes(),
                &self.descriptor,
                self.dictionary.clone(),
                self.trace_path_certificate.clone(),
            )
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).expect("remove isolated callable fixture directory");
    }
}

fn library_filename() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "libcallable-host.dylib"
    }
    #[cfg(target_os = "linux")]
    {
        "libcallable-host.so"
    }
    #[cfg(target_os = "windows")]
    {
        "callable-host.dll"
    }
}

#[cfg(unix)]
fn c_string_literal(path: &Path) -> String {
    let mut escaped = String::from("\"");
    for character in path.to_str().expect("fixture path is UTF-8").chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            value => escaped.push(value),
        }
    }
    escaped.push('"');
    escaped
}

fn provider_source(
    descriptor: &[u8],
    getter_symbol: &str,
    callable_symbol: &str,
    function: &str,
    behavior: ProviderBehavior,
    ordinals: &ProviderOrdinals,
) -> String {
    let descriptor_bytes = descriptor
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(",");
    let response = match behavior {
        ProviderBehavior::PhysicalFailure(code) => format!("return UINT32_C({code});"),
        ProviderBehavior::MalformedResponse => {
            "if (response_cap > 0) { response[0] = UINT8_C(0xff); }\nreturn UINT32_C(0);".to_owned()
        }
        ProviderBehavior::SemanticFailure => {
            let ordinal = ordinals
                .selected_failure
                .expect("failure fixture has a selected failure");
            assert_eq!(ordinals.finalizers.len(), 2);
            let finalize_begin = ordinals.finalizers[0];
            let finalize_end = ordinals.finalizers[1];
            format!(
                "spx_put32(response + 16, UINT32_C(84));\n\
                 memcpy(response + 20, request + 20, 32);\n\
                 memcpy(response + 52, request + 52, 8);\n\
                 spx_put32(response + 60, UINT32_C(2));\n\
                 spx_put32(response + 64, UINT32_C(3));\n\
                 spx_put32(response + 68, UINT32_C({ordinal}));\n\
                 spx_put32(response + 72, UINT32_C({ordinal}));\n\
                 spx_put32(response + 76, UINT32_C({finalize_begin}));\n\
                 spx_put32(response + 80, UINT32_C({finalize_end}));\n\
                 return UINT32_C(0);"
            )
        }
        ProviderBehavior::Success => {
            let ordinal = ordinals
                .result_commit
                .expect("success fixture has a result commit");
            if function == "token.identity" {
                assert_eq!(ordinals.transfers.len(), 2);
                let first_transfer = ordinals.transfers[0];
                let second_transfer = ordinals.transfers[1];
                format!(
                    "spx_put32(response + 16, UINT32_C(88));\n\
                     memcpy(response + 20, request + 20, 32);\n\
                     memcpy(response + 52, request + 52, 8);\n\
                     spx_put32(response + 60, UINT32_C(1));\n\
                     spx_put32(response + 64, UINT32_C(3));\n\
                     spx_put32(response + 68, UINT32_C(2));\n\
                     spx_put32(response + 72, UINT32_C(0));\n\
                     spx_put32(response + 76, UINT32_C({first_transfer}));\n\
                     spx_put32(response + 80, UINT32_C({second_transfer}));\n\
                     spx_put32(response + 84, UINT32_C({ordinal}));\n\
                     return UINT32_C(0);"
                )
            } else {
                assert_eq!(ordinals.finalizers.len(), 2);
                let finalize_begin = ordinals.finalizers[0];
                let finalize_end = ordinals.finalizers[1];
                format!(
                    "spx_put32(response + 16, UINT32_C(92));\n\
                     memcpy(response + 20, request + 20, 32);\n\
                     memcpy(response + 52, request + 52, 8);\n\
                     spx_put32(response + 60, UINT32_C(1));\n\
                     spx_put32(response + 64, UINT32_C(3));\n\
                     spx_put32(response + 68, UINT32_C(1));\n\
                     spx_put64(response + 72, UINT64_C(7));\n\
                     spx_put32(response + 80, UINT32_C({finalize_begin}));\n\
                     spx_put32(response + 84, UINT32_C({finalize_end}));\n\
                     spx_put32(response + 88, UINT32_C({ordinal}));\n\
                     return UINT32_C(0);"
                )
            }
        }
    };
    format!(
        "#include <stdint.h>\n#include <stddef.h>\n#include <string.h>\n\
         #if defined(_WIN32)\n\
         #define SPX_EXPORT __declspec(dllexport)\n\
         #define SPX_CALL __cdecl\n\
         #else\n\
         #define SPX_EXPORT __attribute__((visibility(\"default\")))\n\
         #define SPX_CALL\n\
         #endif\n\
         static const uint8_t descriptor[] = {{{descriptor_bytes}}};\n\
         static void spx_put32(uint8_t *out, uint32_t value) {{\n\
             out[0] = (uint8_t)value; out[1] = (uint8_t)(value >> 8);\n\
             out[2] = (uint8_t)(value >> 16); out[3] = (uint8_t)(value >> 24);\n\
         }}\n\
         static void spx_put64(uint8_t *out, uint64_t value) {{\n\
             for (uint32_t i = 0; i < UINT32_C(8); ++i) out[i] = (uint8_t)(value >> (i * UINT32_C(8)));\n\
         }}\n\
         SPX_EXPORT const uint8_t *SPX_CALL {getter_symbol}(void) {{ return descriptor; }}\n\
         SPX_EXPORT uint32_t SPX_CALL {callable_symbol}(\n\
             const uint8_t *request, uint32_t request_len, uint8_t *response, uint32_t response_cap) {{\n\
             (void)request_len;\n\
             (void)spx_put64;\n\
             if (request == NULL || response == NULL || response_cap < UINT32_C(20)) return UINT32_C(2);\n\
             memset(response, 0, response_cap);\n\
             memcpy(response, \"SPXNRSP1\", 8);\n\
             spx_put32(response + 8, UINT32_C(1));\n\
             spx_put32(response + 12, UINT32_C(20));\n\
             {response}\n\
         }}\n"
    )
}

fn open_fixture(function: &str, behavior: ProviderBehavior) -> (Fixture, NativeCallableHost) {
    let fixture = Fixture::build(function, behavior);
    // SAFETY: Fixture::open documents the exact generated descriptor and
    // bounded synchronous provider image.
    let host = unsafe { fixture.open() }.expect("admit callable fixture");
    (fixture, host)
}

fn adopt(host: &mut NativeCallableHost, payload: u64) -> NativeOwner {
    // SAFETY: Each call creates one fresh exclusive logical Token payload.
    unsafe { host.adopt_trusted_owner(0, payload) }.expect("adopt fixture owner")
}

fn executed<T>(
    result: Result<NativeCallableExecution<T>, RejectedCall>,
) -> NativeCallableExecution<T> {
    match result {
        Ok(execution) => execution,
        Err(rejection) => panic!(
            "unexpected precommit rejection: {:?}",
            rejection.rejection()
        ),
    }
}

fn rejected_call<T>(result: Result<NativeCallableExecution<T>, RejectedCall>) -> RejectedCall {
    match result {
        Ok(_) => panic!("expected precommit rejection"),
        Err(rejection) => rejection,
    }
}

#[test]
fn callable_scalar_success_executes_real_shared_library_and_consumes_owner() {
    let (_fixture, mut host) = open_fixture("token.checked-discard", ProviderBehavior::Success);
    let owner = adopt(&mut host, u64::MAX);
    let execution =
        executed(host.call_scalar_with_values(vec![owner], vec![ScalarValue::I64(i64::MAX)]));
    let (result, events) = execution.into_parts();
    assert_eq!(result, Ok(7));
    assert_eq!(events.len(), 3);
    assert!(matches!(
        events[2].event,
        TraceEventKind::ResultCommit { .. }
    ));
    assert_eq!(host.live_owner_count(), 0);
}

#[test]
fn callable_owned_success_rotates_and_republishes_the_exact_owner() {
    let (_fixture, mut host) = open_fixture("token.identity", ProviderBehavior::Success);
    let instance = host.module_instance_id();
    let owner = adopt(&mut host, 0);
    let (result, events) = executed(host.call_owned(vec![owner])).into_parts();
    let owner = match result {
        Ok(owner) => owner,
        Err(status) => panic!("unexpected callable failure: {status:?}"),
    };
    assert_eq!(owner.module_instance_id(), instance);
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0].event, TraceEventKind::Transfer { .. }));
    assert!(matches!(events[1].event, TraceEventKind::Transfer { .. }));
    assert!(matches!(
        events[2].event,
        TraceEventKind::ResultCommit { .. }
    ));
    assert_eq!(host.live_owner_count(), 1);

    let (result, _) = executed(host.call_owned(vec![owner])).into_parts();
    let owner = match result {
        Ok(owner) => owner,
        Err(status) => panic!("unexpected second callable failure: {status:?}"),
    };
    drop(owner);
    assert_eq!(host.live_owner_count(), 0);
}

#[cfg(unix)]
#[test]
fn callable_owned_result_pins_module_until_the_final_owner_drops() {
    let fixture = Fixture::build_generated("token.identity");
    let unload_marker = fixture.unload_marker.clone();
    // SAFETY: The compiler-generated fixture supplies the exact immutable
    // descriptor getter and synchronous bounded callable for its full lease.
    let mut host = unsafe { fixture.open() }.expect("admit compiler-generated callable");
    let owner = adopt(&mut host, 0);
    let (result, _) = executed(host.call_owned(vec![owner])).into_parts();
    let owner = result.expect("owned identity publishes its input");

    drop(host);
    assert!(
        !unload_marker.exists(),
        "the callable result credential must retain the exact module"
    );
    drop(owner);
    assert!(
        unload_marker.exists(),
        "the callable module becomes releasable after its final owner"
    );
}

#[test]
fn compiler_generated_callable_executes_through_host_without_shadow_closure() {
    let fixture = Fixture::build_generated("token.identity");
    // SAFETY: The compiler emitted the complete descriptor getter, strict byte
    // provider, and direct verified cleanup implementation in this exact image.
    let mut host = unsafe { fixture.open() }.expect("admit compiler-generated callable");
    let owner = adopt(&mut host, u64::MAX);
    let (result, events) = executed(host.call_owned(vec![owner])).into_parts();
    let owner = match result {
        Ok(owner) => owner,
        Err(status) => panic!("generated callable failed: {status:?}"),
    };
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0].event, TraceEventKind::Transfer { .. }));
    assert!(matches!(events[1].event, TraceEventKind::Transfer { .. }));
    assert!(matches!(
        events[2].event,
        TraceEventKind::ResultCommit { .. }
    ));
    assert_eq!(host.live_owner_count(), 1);
    drop(owner);
    assert_eq!(host.live_owner_count(), 0);
}

#[test]
fn compiler_generated_callable_preserves_scalar_success_and_semantic_failure() {
    let success_fixture = Fixture::build_generated("token.checked-discard");
    // SAFETY: This is the compiler-emitted complete callable translation unit.
    let mut success_host =
        unsafe { success_fixture.open() }.expect("admit generated scalar callable");
    let success_owner = adopt(&mut success_host, 0);
    let (result, events) = executed(
        success_host.call_scalar_with_values(vec![success_owner], vec![ScalarValue::I64(i64::MAX)]),
    )
    .into_parts();
    assert_eq!(result, Ok(7));
    assert!(matches!(
        events.last().map(|event| &event.event),
        Some(TraceEventKind::ResultCommit { .. })
    ));
    assert_eq!(success_host.live_owner_count(), 0);

    let failure_fixture = Fixture::build_generated("token.checked-discard");
    // SAFETY: This is a distinct exact open of the same compiler-emitted
    // translation unit and dictionary.
    let mut failure_host =
        unsafe { failure_fixture.open() }.expect("admit generated failure callable");
    let failure_owner = adopt(&mut failure_host, u64::MAX);
    let (result, events) = executed(
        failure_host.call_scalar_with_values(vec![failure_owner], vec![ScalarValue::I64(-1)]),
    )
    .into_parts();
    let status = result.expect_err("negative count violates generated requires contract");
    assert_eq!(status.domain_id(), "semaprax.contract.v1");
    assert_eq!(status.code(), 1);
    assert!(events
        .iter()
        .any(|event| matches!(event.event, TraceEventKind::SelectFailure { .. })));
    assert_eq!(failure_host.live_owner_count(), 0);
}

#[test]
fn callable_semantic_failure_returns_authenticated_status_and_no_publication() {
    let (_fixture, mut host) =
        open_fixture("token.checked-discard", ProviderBehavior::SemanticFailure);
    let owner = adopt(&mut host, 9);
    let (result, events) =
        executed(host.call_scalar_with_values(vec![owner], vec![ScalarValue::I64(-1)]))
            .into_parts();
    let status = result.expect_err("requires failure is semantic failure");
    assert_eq!(status.domain_id(), "semaprax.contract.v1");
    assert_eq!(status.code(), 1);
    assert_eq!(events.len(), 3);
    assert!(matches!(
        events[0].event,
        TraceEventKind::SelectFailure { .. }
    ));
    assert_eq!(host.live_owner_count(), 0);
}

#[test]
fn postcommit_physical_and_malformed_failures_abandon_every_input() {
    for behavior in [
        ProviderBehavior::PhysicalFailure(1),
        ProviderBehavior::PhysicalFailure(2),
        ProviderBehavior::PhysicalFailure(3),
        ProviderBehavior::PhysicalFailure(99),
        ProviderBehavior::MalformedResponse,
    ] {
        let (_fixture, mut host) = open_fixture("token.identity", behavior);
        let owner = adopt(&mut host, 17);
        let (result, events) = executed(host.call_owned(vec![owner])).into_parts();
        let status = match result {
            Ok(_) => panic!("provider fault must not publish an owner"),
            Err(status) => status,
        };
        assert_eq!(status.domain_id(), "semaprax.adapter.host-ownership.v1");
        assert_eq!(status.code(), 1);
        assert!(events.is_empty());
        assert_eq!(host.live_owner_count(), 0);
    }
}

#[test]
fn precommit_rejection_returns_reusable_owner_and_cross_instance_is_exact() {
    let (_first_fixture, mut first) =
        open_fixture("token.checked-discard", ProviderBehavior::Success);
    let owner = adopt(&mut first, 33);
    let rejection = rejected_call(first.call_scalar(vec![owner]));
    assert_eq!(
        rejection.rejection(),
        CallRejection::ScalarInputCountMismatch
    );
    let owner = rejection.into_owners().pop().unwrap();
    let (_second_fixture, mut second) =
        open_fixture("token.checked-discard", ProviderBehavior::Success);
    let rejection =
        rejected_call(second.call_scalar_with_values(vec![owner], vec![ScalarValue::I64(1)]));
    assert_eq!(rejection.rejection(), CallRejection::WrongModuleInstance);
    let owner = rejection.into_owners().pop().unwrap();
    assert_eq!(first.live_owner_count(), 1);
    let result = executed(first.call_scalar_with_values(vec![owner], vec![ScalarValue::I64(1)]))
        .into_parts()
        .0;
    assert_eq!(result, Ok(7));
}

#[test]
fn callable_draining_is_one_way_and_rejects_without_consuming_owner() {
    let (_fixture, mut host) = open_fixture("token.identity", ProviderBehavior::Success);
    let owner = adopt(&mut host, 21);
    host.begin_draining();
    assert!(host.is_draining());
    let rejection = rejected_call(host.call_owned(vec![owner]));
    assert_eq!(rejection.rejection(), CallRejection::Draining);
    let owner = rejection.into_owners().pop().unwrap();
    assert_eq!(host.live_owner_count(), 1);
    drop(owner);
    assert_eq!(host.live_owner_count(), 0);
    // SAFETY: The payload would otherwise be a valid fresh fixture owner, but
    // draining rejects before any ledger registration.
    assert_eq!(
        unsafe { host.adopt_trusted_owner(0, 22) }.err(),
        Some(CallRejection::Draining)
    );
}

#[test]
fn callable_admission_rejects_wrong_dictionary_before_execution() {
    let identity = Fixture::build("token.identity", ProviderBehavior::Success);
    let other = Fixture::build("token.checked-discard", ProviderBehavior::Success);
    // SAFETY: The image itself is trusted; this call deliberately supplies an
    // unrelated authenticated dictionary and must reject before opening it.
    let error = unsafe {
        NativeCallableHost::open_admitted_callable_exact(
            &identity.library,
            identity.getter_symbol.as_bytes(),
            identity.callable_symbol.as_bytes(),
            &identity.descriptor,
            other.dictionary.clone(),
            identity.trace_path_certificate.clone(),
        )
    }
    .err()
    .expect("wrong dictionary must reject admission");
    assert!(matches!(error, AdmissionError::SemanticDictionary));
}

#[test]
fn callable_admission_rejects_other_function_certificate_before_loading() {
    let identity = Fixture::build("token.identity", ProviderBehavior::Success);
    let other = Fixture::build("token.checked-discard", ProviderBehavior::Success);
    let missing_library = identity.directory.join(library_filename());
    fs::rename(
        &identity.library,
        missing_library.with_extension("not-loadable"),
    )
    .expect("move fixture image out of the admission path");

    // SAFETY: Certificate authentication must reject before the deliberately
    // missing library path reaches the unsafe loader boundary.
    let error = unsafe {
        NativeCallableHost::open_admitted_callable_exact(
            &identity.library,
            identity.getter_symbol.as_bytes(),
            identity.callable_symbol.as_bytes(),
            &identity.descriptor,
            identity.dictionary.clone(),
            other.trace_path_certificate.clone(),
        )
    }
    .err()
    .expect("other-function certificate must reject admission");
    assert!(matches!(error, AdmissionError::SemanticDictionary));
}
