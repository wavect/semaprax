//! Helpers shared by `std.text`'s native and Wasm conformance checks, plus
//! the generic C-compile/run helpers `run_examples_and_conformance` and
//! sibling modules (e.g. `testing`) use for every package's native backend.

use std::path::Path;
use std::process::Command;

use semaprax::{codegen, project};

pub(super) fn compile_c(source: &str, output: &Path, optimization: &str) {
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

pub(super) fn run_returns_zero(path: &Path) {
    let output = Command::new(path).output().unwrap();
    assert!(output.status.success(), "{} failed", path.display());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "0",
        "{} did not report success",
        path.display()
    );
}

pub(super) fn run_text_package_wasm_conformance(
    snapshot: &mut project::ProjectSnapshot,
    scratch: &Path,
) -> Result<(), Vec<semaprax::diagnostic::Diagnostic>> {
    let output = scratch.join("text-npm");
    snapshot.build_npm(&output)?;
    let script = scratch.join("text-conformance.mjs");
    std::fs::write(
        &script,
        r#"import assert from "node:assert/strict";
import fs from "node:fs";
import { instantiate } from "./text-npm/semaprax.bindings.js";
const runtime = instantiate(fs.readFileSync("./text-npm/app.wasm"));
assert.equal(runtime.functions["std.text.byte_len"]("a\0é世界"), 10n);
assert.equal(runtime.functions["std.text.byte_len"](""), 0n);
assert.equal(runtime.functions["std.text.is_empty"](""), true);
assert.equal(runtime.functions["std.text.is_empty"]("é"), false);
assert.equal(runtime.functions["std.text.starts_with"]("a\0é世界", "a\0"), true);
assert.equal(runtime.functions["std.text.starts_with"]("a\0", "a\0é世界"), false);
assert.equal(runtime.functions["std.text.contains"]("a\0é世界", "é世"), true);
assert.equal(runtime.functions["std.text.contains"]("a\0é世界", "界a"), false);
assert.equal(runtime.functions["std.text.equals"]("a\0é世界", "a\0é世界"), true);
assert.equal(runtime.functions["std.text.equals"]("a\0é世界", "a\0é世"), false);
assert.equal(runtime.functions["std.text.equals"]("", ""), true);
"#,
    )
    .unwrap();
    let node = Command::new("node")
        .arg(script.file_name().unwrap())
        .current_dir(scratch)
        .output()
        .unwrap();
    assert!(
        node.status.success(),
        "std.text: Node conformance failed: {}",
        String::from_utf8_lossy(&node.stderr)
    );
    Ok(())
}

pub(super) fn assert_text_interpreter_conformance(snapshot: &project::ProjectSnapshot) {
    use project::{
        PublicApiArgument as Arg, PublicApiEvaluationOutcome as Outcome, PublicApiValue,
    };

    let evaluate = |id: &str, arguments: &[Arg<'_>]| {
        snapshot
            .evaluate_text_api_v1(id, arguments, 1_000)
            .unwrap()
            .outcome
    };
    assert_eq!(
        evaluate("std.text.byte_len", &[Arg::BorrowStr("a\0é世界")]),
        Outcome::Returned(PublicApiValue::I64(10))
    );
    assert_eq!(
        evaluate("std.text.is_empty", &[Arg::BorrowStr("")]),
        Outcome::Returned(PublicApiValue::Bool(true))
    );
    assert_eq!(
        evaluate(
            "std.text.starts_with",
            &[Arg::BorrowStr("a\0é世界"), Arg::BorrowStr("a\0")],
        ),
        Outcome::Returned(PublicApiValue::Bool(true))
    );
    assert_eq!(
        evaluate(
            "std.text.contains",
            &[Arg::BorrowStr("a\0é世界"), Arg::BorrowStr("é世")],
        ),
        Outcome::Returned(PublicApiValue::Bool(true))
    );
    assert_eq!(
        evaluate(
            "std.text.equals",
            &[Arg::BorrowStr("a\0é世界"), Arg::BorrowStr("a\0é世界"),],
        ),
        Outcome::Returned(PublicApiValue::Bool(true))
    );
    assert_eq!(
        evaluate(
            "std.text.equals",
            &[Arg::BorrowStr("a\0é世界"), Arg::BorrowStr("a\0é世")],
        ),
        Outcome::Returned(PublicApiValue::Bool(false))
    );
}

