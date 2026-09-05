use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, parse, verify, wasm};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn generic_owned_backends_required() -> bool {
    std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some()
}

const SOURCE: &str = r#"
module test.nested_owned_record_runtime;

@id("runtime.leaf") record Leaf {
    @id("runtime.leaf.payload") payload: Bytes,
    @id("runtime.leaf.marker") marker: i64,
}
@id("runtime.branch") record Branch {
    @id("runtime.branch.leaf") leaf: Leaf,
    @id("runtime.branch.enabled") enabled: bool,
}
@id("runtime.envelope") record Envelope {
    @id("runtime.envelope.left") left: Branch,
    @id("runtime.envelope.right") right: Branch,
    @id("runtime.envelope.sequence") sequence: i64,
}

@id("runtime.metadata") record Metadata {
    @id("runtime.metadata.marker") marker: i64,
}
@id("runtime.direct-envelope") record DirectEnvelope {
    @id("runtime.direct-envelope.payload") payload: Bytes,
    @id("runtime.direct-envelope.metadata") metadata: Metadata,
}

@id("runtime.identity")
fn identity(packet: own Envelope) -> Envelope { packet }

@id("runtime.update")
fn update(packet: own Envelope) -> Envelope {
    let replacement = [21u8, 22u8, 23u8];
    packet with {
        right: Branch {
            leaf: Leaf { payload: bytes_copy(array_as_slice(replacement)), marker: 2 },
            enabled: false,
        },
    }
}

@id("runtime.update-failure")
fn update_failure(packet: own Envelope) -> Envelope {
    let replacement = [31u8, 32u8, 33u8, 34u8];
    packet with {
        right: Branch {
            leaf: Leaf { payload: bytes_copy(array_as_slice(replacement)), marker: 4 },
            enabled: true,
        },
        sequence: 9223372036854775807 + 1,
    }
}

@id("runtime.update-direct-bytes-copy-subtree")
fn update_direct_bytes_copy_subtree(packet: own DirectEnvelope) -> DirectEnvelope {
    packet with { metadata: Metadata { marker: 9 } }
}

@id("runtime.run-direct-bytes-copy-subtree")
fn run_direct_bytes_copy_subtree() -> i64 {
    let input = [31u8, 32u8];
    let packet = DirectEnvelope {
        payload: bytes_copy(array_as_slice(input)),
        metadata: Metadata { marker: 1 },
    };
    let retained_metadata = packet.metadata;
    let updated = update_direct_bytes_copy_subtree(packet);
    match own updated {
        DirectEnvelope { payload, metadata: Metadata { marker } } =>
            if byte_len(bytes_as_slice(payload)) == 2usize { marker } else { 0 },
    }
}

@id("runtime.copy-only-construction")
fn copy_only_construction() -> i64 {
    let metadata = Metadata { marker: 1 };
    metadata.marker
}

@id("runtime.inspect")
fn inspect(packet: own Envelope) -> i64 {
    let left = bytes_as_slice(packet.left.leaf.payload);
    let right = bytes_as_slice(packet.right.leaf.payload);
    if byte_len(left) == 1usize && byte_len(right) == 2usize { 42 } else { 0 }
}

@id("runtime.destructure-borrow")
fn destructure_borrow(packet: borrow Envelope) -> i64 {
    match borrow packet {
        Envelope {
            left: Branch { leaf: Leaf { payload: left_payload, marker: _ }, enabled: _ },
            right: Branch { leaf: Leaf { payload: right_payload, marker: _ }, enabled: _ },
            sequence: _,
        } => {
            let left = bytes_as_slice(left_payload);
            let left_again = byte_range(left, 0usize, byte_len(left));
            let right = bytes_as_slice(right_payload);
            if byte_len(left_again) == 1usize && byte_len(right) == 3usize { 5 } else { 0 }
        },
    }
}

@id("runtime.destructure-own")
fn destructure_own(packet: own Envelope) -> i64 {
    match own packet {
        Envelope {
            left: Branch { leaf: Leaf { payload: left_payload, marker: left_marker }, enabled: _ },
            right: Branch { leaf: Leaf { payload: right_payload, marker: right_marker }, enabled: _ },
            sequence,
        } => if byte_len(bytes_as_slice(left_payload)) == 1usize
            && byte_len(bytes_as_slice(right_payload)) == 3usize
            && left_marker + right_marker + sequence == 6 {
                37
            } else {
                0
            },
    }
}

