//! Physical settlement at the additive owned-leaf vector call boundary.
//!
//! These tests exercise generated C, not a handwritten model of its ownership.
//! Every invocation must release every tracked heap owner and vector authority;
//! a failure must retain its first status and leave the result sentinel intact.
use semaprax::{codegen, graph, hir};

#[path = "owned_leaf_collections/native.rs"]
mod native;

const SHARED: &str = include_str!("../fixtures/owned-leaf-collections.spx");
const FIELDS: &str = r#"
    @id("owned.leaf.key") key: string,
    @id("owned.leaf.count") count: i64,
    @id("owned.leaf.payload") payload: Bytes,
"#;
const ROW: &str = r#"Entry { key: "key\u{0}x", count: 7, payload: bytes_zeroed(1usize) }"#;

fn program(fields: &str, body: &str) -> String {
    format!(
        "module owned.leaf;\n@id(\"owned.leaf.entry\") record Entry {{{fields}}}\n\
         @id(\"owned.leaf.main\") fn main() -> i64 {{ {body} }}"
    )
}

#[test]
fn shared_owned_leaf_fixture_settles_at_o0_and_o2() {
    native::run(SHARED, 0, 0, native::Failure::None);
}

#[test]
fn cloned_record_remains_independent_after_clear_and_replacement() {
    let body = format!(
        r#"
    let mut rows = vec_with_capacity<Entry>(1usize);
    rows = vec_push<Entry>(rows, {ROW});
    let first = vec_clone_at<Entry>(rows, 0usize);
    rows = vec_replace<Entry>(rows, 0usize,
        Entry {{ key: "replacement", count: 9, payload: bytes_zeroed(2usize) }});
    let second = vec_clone_at<Entry>(rows, 0usize);
    rows = vec_clear<Entry>(rows);
    let empty = vec_len<Entry>(rows) == 0usize;
    let first_ok = match own first {{ Entry {{ key, count, payload }} => {{
        let byte_ok = {{ let view = bytes_as_slice(payload);
            match byte_get(view, 0usize) {{
                Option::Some {{ value }} => value == 0u8,
                Option::None {{}} => false,
            }}
        }};
        key == "key\u{{0}}x" && count == 7 && byte_ok
    }}, }};
    let second_ok = match own second {{ Entry {{ key, count, payload }} => {{
        let length = {{ let view = bytes_as_slice(payload); byte_len(view) }};
        key == "replacement" && count == 9 && length == 2usize
    }}, }};
    if empty && first_ok && second_ok {{ 0 }} else {{ 1 }}
"#
    );
    native::run(&program(FIELDS, &body), 0, 0, native::Failure::None);
}

#[test]
fn failed_clone_replace_push_and_reserve_keep_first_status_and_settle() {
    let populated = format!(
        "let mut rows = vec_with_capacity<Entry>(1usize); \
         rows = vec_push<Entry>(rows, {ROW});"
    );
    for (body, status) in [
        (
            format!("{populated} let result = vec_clone_at<Entry>(rows, 1usize); 42"),
            2,
        ),
        (
            format!("{populated} rows = vec_replace<Entry>(rows, 1usize, {ROW}); 42"),
            2,
        ),
        (
            format!("{populated} rows = vec_push<Entry>(rows, {ROW}); 42"),
            1,
        ),
        (
            format!(
                "{populated} rows = vec_reserve_owned<Entry>(rows, 18446744073709551615usize); 42"
            ),
            3,
        ),
        (
            format!("{populated} rows = vec_reserve_owned<Entry>(rows, 4096usize); 42"),
            3,
        ),
        (
            "let rows = vec_with_capacity<Entry>(4097usize); 42".to_owned(),
            3,
        ),
        // Index evaluation fails while the outer vector is staged. The final
        // element argument and replacement preflight must not run afterward.
        (
            format!(
                r#"{populated}
            rows = vec_replace<Entry>(rows, {{
                let mut words = vec_with_capacity<string>(0usize);
                words = vec_push<string>(words, "index failure");
                1usize
            }}, {ROW}); 42"#
            ),
            1,
        ),
    ] {
        native::run(&program(FIELDS, &body), status, 42, native::Failure::None);
    }
    let reserve = format!("{populated} rows = vec_reserve_owned<Entry>(rows, 1usize); 42");
    native::run(&program(FIELDS, &reserve), 3, 42, native::Failure::Realloc);
}

#[test]
fn every_partial_clone_prefix_settles_without_publishing_a_record() {
    for (first, second, a, b) in [
        ("string", "Bytes", "\"first\"", "bytes_zeroed(2usize)"),
        ("Bytes", "string", "bytes_zeroed(1usize)", "\"second\""),
        ("string", "string", "\"first\"", "\"second\""),
        (
            "Bytes",
            "Bytes",
            "bytes_zeroed(1usize)",
            "bytes_zeroed(2usize)",
        ),
    ] {
        let fields = format!(
            "@id(\"owned.leaf.first\") first: {first}, \
             @id(\"owned.leaf.marker\") marker: i64, \
             @id(\"owned.leaf.second\") second: {second},"
        );
        let body = format!(
            "let mut rows = vec_with_capacity<Entry>(1usize); \
             rows = vec_push<Entry>(rows, Entry {{ first: {a}, marker: 7, second: {b} }}); \
             let clone = vec_clone_at<Entry>(rows, 0usize); 42"
        );
        let source = program(&fields, &body);
        for leaf in [1, 2] {
            native::run(&source, 3, 42, native::Failure::CloneLeaf(leaf));
        }
    }
    let primitive = "module owned.leaf; @id(\"owned.leaf.main\") fn main()->i64 {\
        let mut words=vec_with_capacity<string>(1usize);\
        words=vec_push<string>(words,\"word\");\
        let clone=vec_clone_at<string>(words,0usize);42}";
    native::run(primitive, 3, 42, native::Failure::CloneLeaf(1));
}

#[test]
fn native_emission_replays_owned_leaf_identity_layout_and_source_authority() {
    let ast = semaprax::check(SHARED, "owned-leaf-native.spx").unwrap();
    let canonical = semaprax::format::canonical(&ast);
    let round = semaprax::check(&canonical, "owned-leaf-native.spx").unwrap();
    let expected = codegen::emit_c(&ast).unwrap();
    assert_eq!(expected, codegen::emit_c(&round).unwrap());
    let graph = graph::to_json(&ast).unwrap();
    let changed = semaprax::check(
        &SHARED.replace("count: 20", "count: 21"),
        "owned-leaf-native.spx",
    )
    .unwrap();
    assert!(graph::verify_json(&changed, &graph).is_err());
    let resolved = hir::resolve(&ast).unwrap();
    for drift in 0..3 {
        let mut forged = resolved.clone();
        if drift == 0 {
            forged
                .declarations
                .declarations
                .get_mut(&hir::DeclarationId::new("owned.leaf.key"))
                .unwrap()
                .identity_origin = hir::IdentityOrigin::Automatic;
        } else {
            let fields = forged
                .declarations
                .record_fields
                .get_mut(&hir::DeclarationId::new("owned.leaf.entry"))
                .unwrap();
            if drift == 1 {
                fields[0].ty = hir::ResolvedType::Bytes;
            } else {
                fields.swap(0, 2);
            }
        }
        assert!(
            hir::validate(&forged).is_err(),
            "accepted declaration drift {drift}"
        );
        assert!(
            codegen::emit_hir_c(&forged).is_err(),
            "emitted declaration drift {drift}"
        );
    }
}
