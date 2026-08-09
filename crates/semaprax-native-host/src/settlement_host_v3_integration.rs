#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::codegen::{emit_private_native_callable_v3_fixture, PrivateNativeCallableV3Fixture};
use semaprax::hir::DeclarationId;
use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;
use semaprax_native_loader::open_admitted_settlement_exact;

use crate::callable_wire_v3::{ExecuteOutcome, Publication};
use crate::descriptor_v3::Descriptor;
use crate::settlement_host_v3::{PrivateSettlementExecutionError, PrivateSettlementHostV3};
use crate::settlement_ledger::SettlementLedgerError;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);
const REQUIRED_SANITIZERS_ENV: &str = "SEMAPRAX_REQUIRE_CALLABLE_HOST_SANITIZERS";
const REQUIRED_RUST_HOST_ASAN_ENV: &str = "SEMAPRAX_REQUIRE_RUST_HOST_ASAN";

#[derive(Clone, Copy, Eq, PartialEq)]
enum RequiredSanitizers {
    None,
    Address,
    AddressAndUndefined,
}

struct Fixture {
    directory: PathBuf,
    library: PathBuf,
    descriptor: Vec<u8>,
    finalizer_marker: PathBuf,
    unload_marker: PathBuf,
}

impl Fixture {
    fn build(function: &str, fixture: PrivateNativeCallableV3Fixture, optimization: &str) -> Self {
        Self::build_with_provider_mutation(function, fixture, optimization, |source, _, _| source)
    }

    fn build_with_provider_mutation(
        function: &str,
        fixture: PrivateNativeCallableV3Fixture,
        optimization: &str,
        mutate: impl FnOnce(String, &str, &str) -> String,
    ) -> Self {
        let corpus = build_owned_resource_corpus_v1().expect("build owned corpus");
        let artifact = emit_private_native_callable_v3_fixture(
            &corpus.program,
            &DeclarationId::new(function),
            fixture,
        )
        .expect("derive private generated v3 provider");
        let directory = fixture_directory(function);
        let finalizer_marker = directory.join("finalizers.marker");
        let unload_marker = directory.join("unloaded.marker");
        let descriptor = Descriptor::parse(artifact.descriptor()).expect("parse v3 descriptor");
        let provider = mutate(
            artifact.source().to_owned(),
            &descriptor.execute_symbol,
            &descriptor.settle_symbol,
        );
        let source = format!(
            r#"{provider}
#include <stdio.h>
#if defined(_WIN32)
#include <windows.h>
#endif
static void spx_v3_generated_finalize(uint32_t owner,uint64_t payload){{
  FILE *file=fopen({finalizer},"ab");
  if(file!=NULL){{(void)fprintf(file,"%u:%llu\n",(unsigned)owner,(unsigned long long)payload);(void)fclose(file);}}
}}
#if defined(_WIN32)
BOOL WINAPI DllMain(HINSTANCE module,DWORD reason,LPVOID reserved){{
  (void)module;
  if(reason==DLL_PROCESS_DETACH&&reserved==NULL){{
    HANDLE file=CreateFileA({unload},GENERIC_WRITE,0,NULL,CREATE_ALWAYS,FILE_ATTRIBUTE_NORMAL,NULL);
    if(file!=INVALID_HANDLE_VALUE){{DWORD written=0;const char text[]="unloaded";(void)WriteFile(file,text,8,&written,NULL);CloseHandle(file);}}
  }}
  return TRUE;
}}
#else
__attribute__((destructor)) static void spx_v3_on_unload(void){{
  FILE *file=fopen({unload},"wb");if(file!=NULL){{(void)fputs("unloaded",file);(void)fclose(file);}}
}}
#endif
"#,
            provider = provider,
            finalizer = c_string_literal(&finalizer_marker),
            unload = c_string_literal(&unload_marker),
        );
        let library = compile_provider(&directory, &source, optimization);
        Self {
            directory,
            library,
            descriptor: artifact.descriptor().to_vec(),
            finalizer_marker,
            unload_marker,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).expect("remove joint v3 fixture directory");
    }
}