@id("runtime.run")
fn run() -> i64 {
    let left = [11u8];
    let right = [22u8, 23u8];
    let packet = Envelope {
        left: Branch {
            leaf: Leaf { payload: bytes_copy(array_as_slice(left)), marker: 1 },
            enabled: true,
        },
        right: Branch {
            leaf: Leaf { payload: bytes_copy(array_as_slice(right)), marker: 2 },
            enabled: false,
        },
        sequence: 3,
    };
    let updated = update(packet);
    let borrowed = destructure_borrow(updated);
    let moved = identity(updated);
    borrowed + destructure_own(moved)
}

@id("app.main") fn main() -> i64 { run() }

@id("runtime.fail")
fn fail() -> i64 {
    let left = [11u8];
    let right = [12u8, 13u8];
    let packet = Envelope {
        left: Branch { leaf: Leaf { payload: bytes_copy(array_as_slice(left)), marker: 1 }, enabled: true },
        right: Branch { leaf: Leaf { payload: bytes_copy(array_as_slice(right)), marker: 2 }, enabled: false },
        sequence: 3,
    };
    destructure_own(update_failure(packet))
}
"#;

const GENERIC_OWNED_SOURCE: &str = r#"
module test.generic_owned_record_runtime;

@id("generic.box") record Box<T> {
    @id("generic.box.value") value: T,
}
@id("generic.pair") record Pair<T, U> {
    @id("generic.pair.left") left: T,
    @id("generic.pair.right") right: U,
}

@id("generic.inspect")
fn inspect(packet: borrow Pair<Bytes, bool>) -> i64 {
    match borrow packet {
        Pair { left: payload, right: enabled } =>
            if enabled && byte_len(bytes_as_slice(payload)) == 1usize { 41 } else { 0 },
    }
}

@id("generic.consume")
fn consume(packet: own Pair<Bytes, bool>) -> i64 {
    match own packet {
        Pair { left: payload, right: enabled } => if enabled { 1 } else { 0 },
    }
}

@id("generic.make")
fn make() -> Pair<Bytes, bool> {
    let input = [42u8];
    Pair<Bytes, bool> { left: bytes_copy(array_as_slice(input)), right: true }
}

@id("generic.run")
fn run() -> i64 {
    let packet = make();
    inspect(packet) + consume(packet)
}

@id("generic.inspect-u8") fn inspect_u8(packet: borrow Pair<Bytes, u8>) -> i64 {
    match borrow packet { Pair { left: payload, right: marker } =>
        if marker == 7u8 && byte_len(bytes_as_slice(payload)) == 1usize { 1 } else { 0 }, }
}
@id("generic.consume-u8") fn consume_u8(packet: own Pair<Bytes, u8>) -> i64 {
    match own packet { Pair { left: payload, right: marker } => if marker == 7u8 { 1 } else { 0 }, }
}
@id("generic.check-u8") fn check_u8() -> i64 {
    let input = [1u8]; let packet = Pair<Bytes, u8> { left: bytes_copy(array_as_slice(input)), right: 7u8 };
    if inspect_u8(packet) + consume_u8(packet) == 2 { 0 } else { 100 }
}

@id("generic.inspect-i64") fn inspect_i64(packet: borrow Pair<Bytes, i64>) -> i64 {
    match borrow packet { Pair { left: payload, right: marker } =>
        if marker == -7 && byte_len(bytes_as_slice(payload)) == 1usize { 1 } else { 0 }, }
}
@id("generic.consume-i64") fn consume_i64(packet: own Pair<Bytes, i64>) -> i64 {
    match own packet { Pair { left: payload, right: marker } => if marker == -7 { 1 } else { 0 }, }
}
@id("generic.check-i64") fn check_i64() -> i64 {
    let input = [7u8]; let packet = Pair<Bytes, i64> { left: bytes_copy(array_as_slice(input)), right: -7 };
    if inspect_i64(packet) + consume_i64(packet) == 2 { 0 } else { 100 }
}