fn native_symbol(id: &str) -> String {
    let mut symbol = String::from("spx_decl_");
    for byte in id.bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

pub(super) fn run_text_package_native_conformance(
    snapshot: &project::ProjectSnapshot,
    scratch: &Path,
) -> Result<(), Vec<semaprax::diagnostic::Diagnostic>> {
    let generated = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
    let source = format!(
        "#define SPX_NO_ENTRY_WRAPPER 1\n{generated}\nint main(void) {{\n\
         const uint8_t value_bytes[] = {{0x61,0x00,0xc3,0xa9,0xe4,0xb8,0x96,0xe7,0x95,0x8c}};\n\
         const uint8_t prefix_bytes[] = {{0x61,0x00}};\n\
         const uint8_t middle_bytes[] = {{0xc3,0xa9,0xe4,0xb8,0x96}};\n\
         const uint8_t absent_bytes[] = {{0xe7,0x95,0x8c,0x61}};\n\
         spx_str_v1 value = {{value_bytes, UINT64_C(10)}};\n\
         spx_str_v1 prefix = {{prefix_bytes, UINT64_C(2)}};\n\
         spx_str_v1 middle = {{middle_bytes, UINT64_C(5)}};\n\
         spx_str_v1 absent = {{absent_bytes, UINT64_C(4)}};\n\
         spx_str_v1 empty = {{NULL, UINT64_C(0)}};\n\
         struct spx_status_entry entries[UINT32_C(1)];\n\
         struct spx_context context = {{0}};\n\
         if (!spx_context_init(&context, UINT64_C(1), entries, UINT32_C(1), NULL, NULL, NULL)) return 10;\n\
         int64_t length = -1; bool result = false;\n\
         if ({byte_len}(&context, value, &length) != SPX_STATUS_SUCCESS || length != INT64_C(10)) return 11;\n\
         if ({is_empty}(&context, empty, &result) != SPX_STATUS_SUCCESS || !result) return 12;\n\
         if ({starts_with}(&context, value, prefix, &result) != SPX_STATUS_SUCCESS || !result) return 13;\n\
         if ({contains}(&context, value, middle, &result) != SPX_STATUS_SUCCESS || !result) return 14;\n\
         if ({contains}(&context, value, absent, &result) != SPX_STATUS_SUCCESS || result) return 15;\n\
         if ({equals}(&context, value, value, &result) != SPX_STATUS_SUCCESS || !result) return 16;\n\
         if ({equals}(&context, value, absent, &result) != SPX_STATUS_SUCCESS || result) return 17;\n\
         if ({equals}(&context, empty, empty, &result) != SPX_STATUS_SUCCESS || !result) return 18;\n\
         return 0;\n\
         }}\n",
        byte_len = native_symbol("std.text.byte_len"),
        is_empty = native_symbol("std.text.is_empty"),
        starts_with = native_symbol("std.text.starts_with"),
        contains = native_symbol("std.text.contains"),
        equals = native_symbol("std.text.equals"),
    );
    for optimization in ["-O0", "-O2"] {
        let binary = scratch.join(format!("text-native-{}", optimization.to_lowercase()));
        compile_c(&source, &binary, optimization);
        let executed = Command::new(&binary).output().unwrap();
        assert!(
            executed.status.success(),
            "std.text native conformance {optimization} failed with {:?}",
            executed.status.code()
        );
    }
    Ok(())
}
