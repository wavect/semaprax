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

const MULTI_STRING_SCHEMA: &str = r#"module schema;
@id("response.item") record Item {
 @id("response.item.id") id:string,
 @id("response.item.server") server:string,
 @id("response.item.arrival") arrival:i64,
 @id("response.item.start") start:i64,
 @id("response.item.finish") finish:i64,
 @id("response.item.wait") wait:i64,
 @id("response.item.late") late:bool,
 @id("response.item.ordinal") ordinal:i64,
}
@id("response.metrics") record Metrics {@id("response.metrics.count") count:usize,}
@id("response.root") record Report {
 @id("response.root.items") entries:Vec<Item>,
 @id("response.root.metrics") stats:Metrics,
}
@id("schema.anchor") fn anchor()->i64{0}
"#;

const ROW_SOURCE: &str = r#"@id("response.row.json.collection-response.object-len")
fn json_Row_response_object_len(value:borrow Row)->usize {
if !(json_Row_response_owned_valid(string_as_str(value.label))){18446744073709551615usize}else{30usize+(usize_from_i64(jv_i64_len(value.number)))+(json_Row_response_utf8_quoted_len(string_as_str(value.label)))+(if value.active{4usize}else{5usize})}}
@id("response.row.json.collection-response.object-render")
fn json_Row_response_object_render(value:borrow Row)->string {
if !(json_Row_response_owned_valid(string_as_str(value.label))){""}else{let output_0="{";
let label_0=string_concat(output_0,"\"number\":");let rendered_0=string_from_i64(value.number);let output_1=string_concat(label_0,rendered_0);
let label_1=string_concat(output_1,",\"label\":");let rendered_1=json_Row_response_utf8_quote(string_as_str(value.label));let output_2=string_concat(label_1,rendered_1);
let label_2=string_concat(output_2,",\"active\":");let rendered_2=if value.active{"true"}else{"false"};let output_3=string_concat(label_2,rendered_2);
string_concat(output_3,"}")}}
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
    assert!(output.contains("string_as_str(value.label)"));
    assert!(output.contains("json_Row_response_utf8_quoted_len(string_as_str(value.label))"));
    assert!(output.contains("json_Row_response_utf8_quote(string_as_str(value.label))"));
    assert!(!output.contains("match borrow value"));
    assert!(!output.contains("response_field_"));
    // Independent exact projection contract: no borrow-match wrapper or synthetic
    // String binding, and identical validity/length/render ordering.
    assert_eq!(record::source(&program.types[0]), ROW_SOURCE);
    // Authored fields can resemble helper locals; direct roots must retain the
    // actual declaration field, not accidentally read a generated local.
    let field_names = SCHEMA
        .replace("number:i64", "value:i64")
        .replace("label:string", "output_0:string")
        .replace("active:bool", "response_field_1:bool");
    let renamed_fields = crate::parse(&field_names, "field-names.spx").unwrap();
    let field_output = record::source(&renamed_fields.types[0]);
    assert!(field_output.contains("string_as_str(value.output_0)"));
    assert!(field_output.contains("jv_i64_len(value.value)"));
    assert!(field_output.contains("if value.response_field_1"));
    let field_generated = source(&renamed_fields, &renamed_fields.types[2], 64).unwrap();
    let field_parsed = crate::parse(&field_generated, "field-projection.spx").unwrap();
    let field_canonical = crate::format::canonical(&field_parsed);
    assert_eq!(
        field_canonical,
        crate::format::canonical(&crate::parse(&field_canonical, "field-canonical.spx").unwrap())
    );
    // Full generator canonical round-trip below remains the authority-neutral
    // source gate; the declaration field assertions exercise the renamed shape.
    assert!(output.contains("count<=256usize"));
    assert!(output.contains("length<=64usize"));
    assert!(output.contains("required>output_limit"));
    assert!(output.contains("size>131072usize-total-comma"));
    assert!(!output.contains("ju_escape_scalar"));
    assert!(!output.contains("vec_sort"));
    assert!(!output.contains("stdin"));
    let parsed = crate::parse(&output, "response.spx").unwrap();
    // Imported library identities retain their exact owning namespaces. Only
    // generated declarations belong to this response profile's namespace.
    assert!(parsed
        .module_uses
        .iter()
        .all(|import| { import.kind == crate::ast::ModuleUseKind::Function }));
    let imports: Vec<_> = parsed
        .module_uses
        .iter()
        .map(|import| {
            (
                import.persistent_id.as_str(),
                import.target_module.as_str(),
                import.alias.as_str(),
            )
        })
        .collect();
    assert_eq!(
        imports,
        vec![
            (
                "std.data.json.digits.i64_len",
                "std.data.json.digits",
                "jv_i64_len"
            ),
            (
                "std.data.json.write.usize_len",
                "std.data.json.write",
                "jv_usize_len"
            ),
            (
                "std.data.json.utf8.scalar_at",
                "std.data.json.utf8",
                "ju_raw_scalar"
            ),
            (
                "std.data.json.utf8.sequence_end",
                "std.data.json.utf8",
                "ju_raw_end"
            ),
            (
                "std.data.json.utf8.utf8_end",
                "std.data.json.utf8",
                "ju_raw_utf8_end"
            ),
        ]
    );
    for function in &parsed.functions {
        assert!(!function.stable_id.contains(".json.utf8."));
        assert!(function.stable_id.starts_with("response."));
        assert!(function.stable_id.contains(".json.collection-response."));
    }
    for declaration in &parsed.types {
        assert!(!declaration.stable_id.contains(".json.utf8."));
        assert!(declaration.stable_id.contains(".json.collection-response."));
        let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
            panic!("response-generated variant")
        };
        for case in cases {
            assert!(!case.stable_id.contains(".json.utf8."));
            assert!(case.stable_id.contains(".json.collection-response."));
            for field in &case.fields {
                assert!(!field.stable_id.contains(".json.utf8."));
                assert!(field.stable_id.contains(".json.collection-response."));
            }
        }
    }
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