#[test]
fn generated_provider_loader_host_v3_rejects_resealed_wrong_cell_before_settle() {
    let fixture = Fixture::build_with_provider_mutation(
        "token.identity",
        PrivateNativeCallableV3Fixture::OwnedIdentity,
        "-O2",
        |source, execute_symbol, settle_symbol| {
            let execute_inner = format!("{execute_symbol}_inner");
            let settle_inner = format!("{settle_symbol}_inner");
            let source = source.replacen(
                &format!("{execute_symbol}("),
                &format!("{execute_inner}("),
                1,
            );
            let source =
                source.replacen(&format!("{settle_symbol}("), &format!("{settle_inner}("), 1);
            assert!(source.contains(&format!("{execute_inner}(")));
            assert!(source.contains(&format!("{settle_inner}(")));
            format!(
                r#"{source}
SPX_V3_API uint32_t SPX_V3_CALL {execute_symbol}(const uint8_t *request,uint32_t request_len,uint8_t *frame,uint32_t frame_len,uint8_t *response,uint32_t response_len){{
  uint32_t code={execute_inner}(request,request_len,frame,frame_len,response,response_len);
  if(code==UINT32_C(0)){{uint32_t cell=spx_v3_cell(UINT32_C(0));spx_v3_store_u64(frame+cell+UINT32_C(4),spx_v3_load_u64(frame+cell+UINT32_C(4))^UINT64_C(1));spx_v3_refresh_frame(frame);}}
  return code;
}}
SPX_V3_API uint32_t SPX_V3_CALL {settle_symbol}(uint8_t *frame,uint32_t frame_len,const uint8_t *decision,uint32_t decision_len,uint8_t *candidate,uint32_t candidate_len){{
  spx_v3_generated_finalize(UINT32_MAX,UINT64_C(0x5354544c));
  return {settle_inner}(frame,frame_len,decision,decision_len,candidate,candidate_len);
}}
"#
            )
        },
    );
    // SAFETY: The test-generated root implements the exact synchronous ABI;
    // its only hostile behavior is a canonical frame reseal after changing one
    // payload cell and an observable marker if settle is ever entered.
    let lease =
        unsafe { open_admitted_settlement_exact(&fixture.library, &fixture.descriptor) }.unwrap();
    let host = PrivateSettlementHostV3::from_admitted(lease, &fixture.descriptor).unwrap();
    let owner = host.register_owner(91, 4).unwrap();
    assert!(matches!(
        host.execute_owned_success(&[owner], &[99]),
        Err(PrivateSettlementExecutionError::Wire(
            crate::callable_wire_v3::WireError::ReplayMismatch
        ))
    ));
    assert!(
        !fixture.finalizer_marker.exists(),
        "pre-settle rejection must occur before provider settle or any settle effect"
    );
    assert!(host.is_poisoned());
    assert!(host.is_draining());
    drop(host);
    assert!(fixture.unload_marker.exists());
}

#[test]
fn generated_provider_loader_host_v3_end_to_end_is_exact() {
    for optimization in ["-O0", "-O2"] {
        scalar_discard_is_exact(optimization);
        owned_identity_rotates_generation(optimization);
    }
}

