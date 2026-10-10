use crate::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedType};

const SOURCE: &str = r#"module test.bulk_utf8;
@id("text.copy") fn copy(input:borrow Slice<u8>)->string {string_from_utf8(input)}
@id("app.main") fn main()->i64 {let raw=[65u8,0u8,195u8,169u8];let text=copy(array_as_slice(raw));str_len_bytes(string_as_str(text))}
"#;

fn call(program: &mut ResolvedProgram) -> &mut ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "text.copy")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("copy body")
    };
    tail
}

#[test]
fn bulk_utf8_source_graph_cache_and_signature_roundtrip() {
    let ast = crate::check(SOURCE, "bulk.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let round = crate::check(&canonical, "bulk.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&round));
    let graph = crate::graph::to_json(&ast).unwrap();
    assert!(graph.contains("\"callee\":\"core.string.from_utf8\""));
    crate::graph::verify_json(&round, &graph).unwrap();
    let hir = hir::resolve(&ast).unwrap();
    let wire = crate::cache_codec::encode(&hir).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    hir::validate(&restored).unwrap();
    assert_eq!(wire, crate::cache_codec::encode(&restored).unwrap());
    let changed = crate::check(&SOURCE.replace("65u8", "66u8"), "bulk.spx").unwrap();
    assert!(crate::graph::verify_json(&changed, &graph).is_err());
    assert!(crate::graph::verify_json(
        &ast,
        &graph.replace("core.string.from_utf8", "core.string.from_str")
    )
    .is_err());
    let op = super::by_name("string_from_utf8").unwrap();
    assert_eq!(super::by_id(op.id()), Some(op));
    assert_eq!(op.param_types(), &[ResolvedType::SliceU8]);
    assert_eq!(op.param_ownership(0), hir::OwnershipMode::Borrow);
    assert_eq!(op.return_type(), ResolvedType::String);
    assert_eq!(super::StringOp::CONVERSIONS.len(), 5);
    assert!(crate::project::validate_owned_utf8_closure_function(
        hir.functions
            .iter()
            .find(|f| f.id.as_str() == "text.copy")
            .unwrap()
    )
    .is_err());
    assert!(crate::wasm::emit_module_with_scalar_exports(&ast, &["app.main".into()]).is_err());
    // Selecting an existing map adapter must not bypass the frozen scalar
    // export refusal through its additive internal aggregate implementation.
    let map = crate::check(
        &SOURCE.replace("let raw=", "let values=string_map_new();let raw="),
        "map.spx",
    )
    .unwrap();
    assert!(crate::wasm::emit_module_with_scalar_exports(&map, &["app.main".into()]).is_err());
    // The standalone arena does not acquire the aggregate byte-slice ABI.
    assert!(crate::wasm::internal_strings::emit_module(
        &ast,
        &["app.main".into()],
        Default::default()
    )
    .is_err());
}

#[test]
fn bulk_utf8_hostile_retained_calls_and_borrow_provenance_fail_closed() {
    let program = hir::resolve(&crate::check(SOURCE, "bulk.spx").unwrap()).unwrap();
    for mutation in 0..7 {
        let mut forged = program.clone();
        let node = call(&mut forged);
        let ResolvedExprKind::Call {
            callee,
            args,
            type_arguments,
            instance,
        } = &mut node.kind
        else {
            panic!("call")
        };
        match mutation {
            0 => node.ty = ResolvedType::Str,
            1 => args[0].ty = ResolvedType::Str,
            2 => {
                args.clear();
            }
            3 => type_arguments.push(ResolvedType::U8),
            4 => *instance = Some(hir::FunctionInstanceId::derive(callee, &[ResolvedType::U8])),
            5 => node.ownership = hir::OwnershipMode::Borrow,
            _ => *callee = hir::DeclarationId::new("core.string.from_str"),
        }
        assert_eq!(
            hir::validate(&forged).unwrap_err().code,
            "SPX-H006",
            "mutation {mutation}"
        );
        let wire = crate::cache_codec::encode(&forged).unwrap();
        let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        assert_eq!(hir::validate(&restored).unwrap_err().code, "SPX-H006");
    }
    let invalid = r#"module loans; @id("app.main") fn main()->i64 {let original=bytes_zeroed(1usize);let view=bytes_as_slice(original);let moved=original;let text=string_from_utf8(view);str_len_bytes(string_as_str(text))}"#;
    assert!(
        crate::check(invalid, "loan.spx").is_err(),
        "a later bulk read must retain the input owner loan"
    );
}

#[test]
fn bulk_utf8_source_type_arity_and_exact_reserved_identity_are_checked() {
    for (expression, code) in [
        ("string_from_utf8(1)", "SPX-T205"),
        ("string_from_utf8(\"text\")", "SPX-T205"),
        ("string_from_utf8()", "SPX-T204"),
        ("string_from_utf8<i64>(1)", "SPX-T225"),
    ] {
        let source =
            format!("module t;@id(\"app.main\") fn main()->i64{{let text={expression};0}}");
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
    for declaration in [
        "@id(\"authored.copy\") fn string_from_utf8()->i64{0}",
        "@id(\"core.string.from_utf8\") record Alias{@id(\"alias.f\") value:i64,}",
    ] {
        let ast = crate::parse(
            &format!("module t;{declaration}@id(\"app.main\") fn main()->i64{{0}}"),
            "alias.spx",
        )
        .unwrap();
        assert!(crate::verify::verify(&ast)
            .iter()
            .any(|d| d.code == "SPX-S113"));
    }
}
