//! Functions whose independent decisions multiply far past the 65,536-path
//! enumeration ceiling, yet change no cleanup state between decisions. The
//! factored typed-control skeleton comparison
//! (`src/cleanup_plan/replay/factored.rs`) verifies them, and every program
//! here returns the same observation from the reference interpreter, native
//! C, and the String-settling Core Wasm profile. Each one failed with
//! `SPX-H006` before that comparison existed.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::wasm::internal_strings::{emit_module, InternalStringOptions};
use semaprax::{codegen, format, interpreter, parse, verify, wasm};
use serde_json::Value;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn module(name: &str, declarations: &str, body: &str) -> String {
    format!(
        "module test.independent_branches.{name};\n\n{declarations}@id(\"app.main\")\nfn main() -> i64\n{{\n{body}}}\n"
    )
}

/// Thirty independent statement-level `if`s, each a checked addition.
fn statement_ifs() -> String {
    let mut body = String::from("    let n = 17;\n    let mut total = 0;\n");
    for index in 0..30 {
        writeln!(
            body,
            "    total = total + if n > {index} {{ {} }} else {{ 0 }};",
            index + 1
        )
        .unwrap();
    }
    body.push_str("    total\n");
    module("statement_ifs", "", &body)
}

/// Twenty lazy `&&` conditions in sequence.
fn lazy_chains() -> String {
    let mut body = String::from("    let a = 9;\n    let b = 30;\n    let mut count = 0;\n");
    for index in 0..20 {
        writeln!(
            body,
            "    count = count + if a > {index} && b < {} && a != b {{ 1 }} else {{ 0 }};",
            40 - index
        )
        .unwrap();
    }
    body.push_str("    count\n");
    module("lazy_chains", "", &body)
}

/// One expression summing twenty-four independent conditional terms: the
/// `kernel_boundary` shape, where products form inside a single statement.
fn expression_sum() -> String {
    let terms = (0..24)
        .map(|index| format!("(if value < {index} {{ 1 }} else {{ 0 }})"))
        .collect::<Vec<_>>()
        .join(" + ");
    module(
        "expression_sum",
        "",
        &format!("    let value = 5;\n    {terms}\n"),
    )
}

const FIELD_CODES: [u8; 12] = [73, 78, 70, 79, 32, 50, 48, 50, 54, 58, 48, 55];

/// A log-line field parser: twelve scalar `match`es on `string_byte_at`
/// over one owned line that stays live across every decision. Text Toolkit
/// v1 does not lower `string_byte_at` to Core Wasm (`SPX-W116`), so this one
/// runs on the interpreter and native C; `parse_bytes` is its Wasm twin.
fn parse_text() -> String {
    let mut body = String::from("    let line = \"INFO 2026:07 rest\";\n");
    for (index, code) in FIELD_CODES.iter().enumerate() {
        writeln!(
            body,
            "    let f{index} = match string_byte_at(line, {index}) {{ {code} => 1, 45 => 0, _ => 0, }};"
        )
        .unwrap();
    }
    let sum = (0..FIELD_CODES.len())
        .map(|index| format!("f{index}"))
        .collect::<Vec<_>>()
        .join(" + ");
    writeln!(body, "    ({sum}) * 100 + string_len(line)").unwrap();
    module("parse_text", "", &body)
}

