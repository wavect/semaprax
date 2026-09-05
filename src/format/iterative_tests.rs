use super::*;
use std::path::Path;

#[test]
fn expression_blocks_and_empty_match_keep_exact_separators() {
    let block = crate::parse(
        "module t; fn main()->i64 { { let x = 1; let y = 2; x + y } }",
        Path::new("format-block.spx"),
    )
    .unwrap();
    let ExprKind::Block { tail, .. } = &block.functions[0].body.kind else {
        unreachable!()
    };
    assert_eq!(expr(tail, 0), "{ let x = 1; let y = 2; x + y }");

    let empty = crate::parse(
        "module t; fn main(value:i64)->i64 { match value { } }",
        Path::new("format-empty-match.spx"),
    )
    .unwrap();
    let ExprKind::Block { tail, .. } = &empty.functions[0].body.kind else {
        unreachable!()
    };
    assert_eq!(expr(tail, 0), "match value {  }");
}

#[test]
fn measured_render_records_each_subtree_once() {
    let mut sum = String::from("value");
    for _ in 1..64 {
        sum.push_str(" + value");
    }
    let source = format!("module t; fn main(value: i64) -> i64 {{ {sum} }}");
    let program = crate::parse(&source, Path::new("format-measured.spx")).unwrap();
    let ExprKind::Block { tail, .. } = &program.functions[0].body.kind else {
        unreachable!()
    };
    let lengths = rendered_expr_lengths(tail, 0);
    // The left-associated sum has 64 identifiers and 63 binary nodes.
    // Keep this assertion independent of crate-private AST traversal so the
    // formatter's shared-source consumer exercises the same regression.
    assert_eq!(lengths.len(), 127);
    assert_eq!(
        lengths[&(tail.as_ref() as *const Expr as usize, 0)],
        sum.len()
    );
}

#[test]
fn unsafe_statement_in_inline_block_stays_parseable() {
    // The grammar terminates an unsafe boundary statement at its block;
    // the enclosing inline block's tail expression follows directly.
    let source = r#"
module t;
permit { unsafe }
fn main(value:i64)->i64 {
    { @audit("checked boundary") unsafe { value } value + 1 }
}
"#;
    let program = crate::parse(source, Path::new("format-unsafe.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    // The canonical text must re-parse: the unsafe statement is not
    // semicolon-terminated by the grammar.
    let reparsed = crate::parse(&canonical, Path::new("format-unsafe-2.spx"))
        .unwrap_or_else(|error| panic!("canonical text must re-parse: {error}\n{canonical}"));
    assert_eq!(
        canonical,
        crate::format::canonical(&reparsed),
        "canonical form must be idempotent"
    );
    assert!(
        canonical.contains("@audit(\"checked boundary\") unsafe { value } value + 1"),
        "{canonical}"
    );
}
