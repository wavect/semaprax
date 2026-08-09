#![cfg(any(target_os = "linux", target_os = "macos"))]

use semaprax_native_loader::{
    open_admitted_settlement_exact, OpenError, SettlementCallError, MAX_CALL_WIRE_BYTES,
    MAX_DESCRIPTOR_BYTES,
};
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

struct Fixture {
    directory: PathBuf,
    library: PathBuf,
    descriptor: Vec<u8>,
    unload_marker: PathBuf,
}

impl Fixture {
    fn good() -> Self {
        let directory = fixture_directory("good");
        let unload_marker = directory.join("unloaded.marker");
        let descriptor = descriptor(GETTER, EXECUTE, SETTLE, None);
        let source = format!(
            r#"#include <stdint.h>
#include <stdio.h>
#include <string.h>
#define SPX_API __attribute__((visibility("default")))
static const uint8_t descriptor[] = {{ {descriptor_bytes} }};
SPX_API const uint8_t *{GETTER}(void) {{ return descriptor; }}
SPX_API uint32_t {EXECUTE}(const uint8_t *request, uint32_t request_len, uint8_t *frame, uint32_t frame_len, uint8_t *response, uint32_t response_len) {{
  if (request == NULL || frame == NULL || response == NULL) return UINT32_C(91);
  if (request_len != UINT32_C({REQUEST_BYTES}) || frame_len != UINT32_C({FRAME_BYTES}) || response_len != UINT32_C({RESPONSE_BYTES})) return UINT32_C(92);
  if (ranges_overlap(request, request_len, frame, frame_len) || ranges_overlap(request, request_len, response, response_len) || ranges_overlap(frame, frame_len, response, response_len)) return UINT32_C(93);
  response[0] = request[0]; frame[0] = UINT8_C(0x61); return UINT32_C(7);
}}
SPX_API uint32_t {SETTLE}(uint8_t *frame, uint32_t frame_len, const uint8_t *decision, uint32_t decision_len, uint8_t *candidate, uint32_t candidate_len) {{
  if (frame == NULL || decision == NULL || candidate == NULL) return UINT32_C(94);
  if (frame_len != UINT32_C({FRAME_BYTES}) || decision_len != UINT32_C({DECISION_BYTES}) || candidate_len != UINT32_C({CANDIDATE_BYTES})) return UINT32_C(95);
  if (ranges_overlap(frame, frame_len, decision, decision_len) || ranges_overlap(frame, frame_len, candidate, candidate_len) || ranges_overlap(decision, decision_len, candidate, candidate_len)) return UINT32_C(96);
  candidate[0] = decision[0]; frame[1] = UINT8_C(0x62); return UINT32_C(9);
}}
__attribute__((destructor)) static void on_unload(void) {{ FILE *file = fopen({marker}, "wb"); if (file != NULL) {{ fputs("unloaded", file); fclose(file); }} }}
"#,
            descriptor_bytes = c_bytes(&descriptor),
            marker = c_string_literal(&unload_marker),
        );
        let source = format!(
            "#include <stddef.h>\n#include <stdbool.h>\n#include <stdint.h>\nstatic bool ranges_overlap(const void *a, uint32_t an, const void *b, uint32_t bn) {{ uintptr_t x=(uintptr_t)a,y=(uintptr_t)b; return x < y + bn && y < x + an; }}\n{source}"
        );
        let library = compile_root(&directory, &source, &[]);
        Self {
            directory,
            library,
            descriptor,
            unload_marker,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).expect("remove settlement fixture directory");
    }
}

