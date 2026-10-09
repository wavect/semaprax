use super::*;
const SOURCE: &str = r#"module logical.request;
@id("patient") record Patient {
 @id("patient.id") id:string, @id("patient.arrival") arrival:i64,
 @id("patient.service") service:i64, @id("patient.priority") priority:i64,
 @id("patient.deadline") deadline:i64,
}
@id("request") record Request {
 @id("request.servers") servers:Vec<string>, @id("request.patients") patients:Vec<Patient>,
}
@id("wrapper") record Wrapper { @id("wrapper.request") request:Request, }
@id("app.main") fn main()->i64 {0}
"#;
fn nominal(id: &str) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments: Vec::new(),
    }
}
fn checked() -> ResolvedProgram {
    resolve(&crate::check(SOURCE, "logical-request.spx").unwrap()).unwrap()
}
#[test]
fn logical_collection_declarations_keep_source_graph_and_affine_facts() {
    let ast = crate::check(SOURCE, "logical-request.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let round = crate::check(&canonical, "logical-request.spx").unwrap();
    assert_eq!(crate::format::canonical(&round), canonical);
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
    assert!(graph.contains("request.servers") && graph.contains("patient.id"));
    let changed = crate::check(
        &SOURCE.replace("priority:i64", "priority:i32"),
        "logical-request.spx",
    )
    .unwrap();
    assert_ne!(crate::graph::to_json(&changed).unwrap(), graph);
    assert!(crate::graph::verify_json(&changed, &graph).is_err());
    let program = checked();
    validate(&program).unwrap();
    for element in [ResolvedType::String, nominal("patient")] {
        let ty = ResolvedType::Nominal {
            declaration: DeclarationId::new(crate::prelude::VEC_ID),
            arguments: vec![element],
        };
        assert!(vector(&program.declarations, &ty));
        let facts = program.declarations.type_facts(&ty).unwrap();
        assert!(!facts.copy && facts.needs_drop && facts.sized && !facts.contains_resource);
        assert!(facts.layout_key.starts_with("declared-vector-only:"));
        assert!(!crate::cleanup::is_owned_bounded_vec_type(&ty));
        assert!(!super::super::copy_record_collection::is_vec(
            &program.declarations,
            &ty
        ));
    }
    assert!(contains(&program.declarations, &nominal("wrapper")));
    // The profile considers only authenticated runtime closure; logical type names
    // confer no exception when someone actually places one in that closure.
    super::super::validate_stream_record_program(&program, None).unwrap();
}
#[test]
fn logical_collection_declarations_refuse_all_unused_runtime_uses() {
    for suffix in [
        "@id(\"unused\") fn unused(input:own Request)->i64 {0}",
        "@id(\"unused\") fn unused(input:borrow Wrapper)->i64 {0}",
        "@id(\"unused\") fn unused(input:own Vec<string>)->i64 {0}",
        "@id(\"unused\") fn unused()->i64 {let v=vec_with_capacity<string>(0usize);0}",
        "@id(\"unused\") fn unused()->i64 {let v=vec_with_capacity<Patient>(0usize);0}",
        "@id(\"unused\") fn unused()->i64 {let r=Request{servers:vec_with_capacity<string>(0usize),patients:vec_with_capacity<Patient>(0usize)};0}",
        "@id(\"unused\") fn unused()->i64 {let r:Request=0;0}",
    ] {
        let diagnostics=crate::check(&format!("{SOURCE}{suffix}"),"unused-logical.spx").unwrap_err();
        assert!(diagnostics.iter().any(|d|d.code=="SPX-T281"),"{diagnostics:?}");
    }
}
#[test]
fn logical_collection_hir_replays_declaration_identity_and_refuses_runtime_authority() {
    let program = checked();
    let mut origin = program.clone();
    origin
        .declarations
        .declarations
        .get_mut(&DeclarationId::new("patient"))
        .unwrap()
        .identity_origin = IdentityOrigin::Automatic;
    assert!(!text_element(&origin.declarations, &nominal("patient")));
    assert!(validate(&origin).is_err());
    let mut fields = program.clone();
    fields
        .declarations
        .record_fields
        .get_mut(&DeclarationId::new("patient"))
        .unwrap()[0]
        .ty = ResolvedType::Bytes;
    assert!(!text_element(&fields.declarations, &nominal("patient")));
    assert!(validate(&fields).is_err());
    let mut stale = program.clone();
    let request = nominal("request");
    let mut facts = stale.declarations.type_facts(&request).unwrap();
    facts.copy = true;
    facts.needs_drop = false;
    stale
        .declarations
        .type_facts_by_id
        .insert(request.identity_key(), facts);
    assert!(validate(&stale).is_err());
    let mut body = program.clone();
    body.functions[0].body.ty = request;
    assert!(validate(&body).is_err());
    assert!(crate::codegen::emit_hir_c(&body).is_err());
    assert!(crate::wasm::emit_resolved_module(&body).is_err());
    assert!(super::super::validate_stream_record_program(&body, None).is_err());
}
