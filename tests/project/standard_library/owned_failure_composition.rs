//! Issue #103: an owned standard-library cursor chain whose final checked
//! contract failure must preserve the selected failure while releasing every
//! staged byte owner exactly once.
//!
//! This is intentionally a project-level consumer of the real `std.io`,
//! `std.data.json.dec`, and `std.data.json.write` packages.  Decode and quote
//! each borrow their Reader while consuming their Writer, so the input Reader,
//! decoded Reader, and final Bytes owner are simultaneously live at the
//! failing call.  The Project interpreter and C11 O0/O2 lanes agree on the contract
//! status.  The Core-Wasm host additionally records the private byte arena's
//! allocation/drop sequence, so a backend cannot report the sticky failure
//! while leaving the consumed chain live or dropping a token twice.

use std::path::Path;
use std::process::Command;

use semaprax::{codegen, conformance, format, parse, project, wasm};

use super::temporary::temporary;

const MANIFEST: &str = r#"schema = "semaprax.manifest.v1"

[package]
name = "owned-failure-composition"
version = "0.1.0"
profile = "owned-data-api.v1"

[modules]
entry = "consumer.app"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = []

[dependencies]
std.data.json.dec = "=0.1.0"
std.data.json.write = "=0.1.0"
std.io = "=0.1.0"
"#;

const APP: &str = r#"module consumer.app;
use type @id("std.io.reader") from std.io as Reader;
use type @id("std.io.writer") from std.io as Writer;
use function @id("std.data.json.dec.decode-into") from std.data.json.dec as decode_into;
use function @id("std.data.json.write.quoted-into") from std.data.json.write as quoted_into;
use function @id("std.io.reader.from-bytes") from std.io as reader_from_bytes;
use function @id("std.io.writer.finish") from std.io as writer_finish;
use function @id("std.io.writer.from-bytes") from std.io as writer_from_bytes;

@id("consumer.must-fail")
fn must_fail(ready: bool) -> i64
    requires ready
{
    0
}

@id("consumer.pipeline")
fn run_pipeline() -> i64
{
    let encoded = [34u8, 92u8, 117u8, 48u8, 48u8, 54u8, 49u8, 34u8];
    let input = reader_from_bytes(bytes_copy(array_as_slice(encoded)));
    let decoded = decode_into(input, writer_from_bytes(bytes_zeroed(1usize)));
    let plain = reader_from_bytes(writer_finish(decoded));
    let rendered = quoted_into(plain, writer_from_bytes(bytes_zeroed(3usize)));
    let written = writer_finish(rendered);
    let view = bytes_as_slice(written);
    let exact = byte_len(view) == 3usize && match byte_get(view, 1usize) { Option::Some { value } => value == 97u8, Option::None {} => false, };
    if exact { must_fail(false) } else { 1 }
}

@id("consumer.main")
fn main() -> i64
{
    run_pipeline()
}
"#;

const TESTS: &str = r#"module consumer.tests;
use function @id("consumer.pipeline") from consumer.app as run_pipeline;

@id("consumer.tests.main")
fn main() -> i64
{
    run_pipeline()
}
"#;

const RENEWAL_MANIFEST: &str = r#"schema = "semaprax.manifest.v1"

[package]
name = "owned-renewal-failure"
version = "0.1.0"
profile = "owned-data-api.v1"

[modules]
entry = "consumer.app"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = []

[dependencies]
std.io.lines = "=0.1.0"
"#;

const RENEWAL_APP: &str = r#"module consumer.app;
use type @id("std.io.reader") from std.io as Reader;
use type @id("std.io.writer") from std.io as Writer;
use function @id("std.io.reader.finish") from std.io as reader_finish;
use function @id("std.io.reader.from-bytes") from std.io as reader_from_bytes;
use function @id("std.io.reader.remaining") from std.io as reader_remaining;
use function @id("std.io.writer.finish") from std.io as writer_finish;
use function @id("std.io.writer.from-bytes") from std.io as writer_from_bytes;
use function @id("std.io.lines.reader.line-into") from std.io.lines as reader_line_into;
use function @id("std.io.lines.reader.next-line") from std.io.lines as reader_next_line;