@id("generic.inspect-i32") fn inspect_i32(packet: borrow Pair<Bytes, i32>) -> i64 {
    match borrow packet { Pair { left: payload, right: marker } =>
        if marker == -7i32 && byte_len(bytes_as_slice(payload)) == 1usize { 1 } else { 0 }, }
}
@id("generic.consume-i32") fn consume_i32(packet: own Pair<Bytes, i32>) -> i64 {
    match own packet { Pair { left: payload, right: marker } => if marker == -7i32 { 1 } else { 0 }, }
}
@id("generic.check-i32") fn check_i32() -> i64 {
    let input = [2u8]; let packet = Pair<Bytes, i32> { left: bytes_copy(array_as_slice(input)), right: -7i32 };
    if inspect_i32(packet) + consume_i32(packet) == 2 { 0 } else { 100 }
}

@id("generic.inspect-usize") fn inspect_usize(packet: borrow Pair<Bytes, usize>) -> i64 {
    match borrow packet { Pair { left: payload, right: marker } =>
        if marker == 7usize && byte_len(bytes_as_slice(payload)) == 1usize { 1 } else { 0 }, }
}
@id("generic.consume-usize") fn consume_usize(packet: own Pair<Bytes, usize>) -> i64 {
    match own packet { Pair { left: payload, right: marker } => if marker == 7usize { 1 } else { 0 }, }
}
@id("generic.check-usize") fn check_usize() -> i64 {
    let input = [3u8]; let packet = Pair<Bytes, usize> { left: bytes_copy(array_as_slice(input)), right: 7usize };
    if inspect_usize(packet) + consume_usize(packet) == 2 { 0 } else { 100 }
}

@id("generic.inspect-char") fn inspect_char(packet: borrow Pair<Bytes, char>) -> i64 {
    match borrow packet { Pair { left: payload, right: marker } =>
        if marker == 'x' && byte_len(bytes_as_slice(payload)) == 1usize { 1 } else { 0 }, }
}
@id("generic.consume-char") fn consume_char(packet: own Pair<Bytes, char>) -> i64 {
    match own packet { Pair { left: payload, right: marker } => if marker == 'x' { 1 } else { 0 }, }
}
@id("generic.check-char") fn check_char() -> i64 {
    let input = [4u8]; let packet = Pair<Bytes, char> { left: bytes_copy(array_as_slice(input)), right: 'x' };
    if inspect_char(packet) + consume_char(packet) == 2 { 0 } else { 100 }
}

@id("generic.inspect-f32") fn inspect_f32(packet: borrow Pair<Bytes, f32>) -> i64 {
    match borrow packet { Pair { left: payload, right: marker } =>
        if marker == 1.5f32 && byte_len(bytes_as_slice(payload)) == 1usize { 1 } else { 0 }, }
}
@id("generic.consume-f32") fn consume_f32(packet: own Pair<Bytes, f32>) -> i64 {
    match own packet { Pair { left: payload, right: marker } => if marker == 1.5f32 { 1 } else { 0 }, }
}
@id("generic.check-f32") fn check_f32() -> i64 {
    let input = [5u8]; let packet = Pair<Bytes, f32> { left: bytes_copy(array_as_slice(input)), right: 1.5f32 };
    if inspect_f32(packet) + consume_f32(packet) == 2 { 0 } else { 100 }
}

@id("generic.inspect-f64") fn inspect_f64(packet: borrow Pair<Bytes, f64>) -> i64 {
    match borrow packet { Pair { left: payload, right: marker } =>
        if marker == 2.5 && byte_len(bytes_as_slice(payload)) == 1usize { 1 } else { 0 }, }
}
@id("generic.consume-f64") fn consume_f64(packet: own Pair<Bytes, f64>) -> i64 {
    match own packet { Pair { left: payload, right: marker } => if marker == 2.5 { 1 } else { 0 }, }
}
@id("generic.check-f64") fn check_f64() -> i64 {
    let input = [6u8]; let packet = Pair<Bytes, f64> { left: bytes_copy(array_as_slice(input)), right: 2.5 };
    if inspect_f64(packet) + consume_f64(packet) == 2 { 0 } else { 100 }
}

@id("generic.fail")
fn fail() -> i64 {
    let packet = make();
    let failure = 1 / 0;
    consume(packet) + failure
}

@id("app.main") fn main() -> i64 {
    run() + check_u8() + check_i64() + check_i32() + check_usize() + check_char() + check_f32() + check_f64()
}
"#;

fn symbol(id: &str) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

