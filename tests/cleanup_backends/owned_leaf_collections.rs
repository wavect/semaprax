//! Physical settlement at the additive owned-leaf vector call boundary.
//!
//! These tests exercise generated C, not a handwritten model of its ownership.
//! Every invocation must release every tracked heap owner and vector authority;
//! a failure must retain its first status and leave the result sentinel intact.
use semaprax::{codegen, hir};

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
fn iterator_only_headers_next_and_empty_step_select_owning_native_runtime() {
    for source in [
        r#"module owned.leaf;
@id("discard") fn discard(value:own Iter<string>)->i64 {0}
@id("discard-step") fn discard_step(value:own IterStep<string>)->i64 {0}
@id("owned.leaf.main") fn main()->i64 {0}
"#
        .to_owned(),
        format!(
            r#"module owned.leaf;
@id("owned.leaf.entry") record Entry {{{FIELDS}}}
@id("discard") fn discard(value:own Iter<Entry>)->i64 {{0}}
@id("advance") fn advance(value:own Iter<Entry>)->IterStep<Entry> {{iter_next<Entry>(value)}}
@id("owned.leaf.main") fn main()->i64 {{
 let step=IterStep<Entry>::Done{{}};
 match own step {{IterStep::Done{{}}=>0,IterStep::Yield{{item,rest}}=>1,}}
}}
"#
        ),
        r#"module owned.leaf;
@id("owned.leaf.main") fn main()->i64 {
 let step=IterStep<string>::Done{};
 match own step {IterStep::Done{}=>0,IterStep::Yield{item,rest}=>1,}
}
"#
        .to_owned(),
    ] {
        assert!(!source.contains("vec_with_capacity"));
        let ast = semaprax::check(&source, "iterator-only.spx").unwrap();
        let emitted = codegen::emit_c(&ast).unwrap();
        assert!(emitted.contains("spx_leaf_iter_check"));
        assert!(emitted.contains("spx_leaf_iter_drop"));
        assert!(emitted.contains("spx_leaf_storage_v1"));
        native::run(&source, 0, 0, native::Failure::None);
    }
}

#[test]
fn frozen_direct_native_stream_entries_refuse_body_only_owned_vectors() {
    fn checked(body: &str, result: &str) -> hir::ResolvedProgram {
        let source = format!(
            r#"module profile.body;
permit {{process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}}
@id("command") fn command()->{result} uses{{process.stdin.read}} {{let reader=stdin_stream_open();{body}}}
@id("main") fn main()->i64 {{0}}
"#
        );
        hir::resolve(&semaprax::check(&source, "profile-body.spx").unwrap()).unwrap()
    }
    let ordinary = checked("0", "i64");
    for result in [
        codegen::emit_hir_c_with_stdin_stream_exit_status(&ordinary, "command"),
        codegen::emit_hir_c_with_stdin_stream_text(&ordinary, "command"),
        codegen::emit_hir_c_with_stdin_stream_data(&ordinary, "command"),
        codegen::emit_hir_c_with_stdin_stream_records(&ordinary, "command"),
    ] {
        result.expect("exact permit inventory and scalar command root remain admitted");
    }
    let selected = checked("let rows=vec_with_capacity<string>(0usize);0", "i64");
    codegen::emit_hir_c_with_stdin_stream_owned_data(&selected, "command")
        .expect("v30 admits the same ordinary checked body");
    for result in [
        codegen::emit_hir_c_with_stdin_stream_exit_status(&selected, "command"),
        codegen::emit_hir_c_with_stdin_stream_text(&selected, "command"),
        codegen::emit_hir_c_with_stdin_stream_data(&selected, "command"),
        codegen::emit_hir_c_with_stdin_stream_records(&selected, "command"),
    ] {
        let error = result.expect_err("frozen profile must not inherit v30 body admission");
        assert!(error.message.contains("owned"), "{error:?}");
    }
    let old_bool = checked("true", "bool");
    codegen::emit_hir_c_with_stdin_stream(&old_bool, "command")
        .expect("v23 retains its boolean command root");
    let new_bool = checked("let rows=vec_with_capacity<string>(0usize);true", "bool");
    let error = codegen::emit_hir_c_with_stdin_stream(&new_bool, "command")
        .expect_err("v23 must not inherit new body admission");
    assert!(error.message.contains("owned"), "{error:?}");
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
            format!("{populated} let cloned = vec_clone_at<Entry>(rows, 1usize); 42"),
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

// Native emission/declaration mutation coverage lives in the HIR-owned
// native_identity_tests child, which can forge private indexes without exposing
// mutation authority through the public compiler API.
