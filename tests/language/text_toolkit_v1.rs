//! Text Toolkit v1 (`docs/TEXT-TOOLKIT-V1.md`): `string_slice`,
//! `string_find`, `string_to_i64`, `string_trim`, `string_byte_at`, and
//! `file_read_text`, plus single-file command-line programs. The same corpus
//! runs on the reference interpreter and on generated C11 under an
//! allocation-counting allocator that rejects duplicate and foreign frees and
//! requires zero live allocations after every case, including checked text
//! failures inside loops. Core Wasm refuses the family with one stable
//! diagnostic. Command-line programs run through `semaprax run` on both the
//! interpreter and `--native` routes.

use std::path::Path;
use std::process::Command;

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::wasm::internal_strings::{emit_module, InternalStringOptions};
use semaprax::{format, graph, hir, parse, verify};
use serde_json::Value;

// Loaded once by the sibling module; a second `mod` would duplicate it.
use super::owned_string_loops_v1::support;
use support::Fixture;

const SOURCE: &str = r#"
module test.text_toolkit;

@id("text.slice")
fn slice() -> i64
{
    let text = "key=value";
    let eq = string_find(text, "=", 0);
    let key = string_slice(text, 0, eq);
    let value = string_slice(text, eq + 1, string_len(text));
    let empty = string_slice(text, 9, 9);
    string_len(key) * 100 + string_len(value) * 10 + string_len(empty)
}

@id("text.slice_past_end")
fn slice_past_end() -> i64
{
    string_len(string_slice("abc", 2, 4))
}

@id("text.slice_inverted")
fn slice_inverted() -> i64
{
    string_len(string_slice("abc", 2, 1))
}

@id("text.slice_negative")
fn slice_negative() -> i64
{
    string_len(string_slice("abc", -1, 1))
}

@id("text.slice_boundary")
fn slice_boundary() -> i64
{
    string_len(string_slice("é!", 0, 1))
}

@id("text.find")
fn find() -> i64
{
    let text = "a,b,,c";
    let first = string_find(text, ",", 0);
    let second = string_find(text, ",", first + 1);
    let missing = string_find(text, ";", 0);
    let empty = string_find(text, "", 6);
    first * 1000 + second * 100 + (missing + 1) * 10 + empty
}

@id("text.find_past_end")
fn find_past_end() -> i64
{
    string_find("abc", "a", 4)
}

@id("text.to_i64")
fn to_i64() -> i64
{
    let a = match string_to_i64("42") { Option::Some { value: n } => n, Option::None {} => 1000, };
    let b = match string_to_i64("-7") { Option::Some { value: n } => n, Option::None {} => 1000, };
    let c = match string_to_i64("007") { Option::Some { value: n } => n, Option::None {} => 1000, };
    let d = match string_to_i64("-0") { Option::Some { value: n } => n, Option::None {} => 1000, };
    let max = match string_to_i64("9223372036854775807") { Option::Some { value: n } => if n == 9223372036854775807 { 1 } else { 0 }, Option::None {} => 0, };
    let min = match string_to_i64("-9223372036854775808") { Option::Some { value: n } => if n == -9223372036854775808 { 1 } else { 0 }, Option::None {} => 0, };
    (a + b + c + d) * 100 + max * 10 + min
}

@id("text.to_i64_none")
fn to_i64_none() -> i64
{
    let n1 = match string_to_i64("") { Option::Some { value: n } => n, Option::None {} => 1, };
    let n2 = match string_to_i64("-") { Option::Some { value: n } => n, Option::None {} => 1, };
    let n3 = match string_to_i64("+5") { Option::Some { value: n } => n, Option::None {} => 1, };
    let n4 = match string_to_i64(" 5") { Option::Some { value: n } => n, Option::None {} => 1, };
    let n5 = match string_to_i64("5 ") { Option::Some { value: n } => n, Option::None {} => 1, };
    let n6 = match string_to_i64("1a") { Option::Some { value: n } => n, Option::None {} => 1, };
    let n7 = match string_to_i64("9223372036854775808") { Option::Some { value: n } => n, Option::None {} => 1, };
    let n8 = match string_to_i64("-9223372036854775809") { Option::Some { value: n } => n, Option::None {} => 1, };
    n1 + n2 + n3 + n4 + n5 + n6 + n7 + n8
}

