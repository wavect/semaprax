//! Source-side census proofs: iterative depth admission, formatter
//! frames, and declaration DAG expansion.

use super::*;

#[test]
fn deeply_forged_hir_fails_iteratively_without_stack_growth() {
    let (program, _) = fixture();
    let mut resolved = hir::resolve(&program).unwrap();
    let function_index = resolved
        .functions
        .iter()
        .position(|function| function.id.as_str() == "interop.add")
        .unwrap();
    let mut expression = resolved.functions[function_index].body.clone();
    for _ in 0..MAX_SEMANTIC_EXPRESSION_DEPTH {
        let id = expression.id.clone();
        let ty = expression.ty.clone();
        let ownership = expression.ownership;
        let span = expression.span;
        expression = ResolvedExpr {
            id,
            ty,
            ownership,
            kind: ResolvedExprKind::Unary {
                op: crate::ast::UnaryOp::Neg,
                value: Box::new(expression),
            },
            span,
        };
    }
    resolved.functions[function_index].body = expression;
    let capacity = MAX_SEMANTIC_EXPRESSION_DEPTH * 4 + 32;
    let owner = ResolvedProgramOwner::new(resolved, Vec::with_capacity(capacity), capacity);
    let error = validate_native_rust_expression_budget(owner.program()).unwrap_err();
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(
        error.message,
        "Native Rust Interop max_semantic_expression_depth exceeds 512"
    );
    drop(owner);
}

#[test]
fn semantic_expression_depth_512_is_exact_for_source_and_hir() {
    fn wrap_source(mut expression: crate::ast::Expr, count: usize) -> crate::ast::Expr {
        for _ in 0..count {
            let span = expression.span;
            expression = crate::ast::Expr {
                kind: crate::ast::ExprKind::Unary {
                    op: crate::ast::UnaryOp::Neg,
                    value: Box::new(expression),
                },
                span,
            };
        }
        expression
    }

    fn wrap_hir(mut expression: ResolvedExpr, count: usize) -> ResolvedExpr {
        for _ in 0..count {
            expression = ResolvedExpr {
                id: expression.id.clone(),
                ty: expression.ty.clone(),
                ownership: expression.ownership,
                span: expression.span,
                kind: ResolvedExprKind::Unary {
                    op: crate::ast::UnaryOp::Neg,
                    value: Box::new(expression),
                },
            };
        }
        expression
    }

    // The fixture body has depth four: block -> addition -> import call -> argument.
    const EXACT_WRAPPERS: usize = MAX_SEMANTIC_EXPRESSION_DEPTH - 4;
    let (program, _) = fixture();
    let mut exact_source = program.clone();
    let function = exact_source
        .functions
        .iter_mut()
        .find(|function| function.stable_id == "interop.add")
        .unwrap();
    function.body = wrap_source(function.body.clone(), EXACT_WRAPPERS);
    validate_native_rust_source_expression_budget(&exact_source).unwrap();
    let canonical_exact = canonical_source_bounded(&exact_source).unwrap();
    let mut hir_scan_stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let hir_upper =
        hir_pre_resolve_capacity(&exact_source, canonical_exact.len(), &mut hir_scan_stack)
            .unwrap();
    assert!(hir_upper.complete().unwrap() >= canonical_exact.len());
    let resolved_exact_source = hir::resolve(&exact_source).unwrap();
    validate_native_rust_expression_budget(&resolved_exact_source).unwrap();
    assert_resolved_owner_disposes_once_without_growth(
        resolved_exact_source,
        hir_upper.disposal_frames,
    );

    // Keep the HIR boundary independent from source-depth admission: forge
    // exact and over-limit HIR from the shallow fixture, moving each tree
    // into its wrapper so neither construction nor replacement drops it
    // recursively.
    let mut exact_hir = hir::resolve(&program).unwrap();
    let function = exact_hir
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "interop.add")
        .unwrap();
    let placeholder = ResolvedExpr {
        id: function.body.id.clone(),
        ty: function.body.ty.clone(),
        ownership: function.body.ownership,
        kind: ResolvedExprKind::Int(0),
        span: function.body.span,
    };
    let body = std::mem::replace(&mut function.body, placeholder);
    function.body = wrap_hir(body, EXACT_WRAPPERS);
    validate_native_rust_expression_budget(&exact_hir).unwrap();
    assert_resolved_owner_disposes_once_without_growth(exact_hir, hir_upper.disposal_frames);

    let mut over_hir = hir::resolve(&program).unwrap();
    let function = over_hir
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "interop.add")
        .unwrap();
    let placeholder = ResolvedExpr {
        id: function.body.id.clone(),
        ty: function.body.ty.clone(),
        ownership: function.body.ownership,
        kind: ResolvedExprKind::Int(0),
        span: function.body.span,
    };
    let body = std::mem::replace(&mut function.body, placeholder);
    function.body = wrap_hir(body, EXACT_WRAPPERS + 1);
    let over_hir_disposal_frames = hir_upper.disposal_frames.checked_add(4).unwrap();

    let error = validate_native_rust_expression_budget(&over_hir).unwrap_err();
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(
        error.message,
        "Native Rust Interop max_semantic_expression_depth exceeds 512"
    );
    assert_resolved_owner_disposes_once_without_growth(over_hir, over_hir_disposal_frames);

    let mut over_source = exact_source;
    let function = over_source
        .functions
        .iter_mut()
        .find(|function| function.stable_id == "interop.add")
        .unwrap();
    let body = std::mem::replace(
        &mut function.body,
        crate::ast::Expr {
            kind: crate::ast::ExprKind::Int(0),
            span: crate::ast::Span::default(),
        },
    );
    function.body = wrap_source(body, 1);
    let error = validate_native_rust_source_expression_budget(&over_source).unwrap_err();
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(
        error.message,
        "Native Rust Interop max_semantic_expression_depth exceeds 512"
    );
}

