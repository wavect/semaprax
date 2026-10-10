use super::*;

const SCHEMA: &str = r#"module response;
@id("r.config") record Config {@id("r.config.label") label:string,@id("r.config.retry") retry:usize,}
@id("r.row") record Row {@id("r.row.sku") sku:string,@id("r.row.count") count:u8,}
@id("r.root") record Report {@id("r.root.config") config:Config,@id("r.root.rows") rows:Vec<Row>,@id("r.root.ok") ok:bool,}
"#;
fn generate(source: &str, strings: usize, rows: usize) -> Result<String, Vec<Diagnostic>> {
    let program = crate::parse(source, "schema.spx").unwrap();
    derive(&program, &program.types[2], strings, rows)
}

#[test]
fn nested_response_emits_recursive_borrowed_preflight_and_declaration_order() {
    let output = generate(SCHEMA, 16, 8).unwrap();
    assert_eq!(output, generate(SCHEMA, 16, 8).unwrap());
    assert_ne!(output, generate(SCHEMA, 15, 8).unwrap());
    assert_ne!(output, generate(SCHEMA, 16, 7).unwrap());
    assert!(output.contains("object_len_1(value:borrow Report)"));
    assert!(output.contains("object_len_4(values:borrow Vec<Row>,at:usize)"));
    assert!(output.contains("vec_field<Row>(values,at,\"sku\")"));
    assert!(!output.contains("vec_clone_at"));
    assert!(!output.contains("vec_into_iter"));
    assert!(output.contains("count<=8usize"));
    assert!(output.contains("length<=16usize"));
    assert!(output.contains("size>131072usize-total"));
    let api = output
        .split("fn json_Report_nested_response_encode(")
        .nth(1)
        .unwrap();
    assert!(api.find("required>output_limit").unwrap() < api.find("object_render_0").unwrap());
    let body = output
        .split("fn json_Report_nested_response_object_render_0")
        .nth(1)
        .unwrap();
    assert!(body.find("object_render_1(value)").unwrap() < body.find("value.rows").unwrap());
    assert!(body.find("value.rows").unwrap() < body.find("value.ok").unwrap());
    let parsed = crate::parse(&output, "generated.spx").unwrap();
    assert!(parsed.functions.iter().all(|f| f.effects.is_empty()));
    assert!(parsed
        .functions
        .iter()
        .all(|f| f.stable_id.starts_with("r.root.json.nested-response.")));
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "canonical.spx").unwrap())
    );
}

#[test]
fn nested_response_retains_copy_scalar_and_copy_record_vector_reads() {
    for ty in ["i64", "u8", "usize", "bool", "Row"] {
        let schema = SCHEMA
            .replace("sku:string", "sku:i64")
            .replace("Vec<Row>", &format!("Vec<{ty}>"));
        let output = generate(&schema, 16, 8).unwrap();
        assert!(output.contains(&format!("vec_get<{ty}>(values,at)")));
        assert!(!output.contains("vec_field"));
        if ty == "Row" {
            assert!(output.contains("object_len_4(value:Row)"));
        }
        crate::parse(&output, "copy.spx").unwrap();
    }
}

#[test]
fn nested_response_refuses_unborrowable_or_broadened_schema_before_generation() {
    for schema in [
        SCHEMA.replace("Vec<Row>", "Vec<string>"),
        SCHEMA
            .replace("ok:bool", "ok:Vec<i64>")
            .replace("retry:usize", "retry:Vec<u8>"),
        SCHEMA.replace("ok:bool", "ok:Report"),
        SCHEMA.replace("sku:string", "sku:Bytes"),
        SCHEMA.replace("@id(\"r.config.label\")", ""),
    ] {
        assert_eq!(generate(&schema, 16, 8).unwrap_err()[0].code, "SPX-J180");
    }
    for (strings, rows) in [(0, 8), (65, 8), (16, 0), (16, 257)] {
        assert_eq!(
            generate(SCHEMA, strings, rows).unwrap_err()[0].code,
            "SPX-J180"
        );
    }
    // The response route must not describe the frozen request policy. The
    // descriptor's request error remains exact, including its one-Vec boundary.
    let unsupported = SCHEMA.replace("ok:bool", "ok:char");
    let request_message = "nested request supports only explicit acyclic records, bounded Unicode Strings, i64/u8/usize/bool, and one admitted Vec";
    let response = generate(&unsupported, 16, 8).unwrap_err().remove(0);
    assert_eq!(response.code, "SPX-J180");
    assert_eq!(response.message, "nested response supports only explicit acyclic records, bounded Unicode Strings, i64/u8/usize/bool, and at most two admitted Vec fields");
    let program = crate::parse(&unsupported, "request-policy.spx").unwrap();
    assert_eq!(
        descriptor::validate(&program, &program.types[2], 16, 8)
            .err()
            .unwrap()[0]
            .message,
        request_message
    );
    let bounds = generate(SCHEMA, 0, 8).unwrap_err().remove(0);
    assert_eq!(bounds.code, "SPX-J180");
    assert!(bounds.message.starts_with("nested response "));
    let third = SCHEMA
        .replace("ok:bool", "ok:Vec<i64>")
        .replace("retry:usize", "retry:Vec<u8>");
    assert_eq!(
        generate(&third, 16, 8).unwrap_err()[0].message,
        "nested response admits at most two expanded Vec fields"
    );
    // Borrowed response inspection is independent of the decoder's allocation census.
    let two = SCHEMA.replace("count:u8", "count:string");
    assert!(generate(&two, 64, 256).is_ok());
    let program = crate::parse(&two, "request.spx").unwrap();
    assert_eq!(
        descriptor::validate(&program, &program.types[2], 64, 256)
            .err()
            .unwrap()[0]
            .code,
        "SPX-J180"
    );
    let sibling = two.replace("ok:bool", "ok:Vec<i64>");
    assert!(generate(&sibling, 64, 256).is_ok());
}

#[test]
fn nested_response_template_like_names_and_repeated_records_remain_opaque() {
    let schema = SCHEMA
        .replace("Report", "__ROW__")
        .replace("r.root", "r.__BOUND__.__ROW_ID__")
        .replace("ok:bool", "ok:Config");
    let output = generate(&schema, 16, 8).unwrap();
    let parsed = crate::parse(&output, "opaque.spx").unwrap();
    assert!(parsed.functions.iter().all(|f| f
        .stable_id
        .starts_with("r.__BOUND__.__ROW_ID__.json.nested-response.")));
    assert!(output.contains("object_len_7(value:borrow __ROW__)"));
    assert_eq!(
        parsed
            .functions
            .iter()
            .filter(|f| f.name.ends_with("utf8_quote"))
            .count(),
        1
    );
}