#[test]
fn collection_response_checks_and_encodes_each_bounded_string_in_row_order() {
    let program = crate::parse(MULTI_STRING_SCHEMA, "multi-string-schema.spx").unwrap();
    let output = source(&program, &program.types[2], 64).unwrap();
    let row = record::source(&program.types[0]);
    let first_check = "let response_string_valid_0=json_Item_response_owned_valid(string_as_str(value.id));";
    let second_check = "let response_string_valid_1=json_Item_response_owned_valid(string_as_str(value.server));";
    assert!(row.contains(first_check));
    assert!(row.contains(second_check));
    assert!(row.contains("if !(response_string_valid_0&&response_string_valid_1)"));
    assert!(row.contains("json_Item_response_utf8_quoted_len(string_as_str(value.id))"));
    assert!(row.contains("json_Item_response_utf8_quoted_len(string_as_str(value.server))"));
    assert!(row.contains("json_Item_response_utf8_quote(string_as_str(value.id))"));
    assert!(row.contains("json_Item_response_utf8_quote(string_as_str(value.server))"));
    assert!(row.contains("\\\"id\\\":"));
    assert!(row.contains("\\\"server\\\":"));
    assert!(output.contains("vec_len<Item>(value.entries)"));
    let parsed = crate::parse(&output, "multi-string-generated.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "multi-string-canonical.spx").unwrap())
    );

    let too_many_strings = MULTI_STRING_SCHEMA.replace(
        "@id(\"response.item.server\") server:string,",
        "@id(\"response.item.server\") server:string,\n @id(\"response.item.region\") region:string,",
    );
    let malformed = crate::parse(&too_many_strings, "too-many-strings.spx").unwrap();
    assert_eq!(source(&malformed, &malformed.types[2], 64).unwrap_err()[0].code, "SPX-J180");
}
