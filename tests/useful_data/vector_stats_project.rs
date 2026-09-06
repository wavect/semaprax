//! The bounded-vector example project carried end to end.
//!
//! `examples/vector-stats-project` is the first example project built on
//! [Owned Bounded Vec v1]. A loop-carried `vec_push<i64>` accumulates a
//! *variable* number of scalar readings into one owned vector and a second
//! bounded `while` filters them back out through `vec_len` and `vec_get`, so
//! this module proves the project checks, formats canonically, and returns `0`
//! from both its entry and its conformance module on the interpreter, on
//! native C11 at `-O0` and `-O2`, and on Core Wasm under Node - and that the
//! accumulated length really does follow a runtime argument rather than a
//! fixed unrolled sequence.
//!
//! [Owned Bounded Vec v1]: ../../docs/OWNED-BOUNDED-VEC-V1.md

use std::path::{Path, PathBuf};
use std::process::Command;

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, format, parse, project};

const SOURCES: [&str; 4] = [
    "src/app.spx",
    "src/collect.spx",
    "src/readings.spx",
    "src/tests.spx",
];

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/vector-stats-project")
}

fn scratch(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-vector-stats-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path.canonicalize().unwrap()
}

fn compile_c(source: &str, output: &Path, optimization: &str) {
    let c_path = output.with_extension("c");
    std::fs::write(&c_path, source).unwrap();
    let result = Command::new("clang")
        .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
        .arg(&c_path)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "clang {optimization} failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn run_returns_zero(path: &Path) {
    let output = Command::new(path).output().unwrap();
    assert!(output.status.success(), "{} failed", path.display());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "0",
        "{} did not report success",
        path.display()
    );
}

/// Every module of the example is canonical, so `semaprax fmt --check` over the
/// project cannot start passing because a source drifted out of the gate.
#[test]
fn every_module_is_canonical() {
    for source in SOURCES {
        let path = fixture().join(source);
        let text = std::fs::read_to_string(&path).unwrap();
        let program = parse(&text, &path).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            format::canonical(&program),
            text,
            "{source} is not canonical"
        );
    }
}

