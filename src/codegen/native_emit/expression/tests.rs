use std::path::Path;

const EXACT_DEPTH: usize = 512;

#[derive(Clone, Copy)]
enum RecursiveFamily {
    Unary,
    LazyBinary,
    Call,
    Block,
    If,
}

fn assert_native_codegen(family: RecursiveFamily, label: &str) {
    let source = "module test.native_codegen_depth;\n\n@id(\"depth.identity\")\nfn identity(value: bool) -> bool { value }\n\n@id(\"depth.deep\")\nfn deep() -> bool { true }\n\n@id(\"app.main\")\nfn main() -> i64 { if deep() { 0 } else { 1 } }\n";
    let source_path = format!("native-codegen-{label}.spx");
    let mut parsed = crate::parse(source, Path::new(&source_path))
        .expect("native-codegen seed source must parse on the default stack");
    let function = parsed
        .functions
        .iter_mut()
        .find(|function| function.stable_id == "depth.deep")
        .expect("depth fixture function exists");
    for _ in 1..EXACT_DEPTH {
        let child = std::mem::replace(
            &mut function.body,
            crate::ast::Expr {
                kind: crate::ast::ExprKind::Bool(true),
                span: crate::ast::Span::default(),
            },
        );
        function.body = crate::ast::Expr {
            kind: match family {
                RecursiveFamily::Unary => crate::ast::ExprKind::Unary {
                    op: crate::ast::UnaryOp::Not,
                    value: Box::new(child),
                },
                RecursiveFamily::LazyBinary => crate::ast::ExprKind::Binary {
                    op: crate::ast::BinaryOp::And,
                    left: Box::new(crate::ast::Expr {
                        kind: crate::ast::ExprKind::Bool(true),
                        span: crate::ast::Span::default(),
                    }),
                    right: Box::new(child),
                },
                RecursiveFamily::Call => crate::ast::ExprKind::Call {
                    name: "identity".to_owned(),
                    type_arguments: Vec::new(),
                    args: vec![child],
                },
                RecursiveFamily::Block => crate::ast::ExprKind::Block {
                    statements: Vec::new(),
                    tail: Box::new(child),
                },
                RecursiveFamily::If => crate::ast::ExprKind::If {
                    condition: Box::new(crate::ast::Expr {
                        kind: crate::ast::ExprKind::Bool(true),
                        span: crate::ast::Span::default(),
                    }),
                    then_branch: Box::new(child),
                    else_branch: Box::new(crate::ast::Expr {
                        kind: crate::ast::ExprKind::Bool(false),
                        span: crate::ast::Span::default(),
                    }),
                },
            },
            span: crate::ast::Span::default(),
        };
    }
    let resolved = crate::hir::resolve(&parsed)
        .expect("exact-depth native-codegen AST must resolve on the default stack");
    let generated = crate::codegen::emit_hir_c(&resolved)
        .expect("exact-depth HIR must lower on the default 2 MiB test-thread stack");
    assert!(generated.contains("int main(void)"));
}

#[test]
fn native_codegen_handles_deep_unary_on_the_default_stack() {
    assert_native_codegen(RecursiveFamily::Unary, "unary");
}

#[test]
fn native_codegen_handles_deep_binary_on_the_default_stack() {
    assert_native_codegen(RecursiveFamily::LazyBinary, "binary");
}

#[test]
fn native_codegen_handles_deep_calls_on_the_default_stack() {
    assert_native_codegen(RecursiveFamily::Call, "call");
}

#[test]
fn native_codegen_handles_deep_blocks_on_the_default_stack() {
    assert_native_codegen(RecursiveFamily::Block, "block");
}

#[test]
fn native_codegen_handles_deep_if_on_the_default_stack() {
    assert_native_codegen(RecursiveFamily::If, "if");
}

#[test]
fn native_borrowed_bytes_calls_use_const_aliases_without_owned_staging() {
    let source = r#"
module test.native_borrowed_bytes;
@id("packet") record Packet {
@id("packet.payload") payload: Bytes,
@id("packet.marker") marker: i64,
}
@id("bytes.inspect")
fn inspect(value: borrow Bytes) -> usize {
byte_len(bytes_as_slice(value))
}
@id("bytes.consume")
fn consume(value: own Bytes) -> usize {
byte_len(bytes_as_slice(value))
}
@id("bytes.projected")
fn projected(data: borrow Slice<u8>) -> usize {
let packet = Packet { payload: bytes_copy(data), marker: 7 };
inspect(packet.payload)
}
@id("bytes.owned")
fn owned(data: borrow Slice<u8>) -> usize {
let value = bytes_copy(data);
consume(value)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let resolved =
        crate::hir::resolve(&crate::parse(source, Path::new("native-borrowed-bytes.spx")).unwrap())
            .unwrap();
    let generated = crate::codegen::emit_hir_c(&resolved).unwrap();
    let inspect = super::super::c_function_symbol(&crate::hir::DeclarationId::new("bytes.inspect"));
    let consume = super::super::c_function_symbol(&crate::hir::DeclarationId::new("bytes.consume"));

    let inspect_lines = generated
        .lines()
        .filter(|line| line.contains(&inspect))
        .collect::<Vec<_>>();
    assert!(
        generated.contains(&format!(
            "{inspect}(struct spx_context *spx_ctx, const spx_bytes_v1 *spx_param_0"
        )),
        "{inspect_lines:?}"
    );
    let borrow_call = generated
        .lines()
        .find(|line| line.contains("spx_status =") && line.contains(&inspect))
        .expect("projected borrowed call is emitted");
    assert!(borrow_call.contains("&("), "{borrow_call}");
    assert!(!borrow_call.contains("spx_bytes_move"), "{borrow_call}");

    let owned_call = generated
        .lines()
        .find(|line| line.contains("spx_status =") && line.contains(&consume))
        .expect("owned call is emitted");
    assert!(owned_call.contains("spx_bytes_move"), "{owned_call}");
}
