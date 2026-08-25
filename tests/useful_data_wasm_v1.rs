use semaprax::{parse, wasm};
use std::path::Path;
use std::process::Command;

const SOURCE: &str = r#"module test.useful_data_wasm;

@id("bytes.forward")
fn forward(value: own Bytes) -> Bytes { value }

@id("bytes.inspect")
fn inspect() -> i64 {
    let source = [0u8, 255u8, 128u8, 0u8];
    let view = array_as_slice(source);
    let owned = bytes_copy(view);
    let forwarded = forward(owned);
    let copied_view = bytes_as_slice(forwarded);
    match byte_get(copied_view, 1usize) {
        Option::Some { value: byte } => if byte == 255u8 { 41 } else { 2 },
        Option::None {} => 3,
    }
}

@id("bytes.empty")
fn empty() -> i64 {
    let source = [];
    let view = array_as_slice(source);
    let owned = bytes_copy(view);
    let copied_view = bytes_as_slice(owned);
    if byte_len(copied_view) == 0usize { 1 } else { 0 }
}

@id("bytes.choose")
fn choose(flag: bool) -> i64 {
    if flag {
        let source = [1u8];
        let owned = bytes_copy(array_as_slice(source));
        if byte_len(bytes_as_slice(owned)) == 1usize { 1 } else { 0 }
    } else {
        let source = [2u8, 3u8];
        let owned = bytes_copy(array_as_slice(source));
        if byte_len(bytes_as_slice(owned)) == 2usize { 2 } else { 0 }
    }
}

@id("app.main")
fn main() -> i64 {
    let repeated = [9u8; 4];
    let repeated_view = array_as_slice(repeated);
    if byte_len(repeated_view) == 4usize { inspect() + empty() + choose(true) + choose(false) } else { 0 }
}
"#;

#[test]
fn core_wasm_arrays_and_owned_bytes_are_exact_and_execute() {
    let program = parse(SOURCE, Path::new("useful-data-wasm.spx")).unwrap();
    let first = wasm::emit_module(&program).unwrap();
    assert_eq!(first, wasm::emit_module(&program).unwrap());
    assert!(first.windows(14).any(|window| window == b"spx_bytes_copy"));

    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let root =
        std::env::temp_dir().join(format!("semaprax-useful-data-wasm-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    wasm::build_web(&program, &root).unwrap();
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    std::fs::write(
        root.join("probe.mjs"),
        "import {readFile} from 'node:fs/promises';\nimport {instantiateBytes} from './semaprax.js';\nconst bytes=await readFile('./app.wasm');\nfor(let i=0;i<2;i+=1){const {instance}=await instantiateBytes(bytes);if(instance.exports.semaprax_main()!==45n)throw Error('semantic');const memory=instance.exports.__spx_byte_memory;if(memory.buffer.byteLength!==131072)throw Error('memory');let fixed=false;try{memory.grow(1)}catch{fixed=true}if(!fixed)throw Error('grow');}\nconsole.log('useful-data-wasm-v1-ok');\n",
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
    assert_eq!(output.stdout, b"useful-data-wasm-v1-ok\n");
}

#[test]
fn path_specific_owned_bytes_cleanup_is_admitted_by_the_plan_bridge() {
    let source = r#"module test.useful_data_wasm_cleanup_gate;
@id("bytes.fail")
fn fail(value: i64) -> i64
requires value > 0
{ value }
@id("bytes.consume")
fn consume(bytes: own Bytes, value: i64) -> i64 {
    if byte_len(bytes_as_slice(bytes)) == 1usize { value } else { 0 }
}
@id("app.main")
fn main() -> i64 {
    let source = [1u8];
    let view = array_as_slice(source);
    let owned = bytes_copy(view);
    consume(owned, fail(0))
}
"#;
    let program = parse(source, Path::new("useful-data-wasm-cleanup-gate.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert!(bytes.windows(14).any(|window| window == b"spx_bytes_drop"));
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "semaprax-useful-data-wasm-failure-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    wasm::build_web(&program, &root).unwrap();
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    std::fs::write(
        root.join("probe.mjs"),
        "import {readFile} from 'node:fs/promises';\nimport {instantiateBytes} from './semaprax.js';\nconst {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:1});\nfor(let i=0;i<2;i+=1){let failed=false;try{instance.exports.semaprax_main()}catch(error){if(!(error instanceof Error)||error.message!=='SEMAPRAX contract failure')throw error;failed=true}if(!failed)throw Error('missing-failure')}\nconsole.log('useful-data-wasm-failure-cleanup-ok');\n",
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
    assert_eq!(output.stdout, b"useful-data-wasm-failure-cleanup-ok\n");
}
