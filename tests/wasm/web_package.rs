use std::path::Path;
use std::process::Command;

use semaprax::{graph, parse, verify, wasm};
use wasmparser::Validator;

const PROGRAM: &str = r#"
module test.web;
@id("math.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
    ensures result == left + right
{
    left + right
}
@id("app.main")
fn main() -> i64
    ensures result == 42 && true
{
    add(19, 23)
}
"#;

#[test]
fn emits_a_valid_browser_webassembly_package() {
    let program = parse(PROGRAM, Path::new("web.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let module = wasm::emit_module(&program).unwrap();
    assert_eq!(&module[..8], b"\0asm\x01\0\0\0");

    let output = std::env::temp_dir().join(format!("semaprax-web-{}", std::process::id()));
    wasm::build_web(&program, &output).unwrap();
    assert!(output.join("app.wasm").is_file());
    assert!(output.join("semaprax.js").is_file());
    assert!(output.join("index.html").is_file());
    assert!(output.join("package.json").is_file());
    assert!(output.join("semaprax.manifest.json").is_file());
    let manifest = std::fs::read_to_string(output.join("semaprax.manifest.json")).unwrap();
    assert!(manifest.contains("\"schema\":\"semaprax.web.v3\""));
    assert!(manifest.contains("\"schema\":\"semaprax.wasm-owned.v1\""));
    assert!(manifest.contains(&format!(
        "\"graph_revision\":{}",
        semaprax::diagnostic::quote_json(&graph::revision(&program))
    )));

    if Command::new("node").arg("--version").output().is_ok() {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
        let result = Command::new("node")
            .arg(script)
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "node failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");

        let scalar_stress = output.join("verify-scalar-runtime-tags.mjs");
        std::fs::write(
            &scalar_stress,
            r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { join } from "node:path";
const directory = process.argv[2];
const runtime = await import(pathToFileURL(join(directory, "semaprax.js")));
const bytes = await readFile(join(directory, "app.wasm"));
for (let index = 0; index < 2050; index += 1) {
  const result = await runtime.instantiateBytes(bytes);
  assert.equal("owned" in result, false);
  assert.equal(result.instance.exports.semaprax_main(), 42n);
}

console.log("scalar-runtime-tags-ok");
"#,
        )
        .unwrap();
        let stress_result = Command::new("node")
            .arg(&scalar_stress)
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            stress_result.status.success(),
            "node scalar stress failed: {}",
            String::from_utf8_lossy(&stress_result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&stress_result.stdout).trim(),
            "scalar-runtime-tags-ok"
        );
    }

    let _ = std::fs::remove_dir_all(output);
}

#[test]
fn byte_range_failures_keep_their_semantic_domain_in_web_packages() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    for (ordinal, start, end, code) in [(1, 3, 1, 1), (2, 0, 5, 2)] {
        let source = format!(
            r#"
module test.web_byte_range_{ordinal};
@id("app.main")
fn main() -> i64 {{
    let data = [1u8, 2u8, 3u8, 4u8];
    let view = array_as_slice(data);
    let selected = byte_range(view, {start}usize, {end}usize);
    if byte_len(selected) == 0usize {{ 0 }} else {{ 1 }}
}}
"#
        );
        let program = parse(&source, Path::new("web-byte-range.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let output = std::env::temp_dir().join(format!(
            "semaprax-web-byte-range-{}-{ordinal}",
            std::process::id()
        ));
        wasm::build_web(&program, &output).unwrap();
        Validator::new()
            .validate_all(&std::fs::read(output.join("app.wasm")).unwrap())
            .unwrap();
        let script = output.join("verify-status.mjs");
        std::fs::write(
            &script,
            format!(
                r#"import assert from "node:assert/strict";
import {{ readFile }} from "node:fs/promises";
import {{ pathToFileURL }} from "node:url";
import {{ join }} from "node:path";
const directory = process.argv[2];
const runtime = await import(pathToFileURL(join(directory, "semaprax.js")));
const {{ instance }} = await runtime.instantiateBytes(await readFile(join(directory, "app.wasm")));
let observed = null;
try {{ instance.exports.semaprax_main(); }} catch (error) {{ observed = runtime.semanticStatus(error); }}
assert.deepEqual(observed, Object.freeze({{schema:"semaprax.status.v1",domain_id:"semaprax.byte-range.v1",code:{code}}}));
"#
            ),
        )
        .unwrap();
        let result = Command::new("node")
            .arg(&script)
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let _ = std::fs::remove_dir_all(output);
    }
}

#[test]
fn inline_narrow_arithmetic_failures_keep_their_semantic_status() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    for (ordinal, body) in [
        (
            1,
            "let value = 2147483647i32; let ignored = value + 1i32; 0",
        ),
        (2, "let value = 255u8; let ignored = value + 1u8; 0"),
        (
            3,
            "let value = 18446744073709551615usize; let ignored = value + 1usize; 0",
        ),
    ] {
        let source = format!(
            "module test.web_narrow_{ordinal};\n@id(\"app.main\")\nfn main() -> i64 {{ {body} }}\n"
        );
        let program = parse(&source, Path::new("web-narrow-status.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let output = std::env::temp_dir().join(format!(
            "semaprax-web-narrow-status-{}-{ordinal}",
            std::process::id()
        ));
        wasm::build_web(&program, &output).unwrap();
        Validator::new()
            .validate_all(&std::fs::read(output.join("app.wasm")).unwrap())
            .unwrap();
        let script = output.join("verify-status.mjs");
        std::fs::write(
            &script,
            r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { join } from "node:path";
const directory = process.argv[2];
const runtime = await import(pathToFileURL(join(directory, "semaprax.js")));
const { instance } = await runtime.instantiateBytes(await readFile(join(directory, "app.wasm")));
let observed = null;
try {
  instance.exports.semaprax_main();
} catch (error) {
  assert(error instanceof RangeError);
  assert.match(error.message, /SEMAPRAX checked arithmetic failure: addition overflow/);
  observed = runtime.semanticStatus(error);
}
assert.deepEqual(observed, Object.freeze({schema:"semaprax.status.v1",domain_id:"semaprax.arithmetic.v1",code:1}));
assert.throws(
  () => runtime.imports.env.spx_add((1n << 63n) - 1n, 1n),
  error => error instanceof RangeError && error.message.includes("SEMAPRAX checked arithmetic failure: addition overflow")
);
"#,
        )
        .unwrap();
        let result = Command::new("node")
            .arg(&script)
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "ordinal={ordinal} stdout={} stderr={}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let _ = std::fs::remove_dir_all(output);
    }
}
