//! Direct borrowed text byte access uses the existing total byte-read contract.
use semaprax::{format, graph, hir, parse, verify, wasm};
use std::path::Path;

pub(super) const DIRECT: &str = r#"module test.borrowed_byte_widening;

@id("byte.read")
fn read(text: borrow str, index: usize) -> i64
{
    match str_byte_at(text, index) { Option::Some { value: byte } => i64_from_u8(byte), Option::None {} => -1, }
}

@id("byte.scan")
fn scan(text: borrow str) -> i64
{
    let mut index = 0usize;
    let mut total = 0;
    while index < 4usize {
        total = total + match str_byte_at(text, index) { Option::Some { value: byte } => i64_from_u8(byte), Option::None {} => 0, };
        index = index + 1usize;
        0
    }
    total
}

@id("app.main")
fn main() -> i64
{
    let text = "é\u{0}";
    let raw = string_as_str(text);
    let empty = "";
    let raw_empty = string_as_str(empty);
    if scan(raw) == 364 && scan(raw_empty) == 0 {
    read(raw, 0usize) + read(raw, 1usize) + read(raw, 2usize) + read(raw, 3usize) + read(raw, 4294967296usize) + read(raw, 18446744073709551615usize) + read(raw_empty, 0usize)
    } else {
        -1
    }
}
"#;

#[test]
fn borrowed_text_byte_at_graph_signature_and_cleanup_are_exact() {
    let ast = parse(DIRECT, Path::new("direct-byte-read.spx")).unwrap();
    assert!(verify::verify(&ast).is_empty());
    let text = format::canonical(&ast);
    let reparsed = parse(&text, Path::new("canonical.spx")).unwrap();
    assert_eq!(
        graph::to_json(&ast).unwrap(),
        graph::to_json(&reparsed).unwrap()
    );
    assert!(graph::to_json(&ast)
        .unwrap()
        .contains("\"callee\":\"core.str.byte-at\""));
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    let read = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "byte.read")
        .unwrap();
    assert!(
        read.cleanup_plan.slots.is_empty(),
        "read borrows its input and yields Copy data"
    );
    // An inferred unused result must select its Option layout and byte runtime,
    // without a source-authored Option type, pattern, or explicit byte view.
    let inferred = parse("module t; @id(\"t.read\") fn inspect(text:borrow str,index:usize)->i64 { let ignored=str_byte_at(text,index); 0 } @id(\"app.main\") fn main()->i64 { 0 }", Path::new("inferred.spx")).unwrap();
    hir::validate(&hir::resolve(&inferred).unwrap()).unwrap();
    wasm::emit_module(&inferred).unwrap();
    // Frozen public text signatures/call shapes stay closed to Option<u8>.
    assert_eq!(
        wasm::emit_module_with_text_exports(&ast, &["byte.read".to_owned()])
            .unwrap_err()
            .code,
        "SPX-W119"
    );
}

#[test]
fn borrowed_text_byte_at_preserves_source_and_hir_refusals() {
    for (body, code) in [
        ("str_byte_at(text, 0)", "SPX-T205"),
        ("str_byte_at(text)", "SPX-T204"),
        ("str_byte_at(\"owned\", 0usize)", "SPX-T205"),
        ("str_byte_at<u8>(text, 0usize)", "SPX-T225"),
    ] {
        let source=format!("module t; @id(\"t.read\") fn read(text:borrow str)->i64 {{ let ignored = {body}; 0 }} @id(\"app.main\") fn main()->i64 {{ 0 }}");
        let ast = parse(&source, Path::new("invalid.spx")).unwrap();
        assert!(verify::verify(&ast)
            .iter()
            .any(|diagnostic| diagnostic.code == code));
        assert!(hir::resolve(&ast)
            .unwrap_err()
            .iter()
            .any(|diagnostic| diagnostic.code == code));
    }
    let ast=parse("module t; @id(\"t.read\") fn str_byte_at(text:borrow str,index:usize)->i64 { 0 } @id(\"app.main\") fn main()->i64 { 0 }", Path::new("reserved.spx")).unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-S113"));
    let source="module t; @id(\"t.read\") fn read(text:borrow str,index:usize)->i64 { let ignored = str_byte_at(text,index); 0 } @id(\"app.main\") fn main()->i64 { 0 }";
    let ast = parse(source, Path::new("hostile.spx")).unwrap();
    for forged in 0..3 {
        let mut program = hir::resolve(&ast).unwrap();
        let read = program
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "t.read")
            .unwrap();
        let hir::ResolvedExprKind::Block { statements, .. } = &mut read.body.kind else {
            panic!("block")
        };
        let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
            panic!("read result binding")
        };
        let hir::ResolvedExprKind::Call {
            args,
            type_arguments,
            ..
        } = &mut value.kind
        else {
            panic!("call")
        };
        match forged {
            0 => args[0].ownership = hir::OwnershipMode::Own,
            1 => args[1].ty = hir::ResolvedType::I64,
            _ => type_arguments.push(hir::ResolvedType::U8),
        }
        assert_eq!(hir::validate(&program).unwrap_err().code, "SPX-H006");
    }
}

