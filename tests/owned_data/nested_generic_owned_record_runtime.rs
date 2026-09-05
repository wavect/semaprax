use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, hir, parse, verify, wasm};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.nested_generic_owned_record_runtime;

@id("nested.generic.box") record Box<T> {
    @id("nested.generic.box.value") value: T,
}
@id("nested.generic.pair") record Pair<T, U> {
    @id("nested.generic.pair.left") left: T,
    @id("nested.generic.pair.right") right: U,
}

@id("nested.generic.identity-box")
fn identity_box(value: own Box<Pair<Bytes, bool>>) -> Box<Pair<Bytes, bool>> { value }

@id("nested.generic.inspect-box")
fn inspect_box(value: borrow Box<Pair<Bytes, bool>>) -> i64 {
    match borrow value {
        Box { value: Pair { left: payload, right: enabled } } =>
            if enabled && byte_len(bytes_as_slice(payload)) == 1usize { 1 } else { 0 },
    }
}

@id("nested.generic.consume-box")
fn consume_box(value: own Box<Pair<Bytes, bool>>) -> i64 {
    match own value {
        Box { value: Pair { left: payload, right: enabled } } =>
            if enabled && byte_len(bytes_as_slice(payload)) == 1usize { 19 } else { 0 },
    }
}

@id("nested.generic.make-box")
fn make_box() -> Box<Pair<Bytes, bool>> {
    let input = [11u8];
    Box<Pair<Bytes, bool>> {
        value: Pair<Bytes, bool> {
            left: bytes_copy(array_as_slice(input)),
            right: true,
        },
    }
}

@id("nested.generic.consume-pair-box")
fn consume_pair_box(value: own Pair<Box<Bytes>, i64>) -> i64 {
    match own value {
        Pair { left: Box { value: payload }, right: marker } =>
            if byte_len(bytes_as_slice(payload)) == 2usize { marker } else { 0 },
    }
}

@id("nested.generic.make-pair-box")
fn make_pair_box() -> Pair<Box<Bytes>, i64> {
    let input = [21u8, 22u8];
    Pair<Box<Bytes>, i64> {
        left: Box<Bytes> { value: bytes_copy(array_as_slice(input)) },
        right: 22,
    }
}

@id("nested.generic.identity-multi")
fn identity_multi(
    value: own Pair<Pair<Box<Bytes>, Box<Bytes>>, i64>
) -> Pair<Pair<Box<Bytes>, Box<Bytes>>, i64> { value }

@id("nested.generic.consume-multi")
fn consume_multi(value: own Pair<Pair<Box<Bytes>, Box<Bytes>>, i64>) -> i64 {
    match own value {
        Pair {
            left: Pair {
                left: Box { value: first },
                right: Box { value: second },
            },
            right: marker,
        } => if byte_len(bytes_as_slice(first)) == 1usize
            && byte_len(bytes_as_slice(second)) == 2usize { marker } else { 0 },
    }
}

@id("nested.generic.make-multi")
fn make_multi() -> Pair<Pair<Box<Bytes>, Box<Bytes>>, i64> {
    let first = [41u8];
    let second = [42u8, 43u8];
    Pair<Pair<Box<Bytes>, Box<Bytes>>, i64> {
        left: Pair<Box<Bytes>, Box<Bytes>> {
            left: Box<Bytes> { value: bytes_copy(array_as_slice(first)) },
            right: Box<Bytes> { value: bytes_copy(array_as_slice(second)) },
        },
        right: 2,
    }
}

@id("nested.generic.run-multi")
fn run_multi() -> i64 {
    let value = make_multi();
    let moved = identity_multi(value);
    consume_multi(moved)
}

@id("nested.generic.run")
fn run() -> i64 {
    let boxed = make_box();
    let inspected = inspect_box(boxed);
    let moved = identity_box(boxed);
    let base = inspected + consume_box(moved) + consume_pair_box(make_pair_box());
    if run_multi() == 2 { base } else { 0 }
}

