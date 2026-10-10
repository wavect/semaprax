use super::*;

const SCHEMA: &str = r#"module schema;
@id("text.row") record Row {
 @id("text.row.count") count:i64,
 @id("text.row.message") message:string,
 @id("text.row.live") live:bool,
}
@id("text.request") record Request {
 @id("text.request.names") names:Vec<string>,
 @id("text.request.rows") rows:Vec<Row>,
}
@id("schema.anchor") fn anchor()->i64 {0}
"#;

#[test]
fn utf8_owned_policy_is_explicit_bounded_and_uses_distinct_scalar_decoders() {
    let program = crate::parse(SCHEMA, "utf8-schema.spx").unwrap();
    let root = &program.types[1];
    for bound in [0, 65, usize::MAX] {
        assert_eq!(
            source(&program, root, bound).unwrap_err()[0].code,
            "SPX-J180"
        );
    }
    let derived = source(&program, root, 64).unwrap();
    assert_eq!(derived, source(&program, root, 64).unwrap());
    assert_ne!(derived, source(&program, root, 63).unwrap());
    assert!(derived.contains("total<=64usize"));
    assert!(derived.contains("scalar>=0 && scalar<=1114111"));
    assert!(derived.contains("char_from_i64(scalar)"));
    assert!(derived.contains("std.data.json.query.scalar_at"));
    assert!(derived.contains("std.data.json.utf8.scalar_at"));
    assert!(!derived.contains("char_from_u8("));
    assert!(!derived.contains("let mut duplicate"));
    assert!(!derived.contains("error=11"));
    assert!(!derived.contains("total>=1usize"));
    assert!(derived.contains("names:Vec<string>"));
    assert!(derived.contains("rows:Vec<Row>"));
    assert!(derived.contains("required>output_limit"));
    let parsed = crate::parse(&derived, "utf8-generated.spx").unwrap();
    assert!(parsed.permits.is_empty());
    assert!(parsed.functions.iter().all(|f| f.effects.is_empty()));
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "roundtrip.spx").unwrap())
    );
    assert!(canonical.len() <= super::super::MAX_GENERATED_BYTES);
}

#[test]
fn utf8_successor_does_not_change_identifier_policy_or_admit_unbounded_schema() {
    let program = crate::parse(SCHEMA, "utf8-schema.spx").unwrap();
    let old = super::super::owned::source(&program, &program.types[1], false).unwrap();
    assert!(old.contains("total>=1usize && total<=16usize"));
    assert!(old.contains("let mut duplicate"));
    assert!(old.contains("error=11"));
    source(&program, &program.types[1], 64).unwrap();
    assert_eq!(
        old,
        super::super::owned::source(&program, &program.types[1], false).unwrap()
    );
    let mut bad = crate::parse(SCHEMA, "invariant.spx").unwrap();
    bad.types[0].invariants = Some(Box::new(vec![
        crate::parse("module invariant;fn main()->bool{false}", "invariant.spx")
            .unwrap()
            .functions
            .remove(0)
            .body,
    ]));
    assert_eq!(
        source(&bad, &bad.types[1], 64).unwrap_err()[0].code,
        "SPX-J180"
    );
    let nested = SCHEMA.replace("message:string", "message:Row");
    let bad = crate::parse(&nested, "nested.spx").unwrap();
    assert_eq!(
        source(&bad, &bad.types[1], 64).unwrap_err()[0].code,
        "SPX-J180"
    );
}

#[test]
fn utf8_stream_policy_requires_original_permit_and_reuses_exact_grammar_normalizer() {
    let mut program = crate::parse(SCHEMA, "utf8-schema.spx").unwrap();
    assert_eq!(
        stream_source(&program, &program.types[1], 16).unwrap_err()[0].code,
        "SPX-J180"
    );
    assert!(program.permits.is_empty());
    program.permits.push("process.stdin.read".to_owned());
    let pure = source(&program, &program.types[1], 16).unwrap();
    let normalizer =
        super::super::views::stream_normalizer_source(&program, &program.types[1]).unwrap();
    let stream = stream_source(&program, &program.types[1], 16).unwrap();
    assert_eq!(stream, pure + &normalizer);
    assert!(normalizer.contains("fn json_Request_stream_normalize()"));
    assert!(normalizer.contains("bytes_zeroed(131072usize)"));
    assert!(!normalizer.contains("error=11"));
    assert!(!stream.contains("let mut duplicate"));
    assert!(!stream.contains("error=11"));
    assert_eq!(
        stream_source(&program, &program.types[1], 0).unwrap_err()[0].code,
        "SPX-J180"
    );
    let parsed = crate::parse(&stream, "utf8-stream-generated.spx").unwrap();
    assert!(parsed.functions.iter().any(|function| function
        .effects
        .iter()
        .any(|effect| effect == "process.stdin.read")));
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "utf8-stream-roundtrip.spx").unwrap())
    );
}