#[test]
fn canonical_formatter_census_admits_shallow_wide_types_and_patterns() {
    const WIDTH: usize = 128;
    #[allow(clippy::format_collect)]
    let fields = (0..WIDTH)
        .map(|index| format!("    @id(\"wide.record.f{index:03}\")\n    f{index:03}: i64,\n"))
        .collect::<String>();
    let pattern = (0..WIDTH)
        .map(|index| format!("f{index:03}: _"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        "module formatter.wide;\n\n@id(\"wide.record\")\nrecord Wide {{\n{fields}}}\n\n@id(\"wide.read\")\nfn read(value: Wide) -> i64\n{{\n    match value {{\n        Wide {{ {pattern} }} => 0,\n    }}\n}}\n\n@id(\"app.main\")\nfn main() -> i64\n{{\n    0\n}}\n"
    );
    let program = crate::parse(&source, Path::new("formatter-shallow-wide.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let scratch = canonical_format_scratch_capacity(&program).unwrap();
    assert_eq!(
        scratch.bytes(),
        crate::private_format::private_scratch_capacity(3, 1, 1)
            .unwrap()
            .bytes(),
        "width must not be mistaken for recursive formatter depth"
    );
    let exact_peak = scratch.bytes().checked_add(canonical.len()).unwrap();
    let (bounded, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(exact_peak, || canonical_source_bounded(&program));
    let bounded = bounded.unwrap();
    assert!(!overflowed);
    assert_eq!(bounded, canonical);
    assert_eq!(consumed, canonical.len());

    let mut deep_program = crate::parse(
        "module formatter.deep; @id(\"app.main\") fn main() -> i64 { 0 }",
        Path::new("formatter-deep-types.spx"),
    )
    .unwrap();
    let mut deep_type = crate::ast::Type::I64;
    for index in 0..32 {
        deep_type = crate::ast::Type::Named {
            name: format!("T{index}"),
            arguments: vec![deep_type],
        };
    }
    let call = |index| crate::ast::Expr {
        kind: crate::ast::ExprKind::Call {
            name: format!("callee_{index}"),
            type_arguments: vec![deep_type.clone()],
            args: vec![],
        },
        span: crate::ast::Span::default(),
    };
    deep_program.functions[0].body = crate::ast::Expr {
        kind: crate::ast::ExprKind::Block {
            statements: (0..64)
                .map(|index| crate::ast::Statement::Let {
                    name: format!("value_{index}"),
                    name_span: crate::ast::Span::default(),
                    mutable: false,
                    declared: None,
                    value: call(index),
                    span: crate::ast::Span::default(),
                })
                .collect(),
            tail: Box::new(call(64)),
        },
        span: crate::ast::Span::default(),
    };
    let canonical = crate::format::canonical(&deep_program);
    let scratch = canonical_format_scratch_capacity(&deep_program).unwrap();
    assert_eq!(
        scratch.bytes(),
        crate::private_format::private_scratch_capacity(2, 33, 1)
            .unwrap()
            .bytes(),
        "statement and type width must not inflate nesting, but embedded type depth must count"
    );
    let exact_peak = scratch.bytes().checked_add(canonical.len()).unwrap();
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(0));
    let (bounded, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(exact_peak, || {
            canonical_source_bounded(&deep_program)
        });
    assert_eq!(bounded.unwrap(), canonical);
    assert!(!overflowed);
    assert_eq!(
        consumed,
        canonical.len(),
        "private formatting charged legacy temporaries"
    );
    CANONICAL_FORMAT_PASS_COUNT.with(|count| assert_eq!(count.get(), 2));
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(0));
    let (error, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(exact_peak - 1, || {
            canonical_source_bounded(&deep_program)
        });
    assert_eq!(error.unwrap_err().code, "SPX-B109");
    assert!(!overflowed);
    assert_eq!(consumed, 0);
    CANONICAL_FORMAT_PASS_COUNT.with(|count| assert_eq!(count.get(), 1));
}

#[test]
fn formatter_frame_capacity_covers_nested_delimiters_and_helper_stacks() {
    use crate::ast::{Expr, ExprKind, MatchArm, MatchPattern, RecordMatchFieldPattern};

    let span = crate::ast::Span::default();
    let mut ty = crate::ast::Type::I64;
    for index in 0..31 {
        ty = crate::ast::Type::Named {
            name: format!("T{index}"),
            arguments: vec![ty],
        };
    }
    let mut scrutinee = Expr {
        kind: ExprKind::ConstructRecord {
            type_name: "Leaf".into(),
            type_span: span,
            type_arguments: vec![ty.clone()],
            fields: vec![],
        },
        span,
    };
    for _ in 0..64 {
        scrutinee = Expr {
            kind: ExprKind::If {
                condition: Box::new(scrutinee),
                then_branch: Box::new(Expr {
                    kind: ExprKind::Int(1),
                    span,
                }),
                else_branch: Box::new(Expr {
                    kind: ExprKind::Int(0),
                    span,
                }),
            },
            span,
        };
    }
    let mut nested_pattern = RecordMatchFieldPattern::Binding {
        name: "value".into(),
        span,
    };
    for index in 0..31 {
        nested_pattern = RecordMatchFieldPattern::Record {
            type_name: format!("P{index}"),
            type_span: span,
            fields: vec![crate::ast::RecordMatchPatternField {
                name: "next".into(),
                name_span: span,
                pattern: nested_pattern,
                span,
            }],
            span,
        };
    }
    let mut program = crate::parse(
        "module formatter.frames; @id(\"app.main\") fn main() -> i64 { 0 }",
        Path::new("formatter-frames.spx"),
    )
    .unwrap();
    program.functions[0].body = Expr {
        kind: ExprKind::Match {
            mode: crate::ast::MatchMode::Value,
            scrutinee: Box::new(scrutinee),
            arms: vec![MatchArm {
                guard: None,
                pattern: MatchPattern::Record {
                    type_name: "Root".into(),
                    type_span: span,
                    fields: vec![crate::ast::RecordMatchPatternField {
                        name: "next".into(),
                        name_span: span,
                        pattern: nested_pattern,
                        span,
                    }],
                    span,
                },
                value: Expr {
                    kind: ExprKind::Call {
                        name: "typed".into(),
                        type_arguments: vec![ty],
                        args: vec![],
                    },
                    span,
                },
                span,
            }],
        },
        span,
    };

    let capacity = canonical_format_scratch_capacity(&program).unwrap();
    crate::private_format::reset_private_scratch_high_water();
    let mut sink = String::new();
    crate::private_format::write_canonical_with_scratch(&program, &mut sink, capacity);
    let water = crate::private_format::private_scratch_high_water();
    let slots = capacity.slots();
    for (index, ((length, allocated), admitted)) in water.into_iter().zip(slots).enumerate() {
        assert!(length > 0, "formatter helper {index} was not exercised");
        assert!(
            length <= admitted,
            "formatter helper {index} exceeded census"
        );
        assert_eq!(allocated, admitted, "formatter helper {index} grew its Vec");
    }
    assert!(
        water[0].0 > 120,
        "nested delimiter continuations were not retained"
    );
    assert!(
        water[1].0 > 60,
        "nested contains-record traversal was not retained"
    );
    assert!(water[2].0 > 30, "nested type traversal was not retained");
    assert!(water[3].0 > 30, "nested pattern traversal was not retained");
}

#[test]
fn declaration_dag_capacity_counts_layered_leaf_and_layout_expansion_once() {
    fn layered(resource: bool, levels: usize) -> Program {
        let mut source = String::from("module capacity.layers;\n\n");
        if resource {
            source.push_str("@id(\"layer.r0\")\nresource R0 {\n    @id(\"layer.r0.drop\")\n    drop trivial;\n}\n\n");
        } else {
            source.push_str("@id(\"layer.r0\")\nrecord R0 {\n    @id(\"layer.r0.value\")\n    value: i64,\n}\n\n");
        }
        for level in 1..=levels {
            writeln!(
                    source,
                    "@id(\"layer.r{level}\")\nrecord R{level} {{\n    @id(\"layer.r{level}.a\")\n    a: R{},\n    @id(\"layer.r{level}.b\")\n    b: R{},\n}}\n",
                    level - 1,
                    level - 1
                )
                .unwrap();
        }
        source.push_str("@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
        crate::parse(&source, Path::new("layered-capacity.spx")).unwrap()
    }

    let resource = declaration_dag_expansion(&layered(true, 12), 0).unwrap();
    assert_eq!(resource.maximum_resource_leaves, 1 << 12);
    assert_eq!(resource.maximum_type_occurrences, (1 << 13) - 1);
    assert!(resource.maximum_shape_fields >= (1 << 13) - 2);
    assert!(resource.maximum_projection_segments >= 12 * (1 << 12));
    assert!(resource.maximum_shape_identity_bytes > 0);
    assert!(resource.maximum_lifecycle_identity_bytes > 0);
    assert!(resource.maximum_projection_identity_bytes > 0);
    let scalar = declaration_dag_expansion(&layered(false, 12), 0).unwrap();
    assert_eq!(scalar.maximum_resource_leaves, 0);
    assert_eq!(scalar.maximum_type_occurrences, 3 * (1 << 12) - 1);
    assert!(scalar.maximum_shape_fields >= (1 << 13) - 1);
    assert_eq!(scalar.maximum_projection_segments, 0);

    let long = "x".repeat(128);
    let long_source = format!(
        "module capacity.long; @id(\"life.{long}\") resource Leaf {{ @id(\"drop.{long}\") drop trivial; }} @id(\"outer.{long}\") record Outer {{ @id(\"field.{long}\") leaf: Leaf, }} @id(\"app.main\") fn main() -> i64 {{ 0 }}"
    );
    let long_program = crate::parse(&long_source, Path::new("long-capacity.spx")).unwrap();
    let long_expansion = declaration_dag_expansion(&long_program, 0).unwrap();
    assert_eq!(long_expansion.maximum_resource_leaves, 1);
    assert_eq!(long_expansion.maximum_shape_fields, 1);
    assert_eq!(long_expansion.maximum_projection_segments, 1);
    assert!(long_expansion.maximum_shape_identity_bytes >= 3 * 128);
    assert!(long_expansion.maximum_lifecycle_identity_bytes >= 128);
    assert!(long_expansion.maximum_projection_identity_bytes >= 128);

    let cyclic = crate::parse(
            "module capacity.cycle;\n\n@id(\"cycle.a\")\nrecord A {\n    @id(\"cycle.a.next\")\n    next: A,\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
            Path::new("cycle-capacity.spx"),
        )
        .unwrap();
    let error = declaration_dag_expansion(&cyclic, 0).unwrap_err();
    assert_eq!(error.code, "SPX-B107");

    let mut shallow = String::from("module capacity.shallow;\n\n");
    for index in 0..514 {
        writeln!(
                shallow,
                "@id(\"shallow.r{index}\")\nrecord R{index} {{\n    @id(\"shallow.r{index}.value\")\n    value: i64,\n}}\n"
            )
            .unwrap();
    }
    shallow.push_str("@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
    let shallow = crate::parse(&shallow, Path::new("shallow-declarations.spx")).unwrap();
    let expansion = declaration_dag_expansion(&shallow, 0).unwrap();
    assert_eq!(expansion.maximum_resource_leaves, 0);
    assert_eq!(expansion.maximum_type_occurrences, 2);

    let mut chain = String::from(
        "module capacity.chain;\n\n@id(\"chain.r0\")\nrecord R0 {\n    @id(\"chain.r0.value\")\n    value: i64,\n}\n\n",
    );
    for index in 1..514 {
        writeln!(
                chain,
                "@id(\"chain.r{index}\")\nrecord R{index} {{\n    @id(\"chain.r{index}.next\")\n    next: R{},\n}}\n",
                index - 1
            )
            .unwrap();
    }
    chain.push_str("@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
    let chain = crate::parse(&chain, Path::new("long-chain.spx")).unwrap();
    let expansion = declaration_dag_expansion(&chain, 0).unwrap();
    assert_eq!(expansion.maximum_resource_leaves, 0);
    assert_eq!(expansion.maximum_type_occurrences, 515);
}