@id("nested.generic.fail-bytes")
fn fail_bytes() -> Bytes {
    let failure = 1 / 0;
    let input = [51u8];
    if failure == 0 { bytes_copy(array_as_slice(input)) } else { bytes_copy(array_as_slice(input)) }
}

@id("nested.generic.fail-after-first-leaf")
fn fail_after_first_leaf() -> i64 {
    let first = [61u8];
    let packet = Pair<Pair<Box<Bytes>, Box<Bytes>>, i64> {
        left: Pair<Box<Bytes>, Box<Bytes>> {
            left: Box<Bytes> { value: bytes_copy(array_as_slice(first)) },
            right: Box<Bytes> { value: fail_bytes() },
        },
        right: 3,
    };
    consume_multi(packet)
}

@id("nested.generic.fail-after-second-leaf")
fn fail_after_second_leaf() -> i64 {
    let first = [71u8];
    let second = [72u8, 73u8];
    let packet = Pair<Pair<Box<Bytes>, Box<Bytes>>, i64> {
        left: Pair<Box<Bytes>, Box<Bytes>> {
            left: Box<Bytes> { value: bytes_copy(array_as_slice(first)) },
            right: Box<Bytes> { value: bytes_copy(array_as_slice(second)) },
        },
        right: 9223372036854775807 + 1,
    };
    consume_multi(packet)
}

@id("nested.generic.partial-failure")
fn partial_failure() -> i64 {
    let input = [31u8, 32u8, 33u8];
    let packet = Box<Pair<Bytes, bool>> {
        value: Pair<Bytes, bool> {
            left: bytes_copy(array_as_slice(input)),
            right: (1 / 0) == 0,
        },
    };
    consume_box(packet)
}

@id("app.main") fn main() -> i64 { run() }
"#;

fn symbol(id: &str) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

fn require_backends() -> bool {
    std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some()
}

fn assert_value(envelope: &str, expected: &str) {
    let envelope: serde_json::Value = serde_json::from_str(envelope).unwrap();
    assert_eq!(envelope["payload"]["outcome"]["value"], expected);
}

