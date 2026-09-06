use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::hir::{self, DeclarationId, ResolvedType};
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, parse, verify, wasm};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const PREFIX: &str = r#"
module test.generic_owned_function_runtime;
@id("generic.function.pair") record Pair<T, U> {
  @id("generic.function.pair.payload") payload: T,
  @id("generic.function.pair.marker") marker: U,
}
@id("generic.function.box") record Box<T> {
  @id("generic.function.box.value") value: T,
}
@id("generic.function.relay")
fn relay<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> { value }
@id("generic.function.reject")
fn reject<T>(value: own Pair<Bytes, T>, allowed: bool) -> Pair<Bytes, T>
  requires allowed
{ value }
@id("generic.function.nested-relay-box")
fn relay_box<T>(value: own Box<Pair<Bytes, T>>) -> Box<Pair<Bytes, T>> { value }
@id("generic.function.nested-relay-pair")
fn relay_pair<T>(value: own Pair<Box<Bytes>, T>) -> Pair<Box<Bytes>, T> { value }
@id("generic.function.nested-requires")
fn nested_requires<T>(value: own Box<Pair<Bytes, T>>, allowed: bool) -> Box<Pair<Bytes, T>>
  requires allowed
{ value }
@id("generic.function.nested-ensures")
fn nested_ensures<T>(value: own Box<Pair<Bytes, T>>) -> Box<Pair<Bytes, T>>
  ensures false
{ value }
@id("generic.function.nested-stage")
fn nested_stage<T>(value: own Box<Pair<Bytes, T>>, marker: bool) -> Box<Pair<Bytes, T>> {
  value
}
@id("generic.function.consume-u8")
fn consume_u8(value: own Pair<Bytes, u8>) -> i64 {
  match own value {
    Pair { payload: payload, marker: marker } =>
      if byte_len(bytes_as_slice(payload)) == 3usize && marker == 7u8 { 42 } else { 0 },
  }
}
@id("generic.function.consume-bool")
fn consume_bool(value: own Pair<Bytes, bool>) -> i64 {
  match own value {
    Pair { payload: payload, marker: marker } =>
      if byte_len(bytes_as_slice(payload)) == 1usize && marker { 1 } else { 0 },
  }
}
@id("generic.function.consume-nested-box")
fn consume_nested_box(value: own Box<Pair<Bytes, bool>>) -> i64 {
  match own value {
    Box { value: Pair { payload: payload, marker: marker } } =>
      if byte_len(bytes_as_slice(payload)) == 2usize && marker { 19 } else { 0 },
  }
}
@id("generic.function.consume-nested-pair")
fn consume_nested_pair(value: own Pair<Box<Bytes>, i64>) -> i64 {
  match own value {
    Pair { payload: Box { value: payload }, marker: marker } =>
      if byte_len(bytes_as_slice(payload)) == 1usize { marker } else { 0 },
  }
}
@id("generic.function.make-nested-box")
fn make_nested_box() -> Box<Pair<Bytes, bool>> {
  let input = [10u8, 11u8];
  Box<Pair<Bytes, bool>> {
    value: Pair<Bytes, bool> {
      payload: bytes_copy(array_as_slice(input)), marker: true,
    },
  }
}
@id("generic.function.make-nested-pair")
fn make_nested_pair() -> Pair<Box<Bytes>, i64> {
  let input = [12u8];
  Pair<Box<Bytes>, i64> {
    payload: Box<Bytes> { value: bytes_copy(array_as_slice(input)) }, marker: 23,
  }
}
@id("generic.function.nested-success") fn nested_success() -> i64 {
  let boxed = make_nested_box();
  let paired = make_nested_pair();
  consume_nested_box(relay_box<bool>(boxed)) + consume_nested_pair(relay_pair<i64>(paired))
}
@id("generic.function.success") fn success() -> i64 {
  let left = [1u8, 2u8, 3u8];
  let first = Pair<Bytes, u8> {
    payload: bytes_copy(array_as_slice(left)), marker: 7u8,
  };
  let relayed = relay<u8>(relay<u8>(first));
  let right = [9u8];
  let second = Pair<Bytes, bool> {
    payload: bytes_copy(array_as_slice(right)), marker: true,
  };
  let flat = consume_u8(relayed) + consume_bool(relay<bool>(second)) - 1;
  if nested_success() == 42 { flat } else { 0 }
}
@id("generic.function.failure") fn failure() -> i64 {
  let input = [4u8, 5u8, 6u8];
  let value = Pair<Bytes, u8> {
    payload: bytes_copy(array_as_slice(input)), marker: 7u8,
  };
  consume_u8(reject<u8>(value, false))
}
@id("generic.function.nested-requires-failure") fn nested_requires_failure() -> i64 {
  consume_nested_box(nested_requires<bool>(make_nested_box(), false))
}
@id("generic.function.nested-ensures-failure") fn nested_ensures_failure() -> i64 {
  consume_nested_box(nested_ensures<bool>(make_nested_box()))
}
@id("generic.function.nested-argument-failure") fn nested_argument_failure() -> i64 {
  consume_nested_box(nested_stage<bool>(make_nested_box(), (1 / 0) == 0))
}
"#;