@id("consumer.pipeline")
fn run_pipeline() -> i64
{
    let input = [97u8, 10u8, 98u8, 10u8, 99u8, 10u8];
    let mut reader = reader_from_bytes(bytes_copy(array_as_slice(input)));
    let mut writer = writer_from_bytes(bytes_zeroed(2usize));
    while reader_remaining(reader) > 0usize {
        writer = reader_line_into(reader, writer);
        reader = reader_next_line(reader);
        reader_remaining(reader) > 0usize
    }
    let retained = reader_finish(reader);
    let output = writer_finish(writer);
    if byte_len(bytes_as_slice(retained)) == 6usize && byte_len(bytes_as_slice(output)) == 2usize { 0 } else { 1 }
}

@id("consumer.main")
fn main() -> i64
{
    run_pipeline()
}
"#;

fn canonical_source(source: &str, path: &str) -> String {
    let program = parse(source, path).unwrap_or_else(|error| panic!("{path}: {error}"));
    format::canonical(&program)
}

fn assert_contract(outcome: &project::ProjectExecutionOutcome) {
    let project::ProjectExecutionOutcome::LanguageFailure(status) = outcome else {
        panic!("expected a source contract failure, got {outcome:?}");
    };
    assert_eq!(status.class(), conformance::StatusClass::Contract);
    assert_eq!(status.domain_id(), conformance::CONTRACT_STATUS_DOMAIN_V1);
    assert_eq!(status.code(), conformance::CONTRACT_REQUIRES_FALSE_CODE);
}

fn compile_contract_failure(c: &str, root: &Path, optimization: &str) {
    let binary = root.join(format!("owned-failure-{optimization}"));
    let source = binary.with_extension("c");
    std::fs::write(&source, c).unwrap();
    let compiled = Command::new("clang")
        .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "clang {optimization} failed: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = Command::new(&binary).output().unwrap();
    assert!(!output.status.success(), "{optimization} returned success");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("SEMAPRAX contract failure"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[cfg_attr(windows, ignore = "native C11 command-line fixture is Unix-only")]
fn owned_cursor_chain_settles_before_sticky_contract_failure_on_project_backends() {
    let root = temporary("owned-failure-composition");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
    std::fs::write(
        root.join("src/app.spx"),
        canonical_source(APP, "owned-failure-consumer-app.spx"),
    )
    .unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        canonical_source(TESTS, "owned-failure-consumer-tests.spx"),
    )
    .unwrap();

    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        for _ in 0..2 {
            assert_contract(snapshot.execute_entry(&options)?.outcome());
            assert_contract(snapshot.execute_test(&options)?.outcome());
        }
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            compile_contract_failure(&c, &root, optimization);
        }
        let wasm =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        let wasm_path = root.join("owned-failure.wasm");
        std::fs::write(&wasm_path, wasm).unwrap();
        let output = run_wasm_settlement_oracle(&root, &wasm_path, "[3, 2, 1, 6, 5, 4]");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let perturbed = run_wasm_settlement_oracle(&root, &wasm_path, "[3, 1, 2, 6, 5, 4]");
        assert!(
            !perturbed.status.success(),
            "perturbed settlement oracle passed"
        );
        assert!(
            String::from_utf8_lossy(&perturbed.stderr).contains("settlement order"),
            "{}",
            String::from_utf8_lossy(&perturbed.stderr)
        );
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg_attr(windows, ignore = "native C11 command-line fixture is Unix-only")]
fn repeated_reader_writer_renewal_settles_before_sticky_contract_failure() {
    let root = temporary("owned-renewal-failure");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("semaprax.toml"), RENEWAL_MANIFEST).unwrap();
    std::fs::write(
        root.join("src/app.spx"),
        canonical_source(RENEWAL_APP, "owned-renewal-failure-app.spx"),
    )
    .unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        canonical_source(TESTS, "owned-renewal-failure-tests.spx"),
    )
    .unwrap();

    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        for _ in 0..2 {
            assert_contract(snapshot.execute_entry(&options)?.outcome());
            assert_contract(snapshot.execute_test(&options)?.outcome());
        }
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            compile_contract_failure(&c, &root, optimization);
        }
        let wasm =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        let wasm_path = root.join("owned-renewal-failure.wasm");
        std::fs::write(&wasm_path, wasm).unwrap();
        let output = run_wasm_settlement_oracle(&root, &wasm_path, "[2, 1, 4, 3]");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(root);
}