#[test]
fn nested_concrete_generic_storage_replays_and_executes_on_three_engines() {
    let parsed = parse(
        SOURCE,
        Path::new("nested-concrete-generic-owned-record-runtime.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "{diagnostics:?}"
    );
    let program = hir::resolve(&parsed).unwrap();
    hir::validate(&program).expect("nested generic cleanup plan independently replays");

    let cleanup_paths = |function_id: &str| {
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == function_id)
            .unwrap()
            .cleanup
            .flags
            .iter()
            .map(|flag| {
                flag.place
                    .projections
                    .iter()
                    .map(|projection| projection.as_str().to_owned())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    assert!(cleanup_paths("nested.generic.identity-box").contains(&vec![
        "nested.generic.box.value".to_owned(),
        "nested.generic.pair.left".to_owned(),
    ]));
    assert!(
        cleanup_paths("nested.generic.consume-pair-box").contains(&vec![
            "nested.generic.pair.left".to_owned(),
            "nested.generic.box.value".to_owned(),
        ])
    );
    let multi_paths = cleanup_paths("nested.generic.identity-multi");
    assert!(multi_paths.contains(&vec![
        "nested.generic.pair.left".to_owned(),
        "nested.generic.pair.left".to_owned(),
        "nested.generic.box.value".to_owned(),
    ]));
    assert!(multi_paths.contains(&vec![
        "nested.generic.pair.left".to_owned(),
        "nested.generic.pair.right".to_owned(),
        "nested.generic.box.value".to_owned(),
    ]));

    let mut hostile = program.clone();
    let flag = hostile
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "nested.generic.identity-box")
        .unwrap()
        .cleanup
        .flags
        .iter_mut()
        .find(|flag| flag.place.projections.len() == 2)
        .unwrap();
    flag.place.projections.swap(0, 1);
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");

    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-nested-generic-owned-interpreter-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    for _ in 0..4 {
        let success =
            interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        assert!(success.returned);
        assert_value(&success.envelope, "42");
        interpreter::verify_envelope(&success.envelope).unwrap();
    }
    for _ in 0..4 {
        let failure = interpreter::interpret(
            &path,
            "nested.generic.partial-failure",
            &[],
            &InterpreterOptions::default(),
        )
        .unwrap();
        assert!(!failure.returned);
        assert!(failure
            .envelope
            .contains("\"domain_id\":\"semaprax.arithmetic.v1\""));
        interpreter::verify_envelope(&failure.envelope).unwrap();
    }
    for function in [
        "nested.generic.fail-after-first-leaf",
        "nested.generic.fail-after-second-leaf",
    ] {
        for _ in 0..4 {
            let failure =
                interpreter::interpret(&path, function, &[], &InterpreterOptions::default())
                    .unwrap();
            assert!(!failure.returned);
            assert!(failure
                .envelope
                .contains("\"domain_id\":\"semaprax.arithmetic.v1\""));
            interpreter::verify_envelope(&failure.envelope).unwrap();
        }
    }
    let _ = std::fs::remove_file(path);

    let clang_available = Command::new("clang").arg("--version").output().is_ok();
    assert!(
        clang_available || !require_backends(),
        "required nested-generic Clang backend is unavailable"
    );
    if clang_available {
        let generated = codegen::emit_c(&parsed).unwrap();
        assert_eq!(generated, codegen::emit_c(&parsed).unwrap());
        const NON_OWNING_RUNTIME_COPIES: [&str; 2] = [
            "memcpy(payload, value.ptr, (size_t)value.len);",
            "memcpy(entry->domain_storage, status.domain_id, domain_size);",
        ];
        let mut ownership_surface = generated.clone();
        for admitted in NON_OWNING_RUNTIME_COPIES {
            assert_eq!(ownership_surface.matches(admitted).count(), 1);
            ownership_surface = ownership_surface.replacen(admitted, "", 1);
        }
        let remaining_memcpy = ownership_surface
            .lines()
            .filter(|line| line.contains("memcpy("))
            .collect::<Vec<_>>();
        assert!(
            remaining_memcpy.is_empty(),
            "only the two exact non-owning runtime copies may use memcpy: {remaining_memcpy:#?}"
        );
        let tracked = generated
            .replace(
                "uint8_t *payload = (uint8_t *)malloc(",
                "uint8_t *payload = (uint8_t *)spx_test_malloc(",
            )
            .replace("free(value->ptr);", "spx_test_free(value->ptr);");
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
    if (allocation != NULL) {
        if (spx_test_live_allocations == UINT64_C(0)) abort();
        spx_test_live_allocations -= UINT64_C(1);
    }
    free(allocation);
}
"#;
        let probe = format!(
            r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(131), entries, UINT32_C(16), NULL, NULL, NULL)) return 1;
    int64_t result = INT64_C(0);
    for (uint32_t iteration = 0; iteration < UINT32_C(4); ++iteration) {{
        if ({main}(&context, &result) != SPX_STATUS_SUCCESS || result != INT64_C(42)) return 2;
        if (spx_test_live_allocations != UINT64_C(0)) return 3;
    }}
    for (uint32_t iteration = 0; iteration < UINT32_C(4); ++iteration) {{
        if ({failure}(&context, &result) == SPX_STATUS_SUCCESS) return 4;
        if (spx_test_live_allocations != UINT64_C(0)) return 5;
    }}
    if ({first_leaf_failure}(&context, &result) == SPX_STATUS_SUCCESS) return 6;
    if (spx_test_live_allocations != UINT64_C(0)) return 7;
    if ({second_leaf_failure}(&context, &result) == SPX_STATUS_SUCCESS) return 8;
    if (spx_test_live_allocations != UINT64_C(0)) return 9;
    if ({main}(&context, &result) != SPX_STATUS_SUCCESS || result != INT64_C(42)) return 10;
    if (spx_test_live_allocations != UINT64_C(0)) return 11;
    return 0;
}}
"#,
            main = symbol("app.main"),
            failure = symbol("nested.generic.partial-failure"),
            first_leaf_failure = symbol("nested.generic.fail-after-first-leaf"),
            second_leaf_failure = symbol("nested.generic.fail-after-second-leaf"),
        );
        for optimization in ["-O0", "-O2"] {
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "semaprax-nested-generic-native-{}-{serial}",
                std::process::id()
            ));
            let c = root.with_extension("c");
            let executable = root.with_extension(std::env::consts::EXE_EXTENSION);
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
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(Command::new(&executable).status().unwrap().success());
            let _ = std::fs::remove_file(c);
            let _ = std::fs::remove_file(executable);
        }
    }

    let node_available = Command::new("node").arg("--version").output().is_ok();
    assert!(
        node_available || !require_backends(),
        "required nested-generic Node backend is unavailable"
    );
    if node_available {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-nested-generic-wasm-{}-{serial}",
            std::process::id()
        ));
        wasm::build_web(&parsed, &root).unwrap();
        std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
        std::fs::write(
            root.join("probe.mjs"),
            r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const bytes=await readFile('./app.wasm');
const limited=await instantiateBytes(bytes,{maxOwnedByteEntries:1});
let capacityRejected=false;
try{limited.instance.exports.semaprax_main();}catch(error){capacityRejected=String(error).includes('live entry limit exceeded');}
if(!capacityRejected)throw Error('nested generic multi-owner execution ignored its exact capacity');
const {instance}=await instantiateBytes(bytes,{maxOwnedByteEntries:2});
for(let i=0;i<4;i+=1){const value=instance.exports.semaprax_main();if(value!==42n)throw Error(`nested-generic:${value}`);}
"#,
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

        for (failure, expected_code, expected_message) in [
            (
                "partial_failure",
                4,
                "SEMAPRAX checked arithmetic failure: invalid division",
            ),
            (
                "fail_after_first_leaf",
                4,
                "SEMAPRAX checked arithmetic failure: invalid division",
            ),
            (
                "fail_after_second_leaf",
                1,
                "SEMAPRAX checked arithmetic failure: addition overflow",
            ),
        ] {
            let failure_source = SOURCE.replace(
                "@id(\"app.main\") fn main() -> i64 { run() }",
                &format!("@id(\"app.main\") fn main() -> i64 {{ {failure}() }}"),
            );
            let failed = parse(
                &failure_source,
                Path::new("nested-concrete-generic-owned-record-wasm-failure.spx"),
            )
            .unwrap();
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "semaprax-nested-generic-wasm-failure-{}-{serial}",
                std::process::id()
            ));
            wasm::build_web(&failed, &root).unwrap();
            std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
            let probe = format!(
                r#"import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes}} from './semaprax.js';
const bytes=await readFile('./app.wasm');
const {{instance}}=await instantiateBytes(bytes,{{maxOwnedByteEntries:2}});
for(let i=0;i<4;i+=1){{let failed=false;try{{instance.exports.semaprax_main();}}catch(error){{if(error.domainId!=='semaprax.arithmetic.v1'||error.code!=={expected_code}||error.message!=={expected_message:?})throw error;failed=true;}}if(!failed)throw Error('missing nested-generic failure');}}
"#,
            );
            std::fs::write(root.join("probe.mjs"), probe).unwrap();
            let output = Command::new("node")
                .arg("probe.mjs")
                .current_dir(&root)
                .output()
                .unwrap();
            let _ = std::fs::remove_dir_all(root);
            assert!(
                output.status.success(),
                "{failure}: stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
