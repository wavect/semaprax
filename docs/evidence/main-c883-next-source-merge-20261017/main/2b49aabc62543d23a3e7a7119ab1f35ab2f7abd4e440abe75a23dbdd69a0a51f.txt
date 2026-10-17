//! Child-order witnesses compare borrowed pointers, not duplicated identities.
use super::*;
use crate::hir::{ResolvedMatchArm, ResolvedMatchPattern, ResolvedStatement};

fn function(source: &str) -> ResolvedFunction {
    crate::hir::resolve(&crate::check(source, "child-cursor.spx").unwrap())
        .unwrap()
        .functions
        .remove(0)
}
fn scalar_function() -> ResolvedFunction {
    function("module cursor.scalar; @id(\"main\") fn main()->i64 {let mut n=1;n=n+1;n}")
}

#[test]
fn unused_contours_acquire_no_backing_and_forged_bottom_still_replays() {
    let mut program = crate::hir::resolve(
        &crate::check(
            "module cursor.scalar; @id(\"main\") fn main()->i64 {0}",
            "contour-selection.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let function = &program.functions[0];
    let owner = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
    let ((result, metrics), refused, used) = crate::bounded_output::with_limit_usage(0, || {
        crate::test_allocation_observer::measure(|| {
            crate::hir::lexical_exit::validate_function(&program, function, &owner)
        })
    });
    result.unwrap();
    assert!(!refused && used == 0 && !metrics.invalid);
    assert_eq!(
        (metrics.allocations, metrics.allocated, metrics.live),
        (0, 0, 0)
    );

    // A type marker cannot authenticate itself as an absent lexical profile.
    program.functions[0].body.ty = crate::hir::ResolvedType::Never;
    let function = &program.functions[0];
    assert!(!requires(function).unwrap());
    assert!(requires_validation(function).unwrap());
    let error =
        crate::hir::lexical_exit::validate_function(&program, function, &owner).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "lexical completion type differs from independently replayed reachability"
    );
}
fn assert_order(expression: &ResolvedExpr) {
    let mut cursor = ChildCursor::new(expression);
    assert!(std::ptr::eq(cursor.expression(), expression));
    assert!(!cursor.is_started());
    let mut index = 0;
    while let Some(expected) = child(expression, index) {
        let actual = cursor.next().expect("every original child retained");
        assert!(std::ptr::eq(actual, expected), "child {index} order drift");
        assert!(cursor.is_started());
        index += 1;
    }
    assert!(cursor.next().is_none());
    assert!(cursor.next().is_none(), "cursor remains exhausted");
}

#[test]
fn checked_mixed_blocks_and_exit_operands_preserve_original_child_order() {
    let function = function(
        r#"module cursor.mixed;
@id("cursor.probe") fn probe(n:i64)->i64 {
 let mut i=0;
 while i<n {i=i+1;if i==2 {continue;}else{0} if i==3 {break;}else{0} 0}
 if i>0 {return i+1;}else{0}
 return 0;
}
@id("main") fn main()->i64 {probe(4)}
"#,
    );
    assert!(requires(&function).unwrap());
    let mut pending = vec![&function.body];
    let mut exits = 0;
    while let Some(expression) = pending.pop() {
        assert_order(expression);
        if matches!(&expression.kind, K::LexicalExit { .. }) {
            exits += 1;
        }
        pending.extend(ChildCursor::new(expression));
    }
    assert_eq!(exits, 4);
}

#[test]
fn wide_block_cursor_advances_each_statement_and_retains_all_statement_children() {
    let function = scalar_function();
    let K::Block { statements, tail } = &function.body.kind else {
        panic!("block");
    };
    let mut block = function.body.clone();
    let condition = tail.clone();
    let kinds = [
        statements[0].clone(),
        statements[1].clone(),
        ResolvedStatement::Unsafe {
            audit: "cursor shape".into(),
            body: tail.clone(),
            span: tail.span,
        },
        ResolvedStatement::While {
            condition: condition.clone(),
            body: tail.clone(),
            span: tail.span,
        },
        ResolvedStatement::LexicalWhile {
            condition,
            body: tail.clone(),
            latch: crate::hir::lexical_exit::LoopLatchKind::Condition,
            span: tail.span,
        },
    ];
    block.kind = K::Block {
        statements: (0..256).flat_map(|_| kinds.iter().cloned()).collect(),
        tail: tail.clone(),
    };
    assert_order(&block);
    let K::Block { statements, .. } = &block.kind else {
        unreachable!()
    };
    let mut cursor = ChildCursor::new(&block);
    assert!(cursor.last_statement_child().is_none());
    for (statement_index, statement) in statements.iter().enumerate() {
        for index in 0..statement.child_count() {
            let actual = cursor.next().unwrap();
            assert!(std::ptr::eq(actual, statement.child(index).unwrap()));
            let (owner, edge) = cursor.last_statement_child().unwrap();
            assert!(std::ptr::eq(owner, statement));
            assert_eq!(edge, index);
            let Position::Block { statement, child } = cursor.position else {
                panic!("block position");
            };
            assert_eq!(statement, statement_index);
            assert_eq!(child, index + 1);
        }
    }
    assert!(
        cursor.next().is_some(),
        "block tail follows every statement"
    );
    assert!(cursor.last_statement_child().is_none());
    assert!(cursor.next().is_none());
    assert!(cursor.last_statement_child().is_none());
}

#[test]
fn wide_guarded_match_retains_scrutinee_guard_body_and_exhaustion_order() {
    let function = scalar_function();
    let K::Block { tail, .. } = &function.body.kind else {
        panic!("tail");
    };
    let mut expression = function.body.clone();
    let arms = (0..1024)
        .map(|index| ResolvedMatchArm {
            pattern: ResolvedMatchPattern::Wildcard,
            guard: (index % 3 != 0).then(|| tail.clone()),
            value: tail.as_ref().clone(),
            span: tail.span,
        })
        .collect();
    // This is a structural enumeration witness, not a forged HIR admission.
    expression.kind = K::Match {
        scrutinee: tail.clone(),
        arms,
        mode: crate::hir::ResolvedMatchMode::Value,
    };
    assert_order(&expression);
}

#[test]
fn cursor_scans_preserve_depth_refusal_and_fail_closed_contains() {
    let mut function = scalar_function();
    let K::Block { tail, .. } = &function.body.kind else {
        panic!("tail");
    };
    let seed = tail.as_ref().clone();
    let mut nested = seed.clone();
    for _ in 0..crate::cache_codec::MAX_DEPTH + 2 {
        let mut parent = seed.clone();
        parent.kind = K::Unary {
            op: crate::ast::UnaryOp::Neg,
            value: Box::new(nested),
        };
        nested = parent;
    }
    function.body = nested;
    assert!(!requires(&function).unwrap());
    assert!(!contains(&function.body));
    // Keep the original forged 258-node subject: allowing a presence scan to
    // finish grants no HIR authority to its duplicated expression identities.
    let mut forged = crate::hir::resolve(
        &crate::check(
            "module cursor.scalar; @id(\"main\") fn main()->i64 {let mut n=1;n=n+1;n}",
            "child-cursor.spx",
        )
        .unwrap(),
    )
    .unwrap();
    forged.functions[0].body = std::mem::replace(&mut function.body, seed.clone());
    assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
    let mut nested = seed.clone();
    for _ in 0..PRESENCE_DEPTH + 2 {
        let mut parent = seed.clone();
        parent.kind = K::Unary {
            op: crate::ast::UnaryOp::Neg,
            value: Box::new(nested),
        };
        nested = parent;
    }
    function.body = nested;
    assert!(
        requires(&function).is_err(),
        "forged excessive depth remains refused"
    );
    assert!(
        contains(&function.body),
        "unchecked depth remains fail closed"
    );
    let plain = scalar_function();
    assert!(!requires(&plain).unwrap());
    assert!(!contains(&plain.body));
}
