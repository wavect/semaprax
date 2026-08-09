#![cfg(target_os = "windows")]

use semaprax_native_loader::{open_admitted_settlement_exact, OpenError, SettlementCallError};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const GETTER: &str = "spx_descriptor_v3";
const EXECUTE: &str = "spx_execute_v3";
const SETTLE: &str = "spx_settle_v3";
const REQUEST_BYTES: u32 = 124;
const RESPONSE_BYTES: u32 = 164;
const FRAME_BYTES: u32 = 400;
const DECISION_BYTES: u32 = 172;
const ACTION_BYTES: u32 = 196;
const CANDIDATE_BYTES: u32 = 384;
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

#[test]
fn windows_v3_exact_instance_bounds_aliases_and_final_retain_are_closed() {
    let directory = fixture_directory("positive");
    let descriptor = descriptor(GETTER, EXECUTE, SETTLE, None);
    let marker = directory.join("unloaded.marker");
    let root = compile_root(
        &directory,
        &provider_source(&descriptor, &marker, false),
        &[],
    );
    // SAFETY: This private fixture has exact cdecl ABIs, static descriptor
    // storage, bounded synchronous access, and no retained pointers.
    let first = unsafe { open_admitted_settlement_exact(&root, &descriptor) }.unwrap();
    // SAFETY: Same generated trusted root and ABI, separate logical admission.
    let second = unsafe { open_admitted_settlement_exact(&root, &descriptor) }.unwrap();
    let retained = first.retain();
    assert!(first.is_same_instance(&retained));
    assert!(first.descriptor_matches(&descriptor));
    assert!(retained.descriptor_matches(&descriptor));
    assert!(!first.is_same_instance(&second));
    assert_ne!(first.instance_id(), second.instance_id());
    let mut execute = first.prepare_execute().unwrap();
    assert_eq!(
        second.invoke_execute(&mut execute),
        Err(SettlementCallError::WrongModuleInstance)
    );
    execute.request_storage_mut()[0] = 0xa5;
    assert_eq!(first.invoke_execute(&mut execute), Ok(7));
    assert_eq!(execute.response_storage()[0], 0xa5);
    let mut settle = execute.into_settlement().unwrap();
    settle.decision_storage_mut()[0] = 0x5a;
    assert_eq!(first.invoke_settle(&mut settle), Ok(9));
    assert_eq!(settle.candidate_storage()[0], 0x5a);
    assert_eq!(
        first.invoke_settle(&mut settle),
        Err(SettlementCallError::SettleAlreadyInvoked)
    );
    drop(first);
    drop(second);
    assert!(!marker.exists());
    drop(retained);
    assert!(
        marker.exists(),
        "final explicit retain must release one image pin"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn windows_v3_exact_descriptor_binding_rejects_same_capacity_substitution() {
    let directory = fixture_directory("descriptor-binding");
    let descriptor = descriptor(GETTER, EXECUTE, SETTLE, None);
    let marker = directory.join("unloaded.marker");
    let root = compile_root(
        &directory,
        &provider_source(&descriptor, &marker, false),
        &[],
    );
    // SAFETY: The generated provider exposes the exact bounded private ABI.
    let lease = unsafe { open_admitted_settlement_exact(&root, &descriptor) }.unwrap();
    let mut substitute = descriptor.clone();
    *substitute.last_mut().unwrap() ^= 0xa5;
    assert_eq!(substitute.len(), descriptor.len());
    assert!(lease.descriptor_matches(&descriptor));
    assert!(!lease.descriptor_matches(&substitute));
    drop(lease);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn windows_v3_capacity_and_pairwise_symbol_aliases_fail_closed() {
    let directory = fixture_directory("alias");
    let canonical = descriptor(GETTER, EXECUTE, SETTLE, None);
    let marker = directory.join("unloaded.marker");
    let root = compile_root(&directory, &provider_source(&canonical, &marker, true), &[]);
    // SAFETY: Alias entries are ABI-compatible; their identical allocation
    // address must be rejected before provider invocation.
    let error = settlement_error(unsafe { open_admitted_settlement_exact(&root, &canonical) });
    assert!(matches!(error, OpenError::AliasedSettlementSymbols));

    let invalid = descriptor(GETTER, EXECUTE, SETTLE, Some((5, CANDIDATE_BYTES - 1)));
    // SAFETY: Capacity projection rejects before opening the root image.
    assert!(matches!(
        settlement_error(unsafe { open_admitted_settlement_exact(&root, &invalid) }),
        OpenError::InvalidSettlementCapacities
    ));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn windows_v3_dependency_owned_descriptor_bytes_fail_root_provenance() {
    let directory = fixture_directory("dependency-provenance");
    let descriptor = descriptor(GETTER, EXECUTE, SETTLE, None);
    let dependency_source = format!(
        r#"#include <stdint.h>
#define SPX_API __declspec(dllexport)
static const uint8_t dependency_descriptor[] = {{ {bytes} }};
SPX_API const uint8_t *__cdecl spx_dependency_descriptor(void) {{ return dependency_descriptor; }}
"#,
        bytes = c_bytes(&descriptor),
    );
    let import_library = compile_dependency(&directory, &dependency_source);
    let root_source = format!(
        r#"#include <stdint.h>
#define SPX_API __declspec(dllexport)
__declspec(dllimport) const uint8_t *__cdecl spx_dependency_descriptor(void);
SPX_API const uint8_t *__cdecl {GETTER}(void) {{ return spx_dependency_descriptor(); }}
SPX_API uint32_t __cdecl {EXECUTE}(const uint8_t *a,uint32_t b,uint8_t *c,uint32_t d,uint8_t *e,uint32_t f) {{ (void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0; }}
SPX_API uint32_t __cdecl {SETTLE}(uint8_t *a,uint32_t b,const uint8_t *c,uint32_t d,uint8_t *e,uint32_t f) {{ (void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0; }}
"#,
    );
    let root = compile_root(&directory, &root_source, &[&import_library]);
    // SAFETY: All functions have the exact ABI, while returned immutable
    // descriptor storage deliberately belongs to the dependency allocation.
    let error = settlement_error(unsafe { open_admitted_settlement_exact(&root, &descriptor) });
    assert!(matches!(error, OpenError::RootImageProvenanceMismatch));
    fs::remove_dir_all(directory).unwrap();
}

fn provider_source(descriptor: &[u8], marker: &Path, alias_entries: bool) -> String {
    let entries = if alias_entries {
        r#"SPX_API uint32_t __cdecl shared_entry(const uint8_t *a,uint32_t b,uint8_t *c,uint32_t d,uint8_t *e,uint32_t f) { (void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0; }
#pragma comment(linker, "/export:spx_execute_v3=shared_entry")
#pragma comment(linker, "/export:spx_settle_v3=shared_entry")
"#
        .to_owned()
    } else {
        format!(
            r#"SPX_API uint32_t __cdecl {EXECUTE}(const uint8_t *request,uint32_t request_len,uint8_t *frame,uint32_t frame_len,uint8_t *response,uint32_t response_len) {{ if(request_len!={REQUEST_BYTES}||frame_len!={FRAME_BYTES}||response_len!={RESPONSE_BYTES})return 90;response[0]=request[0];frame[0]=0x61;return 7; }}
SPX_API uint32_t __cdecl {SETTLE}(uint8_t *frame,uint32_t frame_len,const uint8_t *decision,uint32_t decision_len,uint8_t *candidate,uint32_t candidate_len) {{ if(frame_len!={FRAME_BYTES}||decision_len!={DECISION_BYTES}||candidate_len!={CANDIDATE_BYTES})return 91;candidate[0]=decision[0];frame[1]=0x62;return 9; }}
"#,
        )
    };
    format!(
        r#"#include <stdint.h>
#include <windows.h>
#define SPX_API __declspec(dllexport)
static const uint8_t descriptor[] = {{ {bytes} }};
SPX_API const uint8_t *__cdecl {GETTER}(void) {{ return descriptor; }}
{entries}
BOOL WINAPI DllMain(HINSTANCE module,DWORD reason,LPVOID reserved) {{
  (void)module;
  if (reason == DLL_PROCESS_DETACH && reserved == NULL) {{
    HANDLE file=CreateFileA({marker},GENERIC_WRITE,0,NULL,CREATE_ALWAYS,FILE_ATTRIBUTE_NORMAL,NULL);
    if(file!=INVALID_HANDLE_VALUE){{DWORD written=0;const char text[]="unloaded";(void)WriteFile(file,text,8,&written,NULL);CloseHandle(file);}}
  }}
  return TRUE;
}}
"#,
        bytes = c_bytes(descriptor),
        marker = c_string_literal(marker),
    )
}

fn descriptor(
    getter: &str,
    execute: &str,
    settle: &str,
    changed_capacity: Option<(usize, u32)>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"SPXNABI3");
    for value in [3, 20, 0] {
        push_u32(&mut bytes, value);
    }
    push_text(&mut bytes, "fixture-target");
    push_u32(&mut bytes, 1);
    for index in 1_u8..=19 {
        bytes.extend_from_slice(&[index; 32]);
    }
    for text in [
        "fixture-module",
        "fixture-function",
        getter,
        execute,
        settle,
    ] {
        push_text(&mut bytes, text);
    }
    push_u32(&mut bytes, 3);
    push_u32(&mut bytes, 0x03ff);
    let reserved = 320_u32
        * (REQUEST_BYTES
            + RESPONSE_BYTES
            + FRAME_BYTES
            + DECISION_BYTES
            + ACTION_BYTES
            + CANDIDATE_BYTES
            + 524);
    let mut capacities = [
        REQUEST_BYTES,
        RESPONSE_BYTES,
        FRAME_BYTES,
        DECISION_BYTES,
        ACTION_BYTES,
        CANDIDATE_BYTES,
        2,
        1,
        1,
        1,
        1,
        1,
        256,
        64,
        reserved,
    ];
    if let Some((index, value)) = changed_capacity {
        capacities[index] = value;
    }
    for value in capacities {
        push_u32(&mut bytes, value);
    }
    for value in [1, 2, 0] {
        push_u32(&mut bytes, value);
    }
    push_text(&mut bytes, "value-owned");
    push_u32(&mut bytes, 0);
    push_text(&mut bytes, "resource.fixture");
    push_text(&mut bytes, "lifecycle.fixture");
    for value in [1, 2, 0] {
        push_u32(&mut bytes, value);
    }
    push_text(&mut bytes, "value-owned");
    for value in [0, 1] {
        push_u32(&mut bytes, value);
    }
    bytes.push(1);
    let total = u32::try_from(bytes.len()).unwrap();
    bytes[16..20].copy_from_slice(&total.to_le_bytes());
    bytes
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_text(bytes: &mut Vec<u8>, value: &str) {
    push_u32(bytes, u32::try_from(value.len()).unwrap());
    bytes.extend_from_slice(value.as_bytes());
}

fn fixture_directory(label: &str) -> PathBuf {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-v3-loader-windows-{label}-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    fs::canonicalize(directory).unwrap()
}

fn compile_root(directory: &Path, source: &str, libraries: &[&str]) -> PathBuf {
    let source_path = directory.join("root.c");
    fs::write(&source_path, source).unwrap();
    let output_name = "spx-v3-root.dll";
    run_compiler(
        directory,
        &[
            vec![
                "-shared".to_owned(),
                "-O2".to_owned(),
                "-std=c11".to_owned(),
                "-Wall".to_owned(),
                "-Wextra".to_owned(),
                "-Werror".to_owned(),
                "root.c".to_owned(),
            ],
            libraries.iter().map(|value| (*value).to_owned()).collect(),
            vec!["-o".to_owned(), output_name.to_owned()],
        ]
        .concat(),
    );
    fs::canonicalize(directory.join(output_name)).unwrap()
}

fn compile_dependency(directory: &Path, source: &str) -> String {
    fs::write(directory.join("dependency.c"), source).unwrap();
    let import = "spx-v3-dependency.lib";
    run_compiler(
        directory,
        &[
            "-shared".to_owned(),
            "-O2".to_owned(),
            "-std=c11".to_owned(),
            "-Wall".to_owned(),
            "-Wextra".to_owned(),
            "-Werror".to_owned(),
            "dependency.c".to_owned(),
            "-o".to_owned(),
            "spx-v3-dependency.dll".to_owned(),
            format!("-Wl,/implib:{import}"),
        ],
    );
    import.to_owned()
}

fn run_compiler(directory: &Path, arguments: &[String]) {
    let output = Command::new(std::env::var_os("CC").unwrap_or_else(|| "clang".into()))
        .current_dir(directory)
        .args(arguments)
        .output()
        .expect("run Windows settlement fixture compiler");
    assert!(
        output.status.success(),
        "Windows settlement compiler failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn settlement_error(
    result: Result<semaprax_native_loader::NativeSettlementModuleLease, OpenError>,
) -> OpenError {
    match result {
        Ok(_) => panic!("settlement admission unexpectedly succeeded"),
        Err(error) => error,
    }
}

fn c_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn c_string_literal(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_str()
            .unwrap()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    )
}
