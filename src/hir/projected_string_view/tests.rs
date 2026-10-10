use super::*;
use crate::hir::{self, ResolvedExprKind, ResolvedProgram, ResolvedStatement};
const SOURCE: &str = include_str!("../../../tests/fixtures/projected-string-views.spx");

fn checked(source: &str) -> ResolvedProgram {
    hir::resolve(&crate::check(source, "projected-string.spx").unwrap()).unwrap()
}
fn rejected(program: &ResolvedProgram) {
    assert_eq!(hir::validate(program).unwrap_err().code, "SPX-H006");
    let wire = crate::cache_codec::encode(program).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    assert!(hir::validate(&restored).is_err());
    assert!(crate::codegen::emit_hir_c(&restored).is_err());
    assert!(crate::wasm::emit_resolved_module(&restored).is_err());
}
fn view_mut(program: &mut ResolvedProgram) -> &mut hir::ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "view.inspect")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!()
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!()
    };
    value
}

#[test]
fn projected_string_roundtrip_graph_cache_and_exact_loan_paths() {
    let ast = crate::check(SOURCE, "projected-string.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    assert_eq!(
        crate::format::canonical(&crate::check(&canonical, "round.spx").unwrap()),
        canonical
    );
    let program = checked(SOURCE);
    hir::validate(&program).unwrap();
    hir::validate_stream_collection_record_program(&program, None).unwrap();
    let path = vec![
        PlaceProjection::Field(DeclarationId::new("view.outer.inner")),
        PlaceProjection::Field(DeclarationId::new("view.inner.text")),
    ];
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "view.inspect")
        .unwrap();
    assert!(function
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.origin.projections == path && !loan.end_edges.is_empty()));
    let borrowed = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "view.measure")
        .unwrap();
    assert!(
        borrowed.loan_plan.loans.is_empty(),
        "borrow-only roots retain their caller's loan"
    );
    assert!(program
        .declarations
        .byte_slice_provenances()
        .any(|(_, p)| p.root_kind == hir::ByteSliceRootKind::OwnedString
            && p.projections == path
            && p.projected_type == ResolvedType::String));
    let graph = crate::graph::to_json(&ast).unwrap();
    assert!(graph.contains("semaprax.graph.v73"));
    assert!(graph.contains("semaprax.projected-string-views.v1"));
    crate::graph::verify_json(&ast, &graph).unwrap();
    assert!(crate::graph::verify_json(&ast, &graph.replace("graph.v73", "graph.v72")).is_err());
    assert!(crate::graph::legacy_graph_schema(&program).is_err());
    assert!(crate::graph::reject_evidence_schema("semaprax.graph.v73").is_err());
    assert_eq!(
        crate::graph::graph_schema_from_parts_and_instances(
            &program.interfaces,
            &program.types,
            &program.functions,
            &program.function_templates,
            &program.function_instances
        )
        .unwrap(),
        "semaprax.graph.v73"
    );
    let wire = crate::cache_codec::encode(&program).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    hir::validate(&restored).unwrap();
    assert_eq!(crate::cache_codec::encode(&restored).unwrap(), wire);
    let changed = crate::check(
        &SOURCE.replace("sibling:\"tail\"", "sibling:\"drift\""),
        "drift.spx",
    )
    .unwrap();
    assert!(crate::graph::verify_json(&changed, &graph).is_err());
}

#[test]
fn projected_string_root_field_origin_and_cache_forgeries_fail_closed() {
    for id in [
        "view.outer",
        "view.outer.inner",
        "view.inner",
        "view.inner.text",
    ] {
        for origin in [IdentityOrigin::Automatic, IdentityOrigin::CompilerOwned] {
            let mut program = checked(SOURCE);
            program
                .declarations
                .declarations
                .get_mut(&DeclarationId::new(id))
                .unwrap()
                .identity_origin = origin;
            rejected(&program);
        }
    }
    for mode in 0..4 {
        let mut program = checked(SOURCE);
        let expr = view_mut(&mut program);
        let ResolvedExprKind::BorrowPlace { place, .. } = &mut expr.kind else {
            panic!()
        };
        match mode {
            0 => place.root = hir::ValueId::new("forged.root".to_owned()),
            1 => {
                place.projections[1] =
                    PlaceProjection::Field(DeclarationId::new("view.inner.sibling"))
            }
            2 => place.projections.reverse(),
            _ => place
                .projections
                .push(PlaceProjection::Field(DeclarationId::new(
                    "view.inner.text",
                ))),
        }
        rejected(&program);
    }
    for mode in 0..3 {
        let mut program = checked(SOURCE);
        let provenance = program
            .declarations
            .byte_slice_roots
            .values_mut()
            .find(|p| {
                p.root_kind == hir::ByteSliceRootKind::OwnedString && !p.projections.is_empty()
            })
            .unwrap();
        match mode {
            0 => provenance.projections.clear(),
            1 => provenance.root_kind = hir::ByteSliceRootKind::BorrowedStr,
            _ => provenance.projected_type = ResolvedType::Bytes,
        }
        rejected(&program);
    }
    let mut program = checked(SOURCE);
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "view.inspect")
        .unwrap();
    let loan = function
        .loan_plan
        .loans
        .iter_mut()
        .find(|l| !l.origin.projections.is_empty())
        .unwrap();
    loan.origin.projections.pop();
    rejected(&program);
}