fn run_wasm_settlement_oracle(
    root: &Path,
    wasm: &Path,
    expected_drops: &str,
) -> std::process::Output {
    let script = root.join("owned-failure.mjs");
    std::fs::write(
        &script,
        WASM_RUNNER.replace("EXPECTED_DROPS", expected_drops),
    )
    .unwrap();
    Command::new("node")
        .arg(script.file_name().unwrap())
        .arg(wasm.file_name().unwrap())
        .current_dir(root)
        .output()
        .unwrap()
}

const WASM_RUNNER: &str = r#"import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const entries = new Map(); const drops = []; let next = 1; let linked;
const decode = carrier => {
  const word = BigInt.asUintN(64, carrier);
  return {word, length:Number(word & 0xffffffffn), root:Number(word >> 32n), token:Number(word >> 32n) & 0x7fffffff};
};
const memory = () => (linked.instance.exports.__spx_byte_memory ?? linked.instance.exports.memory).buffer;
const read = value => {
  if ((value.root & 0xc0000000) === 0x40000000) throw Error("unexpected range owner");
  if ((value.root & 0x80000000) !== 0) { const bytes = entries.get(value.token); if (!(bytes instanceof Uint8Array) || bytes.length !== value.length) throw Error("stale owned carrier"); return bytes; }
  if (value.root > memory().byteLength || value.length > memory().byteLength - value.root) throw Error("memory range");
  return new Uint8Array(memory(), value.root, value.length);
};
const allocate = value => {
  if (entries.size >= 3) throw Error("unexpected live-owner peak");
  const token = next++; entries.set(token, new Uint8Array(value));
  return BigInt.asIntN(64, ((0x80000000n | BigInt(token)) << 32n) | BigInt(value.length));
};
const checked = operation => (...values) => { const result = operation(...values); if (result < -(1n << 63n) || result > (1n << 63n) - 1n) throw Error("i64 overflow"); return result; };
const failure = new Error("requires false");
const env = {
  spx_add: checked((a,b) => a+b), spx_sub: checked((a,b) => a-b), spx_mul: checked((a,b) => a*b),
  spx_div: (a,b) => a/b, spx_rem: (a,b) => a%b, spx_neg: checked(a => -a),
  spx_contract_fail: selector => { assert.equal(selector, 9); throw failure; },
  spx_bytes_copy: carrier => allocate(read(decode(carrier))),
  spx_bytes_zeroed: count => { if (count < 0n || count > 131072n) throw Error("byte capacity"); return allocate(new Uint8Array(Number(count))); },
  spx_bytes_get: (carrier,index) => { const value = read(decode(carrier)); return index < 0n || index >= BigInt(value.length) ? -1 : value[Number(index)]; },
  spx_bytes_set: (carrier,index,byte) => { const value = read(decode(carrier)); if ((decode(carrier).root & 0x80000000) === 0 || index < 0n || index >= BigInt(value.length) || byte < 0 || byte > 255) throw Error("byte write"); value[Number(index)] = byte; return carrier; },
  spx_bytes_set5: () => { throw Error("unexpected five-byte write"); },
  spx_bytes_set1_or5: () => { throw Error("unexpected one-or-five write"); },
  spx_bytes_set1_or6_or48: () => { throw Error("unexpected one-or-six-or-forty-eight write"); },
  spx_bytes_as_slice: carrier => { read(decode(carrier)); return carrier; },
  spx_bytes_drop: carrier => { const value = decode(carrier); read(value); if ((value.root & 0x80000000) === 0 || !entries.delete(value.token)) throw Error("duplicate drop"); drops.push(value.token); },
};
linked = await WebAssembly.instantiate(bytes, {env});
for (let run = 0; run < 2; ++run) {
  assert.throws(() => linked.instance.exports.semaprax_main(), error => error === failure);
  assert.equal(entries.size, 0, "all owners settle after failure");
}
assert.deepEqual(drops, EXPECTED_DROPS, "settlement order");
"#;