#[test]
fn generated_provider_loader_host_v3_rejects_descriptor_and_cross_instance_confusion() {
    let scalar = Fixture::build(
        "token.discard-two",
        PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
        "-O2",
    );
    let corpus = build_owned_resource_corpus_v1().unwrap();
    let identity = emit_private_native_callable_v3_fixture(
        &corpus.program,
        &DeclarationId::new("token.identity"),
        PrivateNativeCallableV3Fixture::OwnedIdentity,
    )
    .unwrap();
    // SAFETY: The generated root implements the exact synchronous v3 ABI and
    // the test controls its complete dependency namespace and lifetime.
    let mismatched =
        unsafe { open_admitted_settlement_exact(&scalar.library, &scalar.descriptor) }.unwrap();
    assert!(matches!(
        PrivateSettlementHostV3::from_admitted(mismatched, identity.descriptor()),
        Err(SettlementLedgerError::DescriptorMismatch)
    ));
    if scalar.unload_marker.exists() {
        fs::remove_file(&scalar.unload_marker).unwrap();
    }

    // SAFETY: Both admissions use the same exact generated trusted root. Each
    // call still receives a fresh logical module-instance identity.
    let first_lease =
        unsafe { open_admitted_settlement_exact(&scalar.library, &scalar.descriptor) }.unwrap();
    let second_lease =
        unsafe { open_admitted_settlement_exact(&scalar.library, &scalar.descriptor) }.unwrap();
    let first = PrivateSettlementHostV3::from_admitted(first_lease, &scalar.descriptor).unwrap();
    let second = PrivateSettlementHostV3::from_admitted(second_lease, &scalar.descriptor).unwrap();
    assert_ne!(first.module_instance_id(), second.module_instance_id());
    let foreign_a = first.register_owner(31, 1).unwrap();
    let foreign_b = first.register_owner(32, 1).unwrap();
    assert_eq!(
        second.execute_owned_success(&[foreign_a, foreign_b], &[41, 73]),
        Err(PrivateSettlementExecutionError::Ledger(
            SettlementLedgerError::WrongInstance
        ))
    );
    assert!(!second.is_poisoned());
    assert!(!second.is_draining());
    drop(first);
    drop(second);
    assert!(
        scalar.unload_marker.exists(),
        "every exact-instance pin must release before the root terminator runs"
    );
}

fn scalar_discard_is_exact(optimization: &str) {
    let fixture = Fixture::build(
        "token.discard-two",
        PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
        optimization,
    );
    // SAFETY: The compiler-generated fixture implements the exact synchronous
    // ABI and retains no provider-visible pointer after either call.
    let lease =
        unsafe { open_admitted_settlement_exact(&fixture.library, &fixture.descriptor) }.unwrap();
    let host = PrivateSettlementHostV3::from_admitted(lease, &fixture.descriptor).unwrap();
    let first = host.register_owner(11, 3).unwrap();
    let second = host.register_owner(12, 7).unwrap();
    let physical = host
        .execute_owned_success(&[first, second], &[41, 73])
        .unwrap();
    assert_eq!(physical.outcome, ExecuteOutcome::Scalar { value: 0 });
    assert_eq!(physical.committed.publication, Publication::NoOwned);
    assert!(physical.committed.published_owner.is_none());
    assert_eq!(
        host.replay_committed(physical.identity, &physical.candidate_bytes)
            .unwrap(),
        physical.committed
    );
    assert_eq!(
        fs::read_to_string(&fixture.finalizer_marker).unwrap(),
        "1:73\n0:41\n"
    );
    assert!(!host.is_poisoned());
    drop(host);
    assert!(fixture.unload_marker.exists());
}

fn owned_identity_rotates_generation(optimization: &str) {
    let fixture = Fixture::build(
        "token.identity",
        PrivateNativeCallableV3Fixture::OwnedIdentity,
        optimization,
    );
    // SAFETY: The compiler-generated fixture implements the exact synchronous
    // ABI and retains no provider-visible pointer after either call.
    let lease =
        unsafe { open_admitted_settlement_exact(&fixture.library, &fixture.descriptor) }.unwrap();
    let host = PrivateSettlementHostV3::from_admitted(lease, &fixture.descriptor).unwrap();
    let original = host.register_owner(21, 5).unwrap();
    let first = host.execute_owned_success(&[original], &[99]).unwrap();
    assert_eq!(
        first.outcome,
        ExecuteOutcome::Owned {
            owner_ordinal: 0,
            payload: 99,
        }
    );
    assert_eq!(first.committed.publication, Publication::Owned(0));
    let refreshed = first.committed.published_owner.unwrap();
    assert_eq!(
        host.replay_committed(first.identity, &first.candidate_bytes)
            .unwrap(),
        first.committed
    );
    assert_eq!(
        host.execute_owned_success(&[original], &[99]),
        Err(PrivateSettlementExecutionError::Ledger(
            SettlementLedgerError::StaleOwner
        ))
    );
    assert!(!host.is_poisoned());
    let second = host.execute_owned_success(&[refreshed], &[99]).unwrap();
    assert_eq!(second.committed.publication, Publication::Owned(0));
    assert!(second.committed.published_owner.is_some());
    assert!(
        !fixture.finalizer_marker.exists(),
        "accepted owned identity must not finalize the published resource"
    );
    drop(host);
    assert!(fixture.unload_marker.exists());
}