@id("text.match_temp")
fn match_temp() -> i64
{
    match string_to_i64(string_slice("12x", 0, 2)) { Option::Some { value: n } => n, Option::None {} => 0, }
}

@id("text.trim")
fn trim() -> i64
{
    let padded = string_trim(" \t a b \r\n");
    let blank = string_trim("   ");
    let bare = string_trim("x");
    string_len(padded) * 100 + string_len(blank) * 10 + string_len(bare)
}

@id("text.byte_at")
fn byte_at() -> i64
{
    let text = "Azé";
    string_byte_at(text, 0) * 1000000 + string_byte_at(text, 1) * 1000 + string_byte_at(text, 2)
}

@id("text.byte_at_past_end")
fn byte_at_past_end() -> i64
{
    string_byte_at("A", 1)
}

@id("text.loop")
fn lines() -> i64
{
    let text = "12\nx\n -7 \n30\n";
    let size = string_len(text);
    let mut out = "[";
    let mut start = 0;
    let mut total = 0;
    let mut letters = 0;
    while start < size {
        let end = string_find(text, "\n", start);
        let line = string_trim(string_slice(text, start, end));
        let value = match string_to_i64(line) { Option::Some { value: n } => n, Option::None {} => 0, };
        letters = letters + if string_byte_at(line, 0) == 120 { 1 } else { 0 };
        out = string_concat(out, line);
        out = string_concat(out, ";");
        total = total + value;
        start = end + 1;
        0
    }
    total * 1000 + letters * 100 + string_len(out)
}

@id("text.loop_failure")
fn loop_failure() -> i64
{
    let text = "ab,cd,ef";
    let mut out = "";
    let mut start = 0;
    let mut i = 0;
    while i < 5 {
        let comma = string_find(text, ",", start);
        let field = string_slice(text, start, comma);
        out = string_concat(out, field);
        start = comma + 1;
        i = i + 1;
        0
    }
    string_len(out)
}

@id("text.loop_temporaries")
fn loop_temporaries() -> i64
{
    let text = "abc";
    let mut i = 0;
    let mut total = 0;
    while i < 3 {
        let n = string_len(text);
        total = total + n + string_len("ab");
        i = i + 1;
        0
    }
    total
}

@id("app.main")
fn main() -> i64
{
    slice()
}
"#;

