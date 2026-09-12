//! Real, Node-hosted evidence that the Core Wasm physical adapter's fixture
//! endpoint (byte-reversal, matching `spx_pg_wasm_endpoint_reverse_bytes_v1`
//! — see `src/public_generic_abi/wasm/provider.rs`) executes for real
//! against a genuine `WebAssembly.Memory` instance, not a Rust-hosted
//! stand-in. This is deliberately narrower than the full carrier protocol —
//! see `src/public_generic_abi/wasm/reverse_probe.mjs`'s own header comment
//! and docs/PUBLIC-GENERIC-CARRIER-V1.md#core-wasm-physical-adapter-issue-155
//! for exactly what this proves and what it does not: the full
//! handle/registry/sticky-settlement protocol is exercised in Rust against
//! `WasmProvider` instead (`src/public_generic_abi/wasm/provider/tests.rs`),
//! which this small standalone Node script has no access to.
//!
//! Known limitation, stated once here rather than hidden in prose: this is
//! local evidence on whatever host this round ran on, gated on `node` being
//! on `PATH` (skips otherwise, matching this repository's existing Node-
//! hosted Wasm test convention in `src/wasm/aggregate/tests/owned_buffer.rs`
//! and `src/wasm/aggregate_range_tests.rs`). It is not hosted CI evidence.

use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

fn run_probe(leaves: &[&[u8]]) -> serde_json::Value {
    let root = std::env::temp_dir().join(format!(
        "semaprax-public-generic-wasm-adapter-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    // Do not canonicalize on Windows: `canonicalize()` returns a `\\?\`
    // verbatim path that `node` (and some `fs` APIs) cannot handle, causing
    // `InvalidFilename: The filename or extension is too long` (206) on
    // `windows-latest` for `real_webassembly_memory_grows_across_a_page_boundary`
    // (103534761111, 34686666474). The temp dir is already absolute and
    // unique, so no canonicalization is needed for isolation.
    eprintln!("retained Core Wasm adapter evidence: {}", root.display());

    let script = root.join("reverse_probe.mjs");
    fs::write(
        &script,
        semaprax::public_generic_abi::wasm::probe::REVERSE_PROBE_MJS,
    )
    .unwrap();

    let mut command = Command::new("node");
    command.arg(&script);
    for leaf in leaves {
        command.arg(hex_encode(leaf));
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn real_webassembly_memory_reverses_every_leaf_and_zeroes_on_release() {
    if !node_available() {
        return;
    }
    let leaves: [&[u8]; 3] = [b"hello", b"", b"a-longer-owned-leaf-payload"];
    let parsed = run_probe(&leaves);
    assert_eq!(parsed["ok"], serde_json::Value::Bool(true));
    let results = parsed["results"].as_array().unwrap();
    assert_eq!(results.len(), leaves.len());
    for (leaf, entry) in leaves.iter().zip(results) {
        let reversed = hex_decode(entry["reversedHex"].as_str().unwrap());
        let expected: Vec<u8> = leaf.iter().rev().copied().collect();
        assert_eq!(reversed, expected);
    }
    // The script itself already asserts the arena is fully zeroed after
    // every leaf's release (throwing and failing the process otherwise);
    // reaching here is additional confirmation the process exited clean.
}

#[test]
fn real_webassembly_memory_grows_across_a_page_boundary() {
    if !node_available() {
        return;
    }
    // One page is 65536 bytes; force a real `memory.grow` call.
    let big_leaf = vec![0xab_u8; 70_000];
    let parsed = run_probe(&[&big_leaf]);
    assert_eq!(parsed["ok"], serde_json::Value::Bool(true));
    let results = parsed["results"].as_array().unwrap();
    let pages = results[0]["pages"].as_u64().unwrap();
    assert!(pages >= 2, "expected a real page grow, got {pages} pages");
    let reversed = hex_decode(results[0]["reversedHex"].as_str().unwrap());
    let expected: Vec<u8> = big_leaf.iter().rev().copied().collect();
    assert_eq!(reversed, expected);
}

#[test]
fn probe_rejects_being_run_with_no_leaves() {
    if !node_available() {
        return;
    }
    let script_dir = std::env::temp_dir().join(format!(
        "semaprax-public-generic-wasm-adapter-empty-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&script_dir).unwrap();
    let script = script_dir.join("reverse_probe.mjs");
    fs::write(
        &script,
        semaprax::public_generic_abi::wasm::probe::REVERSE_PROBE_MJS,
    )
    .unwrap();
    let output = Command::new("node").arg(&script).output().unwrap();
    assert!(!output.status.success());
}
