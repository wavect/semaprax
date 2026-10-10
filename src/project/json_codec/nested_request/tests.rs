use super::*;
const SCHEMA: &str = r#"module orders;
@id("orders.configuration") record Configuration {@id("orders.configuration.label") label:string,@id("orders.configuration.retry") retry:usize,}
@id("orders.line") record Line {@id("orders.line.sku") sku:string,@id("orders.line.quantity") quantity:u8,}
@id("orders.request") record OrderRequest {@id("orders.request.configuration") configuration:Configuration,@id("orders.request.lines") lines:Vec<Line>,@id("orders.request.urgent") urgent:bool,}
@id("schema.anchor") fn anchor()->i64{0}
"#;
fn generate(source: &str, text: usize, array: usize) -> Result<String, Vec<Diagnostic>> {
    let program = crate::parse(source, "schema.spx").unwrap();
    derive(&program, &program.types[2], text, array)
}
#[test]
fn nested_request_is_deterministic_canonical_and_constructs_only_after_validation() {
    let source = generate(SCHEMA, 16, 8).unwrap();
    assert_eq!(source, generate(SCHEMA, 16, 8).unwrap());
    assert_ne!(source, generate(SCHEMA, 15, 8).unwrap());
    assert_ne!(source, generate(SCHEMA, 16, 7).unwrap());
    assert!(source.contains("fn json_OrderRequest_nested_decode(input:borrow Slice<u8>,input_limit:usize)->OrderRequestJsonNestedDecode"));
    assert!(source.contains(".json.nested.decode-result"));
    assert!(!source.contains(".json.utf8."));
    assert!(!source.contains("stdin"));
    assert!(!source.contains("_identifier_valid"));
    let api = source
        .split("fn json_OrderRequest_nested_decode")
        .nth(1)
        .unwrap();
    assert!(api.find("length>input_limit").unwrap() < api.find("jv_strict_end").unwrap());
    assert!(
        api.find("jv_strict_end").unwrap() < api.find("json_OrderRequest_nested_check_0").unwrap()
    );
    assert!(
        api.find("status.code!=0").unwrap() < api.find("json_OrderRequest_nested_build_0").unwrap()
    );
    assert!(!api.contains("vec_with_capacity"));
    assert!(!api.contains("string_concat"));
    let parsed = crate::parse(&source, "generated.spx").unwrap();
    assert!(parsed.permits.is_empty());
    assert!(parsed.functions.iter().all(|f| f.effects.is_empty()));
    let outcome = parsed
        .types
        .iter()
        .find(|t| t.name == "OrderRequestJsonNestedDecode")
        .unwrap();
    let TypeDeclarationKind::Variant { cases } = &outcome.kind else {
        panic!("ordinary variant")
    };
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0].fields.len(), 1);
    assert_eq!(cases[0].fields[0].name, "value");
    assert_eq!(
        cases[1].fields.iter().map(|f| &f.ty).collect::<Vec<_>>(),
        vec![&Type::I64, &Type::Usize, &Type::I64]
    );
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "roundtrip.spx").unwrap())
    );
    assert!(source.len() <= super::super::MAX_GENERATED_BYTES);
}
#[test]
fn nested_request_field_paths_are_preorder_and_wire_order_is_independent() {
    let program = crate::parse(SCHEMA, "schema.spx").unwrap();
    let shape = descriptor::validate(&program, &program.types[2], 16, 8).unwrap();
    assert_eq!(
        shape
            .root
            .fields
            .iter()
            .map(|f| f.ordinal)
            .collect::<Vec<_>>(),
        [1, 4, 7]
    );
    let descriptor::Kind::Record(configuration) = &shape.root.fields[0].kind else {
        panic!("config")
    };
    assert_eq!(
        configuration
            .fields
            .iter()
            .map(|f| f.ordinal)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    let descriptor::Kind::Vector { kind, .. } = &shape.root.fields[1].kind else {
        panic!("array")
    };
    let descriptor::Kind::Record(line) = kind.as_ref() else {
        panic!("row")
    };
    assert_eq!(
        line.fields.iter().map(|f| f.ordinal).collect::<Vec<_>>(),
        [5, 6]
    );
    let renamed = SCHEMA
        .replace("configuration:Configuration", "options:Configuration")
        .replace("lines:Vec<Line>", "entries:Vec<Line>")
        .replace("sku:string", "output_0:string")
        .replace("label:string", "input:string");
    let generated = generate(&renamed, 16, 8).unwrap();
    assert!(generated.contains("options:json_OrderRequest_nested_build_1"));
    assert!(generated.contains("entries:json_OrderRequest_nested_array_build_4"));
    assert!(generated.contains("input:json_OrderRequest_nested_text"));
    assert!(generated.contains("output_0:json_OrderRequest_nested_text"));
    crate::parse(&generated, "renamed.spx").unwrap();
}
#[test]
fn nested_request_refuses_unsupported_shape_identity_cycle_and_policy_bounds() {
    for (text, array) in [(0, 8), (65, 8), (16, 0), (16, 257)] {
        assert_eq!(
            generate(SCHEMA, text, array).unwrap_err()[0].code,
            "SPX-J180"
        );
    }
    for bad in [
        SCHEMA.replace("@id(\"orders.configuration.label\") ", ""),
        SCHEMA.replace("label:string", "label:Bytes"),
        SCHEMA.replace("retry:usize", "retry:f64"),
        SCHEMA.replace("sku:string", "sku:Configuration"),
        SCHEMA.replace("urgent:bool", "urgent:Vec<string>"),
        SCHEMA.replace("configuration:Configuration", "configuration:OrderRequest"),
        SCHEMA.replace("lines:Vec<Line>", "lines:Vec<Vec<Line>>"),
        SCHEMA.replace("lines:Vec<Line>", "lines:Option<Line>"),
    ] {
        assert_eq!(
            generate(&bad, 16, 8).unwrap_err()[0].code,
            "SPX-J180",
            "{bad}"
        );
    }
    let mut program = crate::parse(SCHEMA, "schema.spx").unwrap();
    program.types[0].explicit_id = false;
    assert!(derive(&program, &program.types[2], 16, 8).is_err());
    let wide = SCHEMA.replace("quantity:u8", "quantity:string");
    assert!(generate(&wide, 64, 128).is_ok());
    assert_eq!(generate(&wide, 64, 256).unwrap_err()[0].code, "SPX-J180");
}
#[test]
fn nested_request_uses_existing_scalar_and_vector_shapes_without_name_special_cases() {
    for element in ["string", "i64", "u8", "usize", "bool"] {
        let schema = SCHEMA.replace("Vec<Line>", &format!("Vec<{element}>"));
        let generated = generate(&schema, 64, 256).unwrap();
        assert!(generated.contains(&format!("vec_push<{element}>(values,value)")));
        crate::parse(&generated, "element.spx").unwrap();
    }
    let copy = SCHEMA.replace("sku:string", "sku:i64");
    assert!(generate(&copy, 64, 256).is_ok());
    let source = generate(SCHEMA, 16, 8).unwrap();
    assert!(source.contains("number>1844674407370955161usize"));
    assert!(source.contains("digit>5usize"));
    assert!(source.contains("number<0 || number>255"));
    assert!(source.contains("count>=8usize"));
    assert!(source.contains("error=8;offset=cursor"));
    assert!(source.contains("field=if status.code==0{field}else{status.field}"));
    assert!(source.contains("scalar>=0 && scalar<=1114111"));
    assert!(source.contains("scalar>=55296 && scalar<=57343"));
}
