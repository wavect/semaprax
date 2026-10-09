use super::*;

const SCHEMA: &str = r#"module schema;
@id("app.patient") record Patient {
 @id("app.patient.id") id:string,
 @id("app.patient.arrival") arrival:i64,
 @id("app.patient.service") service:i64,
 @id("app.patient.priority") priority:i64,
 @id("app.patient.deadline") deadline:i64,
}
@id("app.request") record Request {
 @id("app.request.servers") servers:Vec<string>,
 @id("app.request.patients") patients:Vec<Patient>,
}
@id("app.schema.anchor") fn schema_anchor()->i64 {0}
"#;

#[test]
fn identifier_view_derivation_has_checked_spans_and_bounded_array_owners() {
    let program = crate::parse(SCHEMA, "schema.spx").unwrap();
    let fragment = source(&program, &program.types[0]).unwrap();
    let parsed = crate::parse(&fragment, "views.spx").unwrap();
    let renderers: Vec<_> = parsed
        .functions
        .iter()
        .filter(|function| {
            function.name.ends_with("_identifier_render") || function.name.ends_with("_view_render")
        })
        .collect();
    assert_eq!(renderers.len(), 2);
    assert!(renderers
        .iter()
        .all(|function| function.return_type == Type::String));
    let encode = parsed
        .types
        .iter()
        .find(|declaration| declaration.name == "PatientJsonViewEncode")
        .unwrap();
    let TypeDeclarationKind::Variant { cases } = &encode.kind else {
        panic!("encode variant")
    };
    assert_eq!(cases[0].fields[0].ty, Type::String);
    assert!(parsed.permits.is_empty());
    assert!(parsed
        .functions
        .iter()
        .all(|function| function.effects.is_empty()));
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "views.spx").unwrap())
    );
    assert!(fragment.contains("end<=length"));
    assert!(fragment.contains("closed==end && total>=1usize && total<=16usize"));
    assert!(fragment.contains("vec_with_capacity<PatientJsonView>(256usize)"));
    assert!(fragment.contains("jv_decoded_token_eq(input,other.id_start,adjusted.id_start)"));
    assert!(fragment.contains("size>131072usize-total-comma"));
    assert!(fragment.contains("if required==18446744073709551615usize || required>output_limit"));
    assert!(fragment.len() <= super::super::MAX_GENERATED_BYTES);
}

#[test]
fn request_schema_reconstructs_two_collection_outcome_and_refuses_excess_capacity() {
    let mut program = crate::parse(SCHEMA, "schema.spx").unwrap();
    let fragment = request_source(&program, &program.types[1]).unwrap();
    let parsed = crate::parse(&fragment, "request.spx").unwrap();
    assert!(parsed.permits.is_empty());
    assert!(fragment.contains("servers:Vec<RequestJsonIdentifierSpan>"));
    assert!(fragment.contains("patients:Vec<PatientJsonView>"));
    assert!(fragment.contains("servers)>=8usize"));
    assert!(fragment.contains("patients)>=256usize"));
    assert!(fragment.contains("required=if valid{required+patient_size}"));
    assert!(fragment.len() <= super::super::MAX_GENERATED_BYTES);
    // The fixed physical buffer admission is derived from authenticated field
    // widths, not a byte cap on raw whitespace or escaped token spellings.
    let TypeDeclarationKind::Record { fields } = &mut program.types[0].kind else {
        panic!("record")
    };
    for field in fields {
        field.name = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789__".to_owned();
    }
    assert_eq!(
        request_source(&program, &program.types[1]).unwrap_err()[0].code,
        "SPX-J180"
    );
    let program = crate::parse(SCHEMA, "schema.spx").unwrap();
    let mut reverse = program.types[1].clone();
    let TypeDeclarationKind::Record { fields } = &mut reverse.kind else {
        panic!("record")
    };
    fields.reverse();
    assert_eq!(
        request_source(&program, &reverse).unwrap_err()[0].code,
        "SPX-J180"
    );
}

#[test]
fn stream_request_requires_original_permission_and_emits_whole_grammar_before_refusal() {
    let mut program = crate::parse(SCHEMA, "schema.spx").unwrap();
    assert_eq!(
        stream_request_source(&program, &program.types[1]).unwrap_err()[0].code,
        "SPX-J180"
    );
    program.permits.push("process.stdin.read".to_owned());
    let source = stream_request_source(&program, &program.types[1]).unwrap();
    assert!(!source.contains("__FINISH_VALUE__"));
    let parsed = crate::parse(&source, "stream.spx").unwrap();
    // The caller, rather than the generator, supplied module authority.
    assert!(parsed.permits.is_empty());
    let function = parsed
        .functions
        .iter()
        .find(|function| function.name == "json_Request_stream_normalize")
        .unwrap();
    assert_eq!(function.effects, vec!["process.stdin.read".to_owned()]);
    assert!(source.contains("while !stdin_stream_eof(reader)"));
    assert!(source.contains("if grammar!=0 {RequestJsonStreamInput::Error"));
    assert!(source.contains("if overflow {RequestJsonStreamInput::Error{code:9"));
    assert!(source.contains("raw<18446744073709551615usize"));
    assert!(source.contains("bytes_zeroed(131072usize)"));
    assert!(source.len() <= super::super::MAX_GENERATED_BYTES);
}