#[derive(Clone, Copy)]
enum Expected {
    Value(i64),
    Failure(&'static str, u32, &'static str),
}

fn source(entry: &str) -> String {
    let call = match entry {
        "generic.function.success" => "success",
        "generic.function.failure" => "failure",
        "generic.function.nested-requires-failure" => "nested_requires_failure",
        "generic.function.nested-ensures-failure" => "nested_ensures_failure",
        "generic.function.nested-argument-failure" => "nested_argument_failure",
        _ => panic!("unknown entry"),
    };
    format!("{PREFIX}\n@id(\"app.main\") fn main() -> i64 {{ {call}() }}\n")
}

fn checked(entry: &str) -> semaprax::ast::Program {
    let source = source(entry);
    let parsed = parse(&source, Path::new("generic-owned-function-runtime-v1.spx")).unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "{diagnostics:?}"
    );
    let resolved = hir::resolve(&parsed).expect("owned generic function instances resolve");
    assert_nested_instances(&resolved);
    parsed
}

fn nominal(declaration: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(declaration),
        arguments,
    }
}

fn assert_nested_instances(program: &hir::ResolvedProgram) {
    let pair_bytes_bool = nominal(
        "generic.function.pair",
        vec![ResolvedType::Bytes, ResolvedType::Bool],
    );
    let box_pair = nominal("generic.function.box", vec![pair_bytes_bool]);
    let box_bytes = nominal("generic.function.box", vec![ResolvedType::Bytes]);
    let pair_box = nominal("generic.function.pair", vec![box_bytes, ResolvedType::I64]);
    for (template, argument, expected, path) in [
        (
            "generic.function.nested-relay-box",
            ResolvedType::Bool,
            box_pair,
            [
                "generic.function.box.value",
                "generic.function.pair.payload",
            ],
        ),
        (
            "generic.function.nested-relay-pair",
            ResolvedType::I64,
            pair_box,
            [
                "generic.function.pair.payload",
                "generic.function.box.value",
            ],
        ),
    ] {
        let instance = program
            .function_instances
            .iter()
            .find(|instance| instance.template.as_str() == template)
            .unwrap_or_else(|| panic!("missing nested generic instance {template}"));
        assert_eq!(instance.type_arguments, [argument]);
        assert_eq!(
            instance.id,
            hir::FunctionInstanceId::derive(&instance.template, &instance.type_arguments)
        );
        assert_eq!(
            instance.function.params[0].ownership,
            hir::OwnershipMode::Own
        );
        assert_eq!(instance.function.params[0].ty, expected);
        assert_eq!(instance.function.return_type, expected);
        assert_eq!(
            instance.function.cleanup_plan.schema,
            "semaprax.cleanup-plan.v7"
        );
        let expected_path = path.map(DeclarationId::new).to_vec();
        assert_eq!(
            instance
                .function
                .cleanup
                .flags
                .iter()
                .filter(|flag| flag.place.projections == expected_path)
                .count(),
            3,
            "nested relay must track the owned leaf in its parameter, body, and result places"
        );
    }
}

