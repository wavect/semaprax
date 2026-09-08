//! Hostile Core-Wasm v2 directory-result validation.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

use semaprax::{hir, parse, wasm};

const FACADE: &str = include_str!("../project/standard_library/filesystem_v2_facade.mjs");

fn symbol(id: &str) -> String {
    format!(
        "spx_data_{}",
        id.bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn list_source(expected: usize) -> String {
    format!(
        "module test.filesystem_v2_wasm;\npermit {{ fs.read }}\n@id(\"filesystem-v2-wasm.run\") fn run() -> bool uses {{ fs.read }} {{ let root = [0u8]; let listed = file_list(array_as_slice(root), 0usize, 65536usize); byte_len(bytes_as_slice(listed)) == {expected}usize }}\n@id(\"main\") fn main() -> i64 {{ 0 }}\n"
    )
}

fn compiled(source: &str) -> Vec<u8> {
    let program =
        hir::resolve(&parse(source, Path::new("filesystem-v2-wasm.spx")).unwrap()).unwrap();
    wasm::emit_resolved_filesystem_ops_v2(&program, "filesystem-v2-wasm.run").unwrap()
}

fn run(bytes: &[u8], list_expression: &str) -> String {
    let stem = std::env::temp_dir().join(format!(
        "semaprax-filesystem-v2-wasm-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    let wasm_path = stem.with_extension("wasm");
    let script_path = stem.with_extension("mjs");
    std::fs::write(&wasm_path, bytes).unwrap();
    let callback = format!(
        "spx_filesystem_list_v2:(r,c,l,max,p)=>{{calls++;const k=key(r,c,l);if(k!=='')return 2;const data={list_expression};if(data.length>Number(max))return 4;out(p,allocate(data));return 0}},"
    );
    let facade = FACADE
        .replace(
            "spx_filesystem_list_v2:(r,c,l,max,p)=>{calls++;const k=key(r,c,l);let data;if(raw&&k==='')data=new Uint8Array([97,0,255,0]);else if(k==='64'&&directories.has(k)&&files.has('642f61')){if(files.get('642f61')[0]!==255)throw Error('replace');data=new Uint8Array([97,0])}else return 2;if(data.length>max)return 4;out(p,allocate(data));return 0},",
            &callback,
        )
        .replace(
            "for(let i=0;i<2;i++){seed();const value=instance.exports[symbol]();console.log(`run ${value} ${instance.exports.__spx_data_status_v1.value} ${instance.exports.__spx_filesystem_status_v2.value} ${owned.size}`);if(calls!==(raw?2:7)||(!raw&&(files.size!==0||directories.size!==0)))throw Error('settlement')}",
            "for(let i=0;i<2;i++){seed();const value=instance.exports[symbol]();console.log(`run ${value} ${instance.exports.__spx_filesystem_status_v2.value} ${owned.size}`);if(owned.size!==0)throw Error('owned-result-leak')}",
        );
    std::fs::write(&script_path, facade).unwrap();
    let output = Command::new("node")
        .args([
            script_path.to_str().unwrap(),
            wasm_path.to_str().unwrap(),
            &symbol("filesystem-v2-wasm.run"),
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&wasm_path);
    let _ = std::fs::remove_file(&script_path);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn filesystem_v2_wasm_rejects_hostile_directory_wires_and_releases_owned_results() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let bytes = compiled(&list_source(0));
    let cases = [
        ("missing-nul", "new Uint8Array([97])"),
        ("unsorted", "new Uint8Array([98,0,97,0])"),
        ("duplicate", "new Uint8Array([97,0,97,0])"),
        ("empty-name", "new Uint8Array([0])"),
        ("dot", "new Uint8Array([46,0])"),
        ("dotdot", "new Uint8Array([46,46,0])"),
        ("slash", "new Uint8Array([97,47,0])"),
        ("overlong-name", "new Uint8Array([...new Uint8Array(4097).fill(97),0])"),
        ("too-many", "(()=>{const a=[];for(let i=0;i<1025;i++)a.push(...Buffer.from(i.toString(16).padStart(3,'0')),0);return new Uint8Array(a)})()"),
    ];
    for (name, wire) in cases {
        assert_eq!(
            run(&bytes, wire),
            "validate 1\nrun 0 5 0\nrun 0 5 0\n",
            "{name}"
        );
    }
}

#[test]
fn filesystem_v2_wasm_accepts_empty_raw_and_prefix_sorted_directory_wires() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    for (name, expected, wire) in [
        ("empty", 0, "new Uint8Array([])"),
        ("raw-non-utf8", 2, "new Uint8Array([255,0])"),
        ("prefix-order", 5, "new Uint8Array([97,0,97,1,0])"),
    ] {
        let output = run(&compiled(&list_source(expected)), wire);
        assert_eq!(output, "validate 1\nrun 1 0 0\nrun 1 0 0\n", "{name}");
    }
}

#[test]
fn filesystem_v2_wasm_emits_only_injected_v2_directory_imports() {
    let bytes = compiled(&list_source(0));
    for name in [
        b"spx_filesystem_list_v2".as_slice(),
        b"__spx_filesystem_status_v2".as_slice(),
    ] {
        assert!(
            bytes.windows(name.len()).any(|window| window == name),
            "missing {:?}",
            name
        );
    }
    assert!(!bytes
        .windows(b"wasi_snapshot_preview1".len())
        .any(|item| item == b"wasi_snapshot_preview1"));
}
