use super::*;
const SOURCE: &str = include_str!("../../../examples/inventory-field-update/main.spx");
fn resolved(source: &str) -> crate::hir::ResolvedProgram {
    crate::hir::resolve(&crate::check(source, "update.spx").unwrap()).unwrap()
}
#[test]
fn canonical_descriptor_graph_and_cache_replay() {
    let ast = crate::check(SOURCE, "inventory.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::check(&canonical, "inventory.spx").unwrap())
    );
    assert_eq!(
        crate::prelude::selected_for_program(&ast).0,
        crate::prelude::SCHEMA_V19
    );
    let graph = crate::graph::to_json(&ast).unwrap();
    assert!(graph.contains(PREFIX));
    crate::graph::verify_json(&ast, &graph).unwrap();
    assert!(crate::graph::verify_json(
        &ast,
        &graph.replace("semaprax.prelude.v19", "semaprax.prelude.v18")
    )
    .is_err());
    let program = resolved(SOURCE);
    crate::hir::validate(&program).unwrap();
    let wire = crate::cache_codec::encode(&program).unwrap();
    let replay: crate::hir::ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    crate::hir::validate(&replay).unwrap();
    assert_eq!(wire, crate::cache_codec::encode(&replay).unwrap());
    let old = crate::check(
        "module old; @id(\"old.main\") fn main()->i64{42}",
        "old.spx",
    )
    .unwrap();
    assert_ne!(
        crate::prelude::selected_for_program(&old).0,
        crate::prelude::SCHEMA_V19
    );
}
#[test]
fn scalar_update_wrong_selector_type_nominal_shape_and_live_loan_refuse() {
    for (from, to, code) in [
        (
            "available - 1usize, \"count\"",
            "available - 1usize, \"missing\"",
            "SPX-T320",
        ),
        (
            "available - 1usize, \"count\"",
            "available - 1usize, \"sku\"",
            "SPX-T320",
        ),
        (
            "available - 1usize, \"count\"",
            "false, \"count\"",
            "SPX-T205",
        ),
        (
            "0usize, available - 1usize",
            "0, available - 1usize",
            "SPX-T205",
        ),
    ] {
        let changed = SOURCE.replace(from, to);
        assert_ne!(changed, SOURCE);
        let diagnostics = crate::check(&changed, "bad.spx").unwrap_err();
        assert!(
            diagnostics.iter().any(|d| d.code == code),
            "{diagnostics:?}"
        );
    }
    let borrowed = SOURCE
        .replace(
            "let available =",
            "let view = vec_field<Item>(stock, 0usize, \"sku\"); let available =",
        )
        .replace(
            "if vec_field<Item>",
            "if view == str\"nuts\" && vec_field<Item>",
        );
    assert!(crate::check(&borrowed, "loan.spx").is_err());
    let element = ResolvedType::Nominal {
        declaration: DeclarationId::new("inventory.item"),
        arguments: vec![],
    };
    let program = resolved(SOURCE);
    let count = resolve(&program.declarations, &element, "count").unwrap();
    assert_eq!(
        position(&program.declarations, &element, &count.identity().unwrap()),
        Some((1, ResolvedType::Usize))
    );
    let wrong = ResolvedType::Nominal {
        declaration: DeclarationId::new("other.item"),
        arguments: vec![],
    };
    assert!(replay(&program.declarations, &wrong, &count.identity().unwrap()).is_none());
    let mut forged = count.clone();
    forged.field = DeclarationId::new("inventory.item.active");
    // A scalar type change remains a different descriptor; consumers must
    // authenticate the actual RHS against that exact field type.
    assert_eq!(
        position(&program.declarations, &element, &forged.identity().unwrap()),
        Some((2, ResolvedType::Bool))
    );
    forged.layout.push('x');
    assert!(replay(&program.declarations, &element, &forged.identity().unwrap()).is_none());
}
#[test]
fn inventory_success_and_bounds_rhs_failure_precedence() {
    use crate::interpreter::{evaluate_resolved_zero_arg_i64, ResolvedEvaluationOutcome};
    let result =
        evaluate_resolved_zero_arg_i64(&resolved(SOURCE), "inventory.main", 1_000_000).unwrap();
    assert!(
        matches!(result.outcome, ResolvedEvaluationOutcome::ReturnedI64(42)),
        "{:?}",
        result.outcome
    );
    for (index, rhs, domain, code) in [
        ("2usize", "available - 1usize", "semaprax.vec.v1", 2),
        ("2usize", "0usize - 1usize", "semaprax.arithmetic.v1", 2),
    ] {
        let source = SOURCE.replace(
            "0usize, available - 1usize, \"count\"",
            &format!("{index}, {rhs}, \"count\""),
        );
        let result =
            evaluate_resolved_zero_arg_i64(&resolved(&source), "inventory.main", 1_000_000)
                .unwrap();
        let ResolvedEvaluationOutcome::LanguageFailure(status) = result.outcome else {
            panic!("{:?}", result.outcome)
        };
        assert_eq!(status.domain_id(), domain);
        assert_eq!(status.code(), code);
    }
}

