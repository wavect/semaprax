use super::*;
use crate::ast::vec_constructor;
use crate::ast::{
    ClosureParam, Function, MatchArm, MatchMode, MatchPattern, Param, RecordMatchFieldPattern,
    RecordMatchPatternField, Span, Type, UnaryOp,
};

// Compile the complete original receiver, including every pattern/name route.
#[allow(dead_code)]
#[path = "../../../docs/evidence/format-hygiene-scratch-v1/expression_statement.before.rs.txt"]
mod before_heap;

fn leaf() -> Expr {
    Expr {
        kind: ExprKind::Int(1),
        span: Span {
            end: 1,
            ..Span::default()
        },
    }
}
fn block(statements: Vec<Statement>, tail: Expr) -> Expr {
    Expr {
        kind: ExprKind::Block {
            statements,
            tail: Box::new(tail),
        },
        span: Span::default(),
    }
}
fn marker() -> Statement {
    let value = leaf();
    Statement::Let {
        syntax: LetSyntax::ExpressionStatement,
        name: "_if1".into(),
        name_span: value.span,
        mutable: false,
        declared: None,
        span: Span {
            end: 2,
            ..value.span
        },
        value,
    }
}
fn compare(root: &Expr, bindings: &[&str], expected: bool) {
    assert_eq!(
        projection_with_bindings(root, bindings.iter().copied(), |_| true),
        expected
    );
    assert_eq!(
        before_heap::projection_with_bindings(root, bindings.iter().copied(), |_| true),
        expected
    );
}
#[test]
fn complete_old_hygiene_verdicts_cover_markers_readers_bindings_and_patterns() {
    let valid = block(vec![marker()], leaf());
    compare(&valid, &[], true);
    compare(&valid, &["_if1"], false);
    compare(&valid, &["ordinary"], true);
    compare(&block(vec![marker(), marker()], leaf()), &[], false);
    let mut reader = leaf();
    reader.kind = ExprKind::Var("_if1".into());
    compare(&block(vec![marker()], reader.clone()), &[], false);
    for mode in 0..4 {
        let mut forged = marker();
        if let Statement::Let {
            mutable,
            declared,
            name_span,
            syntax,
            ..
        } = &mut forged
        {
            match mode {
                0 => *mutable = true,
                1 => *declared = Some(Type::I64),
                2 => name_span.end += 1,
                _ => *syntax = LetSyntax::Authored,
            }
        }
        compare(&block(vec![forged, marker()], leaf()), &[], false);
    }
    let mut closure = leaf();
    closure.kind = ExprKind::Closure {
        params: vec![ClosureParam {
            name: "_if1".into(),
            ty: Type::I64,
            span: Span::default(),
        }],
        return_type: Type::I64,
        body: Box::new(leaf()),
        owning: false,
        retained: false,
        mutable: false,
    };
    compare(&block(vec![marker()], closure), &[], false);
    let mut call = leaf();
    call.kind = ExprKind::Call {
        name: "_if1".into(),
        type_arguments: vec![],
        args: vec![],
    };
    compare(&block(vec![marker()], call), &[], false);
    for pattern in [
        MatchPattern::Binding {
            name: "_if1".into(),
            span: Span::default(),
        },
        MatchPattern::Or {
            alternatives: vec![MatchPattern::Binding {
                name: "_if1".into(),
                span: Span::default(),
            }],
            span: Span::default(),
        },
        MatchPattern::Record {
            type_name: "R".into(),
            type_span: Span::default(),
            fields: vec![RecordMatchPatternField {
                name: "x".into(),
                name_span: Span::default(),
                span: Span::default(),
                pattern: RecordMatchFieldPattern::Binding {
                    name: "_if1".into(),
                    span: Span::default(),
                },
            }],
            span: Span::default(),
        },
    ] {
        let mut matched = leaf();
        matched.kind = ExprKind::Match {
            mode: MatchMode::Value,
            scrutinee: Box::new(leaf()),
            arms: vec![MatchArm {
                pattern,
                guard: Some(Box::new(leaf())),
                value: leaf(),
                span: Span::default(),
            }],
        };
        compare(&block(vec![marker()], matched), &[], false);
    }
    // All roots share the same borrowed-name census, including later contracts.
    let roots = [&valid, &reader];
    assert!(!projection_roots(
        roots.into_iter(),
        std::iter::empty(),
        |_| true
    ));
    assert!(!before_heap::projection_roots(
        roots.into_iter(),
        std::iter::empty(),
        |_| true
    ));
}
#[test]
fn exact_original_expression_depth_remains_513() {
    let mut root = leaf();
    for depth in 1..=514 {
        compare(&root, &[], depth <= 513);
        root = Expr {
            kind: ExprKind::Unary {
                op: UnaryOp::Neg,
                value: Box::new(root),
            },
            span: Span::default(),
        };
    }
}
#[test]
fn every_replacement_is_prepaid_and_short_refusal_leaves_old_backing_intact() {
    type Frame = (&'static Expr, usize, usize);
    let root = leaf();
    let mut frames = Vec::<Option<(&Expr, usize, usize)>>::new();
    for needed in [1, 3, 5, 9, 17, 33, 65, 129, 257, 513] {
        if !frames.is_empty() {
            assert_eq!(frames[0].unwrap().1, 7);
        }
        let old_pointer = frames.as_ptr();
        let old_length = frames.len();
        let mut bill = 0;
        let (_, denied) = crate::test_allocation_observer::measure(|| {
            assert!(!grow_frames(&mut frames, needed, &mut |bytes| {
                bill = bytes;
                false
            }));
        });
        assert_eq!(frames.as_ptr(), old_pointer);
        assert_eq!(frames.len(), old_length);
        assert_eq!(denied.allocated, 0);
        assert!(!denied.invalid);
        let capacity = old_length.max(1).saturating_mul(2).min(513).max(needed);
        let control =
            2 * std::mem::size_of::<Vec<Option<Frame>>>() + std::mem::size_of::<Option<Frame>>();
        let backing = std::alloc::Layout::array::<Option<Frame>>(capacity)
            .unwrap()
            .size();
        assert_eq!(bill, control + backing);
        let (_, short) = crate::test_allocation_observer::measure(|| {
            assert!(!grow_frames(&mut frames, needed, &mut |bytes| bytes < bill));
        });
        assert_eq!(short.allocated, 0);
        assert!(!short.invalid);
        assert_eq!(frames.as_ptr(), old_pointer);
        assert_eq!(frames.len(), old_length);
        let ((), measured) = crate::test_allocation_observer::measure(|| {
            assert!(grow_frames(&mut frames, needed, &mut |bytes| bytes <= bill));
        });
        assert!(!measured.invalid);
        assert_eq!(measured.allocated, backing);
        frames[0] = Some((&root, 7, 11));
        assert_eq!(frames[0].unwrap().1, 7);
    }
    assert!(!grow_frames(&mut frames, 514, &mut |_| panic!(
        "depth cannot request backing"
    )));
    drop(frames);
    let (_, release) = crate::test_allocation_observer::measure(|| {
        let mut frames = Vec::<Option<(&Expr, usize, usize)>>::new();
        for needed in [1, 3, 5, 9, 17, 33, 65, 129, 257, 513] {
            assert!(grow_frames(&mut frames, needed, &mut |_| true));
        }
        drop(frames);
    });
    assert_eq!(release.live, 0);
    assert_eq!(release.allocated, release.freed);
    assert!(!release.invalid);
}
#[test]
fn live_payer_refusal_stays_sticky_and_releases_every_projection_allocation() {
    let root = block(vec![marker()], leaf());
    let ((accepted, metrics), refused) = crate::bounded_output::with_limit(0, || {
        crate::test_allocation_observer::measure(|| {
            projection_is_hygienic(&root, crate::bounded_output::reserve_active_required)
        })
    });
    assert!(!accepted);
    assert!(refused);
    assert_eq!(metrics.allocated, 0);
    assert!(!metrics.invalid);
    let (_, metrics) = crate::test_allocation_observer::measure(|| compare(&root, &[], true));
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    let mut exact = 0usize;
    assert!(projection_is_hygienic(&root, |bytes| {
        exact += bytes;
        true
    }));
    let (accepted, refused, used) = crate::bounded_output::with_limit_usage(exact, || {
        projection_is_hygienic(&root, crate::bounded_output::reserve_active_required)
    });
    assert!(accepted && !refused);
    assert_eq!(used, exact);
    let (accepted, refused, _) = crate::bounded_output::with_limit_usage(exact - 1, || {
        projection_is_hygienic(&root, crate::bounded_output::reserve_active_required)
    });
    assert!(!accepted && refused);
}
#[test]
fn thirty_two_scalar_functions_fit_the_unchanged_canonical_source_budget() {
    let functions = (0..32)
        .map(|index| Function {
            stable_id: format!("app.main.f{index:02}"),
            explicit_id: true,
            name: format!("f{index:02}"),
            name_span: Span::default(),
            type_parameters: vec![],
            params: Vec::<Param>::new(),
            return_type: Type::I64,
            effects: vec![],
            yields: None,
            follows: None,
            requires: vec![],
            ensures: vec![],
            body: block(vec![], leaf()),
            span: Span::default(),
        })
        .collect();
    let program = crate::ast::Program {
        path: "format-hygiene.spx".into(),
        module: "app.main".into(),
        module_uses: vec![],
        permits: vec![],
        types: vec![],
        interfaces: vec![],
        protocols: vec![],
        implementations: vec![],
        session_protocols: vec![],
        agents: vec![],
        functions,
    };
    let expected = crate::format::canonical(&program);
    let (actual, refused, used) =
        crate::bounded_output::with_limit_usage(1_048_576, || crate::format::canonical(&program));
    assert!(!refused, "canonical scratch used {used}");
    assert_eq!(actual, expected);
    assert!(actual.len() < 10_000);
    assert!(used < 1_048_576);
}

#[test]
fn complete_original_source_and_declared_storage_change_are_bound() {
    use sha2::{Digest, Sha256};
    let original = include_str!(
        "../../../docs/evidence/format-hygiene-scratch-v1/expression_statement.before.rs.txt"
    );
    let digest = |s: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes()))
        )
    };
    assert_eq!(
        digest(original),
        "cefdc29bb8580857812046848522f3644eb4f132ab4f0c799045076d65f7cf18"
    );
    let current = include_str!("../expression_statement.rs");
    let start = current.find("// Grow only physical scratch;").unwrap();
    let end = current.find("fn pattern_names").unwrap();
    assert_eq!(
        digest(&current[start..end]),
        "51465710d1df111f979fa3055fb3712f74070e87b35e82586b00ab035bb3a7c2"
    );
    let mut projected = current.to_owned();
    projected.replace_range(start..end, "");
    let substitutions = [
        (
            "    let mut stack = Vec::new();\n    if !grow_frames(&mut stack, 1, &mut reserve) {",
            "    if !reserve(std::mem::size_of::<[Option<(&Expr, usize, usize)>; 513]>()) {",
        ),
        (
            "    let mut patterns = [None; 129];\n",
            "    let mut patterns = [None; 129];\n    let mut stack = [None; 513];\n",
        ),
        (
            "if len == 513 || !grow_frames(&mut stack, len + 1, &mut reserve) {",
            "if len == stack.len() {",
        ),
        ("\n#[cfg(test)]\nmod tests;\n", ""),
    ];
    for (new, old) in substitutions {
        assert_eq!(projected.matches(new).count(), 1);
        projected = projected.replacen(new, old, 1);
    }
    assert_eq!(projected, original);
}
