//! Content parity has a narrower meaning than resource settlement. The scalar
//! cases run in all three producers; owned user signatures run only in C/Wasm.
//! The bounded Node map observes values, never claims missing Wasm drop hooks.

use super::{
    checked, codegen, compile_and_run, format, fs, symbol, Command, Ordering, Path, OBSERVER,
    SERIAL,
};

const BASE: &str = r#"
module strings.nul_base;
@id("s.main") fn main() -> i64 {
    let leading = "\u{0}a";
    let middle = "a\u{0}b";
    let trailing = "a\u{0}";
    let cloned = middle;
    if leading == "\u{0}a" && leading != "" && leading != "\u{0}b"
        && middle != "a\u{0}c" && middle != "a" && cloned == "a\u{0}b"
        && trailing != "a" && trailing == "a\u{0}"
        && "é\u{0}世界" == "é\u{0}世界" && "é\u{0}世界" != "é\u{0}世間" { 42 } else { 0 }
}
"#;
const V1: &str = r#"
module strings.nul_v1;
@id("s.main") fn main() -> i64 {
    let leading = "\u{0}a";
    let middle = "a\u{0}b";
    let trailing = "a\u{0}";
    let joined = string_concat("a\u{0}", "\u{0}b");
    if string_len(leading) == 2 && string_len(middle) == 3 && string_len(trailing) == 2
        && string_is_empty(leading) == false && string_is_empty("\u{0}") == false
        && string_is_empty("") && string_len(joined) == 4 && joined == "a\u{0}\u{0}b"
        && string_concat("", "\u{0}") == "\u{0}" && string_concat("\u{0}", "") == "\u{0}"
        && string_concat("", "") == "" && string_len("é\u{0}世界") == 9 { 42 } else { 0 }
}
"#;
const V2: &str = r#"
module strings.nul_v2;
@id("s.main") fn main() -> i64 {
    let text = "é\u{0}世界";
    let zero = string_from_char('\u{0}');
    if string_len(text) == 9 && string_len_chars(text) == 4
        && string_len_chars("\u{0}") == 1 && string_len_chars("") == 0
        && string_len(zero) == 1 && string_len_chars(zero) == 1 && zero == "\u{0}"
        && string_is_empty(zero) == false
        && string_starts_with("a\u{0}b", "a\u{0}") && string_starts_with("a\u{0}b", "a\u{0}c") == false
        && string_starts_with("a\u{0}", "a\u{0}b") == false && string_starts_with("\u{0}a", "")
        && string_contains("a\u{0}b", "\u{0}b") && string_contains("a\u{0}b", "\u{0}c") == false
        && string_contains("\u{0}b", "b") && string_contains("a\u{0}", "a\u{0}b") == false
        && string_contains(text, "\u{0}世") && string_contains(text, "")
        && string_concat(string_from_char('\u{0}'), string_from_char('λ')) == "\u{0}λ" { 42 } else { 0 }
}
"#;
const OWNED: &str = r#"
module strings.nul_owned;
@id("s.identity") fn identity(value: string) -> string { let copy = value; copy }
@id("s.pick") fn pick(flag: bool) -> string { if flag { "\u{0}é\u{0}世界" } else { "end\u{0}" } }
@id("s.same") fn same(left: string, right: string) -> bool { left == right }
@id("s.main") fn main() -> i64 {
    if same(identity(pick(true)), "\u{0}é\u{0}世界") && same(identity(pick(false)), "end\u{0}")
        && same(identity("a\u{0}b"), "a\u{0}c") == false { 42 } else { 0 }
}
"#;

fn native(source: &str) -> String {
    let program = checked(source);
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(generated.contains("struct spx_string_v10"));
    let main = symbol("s.main");
    format!(
        r#"{OBSERVER}
{generated}
#undef malloc
#undef free
int main(void) {{
    struct spx_status_entry entries[1];
    struct spx_context context = {{0}};
    REQUIRE(spx_context_init(&context, 19, entries, 1, NULL, NULL, NULL));
    for (unsigned repetition = 0; repetition < 32; ++repetition) {{
        size_t before = fixture_allocations;
        int64_t value = INT64_MIN;
        REQUIRE({main}(&context, &value) == 0 && value == 42);
        REQUIRE(fixture_allocations > before && fixture_allocations == fixture_frees);
        REQUIRE(fixture_live == 0 && context.status_arena.length == 0);
    }}
    (void)puts("native-ordinary-strings-settled");
    return 0;
}}
"#
    )
}

