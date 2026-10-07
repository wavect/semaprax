use super::*;
use std::fmt::Write as _;

fn legacy_revision(program: &Program) -> String {
    super::super::revision_from_canonical_source(&crate::format::canonical(program))
}

fn assert_oracle(source: &str, expected_schema: &str) {
    let program = crate::parse(source, "revision-stream.spx").unwrap();
    let canonical = crate::format::canonical(&program);
    let (schema, contract, _) = crate::prelude::selected_for_source(&canonical);
    assert_eq!(schema, expected_schema);
    // Independent revision-v2 byte oracle, including the reparsed-source
    // selector that the materialized route used before this optimization.
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.graph-revision.v2\0");
    for bytes in [canonical.as_bytes(), schema.as_bytes(), contract.as_slice()] {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    let expected = format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    );
    assert_eq!(revision(&program), expected);
    let reparsed = crate::parse(&canonical, "revision-stream-roundtrip.spx").unwrap();
    assert_eq!(crate::format::canonical(&reparsed), canonical);
    assert_eq!(revision(&reparsed), expected);
}

#[test]
fn streaming_revision_matches_materialized_bytes_for_every_prelude() {
    for (schema, declarations) in [
        (crate::prelude::SCHEMA_V1, "fn main()->i64{42}"),
        (
            crate::prelude::SCHEMA_V2,
            "fn values()->Vec<i64>{vec_with_capacity<i64>(1usize)}",
        ),
        (
            crate::prelude::SCHEMA_V3,
            "fn wipe(values:own Vec<i64>)->Vec<i64>{vec_clear<i64>(values)}",
        ),
        (
            crate::prelude::SCHEMA_V4,
            "fn boxed()->Box<i64>{box_new<i64>(1)}",
        ),
        (
            crate::prelude::SCHEMA_V5,
            "fn boxed(values:own Box<Bytes>)->Box<Bytes>{values}",
        ),
        (
            crate::prelude::SCHEMA_V6,
            "fn values(values:own Vec<Bytes>)->Vec<Bytes>{values}",
        ),
        (
            crate::prelude::SCHEMA_V7,
            "fn values(values:own Iter<i64>)->Iter<i64>{values}",
        ),
        (
            crate::prelude::SCHEMA_V8,
            "fn values(values:own Iter<Bytes>)->Iter<Bytes>{values}",
        ),
        (
            crate::prelude::SCHEMA_V9,
            "fn values(values:own List<i64>)->List<i64>{values}",
        ),
        (
            crate::prelude::SCHEMA_V10,
            "fn reader(value:own StdinReader)->StdinReader{value}",
        ),
        (
            crate::prelude::SCHEMA_V11,
            "fn sorted(values:own Vec<i64>)->Vec<i64>{vec_sort<i64>(values)}",
        ),
    ] {
        assert_oracle(&format!("module revision.prelude; {declarations}"), schema);
    }
    assert_oracle("module revision.sort; use function @id(\"std.collections.vec.sort\") from std.collections as sorted; fn run(values:own Vec<i64>)->Vec<i64>{sorted<i64>(values)}", crate::prelude::SCHEMA_V11);
    for (schema, owner) in [
        (crate::prelude::SCHEMA_V3, "std.collections"),
        (crate::prelude::SCHEMA_V2, "user.collections"),
    ] {
        assert_oracle(&format!("module revision.imports; use function @id(\"std.collections.vec.clear\") from {owner} as wipe; fn clear(values:own Vec<i64>)->Vec<i64>{{wipe<i64>(values)}}"), schema);
    }
    // Text and ordinary local names cannot select an intrinsic profile.
    assert_oracle(
        "module revision.names; fn main()->string{let vec_push=1;\"Vec<i64> vec_clear\"}",
        crate::prelude::SCHEMA_V1,
    );
}

