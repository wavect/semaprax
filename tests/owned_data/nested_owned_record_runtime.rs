use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, parse, verify, wasm};

static SERIAL: AtomicU64 = AtomicU64::new(0);

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
    @id("runtime.envelope.sequence") sequence: usize,
}

@id("runtime.identity")
fn identity(packet: own Envelope) -> Envelope { packet }

@id("runtime.inspect")
fn inspect(packet: own Envelope) -> i64 {
    let left = bytes_as_slice(packet.left.leaf.payload);
    let right = bytes_as_slice(packet.right.leaf.payload);
    if byte_len(left) == 1usize && byte_len(right) == 2usize { 42 } else { 0 }
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
        sequence: 3usize,
    };
    let moved = identity(packet);
    inspect(moved)
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

#[test]
fn interpreter_executes_two_simultaneous_nested_views_after_a_whole_move() {
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

#[test]
fn native_moves_nested_carriers_fieldwise_at_o0_and_o2() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let parsed = parse(SOURCE, Path::new("nested-owned-record-native-v1.spx")).unwrap();
    assert!(verify::verify(&parsed)
        .iter()
        .all(|diagnostic| !diagnostic.severity.is_error()));
    let generated = codegen::emit_c(&parsed).unwrap();
    assert_eq!(generated, codegen::emit_c(&parsed).unwrap());
    assert!(!generated.contains("memcpy(&"));
    assert!(!generated.contains("spx_result = spx_local_"));
    assert!(generated.contains("spx_bytes_move"));

    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(97), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    int64_t result = INT64_C(0);
    if ({main}(&context, &result) != SPX_STATUS_SUCCESS) return 11;
    return result == INT64_C(42) ? 0 : 12;
}}
"#,
        main = symbol("app.main"),
    );
    for optimization in ["-O0", "-O2"] {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-nested-owned-native-{}-{serial}",
            std::process::id()
        ));
        let c = root.with_extension("c");
        let executable = root.with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::write(&c, format!("{generated}\n{probe}")).unwrap();
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
    std::fs::create_dir_all(&root).unwrap();
    wasm::build_web(&parsed, &root).unwrap();
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    std::fs::write(
        root.join("probe.mjs"),
        r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:2});
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