#[test]
fn source_refuses_temporary_roots_and_moves_during_projected_loans() {
    for expression in ["make().inner.text", "Inner{text:\"x\",sibling:\"y\"}.text"] {
        let source = SOURCE.replace(
            "string_as_str(value.inner.text)",
            &format!("string_as_str({expression})"),
        );
        let errors = crate::check(&source, "temporary.spx").unwrap_err();
        let diagnostic = errors
            .iter()
            .find(|e| e.code == "SPX-T266")
            .expect("temporary projected String roots retain SPX-T266");
        assert_eq!(
            diagnostic.message,
            "borrowed view `string_as_str` requires a named String owner or an authenticated named-record path to a String field"
        );
    }
    let source = SOURCE.replace(
        "let count=str_len_bytes(alias)",
        "let moved=sink(value,0);let count=str_len_bytes(alias)",
    );
    let errors = crate::check(&source, "move.spx").unwrap_err();
    assert!(errors.iter().any(|e| e.code == "SPX-T265"), "{errors:?}");
    let source=SOURCE.replace("@id(\"view.sink\")", "@id(\"view.bad\") fn bad(view:borrow str,value:own Outer)->i64 {str_len_bytes(view)}\n@id(\"view.sink\")")
        .replace("sink(value,count)","bad(string_as_str(value.inner.text),value)");
    let errors = crate::check(&source, "grouped.spx").unwrap_err();
    assert!(errors.iter().any(|e| e.code == "SPX-T265"), "{errors:?}");
    let source=SOURCE.replace("@id(\"view.sink\")", "@id(\"view.bad_record\") fn bad_record(view:borrow Outer,value:own Outer)->i64 {0}\n@id(\"view.sink\")")
        .replace("sink(value,count)","bad_record(value,value)");
    let errors = crate::check(&source, "grouped-record.spx").unwrap_err();
    assert!(errors.iter().any(|e| e.code == "SPX-T265"), "{errors:?}");
}

#[test]
fn old_profiles_and_generic_or_unidentified_paths_gain_no_authority() {
    let old = r#"module old.named; @id("app.main") fn main()->i64 {
        let text="x";let bytes=str_as_bytes(string_as_str(text));i64_from_usize(byte_len(bytes))
    }"#;
    let old_ast = crate::check(old, "old.spx").unwrap();
    assert_eq!(
        crate::graph::graph_schema(&hir::resolve(&old_ast).unwrap()).unwrap(),
        "semaprax.graph.v71"
    );
    let plain = SOURCE
        .replace(" @id(\"view.outer.words\") words:Vec<string>,", "")
        .replace(",words:vec_with_capacity<string>(0usize)", "");
    let plain_program = checked(&plain);
    hir::validate(&plain_program).unwrap();
    assert_eq!(
        crate::graph::graph_schema(&plain_program).unwrap(),
        "semaprax.graph.v73"
    );
    crate::codegen::emit_hir_c(&plain_program).unwrap();
    crate::wasm::emit_resolved_module(&plain_program).unwrap();
    let program = checked(SOURCE);
    assert!(hir::validate_stream_owned_program(&program, None).is_err());
    assert!(hir::validate_stream_record_program(&program, None).is_err());
    for id in ["view.inner.text", "view.inner", "view.outer.inner"] {
        let source = SOURCE.replace(&format!("@id(\"{id}\")"), "");
        assert!(crate::check(&source, "implicit.spx").is_err());
    }
    let root = ResolvedType::Nominal {
        declaration: DeclarationId::new("view.outer"),
        arguments: vec![ResolvedType::I64],
    };
    assert!(!admitted(
        &program.declarations,
        &root,
        &[
            PlaceProjection::Field(DeclarationId::new("view.outer.inner")),
            PlaceProjection::Field(DeclarationId::new("view.inner.text"))
        ]
    ));
}

#[test]
fn inline_projected_string_readers_replay_full_root_paths_and_refuse_forged_views() {
    let source = format!("{}\n@id(\"view.inline\") fn inline(value:borrow Outer)->i64 {{i64_from_usize(byte_len(str_as_bytes(string_as_str(value.inner.text))))}}", SOURCE.replace("str_len_bytes(text)", "str_len_bytes(string_as_str(value.inner.text))"));
    let ast = crate::check(&source, "inline-projected.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    assert_eq!(
        crate::format::canonical(&crate::check(&canonical, "inline-round.spx").unwrap()),
        canonical
    );
    let program = checked(&source);
    hir::validate(&program).unwrap();
    crate::codegen::emit_hir_c(&program).unwrap();
    wasmparser::Validator::new()
        .validate_all(&crate::wasm::emit_resolved_module(&program).unwrap())
        .unwrap();
    let wire = crate::cache_codec::encode(&program).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    hir::validate(&restored).unwrap();
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
    for mode in 0..3 {
        let mut hostile = restored.clone();
        let function = hostile
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "view.inline")
            .unwrap();
        let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("body");
        };
        let ResolvedExprKind::Call { args, .. } = &mut tail.kind else {
            panic!("widen");
        };
        let ResolvedExprKind::Call { args, .. } = &mut args[0].kind else {
            panic!("length");
        };
        let ResolvedExprKind::BorrowPlace { operation, place } = &mut args[0].kind else {
            panic!("fused view");
        };
        match mode {
            0 => place.root = hir::ValueId::new("forged.inline.root".to_owned()),
            1 => {
                place.projections.pop();
            }
            _ => *operation = DeclarationId::new(crate::byte_ops::BYTES_AS_SLICE_ID),
        }
        rejected(&hostile);
    }
}