#[test]
fn streaming_revision_matches_escaped_long_and_statement_if_projections() {
    let payload = "λ😀\\\"\n\t".repeat(800);
    let mut source = format!(
        "module revision.large; fn label()->string{{{}}} fn main()->i64{{let mut x=0;",
        crate::format::canonical_string(&payload)
    );
    for _ in 0..128 {
        source.push_str("if true { x=x+1; } else if false { x=x+2; 0 } ");
    }
    source.push_str("if false { x=0; } else { 0 } let _if1=if true {1} else {2}; x }");
    assert_oracle(&source, crate::prelude::SCHEMA_V1);
}

#[test]
fn hash_writer_matches_exact_bytes_across_buffer_and_utf8_boundaries() {
    for length in [0, 1, 4095, 4096, 4097, 8192, 8193] {
        let value = format!("{}λ😀", "x".repeat(length));
        let expected = Sha256::digest(value.as_bytes());
        for width in [1, 7, 4096, 8193] {
            let mut hasher = Sha256::new();
            let mut writer = HashWriter::new(&mut hasher);
            // Chunks remain valid UTF-8; the fixed hash buffer may split a
            // scalar because it consumes bytes, not source characters.
            let ascii = &value[..length];
            for chunk in ascii.as_bytes().chunks(width) {
                writer
                    .write_str(std::str::from_utf8(chunk).unwrap())
                    .unwrap();
            }
            writer.write_str(&value[length..]).unwrap();
            writer.flush();
            assert_eq!(hasher.finalize(), expected);
        }
    }
}

#[test]
fn bounded_revision_preserves_legacy_output_work_and_overflow() {
    let program = crate::parse(
        "module revision.budget; fn main()->i64{let mut x=0; if true {x=1;} x}",
        "revision-budget.spx",
    )
    .unwrap();
    let (_, overflowed, exact) =
        crate::bounded_output::with_limit_usage(usize::MAX, || legacy_revision(&program));
    assert!(!overflowed);
    for limit in [0, 1, 32, exact / 2, exact - 1, exact, exact + 1] {
        let expected = crate::bounded_output::with_limit_usage(limit, || legacy_revision(&program));
        let actual = crate::bounded_output::with_limit_usage(limit, || revision(&program));
        assert_eq!(actual, expected, "formatter-work limit {limit}");
    }
    assert_eq!(revision(&program), legacy_revision(&program));
}

#[test]
fn streaming_revision_keeps_kernel_zero_candidate_and_shadow_traversal() {
    let source = r#"module revision.lanes; fn main()->i64{let label="x"; let glyph='a'; if true && !false {-7+3} else {0}}"#;
    let program = crate::parse(source, "revision-lanes.spx").unwrap();
    let expected = legacy_revision(&program);
    let (actual, counts) =
        crate::kernel_zero::rung_two_authority::with_counts(|| revision(&program));
    assert_eq!(actual, expected);
    assert_eq!(counts, [2, 4, 6, 8, 2]);
    let (actual, comparisons) =
        crate::kernel_zero::canonical_char_renderer::with_shadow(|| revision(&program));
    assert_eq!(actual, expected);
    assert_eq!(comparisons, 2);
}

#[test]
fn edited_retained_ast_changes_revision_and_stale_graph_still_fails() {
    let source = "module revision.edit; @id(\"app.main\") fn main()->i64{42}";
    let mut program = crate::check(source, "revision-edit.spx").unwrap();
    let original = revision(&program);
    let graph = crate::graph::to_json(&program).unwrap();
    let crate::ast::ExprKind::Block { tail, .. } = &mut program.functions[0].body.kind else {
        panic!("fixture has a block body");
    };
    tail.kind = crate::ast::ExprKind::Int(43);
    assert_ne!(revision(&program), original);
    assert_eq!(revision(&program), legacy_revision(&program));
    let diagnostic = crate::graph::verify_json(&program, &graph).unwrap_err();
    assert_eq!(diagnostic[0].code, "SPX-G411");
    let canonical = crate::format::canonical(&program);
    let reparsed = crate::check(&canonical, "revision-edit-roundtrip.spx").unwrap();
    assert_eq!(revision(&reparsed), revision(&program));
    let fresh = crate::graph::to_json(&reparsed).unwrap();
    crate::graph::verify_json(&reparsed, &fresh).unwrap();
}