#[test]
fn concrete_generic_owned_records_execute_and_settle_on_all_three_backends() {
    let parsed = parse(
        GENERIC_OWNED_SOURCE,
        Path::new("generic-owned-record-runtime-v1.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "{diagnostics:?}"
    );

    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-generic-owned-interpreter-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, GENERIC_OWNED_SOURCE).unwrap();
    let result =
        interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    assert_eq!(envelope["payload"]["outcome"]["value"], "42");
    interpreter::verify_envelope(&result.envelope).unwrap();
    let failure =
        interpreter::interpret(&path, "generic.fail", &[], &InterpreterOptions::default()).unwrap();
    assert!(!failure.returned);
    interpreter::verify_envelope(&failure.envelope).unwrap();
    let _ = std::fs::remove_file(path);

    let clang_available = Command::new("clang").arg("--version").output().is_ok();
    assert!(
        clang_available || !generic_owned_backends_required(),
        "required generic-owned Clang backend is unavailable"
    );
    if clang_available {
        let generated = codegen::emit_c(&parsed).unwrap();
        assert_eq!(generated, codegen::emit_c(&parsed).unwrap());
        let tracked = generated
            .replace(
                "uint8_t *payload = (uint8_t *)malloc(",
                "uint8_t *payload = (uint8_t *)spx_test_malloc(",
            )
            .replace("free(value->ptr);", "spx_test_free(value->ptr);");
        let probe = format!(
            r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(101), entries, UINT32_C(16), NULL, NULL, NULL)) return 1;
    int64_t result = INT64_C(0);
    for (uint32_t iteration = 0; iteration < UINT32_C(4); ++iteration) {{
        if ({main}(&context, &result) != SPX_STATUS_SUCCESS || result != INT64_C(42)) return 2;
        if (spx_test_live_allocations != UINT64_C(0)) return 3;
    }}
    if ({fail}(&context, &result) == SPX_STATUS_SUCCESS) return 4;
    if (spx_test_live_allocations != UINT64_C(0)) return 5;
    return 0;
}}
"#,
            main = symbol("app.main"),
            fail = symbol("generic.fail"),
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
    if (allocation != NULL) {
        if (spx_test_live_allocations == UINT64_C(0)) abort();
        spx_test_live_allocations -= UINT64_C(1);
    }
    free(allocation);
}
"#;
        for optimization in ["-O0", "-O2"] {
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "semaprax-generic-owned-native-{}-{serial}",
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
        node_available || !generic_owned_backends_required(),
        "required generic-owned Node backend is unavailable"
    );
    if node_available {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-generic-owned-wasm-{}-{serial}",
            std::process::id()
        ));
        wasm::build_web(&parsed, &root).unwrap();
        std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
        std::fs::write(
            root.join("probe.mjs"),
            r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const bytes=await readFile('./app.wasm');
const {instance}=await instantiateBytes(bytes,{maxOwnedByteEntries:1});
for(let i=0;i<4;i+=1){const value=instance.exports.semaprax_main();if(value!==42n)throw Error(`generic-owned:${value}`);}
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
    }
}

#[test]
fn interpreter_executes_nested_own_and_borrow_destructuring_repeatedly() {
    for _ in 0..4 {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-nested-owned-interpreter-{}-{serial}.spx",
            std::process::id()
        ));
        std::fs::write(&path, SOURCE).unwrap();
        let result =
            interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        let _ = std::fs::remove_file(path);
        let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        assert_eq!(envelope["payload"]["outcome"]["value"], "42");
        interpreter::verify_envelope(&result.envelope).unwrap();
    }
}

#[test]
fn interpreter_update_failure_settles_without_publication() {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-nested-update-failure-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    let result =
        interpreter::interpret(&path, "runtime.fail", &[], &InterpreterOptions::default()).unwrap();
    assert!(!result.returned);
    assert!(result
        .envelope
        .contains("\"domain_id\":\"semaprax.arithmetic.v1\""));
    assert!(result.envelope.contains("\"code\":1"));
    interpreter::verify_envelope(&result.envelope).unwrap();
    let _ = std::fs::remove_file(path);
}

#[test]
fn interpreter_updates_direct_bytes_records_with_copy_only_nested_subtrees() {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-nested-update-direct-bytes-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    let result = interpreter::interpret(
        &path,
        "runtime.run-direct-bytes-copy-subtree",
        &[],
        &InterpreterOptions::default(),
    )
    .unwrap();
    let _ = std::fs::remove_file(path);
    let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    assert_eq!(envelope["payload"]["outcome"]["value"], "9");
    interpreter::verify_envelope(&result.envelope).unwrap();
}

