//! Issue #229: does this compiler's own Wasm backend ever emit a genuinely
//! *compiled* `.wasm` artifact whose own bytecode performs a checked
//! endpoint's computation -- as opposed to `reverse_probe.mjs` (host
//! JavaScript reversing bytes around a bare `WebAssembly.Memory` object, no
//! module involved) or `reference_wasm_module.rs` (real Wasm bytecode, but
//! hand-assembled byte-by-byte, never run through a compiler)?
//!
//! This module answers yes for the fixture's core arithmetic, through
//! `semaprax::wasm::build_web_with_scalar_exports` -- the exact in-process
//! library entry point `semaprax build <file> --target wasm --export <id>`
//! (Public Scalar Export Profile v1) calls, and the same backend and
//! package shape `examples/calculator-web` already ships. `SOURCE` below is
//! ordinary, admitted SPX text; nothing here is hand-assembled and nothing
//! is validated only with `WebAssembly.validate()` -- the Node probe below
//! calls the real exported function through the compiler's own generated
//! JS glue (`semaprax.bindings.js` -> `semaprax.js`) and inspects the real
//! returned value.
//!
//! `reverse_packed_bytes` packs four bytes `(a, b, c, d)` (`a` least
//! significant) into one `i64` and returns them reordered `(d, c, b, a)` --
//! the same byte-reversal shape `spx_pg_wasm_endpoint_reverse_bytes_v1`
//! names, using only `+`, `-`, `*`, `/`, `%` (SEMAPRAX has no bitwise
//! shift/mask operators -- see `src/ast.rs`'s `BinaryOp`, and
//! `std/bytes/src/bytes.spx`'s own `byte_to_i64`, which does the same u8
//! conversion by counting rather than casting).
//!
//! What this does NOT prove, stated once rather than folded into a false
//! "issue #229 closed": this is not `WasmProvider`'s
//! open/input_prepare/call/result_export/release ABI, and no amount of
//! additional SPX source turns it into that ABI. Two concrete, checked
//! facts block it, discovered while attempting the more direct route
//! (return an owned `Bytes` leaf, matching the ABI's actual per-leaf
//! shape) before falling back to this packed-scalar shape:
//!
//! 1. **Every public Wasm export profile rejects the one owned-`Bytes`
//!    builder that can reorder bytes.** SPX's owned `Bytes` builder pair
//!    is `bytes_zeroed(<usize literal>)` / `bytes_set(<chain>, <usize
//!    literal>, <u8>)` (Owned Bounded Byte Buffer v1, `src/byte_ops.rs`).
//!    Both `emit_module_with_byte_exports` and the `owned-data-api.v1`
//!    project profile's Wasm lane refuse a selected export whose body uses
//!    it, verified directly against this compiler: `cargo check`s clean,
//!    but `semaprax build`/`prepare_owned_data_npm_build` reject it with
//!    `SPX-W115: Owned Bounded Byte Buffer v1 is internal-only and has no
//!    public WebAssembly adapter`. The only public owned-`Bytes` producer
//!    left is `bytes_copy` (an unconditional identity copy) -- there is no
//!    admitted way to author "return this input's bytes in a different
//!    order" as a public `Bytes` result at all, fixed-length or not.
//! 2. **Even if (1) did not hold, the calling convention would still not
//!    be the provider ABI's.** Every owned-data Wasm export compiles to
//!    `(parameters..., result_out: i32) -> status: i32` against a
//!    *host-owned* arena (`spx_bytes_copy`/`spx_bytes_get`/`spx_bytes_drop`
//!    JS imports -- see `semaprax.js`'s `createArena`): every call
//!    allocates a *new* host-tracked buffer and returns a fresh handle.
//!    `WasmProvider`'s documented ABI is a five-call
//!    open/input_prepare/call/result_export/release handle protocol with
//!    descriptor/binding byte replay and an in-module registry -- a
//!    different codegen target this compiler has no target profile for.
//!    Building one is a compiler feature (a new Wasm target profile plus
//!    its own admission/verification rules), not an SPX authoring
//!    exercise, and is the concrete blocking step this issue's own
//!    "decision above a bounded worker's authority" already named.
//!
//! Gated on `node` being on `PATH`; skips (never fails) otherwise, matching
//! this repository's existing Node-hosted Wasm test convention (`fixture.rs`
//! in this same directory).

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

const EXPORT_ID: &str = "pg.wasm.compiled-endpoint.reverse-packed-bytes-v1";

