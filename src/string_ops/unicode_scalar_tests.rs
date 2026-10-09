use crate::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedType};

const SOURCE: &str = r#"module test.scalar_auth;
@id("scalar.convert") fn convert(value: i64) -> char { char_from_i64(value) }
@id("app.main") fn main() -> i64 { if convert(1114111) == '\u{10ffff}' { 0 } else { 1 } }
"#;

fn call_mut(program: &mut ResolvedProgram) -> &mut ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "scalar.convert")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("ordinary conversion body")
    };
    tail
}

#[test]
fn unicode_scalar_call_roundtrips_source_graph_and_cache() {
    let ast = crate::check(SOURCE, "scalar.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let round = crate::check(&canonical, "scalar.spx").unwrap();
    assert_eq!(crate::format::canonical(&round), canonical);
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&round, &graph).unwrap();
    assert!(graph.contains("\"callee\":\"core.num.char_from_i64\""));
    let resolved = hir::resolve(&ast).unwrap();
    let wire = crate::cache_codec::encode(&resolved).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    hir::validate(&restored).unwrap();
    assert_eq!(crate::cache_codec::encode(&restored).unwrap(), wire);
    let actual =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&restored, "app.main", 1000).unwrap();
    assert!(matches!(
        actual.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(0)
    ));
    let drift = crate::check(
        &SOURCE.replace("convert(1114111)", "convert(65536)"),
        "scalar.spx",
    )
    .unwrap();
    assert!(crate::graph::verify_json(&drift, &graph).is_err());
    assert!(crate::graph::verify_json(
        &ast,
        &graph.replace("core.num.char_from_i64", "core.num.char_from_u8")
    )
    .is_err());
    assert_eq!(super::StringOp::CONVERSIONS.len(), 5, "frozen v1 inventory");
    let op = super::by_name("char_from_i64").unwrap();
    assert_eq!(super::by_id(op.id()), Some(op));
    assert_eq!(op.param_types(), &[ResolvedType::I64]);
    assert_eq!(op.return_type(), ResolvedType::Char);
    assert_eq!(op.param_ownership(0), hir::OwnershipMode::Value);
    assert!(op.is_integer_conversion());
    assert!(!op.touches_string());
    assert!(!op.is_wasm_refused());
}

#[test]
fn unicode_scalar_forged_hir_and_retained_calls_fail_closed() {
    let ast = crate::check(SOURCE, "scalar.spx").unwrap();
    let resolved = hir::resolve(&ast).unwrap();
    for mutation in 0..7 {
        let mut forged = resolved.clone();
        let expression = call_mut(&mut forged);
        let ResolvedExprKind::Call {
            callee,
            args,
            type_arguments,
            instance,
        } = &mut expression.kind
        else {
            panic!("ordinary monomorphic call")
        };
        match mutation {
            0 => expression.ty = ResolvedType::I64,
            1 => args[0].ty = ResolvedType::U8,
            2 => {
                args.pop();
            }
            3 => type_arguments.push(ResolvedType::I64),
            4 => {
                *instance = Some(hir::FunctionInstanceId::derive(
                    callee,
                    &[ResolvedType::I64],
                ))
            }
            5 => expression.ownership = hir::OwnershipMode::Own,
            _ => *callee = hir::DeclarationId::new("core.num.char_from_u8"),
        }
        assert_eq!(
            hir::validate(&forged).unwrap_err().code,
            "SPX-H006",
            "mutation {mutation}"
        );
        // Retention is serialization, never admission: identical validation must
        // reject hostile calls after decoding the persisted representation.
        let wire = crate::cache_codec::encode(&forged).unwrap();
        let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        assert_eq!(
            hir::validate(&restored).unwrap_err().code,
            "SPX-H006",
            "retained mutation {mutation}"
        );
    }
}

#[test]
fn unicode_scalar_source_domains_and_reserved_name_are_checked() {
    for (expression, code) in [
        ("char_from_i64(65u8)", "SPX-T205"),
        ("char_from_i64(65i32)", "SPX-T205"),
        ("char_from_i64(65usize)", "SPX-T205"),
        ("char_from_i64(65.0)", "SPX-T205"),
        ("char_from_i64('A')", "SPX-T205"),
        ("char_from_i64(true)", "SPX-T205"),
        ("char_from_i64(\"65\")", "SPX-T205"),
        ("char_from_i64()", "SPX-T204"),
        ("char_from_i64(1,2)", "SPX-T204"),
        ("char_from_i64<i64>(65)", "SPX-T225"),
    ] {
        let source =
            format!("module t; @id(\"app.main\") fn main()->i64 {{ let value={expression}; 0 }}");
        let ast = crate::parse(&source, "invalid.spx").unwrap();
        assert!(
            crate::verify::verify(&ast).iter().any(|d| d.code == code),
            "{expression}"
        );
        assert!(
            hir::resolve(&ast)
                .unwrap_err()
                .iter()
                .any(|d| d.code == code),
            "{expression}"
        );
    }
    let ast = crate::parse("module t; @id(\"authored.convert\") fn char_from_i64(value:i64)->char { 'A' } @id(\"app.main\") fn main()->i64 { 0 }", "reserved.spx").unwrap();
    assert!(crate::verify::verify(&ast)
        .iter()
        .any(|d| d.code == "SPX-S113"));
}
