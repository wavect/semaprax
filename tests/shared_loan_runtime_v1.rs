use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, hir, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.shared_loan_runtime_v1;

@id("loan.consume")
fn consume(value: own Bytes) -> i64 {
    match byte_get(bytes_as_slice(value), 0usize) {
        Option::Some { value: byte } => if byte == 7u8 { 7 } else { 0 },
        Option::None {} => 0,
    }
}

@id("loan.multiple-reborrow")
fn multiple_reborrow() -> i64 {
    let source = [7u8, 8u8, 9u8, 10u8];
    let owned = bytes_copy(array_as_slice(source));
    let left = bytes_as_slice(owned);
    let right = bytes_as_slice(owned);
    let child = byte_range(left, 1usize, 3usize);
    let valid = byte_len(left) == 4usize
        && byte_len(right) == 4usize
        && byte_len(child) == 2usize
        && match byte_get(child, 0usize) {
            Option::Some { value: byte } => byte == 8u8,
            Option::None {} => false,
        }
        && match byte_get(right, 3usize) {
            Option::Some { value: byte } => byte == 10u8,
            Option::None {} => false,
        };
    let observed = if valid { 35 } else { 0 };
    consume(owned) + observed
}

@id("app.main")
fn main() -> i64 { multiple_reborrow() }
"#;

const ADAPTER_SOURCE: &str = r#"
module test.shared_loan_adapter_hostile_v1;
@id("app.main")
fn main() -> i64 { 0 }
"#;

fn source_file() -> PathBuf {
    let serial = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-shared-loan-runtime-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    path
}

fn symbol(id: &str) -> String {
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

fn returned_value(envelope: &str) -> String {
    let document: serde_json::Value = serde_json::from_str(envelope).unwrap();
    document["payload"]["outcome"]["value"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn interpreter_executes_multiple_shared_views_reborrow_and_post_last_use_move() {
    let parsed = parse(SOURCE, Path::new("shared-loan-runtime-v1.spx")).unwrap();
    assert!(verify::verify(&parsed).is_empty());
    hir::validate(&hir::resolve(&parsed).unwrap()).unwrap();

    let path = source_file();
    let result = interpreter::interpret(
        &path,
        "loan.multiple-reborrow",
        &[],
        &InterpreterOptions::default(),
    )
    .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(result.returned);
    assert_eq!(returned_value(&result.envelope), "42");
    interpreter::verify_envelope(&result.envelope).unwrap();
}

#[test]
fn native_o0_and_o2_execute_the_same_shared_loan_program() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = parse(SOURCE, Path::new("shared-loan-runtime-native-v1.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(301), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    int64_t result = INT64_C(0);
    if ({main}(&context, &result) != SPX_STATUS_SUCCESS) return 11;
    return result == INT64_C(42) ? 0 : 12;
}}
"#,
        main = symbol("app.main"),
    );

    for optimization in ["-O0", "-O2"] {
        let serial = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-shared-loan-native-{}-{serial}",
            std::process::id()
        );
        let c_path = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&c_path, format!("{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&c_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(c_path);
        let _ = std::fs::remove_file(executable);
        assert!(executed.status.success(), "probe failed: {executed:?}");
    }
}

#[test]
fn node_wasm_executes_repeated_shared_loan_reentry_without_leaking_owned_slots() {
    let program = parse(SOURCE, Path::new("shared-loan-runtime-wasm-v1.spx")).unwrap();
    let first = wasm::emit_module(&program).unwrap();
    assert_eq!(first, wasm::emit_module(&program).unwrap());
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }

    let serial = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-shared-loan-wasm-{}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    wasm::build_web(&program, &root).unwrap();
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    std::fs::write(
        root.join("probe.mjs"),
        r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:1});
for(let i=0;i<4;i+=1){const value=instance.exports.semaprax_main();if(value!==42n)throw Error(`semantic-or-settlement:${value}`);}
console.log('shared-loan-runtime-v1-ok');
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"shared-loan-runtime-v1-ok\n");
}

#[test]
fn generated_node_adapter_rejects_every_forged_byte_range_descriptor_field() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = parse(
        ADAPTER_SOURCE,
        Path::new("shared-loan-adapter-hostile-v1.spx"),
    )
    .unwrap();
    let serial = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-shared-loan-adapter-hostile-{}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    wasm::build_web(&program, &root).unwrap();
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();

    let runtime = std::fs::read_to_string(root.join("semaprax.js")).unwrap();
    let exposed = runtime.replacen(
        "function createByteDataRuntime(options = {})",
        "export function createByteDataRuntime(options = {})",
        1,
    );
    assert_ne!(
        runtime, exposed,
        "generated adapter factory must remain present"
    );
    std::fs::write(root.join("adapter-harness.mjs"), exposed).unwrap();
    std::fs::write(
        root.join("hostile.mjs"),
        r#"import {createByteDataRuntime} from './adapter-harness.mjs';
const runtime=createByteDataRuntime();
const memory=new WebAssembly.Memory({initial:2,maximum:2});
runtime.bind({exports:{__spx_byte_memory:memory}});
const imports=runtime.imports;
const bytes=new Uint8Array(memory.buffer);
const view=new DataView(memory.buffer);
const pointer=64,identity=1,ultimateRoot=1024,ultimateLength=4,selectedLength=2;
bytes.set([7,8,9,10],ultimateRoot);
const pack=(root,length)=>BigInt.asIntN(64,(BigInt(root>>>0)<<32n)|BigInt(length));
const root=(0x40000000|(identity<<16)|(pointer/8))>>>0;
const carrier=pack(root,selectedLength);
const writeValid=()=>{view.setUint32(pointer,identity,true);view.setUint32(pointer+4,pointer,true);view.setBigInt64(pointer+8,pack(ultimateRoot,ultimateLength),true);view.setBigUint64(pointer+16,1n,true);view.setBigUint64(pointer+24,2n,true);};
const rejectsCarrier=(message,candidate)=>{let observed='';try{imports.spx_bytes_get(candidate,0n)}catch(error){observed=error?.message??''}if(observed!==message)throw Error(`expected:${message}:observed:${observed}`)};
const rejects=(message,mutate)=>{writeValid();mutate();rejectsCarrier(message,carrier)};
writeValid();
if(imports.spx_bytes_get(carrier,0n)!==8||imports.spx_bytes_get(carrier,1n)!==9)throw Error('valid-range');
rejectsCarrier('SEMAPRAX byte range descriptor bounds invariant',pack((0x40000000|(identity<<16)|0xffff)>>>0,selectedLength));
rejects('SEMAPRAX byte range descriptor identity invariant',()=>view.setUint32(pointer,2,true));
rejects('SEMAPRAX byte range descriptor identity invariant',()=>view.setUint32(pointer+4,0,true));
rejects('SEMAPRAX byte range descriptor length invariant',()=>view.setBigUint64(pointer+24,3n,true));
rejects('SEMAPRAX byte range descriptor extent invariant',()=>view.setBigUint64(pointer+16,3n,true));
rejects('SEMAPRAX nested byte range descriptor invariant',()=>view.setBigInt64(pointer+8,carrier,true));
console.log('shared-loan-adapter-hostile-v1-ok');
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg("hostile.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"shared-loan-adapter-hostile-v1-ok\n");
}
