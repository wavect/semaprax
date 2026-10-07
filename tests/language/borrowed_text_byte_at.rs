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
        let source=format!("module t; @id(\"t.read\") fn read(text:borrow str)->Option<u8> {{ {body} }} @id(\"app.main\") fn main()->i64 {{ 0 }}");
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
    let source="module t; @id(\"t.read\") fn read(text:borrow str,index:usize)->Option<u8> { str_byte_at(text,index) } @id(\"app.main\") fn main()->i64 { 0 }";
    let ast = parse(source, Path::new("hostile.spx")).unwrap();
    for forged in 0..3 {
        let mut program = hir::resolve(&ast).unwrap();
        let read = program
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "t.read")
            .unwrap();
        let hir::ResolvedExprKind::Block { tail, .. } = &mut read.body.kind else {
            panic!("block")
        };
        let hir::ResolvedExprKind::Call {
            args,
            type_arguments,
            ..
        } = &mut tail.kind
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
