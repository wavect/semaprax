use super::*;

const TYPES: &str = r#"module normalized.join;
@id("input") variant Input {
@id("input.ready") Ready {@id("input.bytes") bytes:Bytes,@id("input.length") length:usize,},
@id("input.error") Error {@id("input.code") code:i64,@id("input.offset") offset:usize,@id("input.field") field:i64,},
}
@id("record") record Payload {@id("record.text") text:string,}
@id("output") variant Output {
@id("output.ready") Ready {@id("output.value") value:Payload,},
@id("output.error") Error {@id("output.code") code:i64,@id("output.offset") offset:usize,@id("output.field") field:i64,},
}
"#;
const BODY: &str = r#"
@id("join") fn join(input:own Input)->Output {
match own input {
Input::Error{code,offset,field}=>Output::Error{code:code,offset:offset,field:field},
Input::Ready{bytes,length}=>Output::Ready{value:Payload{text:string_from_utf8(byte_range(bytes_as_slice(bytes),0usize,length))}},
}}
@id("main") fn main()->i64 {
let first=join(Input::Error{code:7,offset:2usize,field:3});
let a=match own first{Output::Error{code,offset,field}=>code,Output::Ready{value}=>0,};
let second=join(Input::Ready{bytes:bytes_zeroed(0usize),length:0usize});
let b=match own second{Output::Error{code,offset,field}=>0,Output::Ready{value}=>1,};a+b
}
"#;
fn ty(id: &str) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments: vec![],
    }
}

#[test]
fn normalization_to_nested_owning_join_replays_source_hir_and_cleanup() {
    let source = format!("{TYPES}{BODY}");
    let parsed = crate::check(&source, "stream-join.spx").unwrap();
    let program = resolve(&parsed).unwrap();
    assert!(admitted(&program.declarations, &ty("input")));
    assert!(!super::super::runtime_admitted(
        &program.declarations,
        &ty("input")
    ));
    validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "join")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("block")
    };
    assert!(super::super::match_join(&program.declarations, tail));
    crate::hir::validate_stream_nested_outcome_program(&program, None).unwrap();
    assert!(crate::hir::validate_stream_collection_record_program(&program, None).is_err());
    crate::codegen::emit_hir_c(&program).unwrap();
    let wasm = crate::wasm::emit_resolved_module(&program).unwrap();
    wasmparser::Validator::new().validate_all(&wasm).unwrap();
    for mode in [ResolvedMatchMode::Borrow, ResolvedMatchMode::Value] {
        assert!(!match_result(
            &program.declarations,
            &ty("input"),
            mode,
            &ty("output"),
            OwnershipMode::Own
        ));
    }
    assert!(!match_result(
        &program.declarations,
        &ty("input"),
        ResolvedMatchMode::Own,
        &ResolvedType::String,
        OwnershipMode::Own
    ));
    assert!(!match_result(
        &program.declarations,
        &ty("input"),
        ResolvedMatchMode::Own,
        &ty("output"),
        OwnershipMode::Value
    ));
}

#[test]
fn normalization_join_refuses_near_shapes_identity_loss_and_general_results() {
    for renamed in [TYPES
        .replace("Input", "UnrelatedCarrier")
        .replace("Ready", "ArbitrarySuccess")
        .replace("Error", "ArbitraryFailure")]
    {
        let source = format!("{renamed}@id(\"main\") fn main()->i64{{0}}");
        let program = resolve(&crate::check(&source, "opaque-stream-input.spx").unwrap()).unwrap();
        assert!(admitted(&program.declarations, &ty("input")));
    }
    for changed in [
        TYPES.replace("bytes:Bytes", "bytes:string"),
        TYPES.replace("length:usize", "length:i64"),
        TYPES.replace("@id(\"input.ready\")", ""),
        TYPES.replace("@id(\"input.bytes\")", ""),
        TYPES.replace("@id(\"input\")", ""),
        TYPES.replace(
            "@id(\"input.field\") field:i64",
            "@id(\"input.field\") field:bool",
        ),
        TYPES.replace(
            "length:usize",
            "length:usize,@id(\"input.extra\") extra:i64",
        ),
    ] {
        let declarations = format!("{changed}@id(\"main\") fn main()->i64{{0}}");
        let parsed = crate::check(&declarations, "hostile-types.spx").unwrap();
        let program = resolve(&parsed).unwrap();
        assert!(!admitted(&program.declarations, &ty("input")), "{changed}");
        let source = format!("{changed}{BODY}");
        let diagnostics = crate::check(&source, "hostile-join.spx").unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == "SPX-T258" || d.code == "SPX-T216"),
            "{diagnostics:?}"
        );
    }
    let general = format!(
        r#"{TYPES}
@id("bad") fn bad(input:own Input)->string {{match own input {{
Input::Error{{code,offset,field}}=>"error",
Input::Ready{{bytes,length}}=>"ready",
}}}}
@id("main") fn main()->i64{{0}}
"#
    );
    let errors = crate::check(&general, "general-result.spx").unwrap_err();
    assert!(errors.iter().any(|d| d.code == "SPX-T258"));
    assert!(errors.iter().any(|d| d.code == "SPX-T216"));
}