/// The same twelve fields read with `byte_get` from a borrowed byte view.
fn parse_bytes() -> String {
    let literal = FIELD_CODES
        .iter()
        .map(|code| format!("{code}u8"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut body =
        format!("    let sample = [{literal}];\n    let view = array_as_slice(sample);\n");
    for (index, code) in FIELD_CODES.iter().enumerate() {
        writeln!(
            body,
            "    let g{index} = match byte_get(view, {index}usize) {{ Option::Some {{ value: byte }} => if byte == {code}u8 {{ 1 }} else {{ 0 }}, Option::None {{}} => 0, }};"
        )
        .unwrap();
    }
    let sum = (0..FIELD_CODES.len())
        .map(|index| format!("g{index}"))
        .collect::<Vec<_>>()
        .join(" + ");
    writeln!(body, "    {sum}").unwrap();
    module("parse_bytes", "", &body)
}

/// Payload-free variant equality, two comparisons per statement.
fn variant_equality() -> String {
    let declarations = "@id(\"levels.level\")\nvariant Level {\n    @id(\"levels.debug\")\n    Debug,\n    @id(\"levels.info\")\n    Info,\n    @id(\"levels.warn\")\n    Warn,\n    @id(\"levels.error\")\n    Error,\n}\n\n@id(\"levels.pick\")\nfn pick(n: i64) -> Level\n{\n    if n % 4 == 0 { Level::Debug {} } else { if n % 4 == 1 { Level::Info {} } else { if n % 4 == 2 { Level::Warn {} } else { Level::Error {} } } }\n}\n\n";
    let mut body = String::from("    let mut total = 0;\n");
    for index in 0..16 {
        writeln!(body, "    let l{index} = pick({index});").unwrap();
        writeln!(
            body,
            "    total = total + if l{index} == Level::Warn {{}} || l{index} == Level::Error {{}} {{ {index} }} else {{ 0 }};"
        )
        .unwrap();
    }
    body.push_str("    total\n");
    module("variant_equality", declarations, &body)
}

/// Sixteen decisions in a `while` body and sixteen in a block nested in an
/// `if` arm.
fn nested_blocks() -> String {
    let mut body = String::from(
        "    let n = 7;\n    let mut total = 0;\n    let mut i = 0;\n    while i < 3 {\n",
    );
    for index in 0..16 {
        writeln!(
            body,
            "        total = total + if n > {index} {{ 1 }} else {{ 0 }};"
        )
        .unwrap();
    }
    body.push_str("        i = i + 1;\n        i < 3\n    }\n    let extra = if n > 2 {\n");
    for index in 0..16 {
        writeln!(
            body,
            "        let e{index} = if n > {index} {{ {index} }} else {{ 0 }};"
        )
        .unwrap();
    }
    let sum = (0..16)
        .map(|index| format!("e{index}"))
        .collect::<Vec<_>>()
        .join(" + ");
    writeln!(body, "        {sum}\n    }} else {{\n        0\n    }};").unwrap();
    body.push_str("    total + extra\n");
    module("nested_blocks", "", &body)
}

/// The same thirty decisions, then a checked overflow: every lane must
/// report the selected failure, not a value.
fn selected_failure() -> String {
    let mut body = String::from("    let n = 17;\n    let mut total = 0;\n");
    for index in 0..30 {
        writeln!(
            body,
            "    total = total + if n > {index} {{ {} }} else {{ 0 }};",
            index + 1
        )
        .unwrap();
    }
    body.push_str("    total * 9223372036854775807\n");
    module("selected_failure", "", &body)
}

/// An owned string stays live across twenty decisions and is read after
/// them, so its single release follows every path.
fn text_live() -> String {
    let mut body =
        String::from("    let text = \"hello\";\n    let n = 7;\n    let mut total = 0;\n");
    for index in 0..20 {
        writeln!(
            body,
            "    total = total + if n > {index} {{ 1 }} else {{ 0 }};"
        )
        .unwrap();
    }
    body.push_str("    total + string_len(text)\n");
    module("text_live", "", &body)
}

/// The Core Wasm lowering that admits a case.
#[derive(Clone, Copy, PartialEq)]
enum WasmLane {
    /// `build --target web`: the scalar Web package.
    Scalar,
    /// `build --target web --profile internal-strings-v1`.
    Strings,
    /// Text Toolkit v1 `string_byte_at` is not lowered (`SPX-W116`).
    None,
}

/// Name, source, expected observation, and the Core Wasm lane.
fn cases() -> [(&'static str, String, &'static str, WasmLane); 9] {
    [
        ("statement_ifs", statement_ifs(), "ok|153", WasmLane::Scalar),
        ("lazy_chains", lazy_chains(), "ok|9", WasmLane::Scalar),
        (
            "expression_sum",
            expression_sum(),
            "ok|18",
            WasmLane::Scalar,
        ),
        ("parse_text", parse_text(), "ok|1217", WasmLane::None),
        ("parse_bytes", parse_bytes(), "ok|12", WasmLane::Scalar),
        (
            "variant_equality",
            variant_equality(),
            "ok|68",
            WasmLane::Scalar,
        ),
        ("nested_blocks", nested_blocks(), "ok|42", WasmLane::Scalar),
        ("text_live", text_live(), "ok|12", WasmLane::Strings),
        (
            "selected_failure",
            selected_failure(),
            "failed",
            WasmLane::Strings,
        ),
    ]
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn scratch(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-independent-branches-{label}-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}

fn checked(name: &str, source: &str) -> semaprax::ast::Program {
    let program = parse(source, Path::new(&format!("{name}.spx"))).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}");
    program
}

/// `ok|value`, or `failed` plus the interpreter's selected status.
fn interpreted(name: &str, source: &str) -> (String, Option<String>) {
    let root = scratch("interpreter");
    let path = root.join(format!("{name}.spx"));
    fs::write(&path, source).unwrap();
    let result = interpreter::interpret(
        &path,
        "app.main",
        &[],
        &interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    fs::remove_dir_all(root).unwrap();
    let envelope: Value = serde_json::from_str(&result.envelope).unwrap();
    let outcome = &envelope["payload"]["outcome"];
    if outcome["kind"] == "returned" {
        (format!("ok|{}", outcome["value"].as_str().unwrap()), None)
    } else {
        assert_eq!(outcome["kind"], "failed", "{name}: {}", result.envelope);
        let status = format!(
            "{}|{}",
            outcome["status"]["domain_id"].as_str().unwrap(),
            outcome["status"]["code"].as_u64().unwrap()
        );
        ("failed".to_owned(), Some(status))
    }
}

fn native(name: &str, program: &semaprax::ast::Program) -> String {
    let root = scratch("native");
    let executable = root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    codegen::build(program, &executable).unwrap();
    let output = Command::new(&executable).output().unwrap();
    fs::remove_dir_all(root).unwrap();
    if output.status.success() {
        format!("ok|{}", String::from_utf8_lossy(&output.stdout).trim())
    } else {
        assert!(
            !output.stderr.is_empty(),
            "{name}: failure without a status"
        );
        "failed".to_owned()
    }
}

fn node_probe(name: &str, root: &Path, script: &str) -> String {
    fs::write(root.join("probe.mjs"), script).unwrap();
    let output = Command::new("node")
        .current_dir(root)
        .arg("probe.mjs")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// `ok|value`, or `failed` plus, on the String profile, the selected status
/// domain and code.
fn wasm(name: &str, program: &semaprax::ast::Program, lane: WasmLane) -> (String, Option<String>) {
    let root = scratch("wasm");
    let observed = if lane == WasmLane::Strings {
        let artifact = emit_module(
            program,
            &["app.main".to_owned()],
            InternalStringOptions::default(),
        )
        .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        fs::write(root.join("program.wasm"), artifact.wasm_bytes()).unwrap();
        fs::write(root.join("program.mjs"), artifact.runtime_source()).unwrap();
        node_probe(
            name,
            &root,
            "import {readFileSync} from 'node:fs';\nimport {instantiate} from './program.mjs';\nconst api=await instantiate(Uint8Array.from(readFileSync('program.wasm')));\nconst outcome=api.call('app.main');\nprocess.stdout.write(outcome.kind==='success'?`ok|${outcome.value}`:`failed|${outcome.domain}|${outcome.code}`);\n",
        )
    } else {
        let package = root.join("web");
        wasm::build_web(program, &package).unwrap_or_else(|error| panic!("{name}: {error:?}"));
        node_probe(
            name,
            &root,
            "import {readFileSync} from 'node:fs';\nconst {instantiateBytes}=await import('./web/semaprax.js');\nconst {instance}=await instantiateBytes(readFileSync('web/app.wasm'));\nlet observed;\ntry { observed=`ok|${instance.exports.semaprax_main()}`; } catch (error) { observed='failed'; }\nprocess.stdout.write(observed);\n",
        )
    };
    fs::remove_dir_all(root).unwrap();
    match observed.strip_prefix("failed|") {
        Some(status) => ("failed".to_owned(), Some(status.to_owned())),
        None => (observed, None),
    }
}

#[test]
fn independent_decisions_verify_and_agree_on_every_backend() {
    let clang = command_available("clang");
    let node = command_available("node");
    for (name, source, expected, lane) in cases() {
        let program = checked(name, &source);
        let canonical = format::canonical(&program);
        assert_eq!(
            format::canonical(&parse(&canonical, Path::new("canonical.spx")).unwrap()),
            canonical,
            "{name}"
        );
        let (interpreted, interpreted_status) = interpreted(name, &canonical);
        assert_eq!(interpreted, expected, "{name}: interpreter");
        if clang {
            assert_eq!(native(name, &program), expected, "{name}: native");
        }
        if node && lane != WasmLane::None {
            let (observed, status) = wasm(name, &program, lane);
            assert_eq!(observed, expected, "{name}: wasm");
            if lane == WasmLane::Strings {
                assert_eq!(status, interpreted_status, "{name}: wasm status");
            }
        }
    }
}
