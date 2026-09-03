use semaprax::{format, graph, hir, parse, verify};

fn checked(source: &str) -> hir::ResolvedProgram {
    let program = parse(source, "useful-data-frontend-v1.spx").unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    resolved
}

fn error_codes(source: &str) -> Vec<&'static str> {
    match parse(source, "useful-data-invalid-v1.spx") {
        Err(diagnostic) => vec![diagnostic.code],
        Ok(program) => match hir::resolve(&program) {
            Ok(_) => verify::verify(&program)
                .into_iter()
                .filter(|diagnostic| diagnostic.severity.is_error())
                .map(|diagnostic| diagnostic.code)
                .collect(),
            Err(diagnostics) => diagnostics
                .into_iter()
                .filter(|diagnostic| diagnostic.severity.is_error())
                .map(|diagnostic| diagnostic.code)
                .collect(),
        },
    }
}

fn source_verify_error_codes(source: &str) -> Vec<&'static str> {
    let program = parse(source, "useful-data-source-capacity-v1.spx").unwrap();
    verify::verify(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn assert_source_and_resolver_capacity_error(source: &str, expected: &'static str) {
    let source_codes = source_verify_error_codes(source);
    assert!(
        source_codes.contains(&expected),
        "source verifier did not report {expected}: {source_codes:?}"
    );
    let resolved_codes = error_codes(source);
    assert!(
        resolved_codes.contains(&expected),
        "resolver did not agree on {expected}: {resolved_codes:?}"
    );
}

const VALID: &str = r#"
module test.useful_data_frontend;

@id("bytes.consume")
fn consume(value: own Bytes) -> i64 {
    1
}

@id("array.inspect")
fn inspect(value: [u8; 4]) -> usize {
    let view = array_as_slice(value);
    byte_len(view)
}

@id("app.main")
fn main() -> i64 {
    let empty: [u8; 0] = [];
    let repeated: [u8; 4] = [7u8; 4];
    let explicit: [u8; 4] = [0u8, 255u8, 1u8, 0u8];
    let view = array_as_slice(explicit);
    let owned: Bytes = bytes_copy(view);
    if byte_len(array_as_slice(repeated)) == inspect([9u8; 4]) {
        consume(owned)
    } else {
        0
    }
}
"#;

#[test]
fn fixed_arrays_bytes_views_and_graph_v17_are_canonical() {
    let parsed = parse(VALID, "useful-data-frontend-v1.spx").unwrap();
    let canonical = format::canonical(&parsed);
    let reparsed = parse(&canonical, "useful-data-frontend-v1.spx").unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert!(canonical.contains("let empty: [u8; 0] = [];"));
    assert!(canonical.contains("let repeated: [u8; 4] = [7u8; 4];"));
    assert!(canonical.contains("let owned: Bytes = bytes_copy(view);"));

    let resolved = checked(VALID);
    assert_eq!(
        resolved
            .declarations
            .type_facts(&hir::ResolvedType::ArrayU8(4)),
        Some(hir::TypeFacts {
            copy: true,
            contains_resource: false,
            sized: true,
            needs_drop: false,
            layout_key: "array:u8:4".to_owned(),
        })
    );
    assert_eq!(
        resolved.declarations.type_facts(&hir::ResolvedType::Bytes),
        Some(hir::TypeFacts {
            copy: false,
            contains_resource: false,
            sized: true,
            needs_drop: true,
            layout_key: "owned:bytes".to_owned(),
        })
    );

    let graph = graph::to_json(&parsed).unwrap();
    let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(document["schema"], "semaprax.graph.v17");
    let profile = &document["portable_indexed_byte_data"];
    assert_eq!(profile["max_array_bytes"], 65_536);
    assert_eq!(profile["max_active_array_call_path_bytes"], 65_536);
    assert_eq!(profile["wasm_arena_token_max_exclusive"], 2_147_483_648_u64);
    assert_eq!(profile["wasm_arena_tokens_reused"], false);
    assert_eq!(profile["empty_bytes_owns_token"], true);
    assert!(profile["capacity_summaries"]
        .as_array()
        .is_some_and(|summaries| !summaries.is_empty()));
    assert!(graph.contains("\"kind\":\"fixed_array\",\"length\":4"));
    assert!(graph.contains("\"kind\":\"byte_view\""));
    assert!(graph.contains("\"operation\":\"core.array-u8.as-slice\""));
}

#[test]
fn fixed_array_syntax_and_view_roots_fail_with_stable_diagnostics() {
    let too_large = r#"
module test.array_too_large;
@id("app.main") fn main() -> i64 { let value: [u8; 65537] = []; 0 }
"#;
    assert!(error_codes(too_large).contains(&"SPX-T261"));

    let wrong_element = r#"
module test.array_wrong_element;
@id("app.main") fn main() -> i64 { let value = [1u8, 2]; 0 }
"#;
    assert!(error_codes(wrong_element).contains(&"SPX-T262"));

    let wrong_length = r#"
module test.array_wrong_length;
@id("app.main") fn main() -> i64 { let value: [u8; 2] = [1u8]; 0 }
"#;
    assert!(error_codes(wrong_length).contains(&"SPX-T262"));

    let temporary_view = r#"
module test.array_temporary_view;
@id("app.main") fn main() -> i64 { let view = array_as_slice([0u8; 4]); 0 }
"#;
    assert!(error_codes(temporary_view).contains(&"SPX-T266"));
}

#[test]
fn canonical_array_frame_and_call_path_budgets_are_enforced() {
    let oversized_frame = r#"
module test.array_frame;
@id("array.too-wide") fn too_wide(left: [u8; 40000], right: [u8; 30000]) -> i64 { 0 }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert_source_and_resolver_capacity_error(oversized_frame, "SPX-T261");

    let oversized_path = r#"
module test.array_path;
@id("array.child") fn child(value: [u8; 30000]) -> i64 { 0 }
@id("array.parent") fn parent(value: [u8; 30000]) -> i64 { child(value) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert_source_and_resolver_capacity_error(oversized_path, "SPX-T261");

    let recursive_storage = r#"
module test.array_cycle;
@id("array.recur") fn recur(value: [u8; 1]) -> i64 { recur(value) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert_source_and_resolver_capacity_error(recursive_storage, "SPX-T261");
}

#[test]
fn source_checker_independently_rejects_allocation_sites_and_loop_repetition() {
    let mut too_many_sites = String::from(
        r#"
module test.bytes_copy_sites;
@id("app.main") fn main() -> i64 {
    let data = [1u8; 1];
    let view = array_as_slice(data);
"#,
    );
    for index in 0..17 {
        too_many_sites.push_str(&format!("    let copy{index} = bytes_copy(view);\n"));
    }
    too_many_sites.push_str("    0\n}\n");
    assert_source_and_resolver_capacity_error(&too_many_sites, "SPX-T267");

    let loop_copy = r#"
module test.bytes_copy_loop;
@id("app.main") fn main() -> i64 {
    let data = [1u8; 1];
    let view = array_as_slice(data);
    let mut keep_going = false;
    while keep_going {
        let copied = bytes_copy(view);
        0
    }
    0
}
"#;
    assert_source_and_resolver_capacity_error(loop_copy, "SPX-T267");
}

#[test]
fn array_record_pattern_bindings_are_distinct_capacity_slots() {
    let exact = r#"
module test.array_pattern_exact;
@id("packet.type") record Packet {
    @id("packet.data") data: [u8; 32768],
}
@id("packet.inspect") fn inspect(input: Packet) -> i64 {
    match input { Packet { data } => 0, }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    checked(exact);
    assert!(source_verify_error_codes(exact).is_empty());

    let over = r#"
module test.array_pattern_over;
@id("packet.type") record Packet {
    @id("packet.data") data: [u8; 32769],
}
@id("packet.inspect") fn inspect(input: Packet) -> i64 {
    match input { Packet { data } => 0, }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert_source_and_resolver_capacity_error(over, "SPX-T261");
}

#[test]
fn lexical_byte_views_block_owner_transfer() {
    let source = r#"
module test.bytes_lexical_borrow;
@id("bytes.take") fn take(value: own Bytes) -> i64 { 1 }
@id("app.main") fn main() -> i64 {
    let data = [1u8; 1];
    let view = array_as_slice(data);
    let owned = bytes_copy(view);
    let owned_view = bytes_as_slice(owned);
    take(owned) + byte_len(owned_view)
}
"#;
    assert!(error_codes(source).contains(&"SPX-T265"));

    let released = r#"
module test.bytes_lexical_release;
@id("bytes.take") fn take(value: own Bytes) -> i64 { 1 }
@id("app.main") fn main() -> i64 {
    let data = [1u8; 1];
    let view = array_as_slice(data);
    let owned = bytes_copy(view);
    let observed = {
        let owned_view = bytes_as_slice(owned);
        byte_len(owned_view)
    };
    take(owned)
}
"#;
    checked(released);
}