/// Every case and its observation on both lanes.
const CASES: &[(&str, &str)] = &[
    ("text.slice", "ok|350"),
    ("text.slice_past_end", "semaprax.text.v1|1"),
    ("text.slice_inverted", "semaprax.text.v1|1"),
    ("text.slice_negative", "semaprax.text.v1|1"),
    ("text.slice_boundary", "semaprax.text.v1|2"),
    ("text.find", "ok|1306"),
    ("text.find_past_end", "semaprax.text.v1|1"),
    ("text.to_i64", "ok|4211"),
    ("text.to_i64_none", "ok|8"),
    ("text.match_temp", "ok|12"),
    ("text.trim", "ok|301"),
    ("text.byte_at", "ok|65122195"),
    ("text.byte_at_past_end", "semaprax.text.v1|1"),
    ("text.loop", "ok|35112"),
    ("text.loop_failure", "semaprax.text.v1|1"),
    ("text.loop_temporaries", "ok|15"),
];

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn text_toolkit_round_trips_and_projects_compiler_identities() {
    let program = parse(SOURCE, Path::new("text-toolkit.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("text-toolkit-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    let graph = graph::to_json(&program).unwrap();
    assert_eq!(graph, graph::to_json(&reparsed).unwrap());
    for callee in [
        "core.string.slice",
        "core.string.find",
        "core.string.to_i64",
        "core.string.trim",
        "core.string.byte_at",
    ] {
        assert!(
            graph.contains(&format!("\"callee\":\"{callee}\"")),
            "{callee}"
        );
    }
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
}

#[test]
fn reference_interpreter_runs_text_toolkit() {
    let canonical = format::canonical(&parse(SOURCE, Path::new("text-toolkit.spx")).unwrap());
    let fixture = Fixture::new(&canonical);
    for (id, expected) in CASES {
        let result =
            interpreter::interpret(&fixture.source, id, &[], &InterpreterOptions::default())
                .unwrap();
        let envelope: Value = serde_json::from_str(&result.envelope).unwrap();
        let outcome = &envelope["payload"]["outcome"];
        let observed = if outcome["kind"] == "returned" {
            format!("ok|{}", outcome["value"].as_str().unwrap())
        } else {
            assert_eq!(outcome["kind"], "failed", "{}", result.envelope);
            format!(
                "{}|{}",
                outcome["status"]["domain_id"].as_str().unwrap(),
                outcome["status"]["code"].as_u64().unwrap()
            )
        };
        assert_eq!(&observed, expected, "{id}");
    }
    fixture.cleanup();
}

#[test]
fn native_text_toolkit_settles_every_allocation_on_every_exit() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("text-toolkit-native.spx")).unwrap();
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let mut probe = format!(
        "{}\n{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_utf8_settlement_v1/allocations.c")
    );
    let mut expected = String::new();
    for (id, observation) in CASES {
        let symbol = id
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        probe.push_str(&format!(
            r#"{{
    int64_t value=INT64_MIN;
    spx_status_token token=spx_decl_{symbol}(&context,&value);
    if(token==0) {{ (void)printf("{id}|ok|%lld\n",(long long)value); }}
    else {{
        REQUIRE(value==INT64_MIN);
        const struct spx_normalized_status *status=spx_status_resolve(&context,token);
        REQUIRE(status!=NULL);
        (void)printf("{id}|%s|%u\n",status->domain_id,(unsigned)status->code);
    }}
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
}}
"#
        ));
        expected.push_str(&format!("{id}|{observation}\n"));
    }
    probe.push_str("return 0; }\n");
    let mut fixture = Fixture::new(SOURCE);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }
    fixture.cleanup();
}

#[test]
fn core_wasm_refuses_text_toolkit_with_one_stable_diagnostic() {
    let program = parse(SOURCE, Path::new("text-toolkit-wasm.spx")).unwrap();
    let error = emit_module(
        &program,
        &["text.trim".to_owned()],
        InternalStringOptions::default(),
    )
    .expect_err("Text Toolkit v1 is not lowered to Core Wasm");
    assert_eq!(error.code, "SPX-W116");
    assert_eq!(
        error.message,
        "Text Toolkit v1 operation `string_trim` is not lowered to Core Wasm; run it on the reference interpreter or native C11"
    );
}

fn diagnostics(body: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let source =
        format!("module test.refused;\n\n@id(\"app.main\")\nfn main() -> i64\n{{\n{body}\n}}\n");
    let program = parse(&source, Path::new("refused.spx")).unwrap();
    verify::verify(&program)
}