/// A Copy-only carrier inside the nested-record profile owns no cleanup leaf,
/// so the interpreter builds and reads it on every route rather than refusing
/// what `check` verified and both backends execute.
#[test]
fn interpreter_admits_copy_only_record_construction_on_every_route() {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-copy-only-record-construction-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    let result = interpreter::interpret(
        &path,
        "runtime.copy-only-construction",
        &[],
        &InterpreterOptions::default(),
    )
    .expect("standalone Copy-only record construction is inside the interpreter profile");
    let _ = std::fs::remove_file(path);
    let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    assert_eq!(envelope["payload"]["outcome"]["value"], "1");
    interpreter::verify_envelope(&result.envelope).unwrap();
}

#[test]
fn native_moves_nested_carriers_fieldwise_at_o0_and_o2() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let parsed = parse(SOURCE, Path::new("nested-owned-record-native-v1.spx")).unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "{diagnostics:?}"
    );
    let generated = codegen::emit_c(&parsed).unwrap();
    assert_eq!(generated, codegen::emit_c(&parsed).unwrap());
    assert!(!generated.contains("memcpy(&"));
    assert!(!generated.contains("spx_result = spx_local_"));
    assert!(generated.contains("spx_bytes_move"));
    let tracked_generated = generated
        .replace(
            "uint8_t *payload = (uint8_t *)malloc(",
            "uint8_t *payload = (uint8_t *)spx_test_malloc(",
        )
        .replace("free(value->ptr);", "spx_test_free(value->ptr);");
    assert_ne!(tracked_generated, generated);

    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(97), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    int64_t result = INT64_C(0);
    for (uint32_t iteration = UINT32_C(0); iteration < UINT32_C(4); ++iteration) {{
        if ({main}(&context, &result) != SPX_STATUS_SUCCESS) return 11;
        if (result != INT64_C(42)) return 12;
        if (spx_test_live_allocations != UINT64_C(0)) return 14;
    }}
    if ({fail}(&context, &result) == SPX_STATUS_SUCCESS) return 13;
    if (spx_test_live_allocations != UINT64_C(0)) return 15;
    return 0;
}}
"#,
        main = symbol("app.main"),
        fail = symbol("runtime.fail"),
    );
    for optimization in ["-O0", "-O2"] {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-nested-owned-native-{}-{serial}",
            std::process::id()
        ));
        let c = root.with_extension("c");
        let executable = root.with_extension(std::env::consts::EXE_EXTENSION);
        let allocator_probe = r#"
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
        std::fs::write(
            &c,
            format!("{allocator_probe}\n{tracked_generated}\n{probe}"),
        )
        .unwrap();
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

#[test]
fn nested_record_classification_does_not_capture_legacy_owned_variants() {
    let source = include_str!("../owned_byte_variant_v1_fixture.spx");
    let parsed = parse(
        source,
        Path::new("owned-byte-variant-native-regression.spx"),
    )
    .unwrap();
    let generated = codegen::emit_c(&parsed).expect("legacy owned variants keep native lowering");
    assert!(generated.contains("invalid variant tag"));
    assert!(generated.contains("spx_bytes_move"));
}

#[test]
fn wasm_executes_nested_views_repeatedly_without_owner_growth() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let parsed = parse(SOURCE, Path::new("nested-owned-record-wasm-v1.spx")).unwrap();
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-nested-owned-wasm-{}-{serial}",
        std::process::id()
    ));
    wasm::build_web(&parsed, &root).unwrap();
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    std::fs::write(
        root.join("probe.mjs"),
        r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
	const bytes=await readFile('./app.wasm');
	const limited=await instantiateBytes(bytes,{maxOwnedByteEntries:2});
	let capacityRejected=false;
	try{limited.instance.exports.semaprax_main();}catch(error){capacityRejected=String(error).includes('live entry limit exceeded');}
	if(!capacityRejected)throw Error('nested-owned update did not require its exact transient third owner');
	const {instance}=await instantiateBytes(bytes,{maxOwnedByteEntries:3});
	for(let i=0;i<4;i+=1){const value=instance.exports.semaprax_main();if(value!==42n)throw Error(`nested-owned:${value}`);}
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
}