fn fixture_directory(label: &str) -> PathBuf {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-v3-joint-{label}-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("create joint v3 fixture directory");
    fs::canonicalize(path).expect("canonicalize joint v3 fixture directory")
}

fn compile_provider(directory: &Path, source: &str, optimization: &str) -> PathBuf {
    let source_path = directory.join("provider.c");
    fs::write(&source_path, source).expect("write generated v3 provider");
    let output_name = root_library_name();
    let sanitizers = required_sanitizers();
    let compiler = if sanitizers == RequiredSanitizers::None {
        std::env::var_os("CC").unwrap_or_else(|| "clang".into())
    } else {
        std::env::var_os("CC").expect("required v3 sanitizer lane needs explicit CC")
    };
    let mut command = Command::new(compiler);
    command.current_dir(directory);
    command
        .args(shared_flags())
        .args([optimization, "-std=c11", "-Wall", "-Wextra", "-Werror"]);
    match sanitizers {
        RequiredSanitizers::None => {}
        RequiredSanitizers::Address => {
            command.args([
                "-fsanitize=address",
                "-fno-omit-frame-pointer",
                "-fno-sanitize-recover=all",
            ]);
        }
        RequiredSanitizers::AddressAndUndefined => {
            command.args([
                "-fsanitize=address,undefined",
                "-fno-omit-frame-pointer",
                "-fno-sanitize-recover=all",
            ]);
        }
    }
    #[cfg(not(target_os = "windows"))]
    command.arg("-pedantic");
    let output = command
        .arg(&source_path)
        .arg("-o")
        .arg(output_name)
        .output()
        .expect("run generated v3 provider compiler");
    assert!(
        output.status.success(),
        "joint v3 provider compile failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let library =
        fs::canonicalize(directory.join(output_name)).expect("canonical generated v3 provider");
    assert_sanitizer_symbols(&library, sanitizers);
    library
}

fn required_sanitizers() -> RequiredSanitizers {
    let enabled = |name: &str| match std::env::var(name) {
        Err(std::env::VarError::NotPresent) => false,
        Ok(value) if value == "1" => true,
        Ok(value) => panic!("{name} must be unset or exactly `1`, got `{value}`"),
        Err(std::env::VarError::NotUnicode(_)) => panic!("{name} is not valid Unicode"),
    };
    match (
        enabled(REQUIRED_SANITIZERS_ENV),
        enabled(REQUIRED_RUST_HOST_ASAN_ENV),
    ) {
        (false, false) => RequiredSanitizers::None,
        (true, false) => RequiredSanitizers::AddressAndUndefined,
        (false, true) => RequiredSanitizers::Address,
        (true, true) => panic!("v3 sanitizer requirements are mutually exclusive"),
    }
}

fn assert_sanitizer_symbols(_library: &Path, sanitizers: RequiredSanitizers) {
    if sanitizers == RequiredSanitizers::None {
        return;
    }
    #[cfg(not(target_os = "linux"))]
    panic!("required v3 sanitizer evidence is supported only on Linux");
    #[cfg(target_os = "linux")]
    {
        let output = Command::new("nm").arg("-u").arg(_library).output().unwrap();
        assert!(
            output.status.success(),
            "nm failed for v3 sanitizer provider"
        );
        let symbols = String::from_utf8_lossy(&output.stdout);
        assert!(
            symbols.contains("__asan_"),
            "v3 provider lacks ASan callbacks"
        );
        if sanitizers == RequiredSanitizers::AddressAndUndefined {
            assert!(
                symbols.contains("__ubsan_"),
                "v3 provider lacks UBSan callbacks"
            );
        }
    }
}

fn shared_flags() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["-shared"]
    }
    #[cfg(target_os = "macos")]
    {
        &["-dynamiclib", "-fPIC"]
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        &["-shared", "-fPIC"]
    }
}

fn root_library_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "semaprax-v3-joint.dll"
    }
    #[cfg(target_os = "macos")]
    {
        "libsemaprax-v3-joint.dylib"
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "libsemaprax-v3-joint.so"
    }
}

fn c_string_literal(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_str()
            .expect("fixture path is UTF-8")
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    )
}
