use super::*;
use crate::test_allocation_observer::measure;
const LIMIT: usize = 64 * 1024 * 1024;
fn leaf() -> ResolvedExpr {
    let ast = crate::check(
        "module scan; @id(\"main\") fn main()->i64{0}",
        "unsafe-scan.spx",
    )
    .unwrap();
    crate::hir::resolve(&ast).unwrap().functions.remove(0).body
}
fn unsafe_node(leaf: &ResolvedExpr) -> ResolvedExpr {
    let mut expression = leaf.clone();
    expression.kind = ResolvedExprKind::Block {
        statements: vec![ResolvedStatement::Unsafe {
            audit: "scan".to_owned(),
            body: Box::new(leaf.clone()),
            span: leaf.span,
        }],
        tail: Box::new(leaf.clone()),
    };
    expression
}
#[test]
fn full_unsafe_receiver_keeps_capture_only_closures_and_exact_child_order() {
    let leaf = leaf();
    let boundary = unsafe_node(&leaf);
    for body in [&leaf, &boundary] {
        let mut closure = leaf.clone();
        closure.kind = ResolvedExprKind::Closure {
            parameters: Vec::new(),
            captures: Vec::new(),
            body: Box::new(body.clone()),
        };
        assert!(!super::super::contains_unsafe_boundary(&closure));
        assert!(!contains_unsafe_boundary_paid(&closure).unwrap());
    }
    let capture = ResolvedClosureCapture {
        binding: ResolvedBinding {
            id: ValueId::new("capture".to_owned()),
            name: "capture".to_owned(),
            ownership: OwnershipMode::Value,
            ty: ResolvedType::I64,
            span: leaf.span,
        },
        value: boundary.clone(),
    };
    let mut closure = leaf.clone();
    closure.kind = ResolvedExprKind::Closure {
        parameters: Vec::new(),
        captures: vec![capture],
        body: Box::new(leaf.clone()),
    };
    assert!(super::super::contains_unsafe_boundary(&closure));
    assert!(contains_unsafe_boundary_paid(&closure).unwrap());
    let mut wide = leaf.clone();
    wide.kind = ResolvedExprKind::Call {
        callee: DeclarationId::new("wide"),
        type_arguments: Vec::new(),
        instance: None,
        args: vec![leaf.clone(); 65],
    };
    let binding = ResolvedBinding {
        id: ValueId::new("capture.order".to_owned()),
        name: "capture.order".to_owned(),
        ownership: OwnershipMode::Value,
        ty: ResolvedType::I64,
        span: leaf.span,
    };
    for unsafe_first in [false, true] {
        let values = if unsafe_first {
            vec![boundary.clone(), wide.clone()]
        } else {
            vec![wide.clone(), boundary.clone()]
        };
        let mut expression = leaf.clone();
        expression.kind = ResolvedExprKind::Closure {
            parameters: Vec::new(),
            captures: values
                .into_iter()
                .map(|value| ResolvedClosureCapture {
                    binding: binding.clone(),
                    value,
                })
                .collect(),
            body: Box::new(boundary.clone()),
        };
        assert!(super::super::contains_unsafe_boundary(&expression));
        let (current, refused, _) = crate::bounded_output::with_limit_usage(0, || {
            contains_unsafe_boundary_paid(&expression)
        });
        if unsafe_first {
            assert!(current.is_err() && refused);
        } else {
            assert!(current.unwrap() && !refused);
        }
    }
    let mut guarded = leaf.clone();
    guarded.kind = ResolvedExprKind::Match {
        mode: ResolvedMatchMode::Value,
        scrutinee: Box::new(leaf.clone()),
        arms: vec![ResolvedMatchArm {
            pattern: ResolvedMatchPattern::Wildcard,
            guard: Some(Box::new(boundary.clone())),
            value: leaf.clone(),
            span: leaf.span,
        }],
    };
    assert!(!super::super::contains_unsafe_boundary(&guarded));
    assert!(!contains_unsafe_boundary_paid(&guarded).unwrap());
    // The authored child route still sees the same guard; the unsafe route's
    // independent historical exclusion is not generalized across both scans.
    let mut authored = Vec::new();
    crate::hir::ordinary_scan::children(&guarded, &mut |child| {
        authored.push(child);
        Ok(())
    })
    .unwrap();
    assert!(authored.contains(&&boundary));
    for count in [0, 1, 16, 17, 65] {
        for hit in [false, true] {
            let mut expression = leaf.clone();
            let mut args = vec![leaf.clone(); count];
            if hit {
                args.push(boundary.clone());
            }
            expression.kind = ResolvedExprKind::Call {
                callee: DeclarationId::new("scan-only"),
                type_arguments: Vec::new(),
                instance: None,
                args,
            };
            let old = super::super::contains_unsafe_boundary(&expression);
            let ((current, metrics), refused, paid) =
                crate::bounded_output::with_limit_usage(LIMIT, || {
                    measure(|| contains_unsafe_boundary_paid(&expression))
                });
            assert_eq!(current.unwrap(), old);
            assert!(!refused && !metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
            assert!(metrics.allocated <= paid && metrics.highwater <= paid);
            if paid > 0 {
                let (current, refused, exact) =
                    crate::bounded_output::with_limit_usage(paid, || {
                        contains_unsafe_boundary_paid(&expression)
                    });
                assert_eq!(current.unwrap(), old);
                assert!(!refused);
                assert_eq!(exact, paid);
                let (current, refused, _) =
                    crate::bounded_output::with_limit_usage(paid - 1, || {
                        contains_unsafe_boundary_paid(&expression)
                    });
                assert!(current.is_err() && refused);
            }
        }
    }
}
