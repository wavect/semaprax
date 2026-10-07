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

#[test]
fn statement_if_measurement_accounts_for_erased_normalization_nodes() {
    let source="module t; fn main()->i64 { let mut x=0; if true { x=1; x==1 } else if false {x=2;} while x<3 {if true {x=x+1;} 0} x }";
    let program = crate::parse(source, "statement-if-measured.spx").unwrap();
    let body = &program.functions[0].body;
    let lengths = rendered_expr_lengths(body, 0);
    let mut max_depth = 0usize;
    let mut pending = vec![(body, 1usize)];
    while let Some((expression, depth)) = pending.pop() {
        max_depth = max_depth.max(depth);
        assert!(
            lengths
                .keys()
                .any(|(id, _)| *id == expression as *const Expr as usize),
            "unmeasured {:?}",
            expression.kind
        );
        match &expression.kind {
            ExprKind::Block { statements, tail } => {
                pending.push((tail.as_ref(), depth + 1));
                for statement in statements {
                    for index in 0..statement.child_count() {
                        pending.push((statement.child(index).unwrap(), depth + 1));
                    }
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => pending.extend(
                [
                    condition.as_ref(),
                    then_branch.as_ref(),
                    else_branch.as_ref(),
                ]
                .map(|child| (child, depth + 1)),
            ),
            ExprKind::Binary { left, right, .. } => {
                pending.extend([left.as_ref(), right.as_ref()].map(|child| (child, depth + 1)))
            }
            ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Var(_) => {}
            _ => panic!("unexpected fixture expression"),
        }
    }
    assert!(legacy_expr_temporary_bytes(body, 0) > 0);
    let canonical = crate::format::canonical(&program);
    let mut bounded = String::new();
    write_canonical_with_scratch(
        &program,
        &mut bounded,
        private_scratch_capacity(max_depth, 1, 1).unwrap(),
    );
    assert_eq!(bounded, canonical);
    let round = crate::parse(&canonical, "statement-if-measured.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&round));
}