#[test]
fn text_toolkit_misuse_has_stable_diagnostics() {
    let found = diagnostics("    string_len(string_slice(\"abc\", 0))");
    assert!(
        found.iter().any(|diagnostic| diagnostic.code == "SPX-T204"
            && diagnostic.message == "`string_slice` expects 3 arguments, received 2"),
        "{found:?}"
    );
    // Offsets are signed `i64` byte offsets, not `usize`.
    let found = diagnostics("    string_byte_at(\"abc\", 0usize)");
    assert!(
        !found.is_empty() && found[0].code.starts_with("SPX-T"),
        "{found:?}"
    );
    // `file_read_text` borrows a `str` view, so a literal must be bound and
    // viewed first.
    let found = diagnostics("    string_len(file_read_text(\"a.txt\"))");
    assert!(
        found.iter().any(|diagnostic| diagnostic.code == "SPX-T205"),
        "{found:?}"
    );
    // The file read requires the explicit `fs.read` effect.
    let found = diagnostics(
        "    let path = \"a.txt\";\n    let view = string_as_str(path);\n    string_len(file_read_text(view))",
    );
    assert!(
        found.iter().any(|diagnostic| diagnostic.code == "SPX-E102"
            && diagnostic.message
                == "call to `file_read_text` requires effect `fs.read`; add it to `main`"),
        "{found:?}"
    );
    // A while body admits a `string_to_i64` match only as the exact
    // `Some { value }` / `None {}` pair; a near miss names the detail.
    let found = diagnostics(
        "    let mut i = 0;\n    let mut sum = 0;\n    while i < 2 {\n        sum = sum + match string_to_i64(\"4\") { Option::Some { v } => v, Option::None {} => 0, };\n        i = i + 1;\n        0\n    }\n    sum",
    );
    assert!(
        found.iter().any(|diagnostic| diagnostic.code == "SPX-T252"
            && diagnostic.message
                == "a `string_to_i64` match in a while body must be exactly two unguarded arms, `Option::Some { value }` and `Option::None {}`; this one binds `Option::Some { v }` - rename the field to `value`"),
        "{found:?}"
    );
}

const COMMAND: &str = r#"module app.count;

permit { fs.read, process.args.read, process.stderr.write, process.stdout.write }

@id("app.usage")
fn usage() -> i64
    uses { process.stderr.write }
{
    let message = "usage: count <file>\n";
    let view = string_as_str(message);
    let written = stderr_write(str_as_bytes(view));
    2
}

@id("app.report")
fn report() -> i64
    uses { fs.read, process.args.read, process.stdout.write }
{
    let path = arg_utf8(0usize);
    let text = file_read_text(path);
    let size = string_len(text);
    let mut start = 0;
    let mut lines = 0;
    let mut total = 0;
    while start < size {
        let end = string_find(text, "\n", start);
        let value = match string_to_i64(string_trim(string_slice(text, start, end))) { Option::Some { value: n } => n, Option::None {} => 0, };
        total = total + value;
        lines = lines + 1;
        start = end + 1;
        0
    }
    let mut out = "lines: ";
    out = string_concat(out, string_from_i64(lines));
    out = string_concat(out, "\ntotal: ");
    out = string_concat(out, string_from_i64(total));
    out = string_concat(out, "\n");
    let view = string_as_str(out);
    let written = stdout_write(str_as_bytes(view));
    if total < 0 { 3 } else { 0 }
}

@id("app.main")
fn main() -> i64
    uses { fs.read, process.args.read, process.stderr.write, process.stdout.write }
{
    if args_len() != 1usize { usage() } else { report() }
}
"#;