/// The accumulated element count follows a runtime argument.
///
/// The two library modules are joined into one so the interpreter can select
/// `alert_total` directly; only the module header and the cross-module imports
/// are dropped, so every declaration is the committed example's own. With the
/// filter disabled these are the exact running prefix sums of the reading
/// series, which a fixed unrolled sequence of pushes could not produce.
#[test]
fn the_accumulated_length_follows_the_runtime_argument() {
    let mut source = String::from("module vector_stats.probe;\n");
    for module in ["src/readings.spx", "src/collect.spx"] {
        for line in std::fs::read_to_string(fixture().join(module))
            .unwrap()
            .lines()
        {
            if line.starts_with("module ") || line.starts_with("use function ") {
                continue;
            }
            source.push_str(line);
            source.push('\n');
        }
    }
    source.push_str("\n@id(\"vector-stats.probe.main\")\nfn main() -> i64\n{\n    0\n}\n");
    let path = scratch("prefix").join("program.spx");
    std::fs::write(&path, &source).unwrap();

    for (count, expected) in [
        (0, 0),
        (1, 11),
        (2, 59),
        (3, 144),
        (4, 166),
        (9, 431),
        (12, 574),
    ] {
        let outcome = interpreter::interpret(
            &path,
            "vector-stats.alert-total",
            &[count.to_string(), "0".to_owned()],
            &InterpreterOptions::default(),
        )
        .unwrap();
        assert!(
            outcome
                .envelope
                .contains(&format!("\"value\":\"{expected}\"")),
            "count {count} did not accumulate {expected}: {}",
            outcome.envelope
        );
    }
    // The filter is a predicate over the accumulated values, not a constant:
    // the same nine readings sum to 431, 310 and 0 under three thresholds.
    for (threshold, expected) in [(0, 431), (50, 310), (97, 0)] {
        let outcome = interpreter::interpret(
            &path,
            "vector-stats.alert-total",
            &["9".to_owned(), threshold.to_string()],
            &InterpreterOptions::default(),
        )
        .unwrap();
        assert!(
            outcome
                .envelope
                .contains(&format!("\"value\":\"{expected}\"")),
            "threshold {threshold} did not filter to {expected}: {}",
            outcome.envelope
        );
    }
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// Entry and conformance both return `0` on all three execution lanes.
#[test]
fn entry_and_conformance_return_zero_on_interpreter_native_and_wasm() {
    let scratch = scratch("lanes");
    project::with_authenticated_project(&fixture().join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0),
            "the entry failed on the interpreter"
        );
        assert_eq!(
            snapshot.execute_test(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0),
            "conformance failed on the interpreter"
        );
        for (role, program) in [
            ("entry", snapshot.entry_program()),
            ("tests", snapshot.test_program()),
        ] {
            let c = codegen::emit_hir_c(program).map_err(|error| vec![error])?;
            // A vector is one owner regardless of its element count, so no
            // shallow carrier copy may appear in the generated C.
            assert!(!c.contains("memcpy(result, source"));
            for optimization in ["-O0", "-O2"] {
                let binary = scratch.join(format!("{role}{}", optimization.to_lowercase()));
                compile_c(&c, &binary, optimization);
                run_returns_zero(&binary);
            }
        }
        let wasm = snapshot.test_wasm_module()?;
        let wasm_path = scratch.join("tests.wasm");
        std::fs::write(&wasm_path, wasm).unwrap();
        let script = scratch.join("tests.mjs");
        std::fs::write(&script, WASM_HARNESS).unwrap();
        let node = Command::new("node")
            .arg(script.file_name().unwrap())
            .current_dir(&scratch)
            .output()
            .unwrap();
        assert!(
            node.status.success(),
            "Node conformance closure failed: {}",
            String::from_utf8_lossy(&node.stderr)
        );
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

/// One host-side bounded-vector arena behind the frozen `env` import set,
/// asserted empty at the end so a leaked or double-owned carrier fails rather
/// than passing quietly.
const WASM_HARNESS: &str = r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
const bytes = await readFile("./tests.wasm");
const entries = new Map();
let next = 1n;
const key = value => { if (typeof value !== "bigint" || value === 0n) throw new Error("carrier"); return value.toString(); };
const read = (value, tag) => { const entry = entries.get(key(value)); if (!entry || entry.tag !== tag) throw new Error("stale-or-type"); return entry; };
const alloc = (tag, capacity, values = []) => { const token = next++; entries.set(key(token), { tag, capacity, values }); return token; };
const env = {
  spx_add: (a, b) => a + b, spx_sub: (a, b) => a - b, spx_mul: (a, b) => a * b,
  spx_div: (a, b) => a / b, spx_rem: (a, b) => a % b, spx_neg: a => -a,
  spx_contract_fail: code => { throw new Error(`unexpected-status:${code}`); },
  spx_vec_with_capacity: (tag, capacity) => { const n = Number(capacity); return Number.isSafeInteger(n) && n >= 0 && n <= 8192 ? alloc(tag, n) : 0n; },
  spx_vec_push: (source, tag, bits) => { const old = read(source, tag); if (old.values.length >= old.capacity) return 0n; const values = old.values.concat([bits]); entries.delete(key(source)); return alloc(tag, old.capacity, values); },
  spx_vec_len: (source, tag) => BigInt(read(source, tag).values.length),
  spx_vec_capacity: (source, tag) => BigInt(read(source, tag).capacity),
  spx_vec_get: (source, tag, index) => { const entry = read(source, tag), n = Number(index); if (!Number.isSafeInteger(n) || n < 0 || n >= entry.values.length) throw new Error("oob"); return entry.values[n]; },
  spx_vec_drop: source => { if (!entries.delete(key(source))) throw new Error("double-drop"); },
};
const linked = await WebAssembly.instantiate(bytes, { env });
for (let round = 0; round < 4; round += 1) {
  assert.equal(linked.instance.exports.semaprax_main(), 0n);
  assert.equal(entries.size, 0, "the vector arena did not settle");
}
"#;
