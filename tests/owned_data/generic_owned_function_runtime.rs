use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, parse, verify, wasm};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const PREFIX: &str = r#"
module test.generic_owned_function_runtime;
@id("generic.function.pair") record Pair<T, U> {
  @id("generic.function.pair.payload") payload: T,
  @id("generic.function.pair.marker") marker: U,
}
@id("generic.function.relay")
fn relay<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> { value }
@id("generic.function.reject")
fn reject<T>(value: own Pair<Bytes, T>, allowed: bool) -> Pair<Bytes, T>
  requires allowed
{ value }
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
  consume_u8(relayed) + consume_bool(relay<bool>(second)) - 1
}
@id("generic.function.failure") fn failure() -> i64 {
  let input = [4u8, 5u8, 6u8];
  let value = Pair<Bytes, u8> {
    payload: bytes_copy(array_as_slice(input)), marker: 7u8,
  };
  consume_u8(reject<u8>(value, false))
}
"#;

fn source(entry: &str) -> String {
    let call = match entry {
        "generic.function.success" => "success",
        "generic.function.failure" => "failure",
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
    semaprax::hir::resolve(&parsed).expect("owned generic function instances resolve");
    parsed
}

#[test]
fn generic_owned_function_instances_settle_and_reenter_on_three_engines() {
    for (entry, succeeds) in [
        ("generic.function.success", true),
        ("generic.function.failure", false),
    ] {
        let parsed = checked(entry);
        run_interpreter(entry, succeeds);
        if Command::new("clang").arg("--version").output().is_ok() {
            run_native(&parsed, succeeds);
        }
        if Command::new("node").arg("--version").output().is_ok() {
            run_wasm(&parsed, succeeds);
        }
    }
}

fn run_interpreter(entry: &str, succeeds: bool) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-generic-owned-function-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source(entry)).unwrap();
    for _ in 0..4 {
        let result =
            interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        assert_eq!(result.returned, succeeds);
        interpreter::verify_envelope(&result.envelope).unwrap();
        if succeeds {
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            assert_eq!(envelope["payload"]["outcome"]["value"], "42");
        }
    }
    let _ = std::fs::remove_file(path);
}

fn run_native(parsed: &semaprax::ast::Program, succeeds: bool) {
    let generated = codegen::emit_c(parsed).unwrap();
    assert_eq!(generated, codegen::emit_c(parsed).unwrap());
    let tracked = generated
        .replace(
            "uint8_t *payload = (uint8_t *)malloc(",
            "uint8_t *payload = (uint8_t *)spx_test_malloc(",
        )
        .replace("free(value->ptr);", "spx_test_free(value->ptr);");
    let condition = if succeeds {
        "status != SPX_STATUS_SUCCESS || result != INT64_C(42)"
    } else {
        "status == SPX_STATUS_SUCCESS"
    };
    let probe = format!(
        r#"
int main(void) {{
  struct spx_status_entry entries[UINT32_C(32)];
  struct spx_context context = {{0}};
  if (!spx_context_init(&context, UINT64_C(17), entries, UINT32_C(32), NULL, NULL, NULL)) return 1;
  for (uint32_t i = 0; i < UINT32_C(4); ++i) {{
    int64_t result = INT64_C(0);
    spx_status_token status = spx_decl_6170702e6d61696e(&context, &result);
    if ({condition}) return 2;
    if (spx_test_live_allocations != UINT64_C(0)) return 3;
  }}
  return 0;
}}
"#
    );
    let allocator = r#"
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
static uint64_t spx_test_live_allocations = UINT64_C(0);
static void *spx_test_malloc(size_t size) {
  void *allocation = malloc(size);
  if (allocation != NULL) spx_test_live_allocations += UINT64_C(1);
  return allocation;
}
static void spx_test_free(void *allocation) {
  if (allocation != NULL) { spx_test_live_allocations -= UINT64_C(1); free(allocation); }
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

fn run_wasm(parsed: &semaprax::ast::Program, succeeds: bool) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-generic-owned-function-wasm-{}-{serial}",
        std::process::id()
    ));
    wasm::build_web(parsed, &root).unwrap();
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    let expectation = if succeeds {
        "if(instance.exports.semaprax_main()!==42n)throw Error('wrong value');"
    } else {
        "let failed=false;try{instance.exports.semaprax_main();}catch(_){failed=true;}if(!failed)throw Error('missing failure');"
    };
    std::fs::write(
        root.join("probe.mjs"),
        format!(
            r#"import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes}} from './semaprax.js';
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
