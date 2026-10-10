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
    assert!(derived.contains("let mut unescaped=true"));
    assert!(derived.contains("string_from_utf8(byte_range(input,start+1usize,end-1usize))"));
    assert!(derived.contains("if unescaped{string_from_utf8"));
    assert!(derived.contains("if escaped{ju_escape_scalar(input,at)}else{ju_raw_scalar(input,at)}"));
    assert!(derived.contains("std.data.json.query.scalar_at"));
    assert!(derived.contains("std.data.json.utf8.scalar_at"));
    assert!(!derived.contains("char_from_u8("));
    assert!(!derived.contains("let mut duplicate"));
    assert!(!derived.contains("error=11"));
    assert!(!derived.contains("total>=1usize"));
    assert!(derived.contains("names:Vec<string>"));
    assert!(derived.contains("rows:Vec<Row>"));
    assert!(derived.contains("required>output_limit"));
    let row_decode = derived.split("fn json_Row_view_decode").nth(1).unwrap();
    assert!(
        row_decode
            .find("let key_view_0=array_as_slice(key_0)")
            .unwrap()
            < row_decode.find("while error==0 && key<length").unwrap()
    );
    let request_decode = derived
        .split("fn json_Request_request_decode")
        .nth(1)
        .unwrap();
    assert!(
        request_decode
            .find("let servers_key_view=array_as_slice(servers_key)")
            .unwrap()
            < request_decode.find("while error==0 && key<length").unwrap()
    );
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

#[test]
fn utf8_schema_admission_bounds_canonical_output_not_longer_raw_key_spellings() {
    let program = crate::parse(SCHEMA, "utf8-schema.spx").unwrap();
    let generated = source(&program, &program.types[1], 64).unwrap();
    crate::parse(&generated, "utf8-capacity.spx").unwrap();
    // NUL is the worst escaping expansion per decoded UTF-8 byte. Keep all
    // 264 strings at the advertised bound, with complete scalar ranges.
    let rows = (0..256)
        .map(|_| {
            serde_json::json!({
                "number": i64::MIN, "message": "\0".repeat(64), "live": false
            })
        })
        .collect::<Vec<_>>();
    let canonical = serde_json::to_vec(&serde_json::json!({
        "names": vec!["\0".repeat(64); 8], "rows": rows
    }))
    .unwrap();
    assert!(canonical.len() > 100_000);
    assert!(canonical.len() <= 131_072);
    // Equivalent escaped ASCII key spellings may be physically larger. They
    // do not change the descriptor's decoded values or canonical output bound.
    let raw = canonical
        .windows(9)
        .filter(|key| *key == b"\"message\"")
        .count();
    assert_eq!(raw, 256);
    let escaped_keys_upper = canonical.len() + 256 * (6 + 7 + 4) * 5;
    assert!(escaped_keys_upper > 131_072);
    assert!(generated.contains("jv_strict_end(input,32usize,0)"));
    // A truly unrepresentable canonical maximum still refuses derivation.
    let long_keys = SCHEMA
        .replace("message:string", &format!("{}:string", "m".repeat(64)))
        .replace("number:i64", &format!("{}:i64", "n".repeat(64)))
        .replace("live:bool", &format!("{}:bool", "l".repeat(64)));
    let bad = crate::parse(&long_keys, "long-keys.spx").unwrap();
    assert_eq!(
        source(&bad, &bad.types[1], 64).unwrap_err()[0].code,
        "SPX-J180"
    );
}
