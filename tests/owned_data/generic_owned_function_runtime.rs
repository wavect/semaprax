use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::hir::{self, DeclarationId, ResolvedType};
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, parse, verify, wasm};

#[path = "generic_owned_function_runtime/copy_success_result.rs"]
mod copy_success_result;
#[path = "generic_owned_function_runtime/matrix.rs"]
mod matrix;
#[path = "generic_owned_function_runtime/mixed_result.rs"]
mod mixed_result;

#[path = "generic_owned_function_runtime/explicit_forwarding.rs"]
mod explicit_forwarding;

#[path = "generic_owned_function_runtime/nested_composition.rs"]
mod nested_composition;

#[path = "generic_owned_function_runtime/multi_owner.rs"]
mod multi_owner;

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
fn relay_box<T>(value: own Box<Pair<Bytes, T>>) -> Box<Pair<Bytes, T>> {
  relay_box_middle<T>(value)
}
@id("generic.function.nested-relay-box-middle")
fn relay_box_middle<T>(value: own Box<Pair<Bytes, T>>) -> Box<Pair<Bytes, T>> {
  relay_box_leaf<T>(value)
}
@id("generic.function.nested-relay-box-leaf")
fn relay_box_leaf<T>(value: own Box<Pair<Bytes, T>>) -> Box<Pair<Bytes, T>> { value }
@id("generic.function.nested-relay-pair")
fn relay_pair<T>(value: own Pair<Box<Bytes>, T>) -> Pair<Box<Bytes>, T> { value }
@id("generic.function.nested-chain-guarded")
fn nested_chain_guarded<T>(value: own Box<Pair<Bytes, T>>, allowed: bool) -> Box<Pair<Bytes, T>> {
  nested_chain_guarded_middle<T>(value, allowed)
}
@id("generic.function.nested-chain-guarded-middle")
fn nested_chain_guarded_middle<T>(value: own Box<Pair<Bytes, T>>, allowed: bool) -> Box<Pair<Bytes, T>> {
  nested_chain_guarded_leaf<T>(value, allowed)
}
@id("generic.function.nested-chain-guarded-leaf")
fn nested_chain_guarded_leaf<T>(value: own Box<Pair<Bytes, T>>, allowed: bool) -> Box<Pair<Bytes, T>>
  requires allowed
{ value }
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
  consume_nested_box(nested_chain_guarded<bool>(make_nested_box(), false))
}
@id("generic.function.nested-ensures-failure") fn nested_ensures_failure() -> i64 {
  consume_nested_box(nested_ensures<bool>(make_nested_box()))
}
@id("generic.function.nested-argument-failure") fn nested_argument_failure() -> i64 {
  consume_nested_box(nested_chain_guarded<bool>(make_nested_box(), (1 / 0) == 0))
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
    for (template, argument, expected, path, expected_flags) in [
        (
            "generic.function.nested-relay-box",
            ResolvedType::Bool,
            box_pair.clone(),
            [
                "generic.function.box.value",
                "generic.function.pair.payload",
            ],
            4,
        ),
        (
            "generic.function.nested-relay-box-middle",
            ResolvedType::Bool,
            box_pair.clone(),
            [
                "generic.function.box.value",
                "generic.function.pair.payload",
            ],
            4,
        ),
        (
            "generic.function.nested-relay-box-leaf",
            ResolvedType::Bool,
            box_pair.clone(),
            [
                "generic.function.box.value",
                "generic.function.pair.payload",
            ],
            3,
        ),
        (
            "generic.function.nested-chain-guarded",
            ResolvedType::Bool,
            box_pair.clone(),
            [
                "generic.function.box.value",
                "generic.function.pair.payload",
            ],
            4,
        ),
        (
            "generic.function.nested-chain-guarded-middle",
            ResolvedType::Bool,
            box_pair.clone(),
            [
                "generic.function.box.value",
                "generic.function.pair.payload",
            ],
            4,
        ),
        (
            "generic.function.nested-chain-guarded-leaf",
            ResolvedType::Bool,
            box_pair,
            [
                "generic.function.box.value",
                "generic.function.pair.payload",
            ],
            3,
        ),
        (
            "generic.function.nested-relay-pair",
            ResolvedType::I64,
            pair_box,
            [
                "generic.function.pair.payload",
                "generic.function.box.value",
            ],
            3,
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
            expected_flags,
            "nested relay must exactly track its parameter/call/body/result owned-leaf places"
        );
    }
    for (caller, callee) in [
        (
            "generic.function.nested-relay-box",
            "generic.function.nested-relay-box-middle",
        ),
        (
            "generic.function.nested-relay-box-middle",
            "generic.function.nested-relay-box-leaf",
        ),
        (
            "generic.function.nested-chain-guarded",
            "generic.function.nested-chain-guarded-middle",
        ),
        (
            "generic.function.nested-chain-guarded-middle",
            "generic.function.nested-chain-guarded-leaf",
        ),
    ] {
        let caller = program
            .function_instances
            .iter()
            .find(|instance| instance.template.as_str() == caller)
            .unwrap_or_else(|| panic!("missing forwarding instance {caller}"));
        let call = match &caller.function.body.kind {
            hir::ResolvedExprKind::Block { statements, tail } if statements.is_empty() => tail,
            _ => &caller.function.body,
        };
        let hir::ResolvedExprKind::Call {
            callee: called,
            instance,
            type_arguments,
            ..
        } = &call.kind
        else {
            panic!("forwarding instance body is not a call: {caller:?}");
        };
        assert_eq!(called.as_str(), callee);
        assert_eq!(type_arguments, &[ResolvedType::Bool]);
        assert_eq!(
            instance.as_ref(),
            Some(&hir::FunctionInstanceId::derive(
                called,
                &[ResolvedType::Bool]
            ))
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
    run_interpreter_source(entry, &source(entry), expected);
}

fn run_interpreter_source(entry: &str, source: &str, expected: Expected) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-generic-owned-function-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
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
        Expected::Value(value) => {
            format!("status != SPX_STATUS_SUCCESS || result != INT64_C({value})")
        }
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
    let baseline_source = source(entry).replace(
        "consume_nested_box(relay_box<bool>(boxed)) + consume_nested_pair(relay_pair<i64>(paired))",
        "consume_nested_box(boxed) + consume_nested_pair(paired)",
    );
    run_wasm_source(parsed, &baseline_source, expected);
}

fn run_wasm_source(parsed: &semaprax::ast::Program, baseline_source: &str, expected: Expected) {
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
        // program with its owning relays bypassed so any extra aggregate-level
        // shallow copy remains observable.
        let baseline = parse(
            baseline_source,
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
        Expected::Value(value) => {
            format!("if(instance.exports.semaprax_main()!=={value}n)throw Error('wrong value');")
        }
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

fn generic_relay_matrix_source() -> String {
    let mut source = String::from(
        r#"module test.generic_owned_function_hostile;
@id("hostile.pair") record Pair<T, U> {
  @id("hostile.pair.payload") payload: T,
  @id("hostile.pair.marker") marker: U,
}
@id("hostile.box") record Box<T> {
  @id("hostile.box.value") value: T,
}
@id("hostile.leaf")
fn leaf<T>(value: own Box<Pair<Bytes, T>>) -> Box<Pair<Bytes, T>> { value }
@id("hostile.middle")
fn middle<T>(value: own Box<Pair<Bytes, T>>) -> Box<Pair<Bytes, T>> { leaf<T>(value) }
@id("hostile.outer")
fn outer<T>(value: own Box<Pair<Bytes, T>>) -> Box<Pair<Bytes, T>> { middle<T>(value) }
"#,
    );
    for scalar in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
        source.push_str(&format!(
            "@id(\"hostile.invoke.{scalar}\") fn invoke_{scalar}(value: own Box<Pair<Bytes, {scalar}>>) -> Box<Pair<Bytes, {scalar}>> {{ outer<{scalar}>(value) }}\n"
        ));
    }
    source.push_str("@id(\"app.main\") fn main() -> i64 { 0 }\n");
    source
}

fn checked_relay_matrix() -> hir::ResolvedProgram {
    let source = generic_relay_matrix_source();
    let parsed = parse(
        &source,
        Path::new("generic-owned-function-hostile-matrix-v1.spx"),
    )
    .expect("relay matrix parses");
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "relay matrix verifies: {diagnostics:?}"
    );
    hir::resolve(&parsed).expect("relay matrix resolves and replays")
}

#[test]
fn nested_generic_relay_substitution_and_hir_carriers_fail_closed() {
    fn leaf_paths(shape: &semaprax::cleanup::FieldLivenessShape) -> Vec<Vec<DeclarationId>> {
        fn visit(
            shape: &semaprax::cleanup::FieldLivenessShape,
            path: &mut Vec<DeclarationId>,
            paths: &mut Vec<Vec<DeclarationId>>,
        ) {
            match shape {
                semaprax::cleanup::FieldLivenessShape::NoDrop => {}
                semaprax::cleanup::FieldLivenessShape::Leaf { .. } => {
                    paths.push(path.clone());
                }
                semaprax::cleanup::FieldLivenessShape::Record { fields, .. } => {
                    for field in fields {
                        path.push(field.field.clone());
                        visit(&field.shape, path, paths);
                        path.pop();
                    }
                }
                semaprax::cleanup::FieldLivenessShape::Variant { .. } => {
                    panic!("nested relay cleanup shape must remain a record")
                }
                _ => panic!("nested relay cleanup shape widened unexpectedly"),
            }
        }

        let mut paths = Vec::new();
        visit(shape, &mut Vec::new(), &mut paths);
        paths
    }

    let program = checked_relay_matrix();
    let scalars = [
        ResolvedType::I64,
        ResolvedType::I32,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::Char,
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
    ];
    let owned_path = vec![
        DeclarationId::new("hostile.box.value"),
        DeclarationId::new("hostile.pair.payload"),
    ];
    for template in ["hostile.outer", "hostile.middle", "hostile.leaf"] {
        for scalar in &scalars {
            let instance = program
                .function_instances
                .iter()
                .find(|instance| {
                    instance.template.as_str() == template
                        && instance.type_arguments == [scalar.clone()]
                })
                .unwrap_or_else(|| panic!("missing {template}<{scalar:?}>"));
            let pair = nominal("hostile.pair", vec![ResolvedType::Bytes, scalar.clone()]);
            let aggregate = nominal("hostile.box", vec![pair]);
            assert_eq!(
                instance.id,
                hir::FunctionInstanceId::derive(&instance.template, std::slice::from_ref(scalar),)
            );
            assert_eq!(instance.function.params[0].ty, aggregate);
            assert_eq!(instance.function.return_type, aggregate);
            assert_eq!(
                instance.function.cleanup_plan.schema,
                "semaprax.cleanup-plan.v7"
            );
            let (expected_inventory_flags, expected_plan_slots) = if template == "hostile.leaf" {
                (3, 3)
            } else {
                (4, 5)
            };
            assert_eq!(
                instance.function.cleanup.flags.len(),
                expected_inventory_flags
            );
            assert_eq!(
                instance.function.cleanup_plan.slots.len(),
                expected_plan_slots
            );
            assert!(instance
                .function
                .cleanup
                .flags
                .iter()
                .all(|flag| flag.place.projections == owned_path));
            for slot in &instance.function.cleanup_plan.slots {
                assert_eq!(
                    leaf_paths(&slot.field_liveness_shape).as_slice(),
                    std::slice::from_ref(&owned_path)
                );
            }
        }
    }

    let bool_outer = |program: &hir::ResolvedProgram| {
        program
            .function_instances
            .iter()
            .position(|instance| {
                instance.template.as_str() == "hostile.outer"
                    && instance.type_arguments == [ResolvedType::Bool]
            })
            .expect("bool outer instance")
    };

    let mut wrong_identity = program.clone();
    let index = bool_outer(&wrong_identity);
    wrong_identity.function_instances[index].type_arguments[0] = ResolvedType::I64;
    assert_eq!(hir::validate(&wrong_identity).unwrap_err().code, "SPX-H006");

    let mut wrong_signature = program.clone();
    let index = bool_outer(&wrong_signature);
    wrong_signature.function_instances[index].function.params[0].ty = nominal(
        "hostile.box",
        vec![nominal(
            "hostile.pair",
            vec![ResolvedType::Bytes, ResolvedType::I64],
        )],
    );
    assert_eq!(
        hir::validate(&wrong_signature).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_inventory = program.clone();
    let index = bool_outer(&wrong_inventory);
    wrong_inventory.function_instances[index]
        .function
        .cleanup
        .flags[0]
        .place
        .projections
        .reverse();
    assert_eq!(
        hir::validate(&wrong_inventory).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_plan = program.clone();
    let index = bool_outer(&wrong_plan);
    wrong_plan.function_instances[index]
        .function
        .cleanup_plan
        .slots[0]
        .field_liveness_shape = semaprax::cleanup::FieldLivenessShape::NoDrop;
    assert_eq!(hir::validate(&wrong_plan).unwrap_err().code, "SPX-H006");

    let mut wrong_forwarded_call = program.clone();
    let index = bool_outer(&wrong_forwarded_call);
    let body = &mut wrong_forwarded_call.function_instances[index].function.body;
    let call = match &mut body.kind {
        hir::ResolvedExprKind::Block { statements, tail } if statements.is_empty() => tail,
        _ => body,
    };
    let hir::ResolvedExprKind::Call { type_arguments, .. } = &mut call.kind else {
        panic!("outer body must be a forwarded call")
    };
    type_arguments[0] = ResolvedType::I64;
    assert_eq!(
        hir::validate(&wrong_forwarded_call).unwrap_err().code,
        "SPX-H006"
    );
}

fn verification_error_codes(source: &str) -> Vec<&'static str> {
    let parsed = parse(source, Path::new("generic-owned-forwarding-hostile-v1.spx"))
        .expect("hostile forwarding source parses");
    verify::verify(&parsed)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn generic_forwarding_rejects_vector_changes_and_template_cycles() {
    let prelude = r#"module test.generic_owned_forwarding_hostile;
record Pair<T, U> { payload: T, marker: U, }
fn leaf<T, U>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> { value }
"#;
    let admitted = format!(
        "{prelude}fn forward<T, U>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> {{ leaf<T, U>(value) }}\nfn main() -> i64 {{ 0 }}\n"
    );
    assert!(verification_error_codes(&admitted).is_empty());
    for body in ["leaf<U, T>(value)", "leaf<T, T>(value)", "leaf<T>(value)"] {
        let source = format!(
            "{prelude}fn hostile<T, U>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> {{ {body} }}\nfn main() -> i64 {{ 0 }}\n"
        );
        let codes = verification_error_codes(&source);
        assert!(
            codes.iter().any(|code| matches!(*code, "SPX-T225" | "SPX-T205" | "SPX-T103")),
            "{body}: {codes:?}"
        );
    }

    let direct = r#"module test.generic_owned_forwarding_direct_cycle;
record Pair<T, U> { payload: T, marker: U, }
fn cycle<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> { cycle<T>(value) }
fn main() -> i64 { 0 }
"#;
    assert_eq!(verification_error_codes(direct), ["SPX-T226"]);

    let indirect = r#"module test.generic_owned_forwarding_indirect_cycle;
record Pair<T, U> { payload: T, marker: U, }
fn left<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> { right<T>(value) }
fn right<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> { left<T>(value) }
fn main() -> i64 { 0 }
"#;
    let codes = verification_error_codes(indirect);
    assert_eq!(codes, ["SPX-T226", "SPX-T226"]);
}

fn forwarding_chain_source(count: usize) -> String {
    let mut source = String::from(
        "module test.generic_owned_forwarding_bound;\nrecord Pair<T, U> { payload: T, marker: U, }\n",
    );
    for index in 0..count {
        let body = if index + 1 == count {
            String::from("value")
        } else {
            format!("relay_{}<T>(value)", index + 1)
        };
        source.push_str(&format!(
            "fn relay_{index}<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> {{ {body} }}\n"
        ));
    }
    source.push_str(
        "fn invoke(value: own Pair<Bytes, bool>) -> Pair<Bytes, bool> { relay_0<bool>(value) }\nfn main() -> i64 { 0 }\n",
    );
    source
}

#[test]
fn generic_forwarding_instance_closure_bound_is_exact() {
    let at_limit = forwarding_chain_source(256);
    let parsed = parse(
        &at_limit,
        Path::new("generic-owned-forwarding-limit-v1.spx"),
    )
    .expect("at-limit forwarding source parses");
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "at-limit forwarding source verifies: {diagnostics:?}"
    );
    let program = hir::resolve(&parsed).expect("256 forwarding instances are admitted");
    assert_eq!(program.function_instances.len(), 256);
    hir::validate(&program).expect("at-limit forwarding closure replays");

    let over_limit = forwarding_chain_source(257);
    let parsed = parse(
        &over_limit,
        Path::new("generic-owned-forwarding-over-limit-v1.spx"),
    )
    .expect("over-limit forwarding source parses");
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "over-limit rejection belongs to HIR closure discovery: {diagnostics:?}"
    );
    let errors = hir::resolve(&parsed).unwrap_err();
    assert!(
        errors.iter().any(|error| {
            error.code == "SPX-H006"
                && error.message == "generic function instance closure exceeds 256 entries"
        }),
        "{errors:?}"
    );
}
