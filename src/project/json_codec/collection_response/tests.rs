use super::*;

const SCHEMA: &str = r#"module schema;
@id("response.row") record Row {
 @id("response.row.number") number:i64,
 @id("response.row.label") label:string,
 @id("response.row.active") active:bool,
}
@id("response.metrics") record Metrics {@id("response.metrics.count") count:usize,}
@id("response.root") record Report {
 @id("response.root.metrics") stats:Metrics,
 @id("response.root.items") entries:Vec<Row>,
}
@id("schema.anchor") fn anchor()->i64{0}
"#;

#[test]
fn collection_response_is_closed_ordered_and_canonical_without_source_authority() {
    let program = crate::parse(SCHEMA, "schema.spx").unwrap();
    let root = &program.types[2];
    let output = source(&program, root, 64).unwrap();
    assert_eq!(output, source(&program, root, 64).unwrap());
    assert_ne!(output, source(&program, root, 63).unwrap());
    assert!(output.contains("vec_len<Row>(value.entries)"));
    assert!(output.contains("vec_clone_at<Row>(value.entries,at)"));
    assert!(output.contains("fn json_Metrics_response_object_len(value:Metrics)"));
    assert!(output.contains("fn json_Row_response_object_len(value:borrow Row)"));
    assert!(output.contains("match borrow value {Row{number:response_field_0,label:response_field_1,active:response_field_2}=>{"));
    assert!(output.contains("string_as_str(response_field_1)"));
    assert!(!output.contains("string_as_str(value.label)"));
    assert!(output.contains("count<=256usize"));
    assert!(output.contains("length<=64usize"));
    assert!(output.contains("required>output_limit"));
    assert!(output.contains("size>131072usize-total-comma"));
    assert!(!output.contains("ju_escape_scalar"));
    assert!(!output.contains(".json.utf8."));
    assert!(!output.contains("vec_sort"));
    assert!(!output.contains("stdin"));
    let parsed = crate::parse(&output, "response.spx").unwrap();
    assert!(parsed.permits.is_empty());
    assert!(parsed.functions.iter().all(|f| f.effects.is_empty()));
    let encode = parsed
        .functions
        .iter()
        .find(|f| f.name == "json_Report_collection_response_encode")
        .unwrap();
    assert!(encode
        .stable_id
        .ends_with(".json.collection-response.encode"));
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "canonical.spx").unwrap())
    );
    assert!(canonical.len() < super::super::MAX_GENERATED_BYTES);
    // Source field/type names can resemble generated templates. Substitution
    // must only rewrite the template, never authored identity bytes.
    let renamed = SCHEMA
        .replace("Row", "json_Test_utf8_Row")
        .replace("response.row", "response.json.utf8.row");
    let program = crate::parse(&renamed, "renamed.spx").unwrap();
    let output = source(&program, &program.types[2], 8).unwrap();
    assert!(output.contains("response.json.utf8.row.json.collection-response.owned-valid"));
    assert!(output.contains("json_json_Test_utf8_Row_response_owned_valid"));
}

#[test]
fn collection_response_refuses_wrong_bound_nested_shapes_and_unchecked_identities() {
    for bound in [0, 65, usize::MAX] {
        let p = crate::parse(SCHEMA, "schema.spx").unwrap();
        assert_eq!(
            source(&p, &p.types[2], bound).unwrap_err()[0].code,
            "SPX-J180"
        );
    }
    for schema in [
        SCHEMA.replace("stats:Metrics", "stats:Vec<Row>"),
        SCHEMA.replace("entries:Vec<Row>", "entries:Row"),
        SCHEMA.replace("label:string", "label:Bytes"),
        SCHEMA.replace("count:usize", "count:string"),
        SCHEMA.replace("number:i64", "number:i32"),
        SCHEMA.replace("@id(\"response.row.label\")", ""),
        SCHEMA.replace("record Row", "record Row<T>"),
    ] {
        let p = crate::parse(&schema, "bad.spx").unwrap();
        assert_eq!(source(&p, &p.types[2], 64).unwrap_err()[0].code, "SPX-J180");
    }
    let mut p = crate::parse(SCHEMA, "schema.spx").unwrap();
    let invariant = crate::parse("module x;fn main()->bool{false}", "x.spx")
        .unwrap()
        .functions
        .remove(0)
        .body;
    p.types[1].invariants = Some(Box::new(vec![invariant]));
    assert_eq!(source(&p, &p.types[2], 64).unwrap_err()[0].code, "SPX-J180");
}