const SOURCE: &str = r#"module public_generic_wasm_adapter.compiled_reference_endpoint;

@id("pg.wasm.compiled-endpoint.reverse-packed-bytes-v1")
fn reverse_packed_bytes(input: i64) -> i64
{
    let byte0 = input % 256;
    let byte1 = (input / 256) % 256;
    let byte2 = (input / 65536) % 256;
    let byte3 = (input / 16777216) % 256;
    byte0 * 16777216 + byte1 * 65536 + byte2 * 256 + byte3
}

@id("pg.wasm.compiled-endpoint.main")
fn main() -> i64
{
    0
}
"#;

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Pack four bytes into one `i64`, `a` least significant, matching
/// `reverse_packed_bytes`'s own encoding.
fn pack(a: u8, b: u8, c: u8, d: u8) -> i64 {
    i64::from(a) + i64::from(b) * 256 + i64::from(c) * 65_536 + i64::from(d) * 16_777_216
}

/// Compile `SOURCE` through the real, in-process Public Scalar Export
/// Profile v1 pipeline (`semaprax::wasm::build_web_with_scalar_exports`,
/// the library entry point behind `semaprax build --target wasm --export`)
/// into a fresh directory holding a genuinely compiled `app.wasm` plus its
/// generated JS glue -- exactly what `examples/calculator-web` ships and
/// exactly what this repository already treats as genuinely compiled,
/// non-hand-assembled evidence.
fn build_package(label: &str) -> std::path::PathBuf {
    let program = semaprax::check(SOURCE, Path::new("compiled_reference_endpoint.spx")).unwrap();
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "semaprax-pg-wasm-compiled-endpoint-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    semaprax::wasm::build_web_with_scalar_exports(&program, &root, &[EXPORT_ID.to_owned()])
        .unwrap();
    fs::write(
        root.join("compiled_reference_endpoint.mjs"),
        include_str!("compiled_reference_endpoint.mjs"),
    )
    .unwrap();
    root
}

fn run_probe(root: &Path, inputs: &[i64]) -> Vec<serde_json::Value> {
    let mut command = Command::new("node");
    command
        .arg("compiled_reference_endpoint.mjs")
        .arg(EXPORT_ID);
    for input in inputs {
        command.arg(input.to_string());
    }
    let output = command.current_dir(root).output().unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    parsed["results"].as_array().unwrap().clone()
}

#[test]
fn genuinely_compiled_wasm_export_reverses_four_packed_bytes() {
    if !node_available() {
        return;
    }
    let root = build_package("full");
    let input = pack(1, 2, 3, 4);
    let outputs = run_probe(&root, &[input]);
    assert_eq!(outputs[0]["ok"], serde_json::Value::Bool(true));
    let expected = pack(4, 3, 2, 1);
    assert_eq!(
        outputs[0]["value"],
        serde_json::Value::String(expected.to_string()),
        "compiled Wasm export did not reverse the four packed bytes"
    );
    // Negative control: the untouched input is not its own reversal (rules
    // out an identity/no-op bug masquerading as success).
    assert_ne!(
        outputs[0]["value"],
        serde_json::Value::String(input.to_string())
    );
}

#[test]
fn tampering_one_input_byte_changes_exactly_the_corresponding_output_position() {
    if !node_available() {
        return;
    }
    let root = build_package("tamper");
    let base = pack(1, 2, 3, 4);
    let tampered = pack(1, 99, 3, 4); // only the "b" byte changes.
    let outputs = run_probe(&root, &[base, tampered]);
    let base_value = pack(4, 3, 2, 1);
    let tampered_value = pack(4, 3, 99, 1); // "b" lands at the 65536 place in the output.
    assert_eq!(
        outputs[0]["value"],
        serde_json::Value::String(base_value.to_string())
    );
    assert_eq!(
        outputs[1]["value"],
        serde_json::Value::String(tampered_value.to_string())
    );
    assert_ne!(outputs[0]["value"], outputs[1]["value"]);
}

#[test]
fn zero_and_all_bytes_set_round_trip_through_the_compiled_export() {
    if !node_available() {
        return;
    }
    let root = build_package("edges");
    let zero = 0i64;
    let all_ff = pack(255, 255, 255, 255);
    let outputs = run_probe(&root, &[zero, all_ff]);
    assert_eq!(
        outputs[0]["value"],
        serde_json::Value::String("0".to_owned())
    );
    assert_eq!(
        outputs[1]["value"],
        serde_json::Value::String(all_ff.to_string())
    );
}