fn interpreter_and_wasm(source: &str, group: &str, interpreted: bool) {
    let program = checked(source);
    let module = semaprax::wasm::emit_module(&program).unwrap();
    assert_eq!(module, semaprax::wasm::emit_module(&program).unwrap());
    let root = std::env::temp_dir().join(format!(
        "semaprax-string-contents-{group}-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let source_path = root.join("source.spx");
    fs::write(&source_path, format::canonical(&program)).unwrap();
    let outcome = semaprax::interpreter::interpret(
        &source_path,
        "s.main",
        &[],
        &semaprax::interpreter::InterpreterOptions::default(),
    );
    if interpreted {
        let outcome = outcome.unwrap();
        assert!(outcome.returned);
        semaprax::interpreter::verify_envelope_against_source(&outcome.envelope, &source_path)
            .unwrap();
        assert!(outcome
            .envelope
            .contains("\"kind\":\"returned\",\"type\":\"i64\",\"value\":\"42\""));
    } else {
        assert!(outcome
            .unwrap_err()
            .iter()
            .any(|error| error.code == "SPX-F102" && error.message.contains("unsupported_callee")));
    }
    let wasm_path = root.join("module.wasm");
    fs::write(&wasm_path, module).unwrap();
    let probe = root.join("probe.mjs");
    fs::write(&probe, include_str!("contents.mjs")).unwrap();
    let node = std::env::var_os("NODE").unwrap_or_else(|| "node".into());
    let output = Command::new(node)
        .current_dir(&root)
        .arg(&probe)
        .arg(&wasm_path)
        .arg(group)
        .output()
        .expect("Node is required for embedded-NUL String value parity");
    assert!(
        output.status.success(),
        "{}: {}",
        root.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"string-contents-wasm-ok\n");
    assert!(output.stderr.is_empty());
    // As with native fixtures, failures retain diagnostics. Prevalidate every
    // successful artifact before performing the exact, nonrecursive cleanup.
    let mut entries = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    assert_eq!(entries.len(), 3);
    for (entry, expected) in entries
        .iter()
        .zip(["module.wasm", "probe.mjs", "source.spx"])
    {
        assert_eq!(entry.file_name(), expected);
        let metadata = fs::symlink_metadata(entry.path()).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            assert_eq!(metadata.file_attributes() & 0x400, 0);
        }
    }
    for entry in entries {
        fs::remove_file(entry.path()).unwrap();
    }
    fs::remove_dir(root).unwrap();
}

#[test]
fn embedded_nul_values_match_native_wasm_and_admitted_interpreter() {
    for (group, source, interpreted) in [
        ("base", BASE, true),
        ("v1", V1, true),
        ("v2", V2, true),
        ("owned", OWNED, false),
    ] {
        compile_and_run(group, &native(source), false);
        interpreter_and_wasm(source, group, interpreted);
    }
}

#[test]
#[ignore = "requires explicitly provisioned Clang ASan/UBSan runtime"]
fn provisioned_embedded_nul_native_values_asan_ubsan() {
    for (group, source) in [("base", BASE), ("v1", V1), ("v2", V2), ("owned", OWNED)] {
        compile_and_run(group, &native(source), true);
    }
}

#[test]
fn nul_does_not_widen_string_operators_or_invalid_unicode_admission() {
    let invalid_operator = r#"module strings.invalid; @id("s.main") fn main() -> i64 { if "a\u{0}" < "b\u{0}" { 1 } else { 0 } }"#;
    let program = semaprax::parse(invalid_operator, Path::new("invalid-operator.spx")).unwrap();
    assert!(semaprax::verify::verify(&program)
        .iter()
        .any(|error| error.code == "SPX-T250"));
    for (source, expected) in [
        (
            r#"module strings.invalid; @id("s.main") fn main() -> i64 { string_len("\u{D800}") }"#,
            "SPX-P005",
        ),
        (
            r#"module strings.invalid; @id("s.main") fn main() -> i64 { string_len(string_from_char('\u{D800}')) }"#,
            "SPX-P007",
        ),
    ] {
        assert_eq!(
            semaprax::parse(source, Path::new("invalid-unicode.spx"))
                .unwrap_err()
                .code,
            expected
        );
    }
}
