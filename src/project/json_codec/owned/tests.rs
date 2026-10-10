use super::*;

const SCHEMA: &str = r#"module schema;
@id("catalog.item") record Item {
 @id("catalog.item.rank") rank:i64,
 @id("catalog.item.sku") sku:string,
 @id("catalog.item.live") live:bool,
 @id("catalog.item.units") units:usize,
 @id("catalog.item.byte") byte:u8,
}
@id("catalog.request") record Catalog {
 @id("catalog.request.labels") labels:Vec<string>,
 @id("catalog.request.items") items:Vec<Item>,
}
@id("schema.anchor") fn anchor()->i64 {0}
"#;

#[test]
fn owned_request_materialization_uses_authored_field_names_and_record_order() {
    let program = crate::parse(SCHEMA, "owned-schema.spx").unwrap();
    let source = source(&program, &program.types[1], false).unwrap();
    assert_eq!(
        source,
        super::source(&program, &program.types[1], false).unwrap()
    );
    let parsed = crate::parse(&source, "owned-codec.spx").unwrap();
    assert!(parsed.permits.is_empty());
    assert!(parsed
        .functions
        .iter()
        .all(|function| function.effects.is_empty()));
    let outcome = parsed
        .types
        .iter()
        .find(|ty| ty.name == "CatalogJsonOwnedDecode")
        .unwrap();
    let TypeDeclarationKind::Variant { cases } = &outcome.kind else {
        panic!("outcome")
    };
    assert_eq!(cases[0].fields[0].name, "labels");
    assert_eq!(cases[0].fields[1].name, "items");
    assert_eq!(
        cases[0].fields[1].ty,
        Type::Named {
            name: "Vec".into(),
            arguments: vec![Type::Named {
                name: "Item".into(),
                arguments: vec![]
            }]
        }
    );
    assert!(source.contains("rank:value.rank"));
    assert!(source.contains("sku:json_Item_owned_identifier"));
    assert!(source.contains("vec_push<Item>(rows,row)"));
    assert!(source.contains("string_compare(row.sku,other.sku)"));
    assert!(source.contains("required>output_limit"));
    assert!(!source.contains("bytes_copy("));
    assert!(source.len() <= super::super::MAX_GENERATED_BYTES);
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "roundtrip.spx").unwrap())
    );
}

#[test]
fn owned_request_keeps_view_policy_limits_and_original_stream_permit() {
    let mut program = crate::parse(SCHEMA, "owned-schema.spx").unwrap();
    assert_eq!(
        source(&program, &program.types[1], true).unwrap_err()[0].code,
        "SPX-J180"
    );
    program.permits.push("process.stdin.read".into());
    let stream = source(&program, &program.types[1], true).unwrap();
    let parsed = crate::parse(&stream, "owned-stream.spx").unwrap();
    assert!(parsed.permits.is_empty());
    let effects: Vec<_> = parsed
        .functions
        .iter()
        .filter(|f| !f.effects.is_empty())
        .collect();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].name, "json_Catalog_stream_normalize");
    assert_eq!(effects[0].effects, vec!["process.stdin.read"]);
    assert!(stream.contains("bytes_zeroed(131072usize)"));
    assert!(stream.contains("total<=16usize"));
    assert!(stream.contains("patients)>=256usize"));
    assert!(stream.contains("servers)>=8usize"));
    for bad in [
        SCHEMA.replace("sku:string", "sku:Bytes"),
        SCHEMA.replace("rank:i64", "rank:f64"),
        SCHEMA.replace("labels:Vec<string>", "labels:Vec<usize>"),
    ] {
        let bad = crate::parse(&bad, "bad-owned-schema.spx").unwrap();
        assert_eq!(
            source(&bad, &bad.types[1], false).unwrap_err()[0].code,
            "SPX-J180"
        );
    }
}
