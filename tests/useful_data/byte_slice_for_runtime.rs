//! Immutable borrowed `Slice<u8>` traversal against an independent manual
//! `byte_len`/`byte_get` loop and a Rust oracle on the interpreter, native
//! C11 `-O0`/`-O2` and Core-Wasm: empty, singleton, all 256 bytes, a
//! subrange, an owned buffer and a String root; `continue`, `break`, `return`,
//! lazy operands and a body failure that keeps its selected status.
use std::path::Path;
use std::process::Command;

use semaprax::{codegen, hir, interpreter, parse};

const SOURCE: &str = include_str!("../fixtures/byte-slice-traversal.spx");

const FAILURE: &str = r#"module app.byte_traversal_failure;

@id("app.failure.reject")
fn reject(byte: u8) -> i64
    requires byte != 9u8
{
    i64_from_u8(byte)
}

@id("app.main")
fn main() -> i64 {
    let data = [1u8, 9u8, 2u8];
    let owned = bytes_copy(array_as_slice(data));
    let view = bytes_as_slice(owned);
    let mut total = 0;
    for byte in view { total = total + reject(byte); 0 }
    total
}
"#;

/// A traversal-only module: parameter and array views, no owned buffer.
const VIEW_ONLY: &str = r#"module app.byte_traversal_view_only;

@id("app.view.sum")
fn sum(view: borrow Slice<u8>) -> i64 {
    let mut total = 0;
    for byte in view { total = total + i64_from_u8(byte); 0 }
    total
}

@id("app.main")
fn main() -> i64 {
    let data = [1u8, 2u8, 3u8];
    sum(array_as_slice(data))
}
"#;

#[test]
fn little_endian_reads_match_independent_byte_loop_on_every_engine() {
    let source = r#"module app.byte_word;
@id("word.reference")
fn reference(view: borrow Slice<u8>, offset: usize, width: usize) -> usize {
    let length = byte_len(view);
    if width == 0usize || width > 8usize || offset > length || width > length - offset {
        0usize
    } else {
        let mut index = 0usize;
        let mut factor = 1usize;
        let mut total = 0usize;
        while index < width {
            let current = match byte_get(view, offset + index) { Option::Some { value: byte } => usize_from_u8(byte), Option::None {} => 0usize, };
            total = total + current * factor;
            index = index + 1usize;
            if index < width { factor = factor * 256usize; 0 } else { 0 }
            0
        }
        total
    }
}
@id("word.check")
fn check(view: borrow Slice<u8>, offset: usize, width: usize) -> i64 {
    let length = byte_len(view);
    let valid = width != 0usize && width <= 8usize && offset <= length && width <= length - offset;
    let actual = byte_read_le(view, offset, width);
    match actual {
        Option::Some { value: actual_word } => if valid && actual_word == reference(view, offset, width) { 0 } else { 1 },
        Option::None {} => if valid { 1 } else { 0 },
    }
}
@id("word.all")
fn all(view: borrow Slice<u8>) -> i64 {
    let mut expected_prefix = 0usize;
    let mut scanning = true;
    while scanning && expected_prefix < byte_len(view) {
        let advanced = match byte_get(view, expected_prefix) {
            Option::Some { value } => if value == 0u8 { expected_prefix = expected_prefix + 1usize; 0 } else { scanning = false; 0 },
            Option::None {} => { scanning = false; 0 },
        };
        0
    }
    let mut width = 0usize;
    let mut failed = if byte_zero_prefix(view) == expected_prefix { 0 } else { 1 };
    while width < 10usize {
        let mut offset = 0usize;
        while offset <= byte_len(view) + 1usize {
            failed = failed + check(view, offset, width);
            offset = offset + 1usize;
            0
        }
        width = width + 1usize;
        0
    }
    failed + check(view, 18446744073709551615usize, 8usize) + check(view, 18446744073709551612usize, 8usize)
}
@id("app.main")
fn main() -> i64 {
    let data = [0u8, 1u8, 127u8, 128u8, 255u8, 0u8, 254u8, 255u8, 17u8, 42u8];
    let view = array_as_slice(data);
    let empty = [0u8; 0];
    let maximum = [255u8; 8];
    let zeros = [0u8; 64];
    let delayed = [0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 255u8];
    let owned = bytes_copy(view);
    let owned_view = bytes_as_slice(owned);
    let text = "SEMAPRAX";
    let string_view = string_as_str(text);
    let utf8 = str_as_bytes(string_view);
    all(utf8) + all(owned_view) + all(byte_range(owned_view, 1usize, 9usize)) + all(array_as_slice(delayed)) + all(array_as_slice(zeros)) + all(view) + all(byte_range(view, 1usize, 9usize)) + all(array_as_slice(empty)) + all(array_as_slice(maximum))
}
"#;
    let ast = parse(source, Path::new("little-endian-reads.spx")).unwrap();
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    let root = std::env::temp_dir().join(format!("semaprax-byte-read-le-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let source_path = root.join("read.spx");
    std::fs::write(&source_path, source).unwrap();
    let interpreted = interpreter::interpret(
        &source_path,
        "app.main",
        &[],
        &interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    assert!(interpreted.returned, "{interpreted:?}");
    assert!(
        interpreted.envelope.contains("\"value\":\"0\""),
        "{}",
        interpreted.envelope
    );
    let c = root.join("read.c");
    std::fs::write(&c, codegen::emit_c(&ast).unwrap()).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("read{optimization}"));
        let compiled = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(&binary).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"0\n");
    }
    let wasm = semaprax::wasm::emit_module(&ast).unwrap();
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&wasm)
        .unwrap();
    raw_wasm(&root, "word-reads", &wasm, 0);
    std::fs::remove_dir_all(root).unwrap();
}