#[test]
fn generic_owned_function_instances_settle_and_reenter_on_three_engines() {
    for (entry, expected) in [
        ("generic.function.success", Expected::Value(42)),
        (
            "generic.function.failure",
            Expected::Failure("semaprax.contract.v1", 1, "SEMAPRAX contract failure"),
        ),
        (
            "generic.function.nested-requires-failure",
            Expected::Failure("semaprax.contract.v1", 1, "SEMAPRAX contract failure"),
        ),
        (
            "generic.function.nested-ensures-failure",
            Expected::Failure("semaprax.contract.v1", 2, "SEMAPRAX contract failure"),
        ),
        (
            "generic.function.nested-argument-failure",
            Expected::Failure(
                "semaprax.arithmetic.v1",
                4,
                "SEMAPRAX checked arithmetic failure: invalid division",
            ),
        ),
    ] {
        let parsed = checked(entry);
        run_interpreter(entry, expected);
        if Command::new("clang").arg("--version").output().is_ok() {
            run_native(&parsed, expected);
        }
        if Command::new("node").arg("--version").output().is_ok() {
            run_wasm(entry, &parsed, expected);
        }
    }
}

fn run_interpreter(entry: &str, expected: Expected) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-generic-owned-function-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source(entry)).unwrap();
    for _ in 0..4 {
        let result =
            interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        interpreter::verify_envelope(&result.envelope).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        match expected {
            Expected::Value(value) => {
                assert!(result.returned, "{entry}");
                assert_eq!(envelope["payload"]["outcome"]["value"], value.to_string());
            }
            Expected::Failure(domain, code, _) => {
                assert!(!result.returned, "{entry}");
                assert_eq!(
                    envelope["payload"]["outcome"]["status"]["domain_id"],
                    domain
                );
                assert_eq!(envelope["payload"]["outcome"]["status"]["code"], code);
            }
        }
    }
    let _ = std::fs::remove_file(path);
}

fn run_native(parsed: &semaprax::ast::Program, expected: Expected) {
    let generated = codegen::emit_c(parsed).unwrap();
    assert_eq!(generated, codegen::emit_c(parsed).unwrap());
    let mut ownership_surface = generated.clone();
    for admitted in [
        "memcpy(payload, value.ptr, (size_t)value.len);",
        "memcpy(entry->domain_storage, status.domain_id, domain_size);",
    ] {
        assert_eq!(ownership_surface.matches(admitted).count(), 1);
        ownership_surface = ownership_surface.replacen(admitted, "", 1);
    }
    assert!(!ownership_surface.contains("memcpy("));
    let tracked = generated
        .replace(
            "uint8_t *payload = (uint8_t *)malloc(",
            "uint8_t *payload = (uint8_t *)spx_test_malloc(",
        )
        .replace("free(value->ptr);", "spx_test_free(value->ptr);");
    let condition = match expected {
        Expected::Value(value) => format!(
            "status != SPX_STATUS_SUCCESS || result != INT64_C({value})"
        ),
        Expected::Failure(domain, code, _) => format!(
            "status == SPX_STATUS_SUCCESS || result != INT64_C(0x2525252525252525) || spx_status_resolve(&context, status) == NULL || strcmp(spx_status_resolve(&context, status)->domain_id, \"{domain}\") != 0 || spx_status_resolve(&context, status)->code != UINT32_C({code})"
        ),
    };
    let probe = format!(
        r#"
int main(void) {{
  struct spx_status_entry entries[UINT32_C(32)];
  struct spx_context context = {{0}};
  if (!spx_context_init(&context, UINT64_C(17), entries, UINT32_C(32), NULL, NULL, NULL)) return 1;
  for (uint32_t i = 0; i < UINT32_C(4); ++i) {{
    int64_t result = INT64_C(0x2525252525252525);
    uint32_t before = context.status_arena.length;
    spx_status_token status = spx_decl_6170702e6d61696e(&context, &result);
    if ({condition}) return 2;
    if (spx_test_live_allocations != UINT64_C(0)) return 3;
    if (status == SPX_STATUS_SUCCESS && context.status_arena.length != before) return 4;
    if (status != SPX_STATUS_SUCCESS && context.status_arena.length != before + UINT32_C(1)) return 5;
  }}
  return 0;
}}
"#
    );
    let allocator = r#"
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
static uint64_t spx_test_live_allocations = UINT64_C(0);
static void *spx_test_malloc(size_t size) {
  void *allocation = malloc(size);
  if (allocation != NULL) spx_test_live_allocations += UINT64_C(1);
  return allocation;
}
static void spx_test_free(void *allocation) {
  if (allocation != NULL) {
    if (spx_test_live_allocations == UINT64_C(0)) abort();
    spx_test_live_allocations -= UINT64_C(1);
    free(allocation);
  }
}
"#;
    for optimization in ["-O0", "-O2"] {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "semaprax-generic-owned-function-native-{}-{serial}",
            std::process::id()
        ));
        let c = base.with_extension("c");
        let executable = base.with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::write(&c, format!("{allocator}\n{tracked}\n{probe}")).unwrap();
        let output = Command::new("clang")
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg("-DSPX_NO_ENTRY_WRAPPER")
            .arg(&c)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(Command::new(&executable).status().unwrap().success());
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(executable);
    }
}