#[test]
fn exact_v3_execute_and_settle_are_preallocated_disjoint_and_one_shot() {
    let fixture = Fixture::good();
    // SAFETY: This generated root defines the exact synchronous getter,
    // execute, and settle ABIs and retains no supplied pointers.
    let lease = unsafe { open_admitted_settlement_exact(&fixture.library, &fixture.descriptor) }
        .expect("admit v3 fixture");
    assert_eq!(lease.canonical_path(), fixture.library);
    assert_eq!(lease.descriptor_len(), fixture.descriptor.len());
    assert!(lease.descriptor_matches(&fixture.descriptor));
    assert_eq!(lease.capacities().request(), REQUEST_BYTES as usize);
    assert_eq!(
        lease.capacities().execute_response(),
        RESPONSE_BYTES as usize
    );
    assert_eq!(lease.capacities().frame(), FRAME_BYTES as usize);
    assert_eq!(lease.capacities().decision(), DECISION_BYTES as usize);
    assert_eq!(
        lease.capacities().candidate_receipt(),
        CANDIDATE_BYTES as usize
    );

    let mut execute = lease.prepare_execute().expect("reserve all v3 buffers");
    assert!(execute.request_storage().iter().all(|byte| *byte == 0));
    assert!(execute.frame_storage().iter().all(|byte| *byte == 0));
    assert!(execute.response_storage().iter().all(|byte| *byte == 0));
    execute.request_storage_mut()[0] = 0xa5;
    assert_eq!(lease.invoke_execute(&mut execute), Ok(7));
    assert_eq!(execute.execute_return(), Some(7));
    assert_eq!(execute.response_storage()[0], 0xa5);
    assert_eq!(execute.frame_storage()[0], 0x61);
    assert_eq!(
        lease.invoke_execute(&mut execute),
        Err(SettlementCallError::ExecuteAlreadyInvoked)
    );

    let mut settlement = execute.into_settlement().expect("enter settle stage");
    assert_eq!(settlement.execute_return(), 7);
    settlement.decision_storage_mut()[0] = 0x5a;
    assert_eq!(lease.invoke_settle(&mut settlement), Ok(9));
    assert_eq!(settlement.settle_return(), Some(9));
    assert_eq!(settlement.candidate_storage()[0], 0x5a);
    assert_eq!(settlement.frame_storage()[1], 0x62);
    assert_eq!(
        lease.invoke_settle(&mut settlement),
        Err(SettlementCallError::SettleAlreadyInvoked)
    );
}

#[test]
fn v3_prepared_calls_are_exact_instance_bound_and_retains_pin_one_image() {
    let fixture = Fixture::good();
    // SAFETY: Both logical admissions use the exact generated trusted root.
    let first = unsafe { open_admitted_settlement_exact(&fixture.library, &fixture.descriptor) }
        .expect("first v3 open");
    let second = unsafe { open_admitted_settlement_exact(&fixture.library, &fixture.descriptor) }
        .expect("second v3 open");
    let retained = first.retain();
    assert!(first.is_same_instance(&retained));
    assert!(retained.descriptor_matches(&fixture.descriptor));
    assert!(!first.is_same_instance(&second));
    assert_ne!(first.instance_id(), second.instance_id());

    let mut execute = first.prepare_execute().expect("prepare first instance");
    assert_eq!(
        second.invoke_execute(&mut execute),
        Err(SettlementCallError::WrongModuleInstance)
    );
    assert_eq!(first.invoke_execute(&mut execute), Ok(7));
    let mut settlement = execute.into_settlement().unwrap();
    assert_eq!(
        second.invoke_settle(&mut settlement),
        Err(SettlementCallError::WrongModuleInstance)
    );
    assert_eq!(first.invoke_settle(&mut settlement), Ok(9));

    drop(first);
    drop(second);
    assert!(
        !fixture.unload_marker.exists(),
        "explicit retain must keep the single root-image pin live"
    );
    drop(retained);
    assert!(fixture.unload_marker.exists());
}

#[test]
fn v3_exact_descriptor_binding_rejects_same_capacity_cross_instance_substitution() {
    let first_directory = fixture_directory("binding-first");
    let second_directory = fixture_directory("binding-second");
    let first_descriptor = descriptor(GETTER, EXECUTE, SETTLE, None);
    let mut second_descriptor = first_descriptor.clone();
    *second_descriptor
        .last_mut()
        .expect("fixture descriptor graph is nonempty") ^= 0x5a;
    assert_eq!(first_descriptor.len(), second_descriptor.len());

    let first_library = compile_minimal_provider(&first_directory, &first_descriptor);
    let second_library = compile_minimal_provider(&second_directory, &second_descriptor);
    // SAFETY: Each isolated root exposes its own exact static descriptor and
    // the same bounded synchronous private entry ABIs.
    let first = unsafe { open_admitted_settlement_exact(&first_library, &first_descriptor) }
        .expect("admit first exact descriptor");
    // SAFETY: As above, for the separate second root and descriptor.
    let second = unsafe { open_admitted_settlement_exact(&second_library, &second_descriptor) }
        .expect("admit second exact descriptor");

    assert_eq!(first.capacities(), second.capacities());
    assert!(!first.is_same_instance(&second));
    assert!(first.descriptor_matches(&first_descriptor));
    assert!(!first.descriptor_matches(&second_descriptor));
    assert!(second.descriptor_matches(&second_descriptor));
    assert!(!second.descriptor_matches(&first_descriptor));
    assert!(first.retain().descriptor_matches(&first_descriptor));

    drop(first);
    drop(second);
    fs::remove_dir_all(first_directory).expect("remove first binding fixture");
    fs::remove_dir_all(second_directory).expect("remove second binding fixture");
}