#[test]
fn unsupported_record_frontiers_and_descriptor_budget_fail_closed() {
    let ast = crate::parse(SOURCE, "shapes.spx").unwrap();
    let base = ast.types[0].clone();
    let crate::ast::TypeDeclarationKind::Record { fields } = &base.kind else {
        panic!("record")
    };
    assert!(source(&base, "count").is_some());
    for replacement in [
        crate::ast::Type::String,
        crate::ast::Type::Bytes,
        crate::ast::Type::Named {
            name: "Item".into(),
            arguments: vec![],
        },
    ] {
        let mut wrong = base.clone();
        let crate::ast::TypeDeclarationKind::Record { fields } = &mut wrong.kind else {
            unreachable!()
        };
        fields[1].ty = replacement;
        assert!(source(&wrong, "count").is_none());
    }
    let mut implicit = base.clone();
    implicit.explicit_id = false;
    assert!(source(&implicit, "count").is_none());
    let mut implicit_field = base.clone();
    let crate::ast::TypeDeclarationKind::Record { fields: bad } = &mut implicit_field.kind else {
        unreachable!()
    };
    bad[1].explicit_id = false;
    assert!(source(&implicit_field, "count").is_none());
    let mut copy_only = base.clone();
    let crate::ast::TypeDeclarationKind::Record { fields: copy } = &mut copy_only.kind else {
        unreachable!()
    };
    copy[0].ty = crate::ast::Type::I64;
    assert!(source(&copy_only, "count").is_none());
    let mut invariant = base.clone();
    invariant.invariants = Some(Box::new(vec![crate::ast::Expr {
        kind: crate::ast::ExprKind::Bool(true),
        span: crate::ast::Span::default(),
    }]));
    assert!(source(&invariant, "count").is_none());
    assert_eq!(fields.len(), 3);
    for selector in ["", "count:asc", "count,active", "1count", " count"] {
        assert!(source(&base, selector).is_none());
    }
    let mut generic = base.clone();
    generic
        .type_parameters
        .push(crate::ast::TypeParameterDeclaration {
            name: "T".into(),
            span: crate::ast::Span::default(),
        });
    assert!(source(&generic, "count").is_none());
    let mut resource = base.clone();
    resource.kind = crate::ast::TypeDeclarationKind::Resource { lifecycles: vec![] };
    assert!(source(&resource, "count").is_none());
    assert!(super::super::field_unique::literal(&"a".repeat(256)));
    assert!(!super::super::field_unique::literal(&"a".repeat(257)));
    let descriptor = source(&base, "count").unwrap();
    assert!(decode(&(descriptor.identity().unwrap() + "x")).is_none());
    let length = descriptor.identity().unwrap().len();
    let (exact, overflow, bytes) =
        crate::bounded_output::with_limit_usage(length, || descriptor.identity());
    assert!(exact.is_some());
    assert!(!overflow);
    assert_eq!(bytes, length);
    let (short, _, _) =
        crate::bounded_output::with_limit_usage(length - 1, || descriptor.identity());
    assert!(short.is_none());
    let (result, _, _) = crate::bounded_output::with_limit_usage(0, || descriptor.identity());
    assert!(result.is_none());
}

#[test]
fn while_source_selector_is_metadata_and_hir_keeps_three_runtime_children() {
    let source = r#"module field.loop;
@id("field.row") record Row { @id("field.row.title") title:string, @id("field.row.n") n:i64 }
@id("field.main") fn main()->i64 {
 let a=vec_with_capacity<Row>(1usize);
 let mut rows=vec_push<Row>(a,Row{title:"held",n:0});
 let mut i=0;
 while i<3 {
  rows=vec_update_field<Row>(rows,0usize,i,"n");
  i=i+1;
  0
 }
 if vec_field<Row>(rows,0usize,"n")==2 {29}else{0}
}"#;
    let program = resolved(source);
    let mut pending: Vec<_> = program.functions.iter().map(|f| &f.body).collect();
    let mut stores = 0;
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call {
            callee,
            args,
            type_arguments,
            ..
        } = &expression.kind
        {
            if callee.as_str().starts_with(PREFIX) {
                assert_eq!(args.len(), 3, "selector has no runtime child");
                assert_eq!(type_arguments.len(), 1);
                stores += 1;
            }
        }
        for index in 0..expression.child_count() {
            pending.push(expression.child(index).unwrap());
        }
    }
    assert_eq!(stores, 1);
    crate::hir::validate(&program).unwrap();
    let wire = crate::cache_codec::encode(&program).unwrap();
    let replay: crate::hir::ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    crate::hir::validate(&replay).unwrap();
    let outcome =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&program, "field.main", 1_000_000)
            .unwrap();
    assert!(matches!(
        outcome.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(29)
    ));
    let graph = crate::graph::to_json(&crate::check(source, "field-loop.spx").unwrap()).unwrap();
    crate::graph::verify_json(&crate::check(source, "field-loop.spx").unwrap(), &graph).unwrap();
    for (old, new, code) in [
        ("i,\"n\"", "i", "SPX-T283"),
        ("i,\"n\"", "i,\"n\",0", "SPX-T283"),
        ("i,\"n\"", "i,\"title\"", "SPX-T320"),
        ("i,\"n\"", "false,\"n\"", "SPX-T205"),
        ("i,\"n\"", "i,{\"n\"}", "SPX-T320"),
    ] {
        let changed = source.replace(old, new);
        assert_ne!(changed, source);
        let diagnostics = crate::check(&changed, "field-loop-refusal.spx").unwrap_err();
        assert!(
            diagnostics.iter().any(|d| d.code == code),
            "{diagnostics:?}"
        );
    }
}