fn wasm_bulk_memory_counts(bytes: &[u8]) -> (usize, usize) {
    let mut copies = 0;
    let mut grows = 0;
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut operators = body.get_operators_reader().unwrap();
            while !operators.eof() {
                match operators.read().unwrap() {
                    wasmparser::Operator::MemoryCopy { .. } => copies += 1,
                    wasmparser::Operator::MemoryGrow { .. } => grows += 1,
                    _ => {}
                }
            }
        }
    }
    (copies, grows)
}

fn run_wasm(entry: &str, parsed: &semaprax::ast::Program, expected: Expected) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-generic-owned-function-wasm-{}-{serial}",
        std::process::id()
    ));
    wasm::build_web(parsed, &root).unwrap();
    let core = std::fs::read(root.join("app.wasm")).unwrap();
    let (memory_copies, memory_grows) = wasm_bulk_memory_counts(&core);
    assert_eq!(memory_grows, 0, "owned relays must not grow Wasm memory");
    if matches!(expected, Expected::Value(_)) {
        // `bytes_copy` legitimately uses bulk memory. Compare against the same
        // program with only the two nested owning relays bypassed so any extra
        // aggregate-level shallow copy remains observable.
        let baseline_source = source(entry).replace(
            "consume_nested_box(relay_box<bool>(boxed)) + consume_nested_pair(relay_pair<i64>(paired))",
            "consume_nested_box(boxed) + consume_nested_pair(paired)",
        );
        let baseline = parse(
            &baseline_source,
            Path::new("generic-owned-function-runtime-wasm-baseline-v1.spx"),
        )
        .unwrap();
        assert!(verify::verify(&baseline)
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()));
        let baseline_root = root.with_extension("baseline");
        wasm::build_web(&baseline, &baseline_root).unwrap();
        let baseline_core = std::fs::read(baseline_root.join("app.wasm")).unwrap();
        let (baseline_copies, baseline_grows) = wasm_bulk_memory_counts(&baseline_core);
        assert_eq!(baseline_grows, 0);
        assert_eq!(
            memory_copies, baseline_copies,
            "nested owning relay lowering must not add memory.copy"
        );
        let _ = std::fs::remove_dir_all(baseline_root);
    }
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    let expectation = match expected {
        Expected::Value(value) => format!(
            "if(instance.exports.semaprax_main()!=={value}n)throw Error('wrong value');"
        ),
        Expected::Failure(domain, code, message) => format!(
            "let failed=false;try{{instance.exports.semaprax_main();}}catch(error){{const status=semanticStatus(error);if(status===null||status.domain_id!=='{domain}'||status.code!=={code}||error.message!=={message:?})throw error;failed=true;}}if(!failed)throw Error('missing failure');"
        ),
    };
    std::fs::write(
        root.join("probe.mjs"),
        format!(
            r#"import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes,semanticStatus}} from './semaprax.js';
const bytes=await readFile('./app.wasm');
const {{instance}}=await instantiateBytes(bytes,{{maxOwnedByteEntries:2}});
for(let i=0;i<4;i+=1){{{expectation}}}
"#
        ),
    )
    .unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(root);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