fn mix(total: i64, byte: u8) -> i64 {
    (total * 31 + i64::from(byte)) % 1_000_000_007
}

fn fold(bytes: &[u8]) -> i64 {
    bytes.iter().fold(0, |total, byte| mix(total, *byte))
}

fn expected() -> i64 {
    let all = (0..=255u8).collect::<Vec<_>>();
    let cases: [&[u8]; 6] = [&[], &[7], &all, &all[10..20], &all, b"SEMAPRAX"];
    let whole = cases.iter().map(|bytes| fold(bytes)).sum::<i64>();
    let mut early = 0;
    for &byte in &all {
        if byte == 3 {
            continue;
        }
        if byte == 250 {
            break;
        }
        early = mix(early, byte);
    }
    let located = 200 + -1;
    let lazy = 1;
    whole * 2 + early * 2 + located + lazy + fold(&[6, 7])
}

#[test]
fn byte_for_matches_manual_loop_and_oracle_on_every_engine() {
    let expected = expected();
    let program = parse(SOURCE, Path::new("byte-slice-traversal.spx")).unwrap();
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
    let failure = parse(FAILURE, Path::new("byte-slice-traversal-failure.spx")).unwrap();
    hir::validate(&hir::resolve(&failure).unwrap()).unwrap();

    let root = std::env::temp_dir().join(format!("semaprax-byte-slice-for-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let success_source = root.join("success.spx");
    let failure_source = root.join("failure.spx");
    std::fs::write(&success_source, SOURCE).unwrap();
    std::fs::write(&failure_source, FAILURE).unwrap();

    for _ in 0..4 {
        let outcome = interpreter::interpret(
            &success_source,
            "app.main",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(outcome.returned, "{}", outcome.envelope);
        assert!(
            outcome
                .envelope
                .contains(&format!("\"value\":\"{expected}\"")),
            "{}",
            outcome.envelope
        );
        let failed = interpreter::interpret(
            &failure_source,
            "app.main",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(!failed.returned);
        let envelope: serde_json::Value = serde_json::from_str(&failed.envelope).unwrap();
        assert_eq!(
            envelope["payload"]["outcome"]["status"]["domain_id"],
            "semaprax.contract.v1"
        );
        assert_eq!(envelope["payload"]["outcome"]["status"]["code"], 1);
    }

    let success_c = root.join("success.c");
    let failure_c = root.join("failure.c");
    std::fs::write(&success_c, codegen::emit_c(&program).unwrap()).unwrap();
    std::fs::write(&failure_c, codegen::emit_c(&failure).unwrap()).unwrap();
    for optimization in ["-O0", "-O2"] {
        for (source, binary) in [(&success_c, "success"), (&failure_c, "failure")] {
            let binary = root.join(format!("{binary}-{optimization}"));
            let compiled = Command::new("clang")
                .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
                .arg(source)
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "{optimization}: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
        }
        for _ in 0..4 {
            let output = Command::new(root.join(format!("success-{optimization}")))
                .output()
                .unwrap();
            assert!(output.status.success(), "success/{optimization}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                expected.to_string()
            );
            let output = Command::new(root.join(format!("failure-{optimization}")))
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(70), "failure/{optimization}");
            assert!(String::from_utf8_lossy(&output.stderr)
                .contains("requires byte != 9u8 in app.failure.reject"));
        }
    }

    // The complete fixture selects aggregate byte-data lowering. The legacy
    // scalar Web package deliberately has no String runtime; retain its refusal
    // rather than adding an authored aggregate merely to change that selector.
    let refused = semaprax::wasm::build_web(&program, &root.join("legacy-string")).unwrap_err();
    assert_eq!(refused.code, "SPX-W116");
    for (name, program, value) in [("success", &program, expected), ("failure", &failure, 0)] {
        let module = semaprax::wasm::emit_module(program).unwrap();
        raw_wasm(&root, name, &module, value);
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn byte_for_view_traversal_materializes_no_buffer_on_core_wasm() {
    let program = parse(VIEW_ONLY, Path::new("byte-slice-traversal-view-only.spx")).unwrap();
    let module = semaprax::wasm::emit_module(&program).unwrap();
    assert_eq!(module, semaprax::wasm::emit_module(&program).unwrap());
    // Copy is a mandatory aggregate-byte ABI import, even when unused. Prove
    // that no current function body calls its actual imported function index.
    let mut copy = None;
    let mut imports = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(&module) {
        if let wasmparser::Payload::ImportSection(section) = payload.unwrap() {
            for import in section.into_imports() {
                let import = import.unwrap();
                assert!(matches!(import.ty, wasmparser::TypeRef::Func(_)));
                assert_eq!(import.module, "env");
                if import.name == "spx_bytes_copy" {
                    assert!(copy.replace(imports.len() as u32).is_none());
                }
                imports.push(import.name.to_owned());
            }
        }
    }
    assert_eq!(
        imports,
        [
            "spx_add",
            "spx_sub",
            "spx_mul",
            "spx_div",
            "spx_rem",
            "spx_neg",
            "spx_contract_fail",
            "spx_bytes_copy",
            "spx_bytes_get",
            "spx_bytes_drop",
            "spx_bytes_as_slice"
        ]
    );
    let copy = copy.expect("mandatory aggregate byte copy import");
    for payload in wasmparser::Parser::new(0).parse_all(&module) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut operators = body.get_operators_reader().unwrap();
            while !operators.eof() {
                let operator = operators.read().unwrap();
                assert!(
                    !matches!(operator, wasmparser::Operator::Call { function_index } if function_index == copy)
                );
                assert!(!matches!(
                    operator,
                    wasmparser::Operator::MemoryCopy { .. }
                        | wasmparser::Operator::MemoryGrow { .. }
                ));
            }
        }
    }
    let root = std::env::temp_dir().join(format!("semaprax-byte-for-view-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    raw_wasm(&root, "view", &module, 6);
    std::fs::remove_dir_all(root).unwrap();
}

/// Compare literal-slot reuse with an independently authored block-result
/// Copy boundary. Assignments and nested results retain their existing copy.
#[test]
fn literal_array_bindings_reuse_exact_slots_while_nonliteral_copy_boundaries_remain() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-byte-literal-slots-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let header = &VIEW_ONLY[..VIEW_ONLY.find("@id(\"app.main\")").unwrap()];
    let copies = |module: &[u8]| {
        let mut copies = 0;
        for payload in wasmparser::Parser::new(0).parse_all(module) {
            if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
                let mut operators = body.get_operators_reader().unwrap();
                while !operators.eof() {
                    match operators.read().unwrap() {
                        wasmparser::Operator::MemoryCopy { .. } => copies += 1,
                        wasmparser::Operator::MemoryGrow { .. } => {
                            panic!("fixed array grew memory")
                        }
                        _ => {}
                    }
                }
            }
        }
        copies
    };
    for (ordinal, literal, expected) in [
        (0, "[]", 0),
        (1, "[7u8]", 7),
        (2, "[1u8, 2u8, 3u8]", 6),
        (3, "[1u8; 256]", 256),
    ] {
        for old in [false, true] {
            let initializer = if old {
                format!("{{let pad = 0; {literal}}}")
            } else {
                literal.to_owned()
            };
            let source = format!("{header}@id(\"app.main\") fn main()->i64 {{ let data = {initializer}; sum(array_as_slice(data)) }}");
            let path = root.join(format!("literal-{ordinal}-{old}.spx"));
            std::fs::write(&path, &source).unwrap();
            let program = parse(&source, &path).unwrap();
            let resolved = hir::resolve(&program).unwrap();
            hir::validate(&resolved).unwrap();
            let outcome = interpreter::interpret(
                &path,
                "app.main",
                &[],
                &interpreter::InterpreterOptions::default(),
            )
            .unwrap();
            assert!(outcome.returned, "{}", outcome.envelope);
            assert!(
                outcome
                    .envelope
                    .contains(&format!("\"value\":\"{expected}\"")),
                "{}",
                outcome.envelope
            );
            let module = semaprax::wasm::emit_module(&program).unwrap();
            assert_eq!(module, semaprax::wasm::emit_module(&program).unwrap());
            if old && ordinal != 0 {
                assert!(copies(&module) > 0);
            } else {
                assert_eq!(copies(&module), 0);
            }
            raw_wasm(&root, "view", &module, expected);
        }
    }
    // A normally completing preceding call stays before initialization. A
    // later failed precondition keeps the original full-engine failure owner.
    let source = format!("{header}@id(\"app.touch\") fn touch()->i64 {{1}} @id(\"app.main\") fn main()->i64 {{ let mark = touch(); let mut data = [1u8]; data = [7u8]; mark + sum(array_as_slice(data)) }}");
    let path = root.join("assignment.spx");
    std::fs::write(&path, &source).unwrap();
    let program = parse(&source, &path).unwrap();
    let errors = hir::resolve(&program).expect_err("array assignment must stay unadmitted");
    assert!(
        errors.iter().any(|error| error.code == "SPX-U105"),
        "{errors:?}"
    );
    // Repeated calls reinitialize the same function-frame slots. Keep array
    // mutation and loop-local literals as explicit negative controls.
    for old in [false, true] {
        let initializer = if old {
            "{ let pad = 0; [7u8] }"
        } else {
            "[7u8]"
        };
        let source = format!("{header}@id(\"app.main\") fn main()->i64 {{ let mut index = 0; let mut total = 0; while index < 3 {{ let mut data = {initializer}; let first = sum(array_as_slice(data)); data = [9u8]; total = total + first + sum(array_as_slice(data)); index = index + 1; 0 }} total }}");
        let rejected_path = root.join(format!("loop-local-array-{old}.spx"));
        let rejected = parse(&source, &rejected_path).unwrap();
        let errors = match hir::resolve(&rejected) {
            Ok(_) => panic!("loop-local array literals unexpectedly admitted"),
            Err(errors) => errors,
        };
        let diagnostic = format!("{errors:?}");
        assert!(diagnostic.contains("SPX-T252"), "{diagnostic}");
        assert!(
            diagnostic.contains("fixed-array literals are not admitted in bounded while bodies"),
            "{diagnostic}"
        );
        let source = format!("{header}@id(\"app.sample\") fn sample()->i64 {{ let data = {initializer}; let first = sum(array_as_slice(data)); let other = [9u8]; first + sum(array_as_slice(other)) }} @id(\"app.main\") fn main()->i64 {{ sample() + sample() + sample() }}");
        let path = root.join(format!("reinitialize-{old}.spx"));
        std::fs::write(&path, &source).unwrap();
        let program = parse(&source, &path).unwrap();
        let resolved = hir::resolve(&program).unwrap();
        hir::validate(&resolved).unwrap();
        let outcome = interpreter::interpret(
            &path,
            "app.main",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        let expected = 3 * (7 + 9);
        assert!(outcome.returned, "{}", outcome.envelope);
        assert!(
            outcome
                .envelope
                .contains(&format!("\"value\":\"{expected}\"")),
            "{}",
            outcome.envelope
        );
        let module = semaprax::wasm::emit_module(&program).unwrap();
        assert_eq!(module, semaprax::wasm::emit_module(&program).unwrap());
        if old {
            assert!(
                copies(&module) > 0,
                "block result keeps its independent initialization copy"
            );
        } else {
            assert_eq!(
                copies(&module),
                0,
                "both literal bindings reuse their exact initialization slots"
            );
        }
        raw_wasm(&root, "view", &module, expected);
    }
    std::fs::remove_dir_all(root).unwrap();
}

fn raw_wasm(root: &Path, name: &str, module: &[u8], expected: i64) {
    let wasm = root.join(format!("{name}.wasm"));
    let script = root.join("raw-byte-for.mjs");
    std::fs::write(&wasm, module).unwrap();
    std::fs::write(&script, include_str!("byte_slice_for_runtime/host.mjs")).unwrap();
    let output = Command::new("node")
        .arg(&script)
        .arg(&wasm)
        .arg(name)
        .arg(expected.to_string())
        .output()
        .expect("Node is required for byte traversal evidence");
    assert!(
        output.status.success(),
        "{name}: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"byte-slice-for-ok\n");
}

#[path = "byte_slice_for_runtime/catalog_identifier.rs"]
mod catalog_identifier;

#[path = "byte_slice_for_runtime/nested.rs"]
mod nested;
