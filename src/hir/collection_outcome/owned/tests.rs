use super::*;

const SOURCE: &str = r#"module owned.codec;
@id("row") record Row { @id("row.n") n:i64, @id("row.label") label:string, }
@id("out") variant Output {
 @id("out.ok") Ready { @id("out.words") words:Vec<string>, @id("out.rows") rows:Vec<Row>, },
 @id("out.bad") Invalid { @id("out.code") code:i64, @id("out.offset") offset:usize, @id("out.field") field:i64, },
}
@id("forward") fn forward(value:own Output)->Output {value}
@id("app.main") fn main()->i64 {
 let out=forward(Output::Invalid{code:7,offset:0usize,field:0});
 match own out {Output::Ready{words,rows}=>0,Output::Invalid{code,offset,field}=>code,}
}
"#;

fn checked() -> ResolvedProgram {
    resolve(&crate::check(SOURCE, "owned-codec-outcome.spx").unwrap()).unwrap()
}
fn ty() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new("out"),
        arguments: vec![],
    }
}

#[test]
fn owned_collection_outcome_replays_even_without_a_vector_constructor() {
    let program = checked();
    assert!(admitted(&program.declarations, &ty()));
    assert!(!super::super::admitted(&program.declarations, &ty()));
    validate(&program).unwrap();
    crate::hir::validate_stream_owned_program(&program, None).unwrap();
    assert!(crate::hir::validate_stream_record_program(&program, None).is_err());
    assert!(crate::hir::validate_stream_data_program(&program, None).is_err());
    let forward = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "forward")
        .unwrap();
    assert!(owned_leaf_collection::function_requires_profile_by(
        forward,
        |id| program
            .types
            .iter()
            .find(|declaration| &declaration.id == id)
    ));
    let native = crate::codegen::emit_hir_c(&program).unwrap();
    assert!(native.contains("spx_leaf_drop_storage"));
    let wasm = crate::wasm::emit_resolved_module(&program).unwrap();
    wasmparser::Validator::new().validate_all(&wasm).unwrap();
}

#[test]
fn owned_collection_outcome_rejects_origin_field_and_ownership_drift() {
    for id in ["out", "out.ok", "out.rows"] {
        for origin in [IdentityOrigin::Automatic, IdentityOrigin::CompilerOwned] {
            let mut forged = checked();
            forged
                .declarations
                .declarations
                .get_mut(&DeclarationId::new(id))
                .unwrap()
                .identity_origin = origin;
            assert!(!admitted(&forged.declarations, &ty()));
            assert!(validate(&forged).is_err());
        }
    }
    let mut forged = checked();
    forged
        .declarations
        .variant_cases
        .get_mut(&DeclarationId::new("out"))
        .unwrap()[1]
        .fields[1]
        .ty = ResolvedType::I64;
    assert!(!admitted(&forged.declarations, &ty()));
    assert!(validate(&forged).is_err());
    let mut forged = checked();
    forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "forward")
        .unwrap()
        .params[0]
        .ownership = OwnershipMode::Value;
    assert!(validate(&forged).is_err());
    assert!(crate::codegen::emit_hir_c(&forged).is_err());
    assert!(crate::wasm::emit_resolved_module(&forged).is_err());
}

#[test]
fn owned_collection_outcome_refuses_unbounded_or_nonowned_shapes() {
    for source in [
        SOURCE.replace("words:Vec<string>", "words:Vec<i64>"),
        SOURCE.replace("rows:Vec<Row>", "rows:Row"),
        SOURCE.replace(
            "rows:Vec<Row>,",
            "rows:Vec<Row>, @id(\"out.third\") third:Vec<string>,",
        ),
        SOURCE.replace("offset:usize", "offset:bool"),
    ] {
        let errors = crate::check(&source, "refused-owned-codec.spx").unwrap_err();
        assert!(errors.iter().any(|d| d.code == "SPX-T215"), "{errors:?}");
    }
}