fn run(fixture: &Fixture, native: bool, arguments: &[&str]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
    command
        .current_dir(&fixture.root)
        .arg("run")
        .arg("source.spx");
    if native {
        command.arg("--native");
    }
    let output = command.args(arguments).output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn command_line_programs_receive_arguments_files_and_exit_status() {
    let program = parse(COMMAND, Path::new("count.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert_eq!(format::canonical(&program), COMMAND);
    assert!(graph::to_json(&program)
        .unwrap()
        .contains("\"callee\":\"core.host.file-read-text\""));
    let mut fixture = Fixture::new(COMMAND);
    fixture.write("numbers.txt", "1\n 20 \nx\n300\n");
    fixture.write("negative.txt", "-5\n");
    fixture.write("binary.txt", [0xffu8, b'\n']);
    let native_lane = command_available("clang");
    let lanes: &[bool] = if native_lane {
        &[false, true]
    } else {
        &[false]
    };
    for &native in lanes {
        assert_eq!(
            run(&fixture, native, &["--", "numbers.txt"]),
            (0, "lines: 4\ntotal: 321\n".to_owned(), String::new()),
            "native={native}"
        );
        // `main`'s result is the exit status; output is still published.
        assert_eq!(
            run(&fixture, native, &["--", "negative.txt"]),
            (3, "lines: 1\ntotal: -5\n".to_owned(), String::new()),
            "native={native}"
        );
        // A usage error the program reports itself: no CLI hint follows, and
        // a help flag after `--` is the program's argument.
        for arguments in [&["--"][..], &["--", "-h", "x"][..]] {
            assert_eq!(
                run(&fixture, native, arguments),
                (2, String::new(), "usage: count <file>\n".to_owned()),
                "native={native} {arguments:?}"
            );
        }
        // A missing file, an escaping path, and invalid UTF-8 are checked
        // failures: exit 1, one stderr line, no partial stdout.
        for (file, status) in [
            ("missing.txt", "semaprax.filesystem.v1"),
            ("../numbers.txt", "semaprax.filesystem.v1"),
            ("binary.txt", "semaprax.text.v1"),
        ] {
            let (code, stdout, stderr) = run(&fixture, native, &["--", file]);
            assert_eq!((code, stdout.as_str()), (1, ""), "native={native} {file}");
            assert!(stderr.contains(status), "native={native} {file}: {stderr}");
            assert_eq!(stderr.lines().count(), 1, "{stderr}");
        }
    }
    // `--json` on the interpreter route publishes one envelope.
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .current_dir(&fixture.root)
        .args(["run", "source.spx", "--json", "--", "numbers.txt"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["schema"], "semaprax.single-file-command.v1");
    assert_eq!(envelope["outcome"]["value"], "0");
    assert_eq!(
        envelope["stdout"],
        serde_json::to_value(b"lines: 4\ntotal: 321\n".to_vec()).unwrap()
    );
    assert_eq!(envelope["stderr"], serde_json::json!([]));
    // Arguments reach only a program that permits the command-line profile.
    let plain =
        Fixture::new("module app.plain;\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    7\n}\n");
    for &native in lanes {
        let (code, _, stderr) = run(&plain, native, &["--", "x"]);
        assert_eq!(code, 2);
        assert!(
            stderr.starts_with("run passes arguments after `--` only to a command-line program"),
            "{stderr}"
        );
    }
    plain.cleanup();
    fixture.cleanup();
}

#[test]
fn command_line_profile_admits_only_its_closed_authority() {
    // The byte-oriented `file_read` keeps its injected-provider profiles; a
    // command-line program reads text with `file_read_text`.
    let source = "module app.bytes;\n\npermit { fs.read }\n\n@id(\"app.main\")\nfn main() -> i64\n    uses { fs.read }\n{\n    let path = [97u8];\n    let data = file_read(array_as_slice(path), 1usize, 16usize);\n    0\n}\n";
    let program = parse(source, Path::new("bytes.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let error = semaprax::codegen::emit_c_with_source_command(&program).unwrap_err();
    assert_eq!(error.code, "SPX-T273");
    assert_eq!(
        error.message,
        "single-file command-line program authority mismatch: `file_read` is outside the command-line profile; read files with `file_read_text`"
    );
    // Outside the native command-line profile `file_read_text` fails closed.
    let program = parse(COMMAND, Path::new("count.spx")).unwrap();
    let error = semaprax::codegen::emit_c(&program).unwrap_err();
    assert!(
        error
            .message
            .contains("file_read_text requires the native single-file command profile")
            || error
                .message
                .contains("command I/O operation requires the native language-command profile"),
        "{error:?}"
    );
}