#[test]
fn v3_descriptor_schema_symbol_and_capacity_failures_precede_image_loading() {
    let fixture = Fixture::good();
    let invalid = [
        b"SPXNABI2".to_vec(),
        b"SPXNPRF1".to_vec(),
        descriptor(GETTER, GETTER, SETTLE, None),
        descriptor(GETTER, EXECUTE, SETTLE, Some((0, 0))),
        descriptor(GETTER, EXECUTE, SETTLE, Some((3, 171))),
        descriptor(GETTER, EXECUTE, SETTLE, Some((12, 255))),
        descriptor(GETTER, EXECUTE, SETTLE, Some((14, u32::MAX))),
    ];
    for bytes in invalid {
        // SAFETY: Every input is rejected structurally before the image opens.
        let error =
            settlement_error(unsafe { open_admitted_settlement_exact(&fixture.library, &bytes) });
        assert!(matches!(
            error,
            OpenError::InvalidSettlementDescriptorSchema
                | OpenError::InvalidSettlementSymbols
                | OpenError::InvalidSettlementCapacities
        ));
        assert!(!fixture.unload_marker.exists());
    }

    for bytes in [Vec::new(), vec![0; MAX_DESCRIPTOR_BYTES + 1]] {
        // SAFETY: Length validation rejects before any native operation.
        assert!(matches!(
            settlement_error(unsafe { open_admitted_settlement_exact(&fixture.library, &bytes) }),
            OpenError::InvalidExpectedDescriptorLength { .. }
        ));
    }
    assert!(MAX_CALL_WIRE_BYTES >= CANDIDATE_BYTES as usize);
}

