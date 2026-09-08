//! Closed Wasm import surface for Filesystem I/O v1.

use std::path::Path;
use std::process::Command;

use semaprax::{hir, parse, wasm};

const FACADE: &str = include_str!("filesystem_ops_wasm_facade.mjs");

const SOURCE: &str = r#"
module test.filesystem_wasm;

permit { fs.read, fs.write }

@id("test.filesystem_wasm.run")
fn run() -> bool
    uses { fs.read, fs.write }
{
    let path = [102u8, 105u8, 120u8, 116u8, 117u8, 114u8, 101u8, 47u8, 46u8, 46u8];
    let data = [104u8, 101u8, 108u8, 108u8, 111u8];
    let written = file_write_new(array_as_slice(path), 7usize, array_as_slice(data), 5usize);
    let read = file_read(array_as_slice(path), 7usize, 5usize);
    written == 5usize && byte_len(bytes_as_slice(read)) == 5usize
}

@id("main")
fn main() -> i64
{
    0
}
"#;

#[test]
fn filesystem_wasm_rejects_every_invalid_path_shape_before_dispatch() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let cases: &[(&str, &[u8], usize, u32)] = &[
        ("empty", b"", 1, 1),
        ("nul", b"a\0b", 1, 1),
        ("backslash", b"a\\b", 1, 1),
        ("colon", b"a:b", 1, 1),
        ("leading-slash", b"/a", 1, 1),
        ("trailing-slash", b"a/", 1, 1),
        ("empty-component", b"a//b", 1, 1),
        ("dot-component", b"a/./b", 1, 1),
        ("dotdot-component", b"a/../b", 1, 1),
        ("nul-before-file-max", b"a\0b", 65_537, 1),
        ("total-max-before-nul", b"a\0b", 1_048_577, 4),
    ];
    for (label, path_bytes, charge, expected) in cases {
        let literal = if path_bytes.is_empty() {
            "120u8".to_owned()
        } else {
            path_bytes
                .iter()
                .map(|byte| format!("{byte}u8"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let source = format!(
            r#"module test.filesystem_wasm_invalid;
permit {{ fs.write }}
@id("test.filesystem_wasm_invalid.run")
fn run() -> bool uses {{ fs.write }} {{
    let path = [{literal}];
    let data = [1u8];
    file_write_new(array_as_slice(path), {length}usize, array_as_slice(data), {charge}usize) == 1usize
}}
@id("main") fn main() -> i64 {{ 0 }}
"#,
            length = path_bytes.len(),
            charge = charge
        );
        let program =
            hir::resolve(&parse(&source, Path::new("filesystem-wasm-invalid.spx")).unwrap())
                .unwrap();
        let bytes =
            wasm::emit_resolved_filesystem_ops_v1(&program, "test.filesystem_wasm_invalid.run")
                .unwrap();
        let stem = std::env::temp_dir().join(format!(
            "semaprax-filesystem-wasm-invalid-{}-{label}",
            std::process::id()
        ));
        let wasm_path = stem.with_extension("wasm");
        let facade_path = stem.with_extension("mjs");
        std::fs::write(&wasm_path, bytes).unwrap();
        let facade = FACADE.replace("const env={", "let writeCalls=0;const env={").replace("spx_filesystem_write_new_v1:(pathRoot,pathCarrier,pathLength,dataRoot,dataCarrier,dataLength,pointer)=>{", "spx_filesystem_write_new_v1:(pathRoot,pathCarrier,pathLength,dataRoot,dataCarrier,dataLength,pointer)=>{writeCalls++;").replace("for(let i=0;i<2;i++){const value=instance.exports[symbol]();console.log(`run ${value} ${instance.exports.__spx_data_status_v1.value} ${instance.exports.__spx_filesystem_status_v1.value} ${owned.size}`);files.clear()}", "const value=instance.exports[symbol]();console.log(`invalid ${value} ${instance.exports.__spx_filesystem_status_v1.value} ${writeCalls}`)");
        std::fs::write(&facade_path, facade).unwrap();
        let symbol = format!(
            "spx_data_{}",
            "test.filesystem_wasm_invalid.run"
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let output = Command::new("node")
            .args([
                facade_path.to_str().unwrap(),
                wasm_path.to_str().unwrap(),
                &symbol,
            ])
            .output()
            .unwrap();
        let _ = std::fs::remove_file(wasm_path);
        let _ = std::fs::remove_file(facade_path);
        assert!(
            output.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("validate 1\ninvalid 0 {expected} 0\n"),
            "{label}"
        );
    }
}

fn contains(bytes: &[u8], name: &[u8]) -> bool {
    bytes.windows(name.len()).any(|window| window == name)
}

#[test]
fn filesystem_wasm_names_only_its_closed_provider_imports_and_status_marker() {
    let program = hir::resolve(&parse(SOURCE, Path::new("filesystem-wasm.spx")).unwrap()).unwrap();
    let bytes =
        wasm::emit_resolved_filesystem_ops_v1(&program, "test.filesystem_wasm.run").unwrap();
    assert_eq!(
        bytes,
        wasm::emit_resolved_filesystem_ops_v1(&program, "test.filesystem_wasm.run").unwrap()
    );
    assert!(contains(&bytes, b"spx_filesystem_read_v1"));
    assert!(contains(&bytes, b"spx_filesystem_write_new_v1"));
    assert!(contains(&bytes, b"__spx_filesystem_status_v1"));
    assert!(!contains(&bytes, b"wasi_snapshot_preview1"));
    assert!(!contains(&bytes, b"spx_network_connect_v1"));

    if Command::new("node").arg("--version").output().is_ok() {
        let path = std::env::temp_dir().join(format!(
            "semaprax-filesystem-wasm-{}-{}.wasm",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, &bytes).unwrap();
        let facade = path.with_extension("mjs");
        std::fs::write(&facade, FACADE).unwrap();
        let symbol = format!(
            "spx_data_{}",
            "test.filesystem_wasm.run"
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let output = Command::new("node")
            .args([facade.to_str().unwrap(), path.to_str().unwrap(), &symbol])
            .output()
            .unwrap();
        std::fs::write(
            &facade,
            FACADE.replace(
                "out(pointer,BigInt(dataLength));return 0",
                "out(pointer,BigInt(dataLength+1));return 0",
            ),
        )
        .unwrap();
        let mismatched_write = Command::new("node")
            .args([facade.to_str().unwrap(), path.to_str().unwrap(), &symbol])
            .output()
            .unwrap();
        std::fs::write(
            &facade,
            FACADE.replace(
                "const data=files.get(key(pathRoot,pathCarrier,pathLength));if(!data)return 2;if(data.length>max)return 4;out(pointer,allocate(data));return 0",
                "const data=new Uint8Array(max+1);out(pointer,allocate(data));return 0",
            ),
        )
        .unwrap();
        let oversized_read = Command::new("node")
            .args([facade.to_str().unwrap(), path.to_str().unwrap(), &symbol])
            .output()
            .unwrap();
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(facade);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "validate 1\nrun 1 0 0 0\nrun 1 0 0 0\n"
        );
        assert!(
            mismatched_write.status.success(),
            "{}",
            String::from_utf8_lossy(&mismatched_write.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&mismatched_write.stdout),
            "validate 1\nrun 0 5 5 0\nrun 0 5 5 0\n"
        );
        assert!(
            oversized_read.status.success(),
            "{}",
            String::from_utf8_lossy(&oversized_read.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&oversized_read.stdout),
            "validate 1\nrun 0 4 4 0\nrun 0 4 4 0\n"
        );
    }
}

#[test]
fn filesystem_wasm_enforces_cumulative_limits_per_invocation() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let cases = [
        ("operations", "let path = [97u8]; let data = [0u8]; let path_view = array_as_slice(path); let data_view = array_as_slice(data); let mut i = 0usize; while i < 65usize { let written = file_write_new(path_view, 1usize, data_view, 0usize); i = i + 1usize; true } true", 64),
        ("bytes", "let path = [97u8]; let data = bytes_zeroed(65536usize); let path_view = array_as_slice(path); let data_view = bytes_as_slice(data); let mut i = 0usize; while i < 17usize { let written = file_write_new(path_view, 1usize, data_view, 65536usize); i = i + 1usize; true } true", 16),
    ];
    for (label, body, calls) in cases {
        let source = format!("module test.filesystem_wasm_limit;\npermit {{ fs.write }}\n@id(\"test.filesystem_wasm_limit.run\") fn run() -> bool uses {{ fs.write }} {{ {body} }}\n@id(\"main\") fn main() -> i64 {{ 0 }}\n");
        let program =
            hir::resolve(&parse(&source, Path::new("filesystem-wasm-limit.spx")).unwrap()).unwrap();
        let bytes =
            wasm::emit_resolved_filesystem_ops_v1(&program, "test.filesystem_wasm_limit.run")
                .unwrap();
        let stem = std::env::temp_dir().join(format!(
            "semaprax-filesystem-wasm-limit-{}-{label}",
            std::process::id()
        ));
        let wasm_path = stem.with_extension("wasm");
        let facade_path = stem.with_extension("mjs");
        std::fs::write(&wasm_path, bytes).unwrap();
        let facade = FACADE
            .replace("const env={", "let writeCalls=0;const env={\n spx_bytes_zeroed:count=>allocate(new Uint8Array(Number(count))),spx_bytes_set:(value,index,byte)=>{const data=bytes(value);data[Number(index)]=byte;return value},")
            .replace("spx_filesystem_write_new_v1:(pathRoot,pathCarrier,pathLength,dataRoot,dataCarrier,dataLength,pointer)=>{const name=key(pathRoot,pathCarrier,pathLength);if(files.has(name))return 3;files.set(name,prefix(dataRoot,dataCarrier,dataLength));out(pointer,BigInt(dataLength));return 0}", "spx_filesystem_write_new_v1:(_pathRoot,_pathCarrier,_pathLength,_dataRoot,_dataCarrier,dataLength,pointer)=>{writeCalls++;out(pointer,BigInt(dataLength));return 0}")
            .replace("for(let i=0;i<2;i++){const value=instance.exports[symbol]();console.log(`run ${value} ${instance.exports.__spx_data_status_v1.value} ${instance.exports.__spx_filesystem_status_v1.value} ${owned.size}`);files.clear()}", "for(let i=0;i<2;i++){const value=instance.exports[symbol]();console.log(`limit ${value} ${instance.exports.__spx_filesystem_status_v1.value} ${writeCalls} ${owned.size}`);files.clear()}");
        std::fs::write(&facade_path, facade).unwrap();
        let symbol = format!(
            "spx_data_{}",
            "test.filesystem_wasm_limit.run"
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let output = Command::new("node")
            .args([
                facade_path.to_str().unwrap(),
                wasm_path.to_str().unwrap(),
                &symbol,
            ])
            .output()
            .unwrap();
        let _ = std::fs::remove_file(wasm_path);
        let _ = std::fs::remove_file(facade_path);
        assert!(
            output.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!(
                "validate 1\nlimit 0 4 {calls} 0\nlimit 0 4 {} 0\n",
                calls * 2
            ),
            "{label}"
        );
    }
}
