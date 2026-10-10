use super::*;
use crate::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedStatement};
const SOURCE: &str = include_str!("../../../tests/fixtures/scoped-vec-field-reads.spx");
fn checked(source: &str) -> ResolvedProgram {
    hir::resolve(&crate::check(source, "vec-field.spx").unwrap()).unwrap()
}
fn first(program: &mut ResolvedProgram) -> &mut ResolvedExpr {
    let f = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "scoped.inspect")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut f.body.kind else {
        panic!("block")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("view")
    };
    value
}
fn rejected(program: &ResolvedProgram) {
    assert_eq!(hir::validate(program).unwrap_err().code, "SPX-H006");
    let wire = crate::cache_codec::encode(program).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    assert!(hir::validate(&restored).is_err());
    assert!(crate::codegen::emit_hir_c(&restored).is_err());
    assert!(crate::wasm::emit_resolved_module(&restored).is_err());
}
#[test]
fn canonical_cache_roundtrip_retains_vector_field_and_full_carrier_loans() {
    let ast = crate::check(SOURCE, "vec-field.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::check(&canonical, "round.spx").unwrap())
    );
    let program = checked(SOURCE);
    hir::validate(&program).unwrap();
    hir::validate_stream_collection_record_program(&program, None).unwrap();
    let wire = crate::cache_codec::encode(&program).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    hir::validate(&restored).unwrap();
    assert_eq!(wire, crate::cache_codec::encode(&restored).unwrap());
    let f = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "scoped.inspect")
        .unwrap();
    assert!(f.loan_plan.loans.iter().any(|l| l.origin.projections
        == vec![hir::PlaceProjection::Field(hir::DeclarationId::new(
            "scoped.envelope.rows"
        ))]
        && !l.end_edges.is_empty()));
    let facts = program
        .declarations
        .byte_slice_provenances()
        .filter(|(_, p)| p.root_kind == hir::ByteSliceRootKind::OwnedVectorField)
        .collect::<Vec<_>>();
    assert_eq!(
        facts.len(),
        4,
        "named alias, range, fused String and Bytes views"
    );
    assert!(facts.iter().all(|(_, p)| p
        .vector_field
        .as_ref()
        .is_some_and(|v| v.index.as_str().contains(".arg."))
        && p.projections.len() == 1));
}
#[test]
fn source_rejects_dynamic_missing_temporary_and_mutating_index_reads() {
    for source in [
        SOURCE.replacen("0usize,\"text\"","0usize,string_from_i64(1)",1),
        SOURCE.replacen("0usize,\"text\"","0usize,\"absent\"",1),
        SOURCE.replacen("value.rows,0usize,\"text\"","vec_with_capacity<Row>(1usize),0usize,\"text\"",1),
        SOURCE.replace("@id(\"scoped.inspect\")", "@id(\"scoped.steal\") fn steal(value:own Envelope)->usize {0usize}\n@id(\"scoped.inspect\")")
            .replacen("value.rows,0usize,\"text\"","value.rows,steal(value),\"text\"",1),
    ] {
        let errors=crate::check(&source,"bad.spx").unwrap_err();
        assert!(errors.iter().any(|e|matches!(e.code,"SPX-T310"|"SPX-O118"|"SPX-T265")),"{errors:?}");
    }
    let source = SOURCE.replace("let alias=text;", "let moved=value.rows;let alias=text;");
    assert!(crate::check(&source, "move.spx")
        .unwrap_err()
        .iter()
        .any(|e| e.code == "SPX-T265"));
}
#[test]
fn field_identity_result_source_and_child_order_forgeries_fail_replay() {
    for mutation in 0..7 {
        let mut program = checked(SOURCE);
        let expr = first(&mut program);
        let ResolvedExprKind::VecFieldRead {
            element,
            field,
            bytes,
            args,
        } = &mut expr.kind
        else {
            panic!("read")
        };
        match mutation {
            0 => *field = hir::DeclarationId::new("scoped.row.number"),
            1 => *element = ResolvedType::I64,
            2 => *bytes = true,
            3 => args.swap(0, 1),
            4 => args[1].ty = ResolvedType::I64,
            5 => expr.ownership = hir::OwnershipMode::Own,
            _ => {
                let ResolvedExprKind::Place(place) = &mut args[0].kind else {
                    panic!("place")
                };
                place.projections.clear();
            }
        }
        rejected(&program);
    }
}
#[test]
fn declaration_provenance_and_loan_cache_authority_cannot_be_forged() {
    for id in ["scoped.row", "scoped.row.text", "scoped.envelope.rows"] {
        let mut program = checked(SOURCE);
        program
            .declarations
            .declarations
            .get_mut(&hir::DeclarationId::new(id))
            .unwrap()
            .identity_origin = hir::IdentityOrigin::CompilerOwned;
        rejected(&program);
    }
    for mutation in 0..4 {
        let mut program = checked(SOURCE);
        let fact = program
            .declarations
            .byte_slice_roots
            .values_mut()
            .find(|p| p.vector_field.is_some())
            .unwrap();
        match mutation {
            0 => fact.vector_field = None,
            1 => {
                fact.vector_field.as_mut().unwrap().field =
                    hir::DeclarationId::new("scoped.row.payload")
            }
            2 => {
                fact.vector_field.as_mut().unwrap().index =
                    hir::ExpressionId::from_owned("stale.index".to_owned())
            }
            _ => fact.projections.clear(),
        }
        rejected(&program);
    }
    let mut program = checked(SOURCE);
    let f = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "scoped.inspect")
        .unwrap();
    f.loan_plan.loans[0].origin.projections.clear();
    rejected(&program);
}
#[test]
fn reserved_declarations_and_frozen_profile_bodies_do_not_gain_authority() {
    let source = SOURCE.replace("@id(\"scoped.row.text\")", "@id(\"core.vec.field\")");
    assert!(crate::check(&source, "reserved.spx")
        .unwrap_err()
        .iter()
        .any(|e| e.code == "SPX-S113"));
    let program = checked(SOURCE);
    assert!(hir::validate_stream_record_program(&program, None).is_err());
    let f = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "scoped.inspect")
        .unwrap();
    assert!(hir::owned_leaf_collection::function_requires_profile_by(
        f,
        |id| program.types.iter().find(|t| &t.id == id)
    ));
}

#[test]
fn loans_end_after_last_use_but_prevent_view_escape_and_grouped_parent_transfer() {
    let prefix = r#"module loan.fields;
@id("row") record Row{@id("row.text") text:string,@id("row.n") n:i64}
@id("sink") fn sink(rows:own Vec<Row>)->i64{0}
@id("app.main") fn main()->i64 {0}
"#;
    let valid=format!("{prefix}\n@id(\"read\") fn read(rows:own Vec<Row>)->i64 {{ let view=vec_field<Row>(rows,0usize,\"text\");let length=str_len_bytes(view);let sorted=vec_sort_owned<Row>(rows);length+sink(sorted) }}");
    let program = checked(&valid);
    hir::validate(&program).unwrap();
    let live = valid.replace(
        "let sorted=vec_sort_owned<Row>(rows);length+sink(sorted)",
        "let sorted=vec_sort_owned<Row>(rows);str_len_bytes(view)+sink(sorted)",
    );
    assert!(crate::check(&live, "live.spx")
        .unwrap_err()
        .iter()
        .any(|e| e.code == "SPX-T265"));
    let escaping=format!("{prefix}\n@id(\"escape\") fn escape(rows:own Vec<Row>)->str {{vec_field<Row>(rows,0usize,\"text\")}}");
    assert!(crate::check(&escaping, "escape.spx").is_err());
}