#[cfg(target_os = "linux")]
#[test]
fn v3_pairwise_distinct_names_cannot_resolve_to_aliased_addresses() {
    let directory = fixture_directory("aliases");
    let descriptor = descriptor(GETTER, EXECUTE, SETTLE, None);
    let source = format!(
        r#"#include <stdint.h>
#define SPX_API __attribute__((visibility("default")))
static const uint8_t descriptor[] = {{ {bytes} }};
SPX_API const uint8_t *{GETTER}(void) {{ return descriptor; }}
SPX_API uint32_t shared_entry(const uint8_t *a,uint32_t b,uint8_t *c,uint32_t d,uint8_t *e,uint32_t f) {{ (void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0; }}
extern __typeof(shared_entry) {EXECUTE} __attribute__((alias("shared_entry"),visibility("default")));
extern __typeof(shared_entry) {SETTLE} __attribute__((alias("shared_entry"),visibility("default")));
"#,
        bytes = c_bytes(&descriptor),
    );
    let library = compile_root(&directory, &source, &[]);
    // SAFETY: The alias fixture has ABI-compatible entries; admission rejects
    // their identical addresses before either provider entry is invoked.
    let error = settlement_error(unsafe { open_admitted_settlement_exact(&library, &descriptor) });
    assert!(matches!(error, OpenError::AliasedSettlementSymbols));
    fs::remove_dir_all(directory).expect("remove alias fixture");
}

#[test]
fn v3_missing_symbol_fails_closed_after_root_getter_resolution() {
    let directory = fixture_directory("missing-symbol");
    let descriptor = descriptor(GETTER, "spx_missing_execute_v3", SETTLE, None);
    let source = format!(
        r#"#include <stdint.h>
#define SPX_API __attribute__((visibility("default")))
static const uint8_t descriptor[] = {{ {bytes} }};
SPX_API const uint8_t *{GETTER}(void) {{ return descriptor; }}
SPX_API uint32_t {SETTLE}(uint8_t *a,uint32_t b,const uint8_t *c,uint32_t d,uint8_t *e,uint32_t f) {{ (void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0; }}
"#,
        bytes = c_bytes(&descriptor),
    );
    let library = compile_root(&directory, &source, &[]);
    // SAFETY: The root getter/settle ABIs are exact; execute lookup is absent.
    let error = settlement_error(unsafe { open_admitted_settlement_exact(&library, &descriptor) });
    assert!(matches!(error, OpenError::ExecuteLookup(_)));
    fs::remove_dir_all(directory).expect("remove missing-symbol fixture");
}

#[test]
fn v3_dependency_execute_symbol_is_rejected_by_root_image_provenance() {
    let directory = fixture_directory("dependency-symbol");
    let descriptor = descriptor(GETTER, EXECUTE, SETTLE, None);
    let dependency = "spxv3execdependency";
    compile_dependency(
        &directory,
        dependency,
        r#"#include <stdint.h>
__attribute__((visibility("default"))) uint32_t spx_dependency_anchor(void) { return 1; }
__attribute__((visibility("default"))) uint32_t spx_execute_v3(const uint8_t *a,uint32_t b,uint8_t *c,uint32_t d,uint8_t *e,uint32_t f) { (void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0; }
"#,
    );
    let source = format!(
        r#"#include <stdint.h>
#define SPX_API __attribute__((visibility("default")))
extern uint32_t spx_dependency_anchor(void);
static const uint8_t descriptor[] = {{ {bytes} }};
SPX_API const uint8_t *{GETTER}(void) {{ (void)spx_dependency_anchor(); return descriptor; }}
SPX_API uint32_t {SETTLE}(uint8_t *a,uint32_t b,const uint8_t *c,uint32_t d,uint8_t *e,uint32_t f) {{ (void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0; }}
"#,
        bytes = c_bytes(&descriptor),
    );
    let link = dependency_link_args(dependency);
    let library = compile_root(&directory, &source, &link);
    // SAFETY: All symbols have compatible ABIs; provenance must reject the
    // dependency-owned execute address before invocation.
    let error = settlement_error(unsafe { open_admitted_settlement_exact(&library, &descriptor) });
    assert!(matches!(error, OpenError::RootImageProvenanceMismatch));
    fs::remove_dir_all(directory).expect("remove dependency-symbol fixture");
}

#[test]
fn v3_dependency_descriptor_storage_is_rejected_by_root_image_provenance() {
    let directory = fixture_directory("dependency-descriptor");
    let descriptor = descriptor(GETTER, EXECUTE, SETTLE, None);
    let dependency = "spxv3descriptordependency";
    compile_dependency(
        &directory,
        dependency,
        &format!(
            r#"#include <stdint.h>
static const uint8_t dependency_descriptor[] = {{ {bytes} }};
__attribute__((visibility("default"))) const uint8_t *spx_dependency_descriptor(void) {{ return dependency_descriptor; }}
"#,
            bytes = c_bytes(&descriptor),
        ),
    );
    let source = format!(
        r#"#include <stdint.h>
#define SPX_API __attribute__((visibility("default")))
extern const uint8_t *spx_dependency_descriptor(void);
SPX_API const uint8_t *{GETTER}(void) {{ return spx_dependency_descriptor(); }}
SPX_API uint32_t {EXECUTE}(const uint8_t *a,uint32_t b,uint8_t *c,uint32_t d,uint8_t *e,uint32_t f) {{ (void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0; }}
SPX_API uint32_t {SETTLE}(uint8_t *a,uint32_t b,const uint8_t *c,uint32_t d,uint8_t *e,uint32_t f) {{ (void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0; }}
"#,
    );
    let link = dependency_link_args(dependency);
    let library = compile_root(&directory, &source, &link);
    // SAFETY: Getter and entries have exact ABIs; the returned immutable bytes
    // deliberately belong to the dependency and must fail provenance.
    let error = settlement_error(unsafe { open_admitted_settlement_exact(&library, &descriptor) });
    assert!(matches!(error, OpenError::RootImageProvenanceMismatch));
    fs::remove_dir_all(directory).expect("remove dependency-descriptor fixture");
}

fn descriptor(
    getter: &str,
    execute: &str,
    settle: &str,
    changed_capacity: Option<(usize, u32)>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"SPXNABI3");
    push_u32(&mut bytes, 3);
    push_u32(&mut bytes, 20);
    push_u32(&mut bytes, 0);
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
    for capacity in capacities {
        push_u32(&mut bytes, capacity);
    }
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 0);
    push_text(&mut bytes, "value-owned");
    push_u32(&mut bytes, 0);
    push_text(&mut bytes, "resource.fixture");
    push_text(&mut bytes, "lifecycle.fixture");
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 0);
    push_text(&mut bytes, "value-owned");
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 1);
    bytes.push(1);
    let total = u32::try_from(bytes.len()).expect("fixture descriptor fits u32");
    bytes[16..20].copy_from_slice(&total.to_le_bytes());
    bytes
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_text(bytes: &mut Vec<u8>, value: &str) {
    push_u32(
        bytes,
        u32::try_from(value.len()).expect("fixture text fits u32"),
    );
    bytes.extend_from_slice(value.as_bytes());
}

fn settlement_error(
    result: Result<semaprax_native_loader::NativeSettlementModuleLease, OpenError>,
) -> OpenError {
    match result {
        Ok(_) => panic!("settlement admission unexpectedly succeeded"),
        Err(error) => error,
    }
}

fn fixture_directory(label: &str) -> PathBuf {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-v3-loader-{label}-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("create settlement fixture directory");
    fs::canonicalize(path).expect("canonical settlement fixture directory")
}

fn compile_root(directory: &Path, source: &str, extra: &[String]) -> PathBuf {
    let source_path = directory.join("root.c");
    let library = directory.join(root_library_name());
    fs::write(&source_path, source).expect("write settlement root source");
    let mut command = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()));
    command.current_dir(directory);
    command
        .args(shared_flags())
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"]);
    command
        .arg(&source_path)
        .args(extra)
        .arg("-o")
        .arg(&library);
    let output = command.output().expect("run settlement root compiler");
    assert!(
        output.status.success(),
        "settlement root compiler failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::canonicalize(library).expect("canonical settlement root library")
}

fn compile_minimal_provider(directory: &Path, descriptor: &[u8]) -> PathBuf {
    let source = format!(
        r#"#include <stdint.h>
#define SPX_API __attribute__((visibility("default")))
static const uint8_t descriptor[] = {{ {bytes} }};
SPX_API const uint8_t *{GETTER}(void) {{ return descriptor; }}
SPX_API uint32_t {EXECUTE}(const uint8_t *a,uint32_t b,uint8_t *c,uint32_t d,uint8_t *e,uint32_t f) {{ (void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0; }}
SPX_API uint32_t {SETTLE}(uint8_t *a,uint32_t b,const uint8_t *c,uint32_t d,uint8_t *e,uint32_t f) {{ (void)a;(void)b;(void)c;(void)d;(void)e;(void)f;return 0; }}
"#,
        bytes = c_bytes(descriptor),
    );
    compile_root(directory, &source, &[])
}

fn compile_dependency(directory: &Path, stem: &str, source: &str) {
    let source_path = directory.join("dependency.c");
    let library = directory.join(dependency_library_name(stem));
    fs::write(&source_path, source).expect("write settlement dependency source");
    let mut command = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()));
    command.current_dir(directory);
    command
        .args(shared_flags())
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"]);
    #[cfg(target_os = "macos")]
    command.args(["-install_name", &format!("@rpath/lib{stem}.dylib")]);
    let output = command
        .arg(&source_path)
        .arg("-o")
        .arg(&library)
        .output()
        .expect("run settlement dependency compiler");
    assert!(
        output.status.success(),
        "settlement dependency compiler failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn shared_flags() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &["-dynamiclib", "-fPIC"]
    }
    #[cfg(not(target_os = "macos"))]
    {
        &["-shared", "-fPIC"]
    }
}

fn dependency_link_args(stem: &str) -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        vec![
            "-L.".to_owned(),
            format!("-l{stem}"),
            "-Wl,-rpath,@loader_path".to_owned(),
        ]
    }
    #[cfg(not(target_os = "macos"))]
    {
        vec![
            "-L.".to_owned(),
            format!("-l{stem}"),
            "-Wl,-rpath,$ORIGIN".to_owned(),
        ]
    }
}

fn root_library_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "libspxv3root.dylib"
    }
    #[cfg(not(target_os = "macos"))]
    {
        "libspxv3root.so"
    }
}

fn dependency_library_name(stem: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        format!("lib{stem}.dylib")
    }
    #[cfg(not(target_os = "macos"))]
    {
        format!("lib{stem}.so")
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
    let text = path.to_str().expect("fixture path is UTF-8");
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}