#[test]
fn borrowed_text_byte_at_loop_index_is_checked() {
    let source="module t; @id(\"t.scan\") fn scan(text:borrow str)->i64 { let mut index=0usize; let mut total=0; while index<3usize { total=total+match str_byte_at(text,index) { Option::Some { value:byte }=>i64_from_u8(byte), Option::None {}=>0, }; index=index+1usize; 0 } total } @id(\"app.main\") fn main()->i64 { 0 }";
    let ast = parse(source, Path::new("loop.spx")).unwrap();
    assert!(verify::verify(&ast).is_empty());
    hir::validate(&hir::resolve(&ast).unwrap()).unwrap();
    wasm::emit_module(&ast).unwrap();
    let invalid = source.replace(
        "str_byte_at(text,index)",
        "str_byte_at(text,string_len(\"allocation\"))",
    );
    let ast = parse(&invalid, Path::new("invalid-loop.spx")).unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T205"));
}

#[test]
fn native_byte_reads_reuse_only_direct_immutable_checked_views() {
    use super::owned_string_loops_v1::support::Fixture;
    if !super::byte_widening::available("clang") {
        return;
    }
    let mutable = parse("module checked.mutable; @id(\"app.main\") fn main()->i64 { let text=\"é\"; let mut raw=string_as_str(text); 0 }", Path::new("mutable.spx")).unwrap();
    assert!(verify::verify(&mutable)
        .iter()
        .any(|d| d.code == "SPX-T264"));
    assert!(hir::resolve(&mutable)
        .unwrap_err()
        .iter()
        .any(|d| d.code == "SPX-T264"));
    let read = "match str_byte_at(raw, 0usize) { Option::Some { value: byte } => i64_from_u8(byte), Option::None {} => -1, }";
    for (label, setup, body, expected_checks, expected_value) in [
        ("direct", "let raw = string_as_str(text);".to_owned(), format!("let mut i = 0usize; let mut total = 0; while i < 100usize {{ total = total + {read}; i = i + 1usize; 0 }} total"), 2, 19500),
        ("alias", "let original = string_as_str(text); let raw = original;".to_owned(), format!("let first = {read}; first + {read}"), 4, 390),
    ] {
        let source = format!("module checked.byte; @id(\"app.main\") fn main()->i64 {{ let text = \"é\\u{{0}}\"; {setup} {body} }}");
        let ast = parse(&source, Path::new(label)).unwrap();
        assert!(verify::verify(&ast).is_empty(), "{label}: {:?}", verify::verify(&ast));
        let c = semaprax::codegen::emit_c(&ast).unwrap();
        let signature = "static __attribute__((unused)) void spx_str_require_valid(spx_str_v1 value) {";
        assert_eq!(c.matches(signature).count(), 1);
        // Observe the complete validator; its original body remains unchanged.
        let c = c.replacen(signature, &format!("static unsigned validation_calls;\n{signature}\n    ++validation_calls;"), 1);
        let probe = format!("{}\n#define REQUIRE(c) do {{ if (!(c)) return 91; }} while (0)\n{c}\nint main(void) {{ REQUIRE(fixture_binary_stdout()); struct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL)); int64_t value=-1; REQUIRE(spx_decl_6170702e6d61696e(&context,&value)==0); REQUIRE(value==INT64_C({expected_value})); REQUIRE(validation_calls=={expected_checks}); return 0; }}", include_str!("../support/native_fixture_stdio.c"));
        let mut fixture = Fixture::new(&source);
        for optimization in ["-O0", "-O2"] {
            assert_eq!(fixture.native(&probe, optimization), "", "{label}");
        }
        fixture.cleanup();
    }
}

