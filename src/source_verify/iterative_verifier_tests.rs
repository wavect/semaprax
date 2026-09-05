use super::*;
use std::path::Path;

fn source_diagnostics(source: &str) -> Vec<Diagnostic> {
    let program = crate::parse(source, Path::new("borrowed-bytes-call-source-v1.spx"))
        .expect("fixture parses");
    verify(&program)
}

#[test]
fn monomorphic_borrowed_bytes_calls_admit_named_and_direct_owned_field_places() {
    let source = r#"
module test.borrowed_bytes_call_source;
@id("packet.type") record Packet {
  @id("packet.payload") payload: Bytes,
  @id("packet.sibling") sibling: Bytes,
}
@id("packet.measure") fn measure(value: borrow Bytes) -> usize {
  byte_len(bytes_as_slice(value))
}
@id("packet.pair") fn pair(value: borrow Bytes, sibling: own Bytes) -> usize {
  byte_len(bytes_as_slice(value))
}
@id("packet.forward") fn forward(value: borrow Bytes) -> usize { measure(value) }
@id("packet.field") fn field(packet: own Packet) -> usize { measure(packet.payload) }
@id("packet.sibling-call") fn sibling(packet: own Packet) -> usize {
  pair(packet.payload, packet.sibling)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let diagnostics = source_diagnostics(source);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn later_direct_or_nested_parent_transfer_overlapping_a_borrowed_field_is_t265() {
    let cases = [
        r#"
module test.borrowed_bytes_call_overlap;
@id("packet.type") record Packet { @id("packet.payload") payload: Bytes, }
@id("packet.consume") fn consume(value: borrow Bytes, packet: own Packet) -> usize { 0usize }
@id("packet.invalid") fn invalid(packet: own Packet) -> usize {
  consume(packet.payload, packet)
}
@id("app.main") fn main() -> i64 { 0 }
"#,
        r#"
module test.borrowed_bytes_call_nested_overlap;
@id("packet.type") record Packet { @id("packet.payload") payload: Bytes, }
@id("packet.take") fn take(packet: own Packet) -> Packet { packet }
@id("packet.consume") fn consume(value: borrow Bytes, packet: own Packet) -> usize { 0usize }
@id("packet.invalid") fn invalid(packet: own Packet) -> usize {
  consume(packet.payload, take(packet))
}
@id("app.main") fn main() -> i64 { 0 }
"#,
    ];
    for source in cases {
        let diagnostics = source_diagnostics(source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-T265"),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn borrowed_bytes_call_rejects_temporary_borrowed_root_and_generic_projection() {
    let cases = [
        r#"
module test.borrowed_bytes_call_temporary;
@id("packet.type") record Packet { @id("packet.payload") payload: Bytes, }
@id("packet.make") fn make(value: own Bytes) -> Packet { Packet { payload: value } }
@id("packet.measure") fn measure(value: borrow Bytes) -> usize { 0usize }
@id("packet.invalid") fn invalid(value: own Bytes) -> usize { measure(make(value).payload) }
@id("app.main") fn main() -> i64 { 0 }
"#,
        r#"
module test.borrowed_bytes_call_borrowed_root;
@id("packet.type") record Packet { @id("packet.payload") payload: Bytes, }
@id("packet.measure") fn measure(value: borrow Bytes) -> usize { 0usize }
@id("packet.invalid") fn invalid(packet: borrow Packet) -> usize { measure(packet.payload) }
@id("app.main") fn main() -> i64 { 0 }
"#,
        r#"
module test.borrowed_bytes_call_generic;
@id("packet.type") record Packet { @id("packet.payload") payload: Bytes, }
@id("packet.measure") fn measure<T>(value: borrow Bytes, marker: T) -> T { marker }
@id("packet.invalid") fn invalid(packet: own Packet) -> i64 { measure<i64>(packet.payload, 0) }
@id("app.main") fn main() -> i64 { 0 }
"#,
        r#"
module test.borrowed_bytes_call_generic_named;
@id("packet.measure") fn measure<T>(value: borrow Bytes, marker: T) -> T { marker }
@id("packet.invalid") fn invalid(value: own Bytes) -> i64 { measure<i64>(value, 0) }
@id("app.main") fn main() -> i64 { 0 }
"#,
    ];
    for source in cases {
        let diagnostics = source_diagnostics(source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-T266"),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn byte_parameters_retain_the_closed_own_or_monomorphic_borrow_modes() {
    for (label, mode) in [("value", ""), ("shared", "shared ")] {
        let source = format!(
            r#"
module test.borrowed_bytes_parameter_mode;
@id("packet.invalid") fn invalid(value: {mode}Bytes) -> usize {{ 0usize }}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
        );
        let diagnostics = source_diagnostics(&source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-T263"),
            "{label}: {diagnostics:#?}"
        );
    }
}

#[allow(clippy::type_complexity)]
fn diagnostics_key(
    diagnostics: &[Diagnostic],
) -> Vec<(
    &'static str,
    crate::diagnostic::Severity,
    &str,
    Option<&str>,
    Option<Span>,
    Option<&str>,
)> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.code,
                diagnostic.severity,
                diagnostic.message.as_str(),
                diagnostic.path.as_deref(),
                diagnostic.span,
                diagnostic.help.as_deref(),
            )
        })
        .collect()
}

fn compare_scalar_body(source: &str) {
    let program = crate::parse(source, Path::new("iterative-verifier.spx")).unwrap();
    let current = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let expression = match &current.body.kind {
        ExprKind::Block { statements, tail } if statements.is_empty() => tail.as_ref(),
        _ => &current.body,
    };
    let functions = program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<HashMap<_, _>>();
    let types = TypeTable::new(&program);
    let mut oracle_scope = HashMap::new();
    for parameter in &current.params {
        oracle_scope.insert(
            parameter.name.clone(),
            Binding {
                ty: parameter.ty.clone(),
                mode: parameter.mode,
                availability: Availability::Available,
                moved_places: HashMap::new(),
                definitely_partial: HashSet::new(),
                native_unit_discard: false,
                mutable: false,
                active_loans: BTreeSet::new(),
                borrow_origin: None,
            },
        );
    }
    let iterative_scope = oracle_scope.clone();
    let mut oracle_diagnostics = Vec::new();
    let oracle = check_expr(
        &program,
        current,
        expression,
        &mut oracle_scope,
        &functions,
        &types,
        None,
        true,
        &mut oracle_diagnostics,
    );
    let mut iterative_diagnostics = Vec::new();
    let mut iterative = IterativeVerifier::new(
        &program,
        current,
        iterative_scope,
        &functions,
        &types,
        None,
        true,
        &mut iterative_diagnostics,
    );
    let actual = iterative.run(expression).unwrap();
    assert_eq!(
        oracle
            .as_ref()
            .map(|value| (&value.ty, value.mode, value.native_unit)),
        actual
            .as_ref()
            .map(|value| (&value.ty, value.mode, value.native_unit))
    );
    assert_eq!(oracle_scope.len(), iterative.scopes[0].bindings.len());
    for (name, expected) in oracle_scope {
        let actual = &iterative.scopes[0].bindings[&name];
        assert_eq!(expected.ty, actual.ty);
        assert_eq!(expected.mode, actual.mode);
        assert_eq!(expected.availability, actual.availability);
        assert_eq!(expected.moved_places, actual.moved_places);
        assert_eq!(expected.definitely_partial, actual.definitely_partial);
        assert_eq!(expected.native_unit_discard, actual.native_unit_discard);
    }
    drop(iterative);
    assert_eq!(
        diagnostics_key(&oracle_diagnostics),
        diagnostics_key(&iterative_diagnostics)
    );
}

#[test]
fn scalar_frame_machine_matches_recursive_oracle() {
    compare_scalar_body("module t; fn main() -> i64 { -(1 + true) }");
    compare_scalar_body("module t; fn main() -> bool { missing_left == missing_right }");
    compare_scalar_body("module t; fn main(flag: bool) -> bool { flag && missing }");
    compare_scalar_body("module t; fn main(flag: bool) -> i64 { if flag { 1 } else { true } }");
    compare_scalar_body(
            "module t; fn main(flag: bool) -> i64 { if missing_condition { missing_then } else { missing_else } }",
        );
    compare_scalar_body(
            "module t; fn main(flag: bool) -> i64 { let value = 1 + true; let value = missing; if flag { value } else { missing_tail } }",
        );
    compare_scalar_body(
        "module t; fn zero() -> i64 { 0 } fn main() -> i64 { zero(missing_a, missing_b) }",
    );
    compare_scalar_body(
            "module t; fn one(value: i64) -> i64 { value } fn main() -> i64 { one(true) + one(missing) }",
        );
    compare_scalar_body(
            "module t; fn identity<T>(value: T) -> T { value } fn main() -> i64 { identity<i64>(1) + identity<bool>(true) }",
        );
    compare_scalar_body(
            "module t; @id(\"t.host\") interface Host permits {  } { @id(\"t.host.ping\") import rust fn ping(value: i64) -> unit effects {  } failure infallible; } fn main() -> i64 { let acknowledged = ping(1); let copied = acknowledged; 1 }",
        );
    compare_scalar_body(
            "module t; @id(\"t.buffer\") resource Buffer { @id(\"t.buffer.drop\") drop trivial; } fn inspect(value: borrow Buffer) -> i64 { 1 } fn consume(value: own Buffer) -> i64 { 1 } fn main(buffer: own Buffer) -> i64 { let first = consume(buffer) + missing; inspect(buffer) }",
        );
    compare_scalar_body(
            "module t; @id(\"t.buffer\") resource Buffer { @id(\"t.buffer.drop\") drop trivial; } fn inspect(value: borrow Buffer) -> i64 { 1 } fn consume(value: own Buffer) -> bool { true } fn main(buffer: own Buffer, left: bool, right: bool) -> i64 { let selected = left && (right && consume(buffer)); inspect(buffer) }",
        );
    compare_scalar_body(
            "module t; @id(\"t.pair\") record Pair { @id(\"t.pair.x\") x: i64, @id(\"t.pair.y\") y: i64, } fn main() -> Pair { Pair { missing: missing_rhs, x: true, x: missing_duplicate } }",
        );
    compare_scalar_body(
            "module t; @id(\"t.pair\") record Pair { @id(\"t.pair.x\") x: i64, @id(\"t.pair.y\") y: i64, } fn main() -> Pair { let pair = Pair { x: 1, y: 2 }; pair with { missing: missing_rhs, x: true, x: missing_duplicate } }",
        );
    compare_scalar_body(
            "module t; @id(\"t.choice\") variant Choice { @id(\"t.choice.none\") None, @id(\"t.choice.value\") Value { @id(\"t.choice.value.v\") value: i64, }, } fn main(choice: Choice) -> i64 { match choice { Choice::Value { value: item } => item, Choice::None {} => 0, } }",
        );
    compare_scalar_body(
            "module t; @id(\"t.choice\") variant Choice { @id(\"t.choice.none\") None, @id(\"t.choice.value\") Value { @id(\"t.choice.value.v\") value: i64, }, } fn main(choice: Choice) -> i64 { match choice { Choice::Value { missing: binding } => missing_arm, _ => true, } }",
        );
    compare_scalar_body(
            "module t; @id(\"t.choice\") variant Choice { @id(\"t.choice.value\") Value { @id(\"t.choice.value.v\") value: i64, }, } fn main() -> Choice { Choice::Value { missing: missing_rhs, value: true, value: missing_duplicate } }",
        );
    compare_scalar_body(
            "module t; @id(\"t.pair\") record Pair { @id(\"t.pair.x\") x: i64, } fn main(pair: Pair) -> i64 { match pair { Pair { x } => x, _ => missing_unreachable, } }",
        );
    compare_scalar_body(
            "module t; @id(\"t.inner\") record Inner { @id(\"t.inner.value\") value: i64, @id(\"t.inner.flag\") flag: bool, } @id(\"t.outer\") record Outer { @id(\"t.outer.inner\") inner: Inner, @id(\"t.outer.other\") other: i64, } fn main(input: Outer) -> i64 { match input { Outer { inner: Inner { value: item, missing: skipped, value: duplicate }, other: item } => missing_arm, } }",
        );
    compare_scalar_body(
            "module t; @id(\"t.pair\") record Pair { @id(\"t.pair.x\") x: i64, } fn main(pair: Pair) -> i64 { pair.x }",
        );
    compare_scalar_body("module t; fn main() -> i64 { missing.field }");
    compare_scalar_body("module t; fn main() -> i64 { missing? }");
}

#[test]
fn wide_scalar_blocks_and_record_literals_verify_without_quadratic_rescans() {
    use std::fmt::Write as _;

    let mut statements = String::from(
        "module test.wide_scalars;\n@id(\"work.main\") fn main() -> i64 {\nlet v0 = 0;\n",
    );
    for index in 1..1_000 {
        writeln!(statements, "let v{index} = v{} + 1;", index - 1).unwrap();
    }
    statements.push_str("v999\n}\n");
    assert!(source_diagnostics(&statements).is_empty());

    let mut record = String::from("module test.wide_record;\n@id(\"work.wide\") record Wide {\n");
    for index in 0..1_000 {
        writeln!(record, "@id(\"work.wide.f{index}\") f{index}: i64,").unwrap();
    }
    record.push_str("}\n@id(\"work.main\") fn main() -> i64 {\nlet value = Wide { ");
    for index in 0..1_000 {
        write!(record, "f{index}: {index}, ").unwrap();
    }
    record.push_str("};\nvalue.f0\n}\n");
    assert!(source_diagnostics(&record).is_empty());
}

#[test]
fn byte_capacity_reclaims_shallow_wide_sibling_scopes_at_last_continuation() {
    let mut program = crate::parse(
            "module capacity.scope_peak; @id(\"capacity.scope_peak.wide\") fn wide() -> i64 { 0 } @id(\"capacity.scope_peak.main\") fn main() -> i64 { 0 }",
            Path::new("capacity-scope-peak.spx"),
        )
        .unwrap();
    let span = Span::default();
    let wide = program
        .functions
        .iter_mut()
        .find(|function| function.name == "wide")
        .unwrap();
    wide.params.extend((0..256).map(|index| Param {
        name: format!("root_{index}"),
        mode: ParamMode::Value,
        ty: Type::I64,
        span,
    }));
    let statements = (0..256)
        .map(|_| Statement::Unsafe {
            audit: "scope-peak".to_owned(),
            audit_span: span,
            body: Box::new(Expr {
                kind: ExprKind::If {
                    condition: Box::new(Expr {
                        kind: ExprKind::Bool(true),
                        span,
                    }),
                    then_branch: Box::new(Expr {
                        kind: ExprKind::Int(1),
                        span,
                    }),
                    else_branch: Box::new(Expr {
                        kind: ExprKind::Int(2),
                        span,
                    }),
                },
                span,
            }),
            span,
        })
        .collect();
    wide.body = Expr {
        kind: ExprKind::Block {
            statements,
            tail: Box::new(Expr {
                kind: ExprKind::Int(0),
                span,
            }),
        },
        span,
    };

    reset_source_capacity_scope_peak();
    verify_byte_data_capacity(&program, &TypeTable::new(&program)).unwrap();
    assert_eq!(source_capacity_scope_peak(), 2);
    assert_eq!(source_capacity_scope_live(), 0);
}

#[test]
fn transcript_match_arms_share_one_many_root_snapshot() {
    use crate::byte_data_capacity::TranscriptSource;

    let span = Span::default();
    let roots = (0..256)
        .map(|index| (format!("root_{index}"), TranscriptSource::Fixed(1)))
        .collect::<BTreeMap<_, _>>();
    let expression = Expr {
        kind: ExprKind::Match {
            mode: crate::ast::MatchMode::Value,
            scrutinee: Box::new(Expr {
                kind: ExprKind::Int(0),
                span,
            }),
            arms: (0..256)
                .map(|_| crate::ast::MatchArm {
                    pattern: MatchPattern::Wildcard { span },
                    guard: None,
                    value: Expr {
                        kind: ExprKind::Var("root_0".to_owned()),
                        span,
                    },
                    span,
                })
                .collect(),
        },
        span,
    };

    reset_source_transcript_scope_peak();
    assert_eq!(
        source_transcript_source_from_roots(&expression, &roots),
        TranscriptSource::Fixed(1)
    );
    assert_eq!(source_transcript_scope_peak(), 1);
    assert_eq!(source_transcript_scope_live(), 0);
    assert_eq!(source_transcript_owned_map_allocations(), 0);
    assert_eq!(source_transcript_frame_scratch_peak(), (2, 2));

    // Preserve the recursive oracle's authored-order early mismatch: an
    // owned block in every later arm must remain unvisited once the first
    // two sources disagree.
    let mismatch = Expr {
        kind: ExprKind::Match {
            mode: crate::ast::MatchMode::Value,
            scrutinee: Box::new(Expr {
                kind: ExprKind::Int(0),
                span,
            }),
            arms: (0..256)
                .map(|index| crate::ast::MatchArm {
                    pattern: MatchPattern::Wildcard { span },
                    guard: None,
                    value: match index {
                        0 => Expr {
                            kind: ExprKind::Var("root_0".to_owned()),
                            span,
                        },
                        1 => Expr {
                            kind: ExprKind::String("xx".to_owned()),
                            span,
                        },
                        _ => Expr {
                            kind: ExprKind::Block {
                                statements: Vec::new(),
                                tail: Box::new(Expr {
                                    kind: ExprKind::Var("root_0".to_owned()),
                                    span,
                                }),
                            },
                            span,
                        },
                    },
                    span,
                })
                .collect(),
        },
        span,
    };
    reset_source_transcript_scope_peak();
    assert_eq!(
        source_transcript_source_from_roots(&mismatch, &roots),
        TranscriptSource::Unknown
    );
    assert_eq!(source_transcript_scope_peak(), 1);
    assert_eq!(source_transcript_scope_live(), 0);
    assert_eq!(source_transcript_owned_map_allocations(), 0);
    assert_eq!(source_transcript_frame_scratch_peak(), (2, 2));
}

#[test]
fn byte_capacity_serializes_wide_match_arm_scratch_with_a_deep_type() {
    let mut program = crate::parse(
            "module capacity.match_scratch; @id(\"capacity.match_scratch.wide\") fn wide(value: i64) -> i64 { value } @id(\"capacity.match_scratch.main\") fn main() -> i64 { 0 }",
            Path::new("capacity-match-scratch.spx"),
        )
        .unwrap();
    let span = Span::default();
    let mut deep_type = Type::I64;
    for index in 0..512 {
        deep_type = Type::Named {
            name: format!("Deep{index}"),
            arguments: vec![deep_type],
        };
    }
    let expected_type_scratch = ast_type_owned_capacity(&deep_type.clone());
    let wide = program
        .functions
        .iter_mut()
        .find(|function| function.name == "wide")
        .unwrap();
    wide.params.push(Param {
        name: "deep".to_owned(),
        mode: ParamMode::Value,
        ty: deep_type,
        span,
    });
    wide.body = Expr {
        kind: ExprKind::Match {
            mode: crate::ast::MatchMode::Value,
            scrutinee: Box::new(Expr {
                kind: ExprKind::Var("deep".to_owned()),
                span,
            }),
            arms: (0..256)
                .map(|index| crate::ast::MatchArm {
                    pattern: MatchPattern::Wildcard { span },
                    guard: None,
                    value: Expr {
                        kind: ExprKind::Int(index),
                        span,
                    },
                    span,
                })
                .collect(),
        },
        span,
    };

    reset_source_capacity_scope_peak();
    verify_byte_data_capacity(&program, &TypeTable::new(&program)).unwrap();
    assert_eq!(source_capacity_scope_peak(), 2);
    assert_eq!(source_capacity_scope_live(), 0);
    assert_eq!(
        source_capacity_match_next_scratch_peak(),
        (
            1,
            "capacity.match_scratch.wide.body".len(),
            expected_type_scratch,
        )
    );
}

#[test]
fn transcript_many_let_var_queries_borrow_the_growing_root_map() {
    let mut program = crate::parse(
            "module capacity.transcript_borrow; @id(\"capacity.transcript_borrow.wide\") fn wide() -> i64 { 0 } @id(\"capacity.transcript_borrow.main\") fn main() -> i64 { 0 }",
            Path::new("capacity-transcript-borrow.spx"),
        )
        .unwrap();
    let span = Span::default();
    let wide = program
        .functions
        .iter_mut()
        .find(|function| function.name == "wide")
        .unwrap();
    wide.params.extend((0..256).map(|index| Param {
        name: format!("root_{index}"),
        mode: ParamMode::Value,
        ty: Type::I64,
        span,
    }));
    wide.body = Expr {
        kind: ExprKind::Block {
            statements: (0..256)
                .map(|index| Statement::Let {
                    name: format!("alias_{index}"),
                    name_span: span,
                    mutable: false,
                    declared: None,
                    value: Expr {
                        kind: ExprKind::Var(format!("root_{index}")),
                        span,
                    },
                    span,
                })
                .collect(),
            tail: Box::new(Expr {
                kind: ExprKind::Var("alias_255".to_owned()),
                span,
            }),
        },
        span,
    };

    reset_source_transcript_scope_peak();
    verify_byte_data_capacity(&program, &TypeTable::new(&program)).unwrap();
    assert_eq!(source_transcript_scope_peak(), 1);
    assert_eq!(source_transcript_scope_live(), 0);
    assert_eq!(source_transcript_owned_map_allocations(), 0);
    assert_eq!(source_transcript_frame_scratch_peak(), (1, 1));
}

#[test]
fn source_type_inference_mutates_one_owned_scope_for_many_inferred_lets() {
    let mut program = crate::parse(
            "module capacity.type_scope; @id(\"capacity.type_scope.wide\") fn wide() -> i64 { 0 } @id(\"capacity.type_scope.main\") fn main() -> i64 { 0 }",
            Path::new("capacity-type-scope.spx"),
        )
        .unwrap();
    let span = Span::default();
    let wide = program
        .functions
        .iter_mut()
        .find(|function| function.name == "wide")
        .unwrap();
    wide.params.extend((0..256).map(|index| Param {
        name: format!("root_{index}"),
        mode: ParamMode::Value,
        ty: Type::I64,
        span,
    }));
    wide.body = Expr {
        kind: ExprKind::Block {
            statements: (0..256)
                .map(|index| Statement::Let {
                    name: format!("alias_{index}"),
                    name_span: span,
                    mutable: false,
                    declared: None,
                    value: Expr {
                        kind: ExprKind::Var(format!("root_{index}")),
                        span,
                    },
                    span,
                })
                .collect(),
            tail: Box::new(Expr {
                kind: ExprKind::Var("alias_255".to_owned()),
                span,
            }),
        },
        span,
    };

    let wide = program
        .functions
        .iter()
        .find(|function| function.name == "wide")
        .unwrap();
    let types = TypeTable::new(&program);
    let ordinary = source_capacity_functions(&program)
        .into_iter()
        .filter_map(|(owner, function)| {
            owner
                .is_none()
                .then_some((function.name.as_str(), function))
        })
        .collect();
    let context = SourceCapacityContext {
        types: &types,
        ordinary: &ordinary,
        enclosing_class: None,
    };
    let bindings = wide
        .params
        .iter()
        .map(|parameter| (parameter.name.clone(), parameter.ty.clone()))
        .collect::<BTreeMap<_, _>>();
    reset_source_capacity_scope_peak();
    assert_eq!(
        source_capacity_expr_type(&wide.body, &bindings, &context),
        Some(Type::I64)
    );
    assert_eq!(source_type_scope_copy_totals(), (1, 256));
}

#[test]
fn function_count_bound_rejects_before_per_function_capacity_projection() {
    let mut source = String::from("module capacity.many;\n\n");
    for index in 0..=crate::byte_data_capacity::MAX_FUNCTIONS {
        source.push_str(&format!(
            "@id(\"capacity.f{index}\")\nfn f{index}() -> i64\n{{\n    {index}\n}}\n\n"
        ));
    }
    source.push_str("@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
    let program = crate::parse(&source, Path::new("many.spx")).unwrap();
    let diagnostics = super::declaration::verify(&program);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-T270");
    assert_eq!(
        diagnostics[0].message,
        "module declares more than the admitted 4096 functions"
    );
    assert_eq!(diagnostics[0].span, Some(program.functions[4096].name_span));
    assert!(diagnostics[0].help.is_some());
}