#[test]
fn native_byte_reads_preserve_foreign_utf8_validation_at_every_read() {
    use super::owned_string_loops_v1::support::Fixture;
    use std::process::Command;
    if !super::byte_widening::available("clang") {
        return;
    }
    let ast = parse(DIRECT, Path::new("foreign-byte-read.spx")).unwrap();
    let c = semaprax::codegen::emit_c(&ast).unwrap();
    let symbol = "byte.read"
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    // Exercise the ordinary, unmodified generated validator on foreign views.
    for (label, bytes, len, index, success) in [
        ("valid-first", "0xc3,0xa9,0", "3", "0", true),
        ("valid-nul", "0xc3,0xa9,0", "3", "2", true),
        ("empty", "0", "0", "UINT64_MAX", true),
        ("overlong", "0xc0,0x80", "2", "0", false),
        ("truncated", "0xe2,0x82", "2", "0", false),
        ("surrogate", "0xed,0xa0,0x80", "3", "0", false),
        ("bad-continuation", "0xe2,0x28,0xa1", "3", "0", false),
        (
            "invalid-even-out-of-bounds",
            "0xff",
            "1",
            "UINT64_MAX",
            false,
        ),
        ("null-nonempty", "0", "1", "0", false),
    ] {
        let data = if label == "null-nonempty" {
            "NULL"
        } else {
            "bytes"
        };
        let expected = if label == "valid-first" {
            195
        } else if label == "valid-nul" {
            0
        } else {
            -1
        };
        let result_check = if success {
            format!("REQUIRE(status==0); REQUIRE(value==INT64_C({expected}));")
        } else {
            "(void)status;".to_owned()
        };
        let probe = format!("{}\n#define REQUIRE(c) do {{ if (!(c)) return 91; }} while (0)\n{c}\nint main(void) {{ REQUIRE(fixture_binary_stdout()); struct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL)); const uint8_t bytes[]={{ {bytes} }}; (void)bytes; int64_t value=-9; int status=spx_decl_{symbol}(&context,(spx_str_v1){{{data},{len}}},{index},&value); {result_check} return 0; }}", include_str!("../support/native_fixture_stdio.c"));
        let mut fixture = Fixture::new(DIRECT);
        let source = fixture.write("probe.c", probe);
        for optimization in ["-O0", "-O2"] {
            let name = format!(
                "foreign-{label}{optimization}{}",
                std::env::consts::EXE_SUFFIX
            );
            let exe = fixture.write(&name, "");
            if cfg!(windows) {
                for extension in ["lib", "exp", "pdb", "ilk"] {
                    fixture.write(&format!("foreign-{label}{optimization}.{extension}"), "");
                }
            }
            let compile = Command::new(std::env::var_os("CLANG").unwrap_or_else(|| "clang".into()))
                .args([
                    "-std=c11",
                    optimization,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_NO_ENTRY_WRAPPER",
                ])
                .arg(&source)
                .arg("-o")
                .arg(&exe)
                .output()
                .unwrap();
            assert!(
                compile.status.success(),
                "{label}: {}",
                String::from_utf8_lossy(&compile.stderr)
            );
            #[cfg(unix)]
            let result = Command::new("sh")
                .args(["-c", "ulimit -c 0; exec \"$@\"", "spx-byte-read-test"])
                .arg(&exe)
                .output()
                .unwrap();
            #[cfg(not(unix))]
            let result = Command::new(&exe).output().unwrap();
            assert_eq!(
                result.status.success(),
                success,
                "{label} {optimization}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            if !success {
                assert!(
                    String::from_utf8_lossy(&result.stderr).contains("borrowed str"),
                    "{label}: {:?}",
                    result
                );
            }
        }
        fixture.cleanup();
    }
}
