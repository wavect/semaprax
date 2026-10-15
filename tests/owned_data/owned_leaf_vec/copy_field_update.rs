//! Private Core-Wasm store contract over the same exact native source corpus.
#[path = "../../support/record_copy_field_update.rs"]
mod corpus;
use semaprax::{codegen, wasm};
use std::process::Command;
use std::sync::atomic::Ordering;
fn run(source: &str, status: u32, expected: i64, mode: &str) {
    let ast = semaprax::check(source, "field-update-wasm.spx").unwrap();
    let bytes = wasm::emit_module(&ast).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let root = std::env::temp_dir().join(format!(
        "field-update-wasm-{}-{}",
        std::process::id(),
        super::SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("case.wasm");
    std::fs::write(&path, bytes).unwrap();
    let output = Command::new("node")
        .arg(super::HOST)
        .arg(&path)
        .arg(status.to_string())
        .arg(expected.to_string())
        .arg(mode)
        .output()
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert!(
        output.status.success(),
        "{mode}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn wasm_copy_field_updates_keep_all_scalar_lanes_and_owned_identities_without_store_clones_or_drops(
) {
    run(
        &corpus::evaluation_order_source(),
        0,
        29,
        "field-update-one",
    );
    for (ty, value, expected) in corpus::cases() {
        run(
            &corpus::source(ty, value, expected),
            0,
            29,
            "field-update-observe",
        );
        run(
            &corpus::multi_source(ty, value, expected),
            0,
            29,
            "field-update-observe",
        );
    }
    let legacy = corpus::source("i64", "-9", "got==-9")
        .replace("title:string", "title:Bytes")
        .replace("title:\"a\\u{0}é\"", "title:bytes_zeroed(4usize)")
        .replace("title:\"second\"", "title:bytes_zeroed(6usize)")
        .replace("str_len_bytes(title)==4", "byte_len(title)==4usize");
    run(&legacy, 0, 29, "field-update-observe");
    run(&corpus::oracle_source(), 0, 29, "none");
}
#[test]
fn wasm_copy_field_update_index_rhs_and_host_authority_failures_keep_the_single_commit_boundary() {
    run(&corpus::empty_failure(), 14, 0, "field-update-before-store");
    for index in ["1usize", "18446744073709551615usize"] {
        run(
            &corpus::failure(index, "9", ""),
            14,
            0,
            "field-update-before-store",
        );
    }
    run(
        &corpus::failure("1usize", "rhs()", "@id(\"field.rhs\") fn rhs()->i64 {1/0}"),
        4,
        0,
        "field-update-before-store",
    );
    run(
        &corpus::failure(
            "index()",
            "9",
            "@id(\"field.index\") fn index()->usize {1usize/0usize}",
        ),
        4,
        0,
        "field-update-before-store",
    );
    // The strict host probes restored/stale generations, wrong nominal
    // descriptors and owned field ordinals against the real incoming handle.
    run(
        &corpus::source("i64", "-9", "got==-9"),
        0,
        29,
        "field-update-hostile",
    );
}
#[test]
fn field_update_import_and_native_store_are_absent_from_the_complete_old_row_oracle() {
    corpus::assert_sources();
    let ast = semaprax::check(&corpus::oracle_source(), "field-update-before.spx").unwrap();
    let old_c = codegen::emit_c(&ast).unwrap();
    let old_wasm = wasm::emit_module(&ast).unwrap();
    assert!(!old_c.contains("spx_leaf_update_field("));
    assert!(!old_wasm
        .windows("spx_vec_update_field_v1".len())
        .any(|b| b == b"spx_vec_update_field_v1"));
    let ast = semaprax::check(
        &corpus::source("i64", "-9", "got==-9"),
        "field-update-current.spx",
    )
    .unwrap();
    let c = codegen::emit_c(&ast).unwrap();
    let bytes = wasm::emit_module(&ast).unwrap();
    assert!(c.contains("spx_status = spx_leaf_update_field("));
    assert_eq!(
        bytes
            .windows("spx_vec_update_field_v1".len())
            .filter(|b| *b == b"spx_vec_update_field_v1")
            .count(),
        1
    );
}

#[test]
fn wasm_exact_float_payload_matrix_preserves_every_unrelated_copy_word_and_complete_drop() {
    for ty in ["f32", "f64"] {
        run(
            &corpus::boundary_source(ty),
            0,
            29,
            "field-update-bit-matrix",
        );
    }
}
#[test]
fn same_source_interpreter_matches_every_scalar_multi_copy_legacy_and_failure_control() {
    fn interpret(source: &str, expected: Option<(&str, u32)>) {
        let path = std::env::temp_dir().join(format!(
            "field-update-interpreter-{}-{}.spx",
            std::process::id(),
            super::SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, source).unwrap();
        let actual = semaprax::interpreter::interpret(
            &path,
            "owned.leaf.main",
            &[],
            &semaprax::interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        std::fs::remove_file(path).unwrap();
        semaprax::interpreter::verify_envelope(&actual.envelope).unwrap();
        let wire: serde_json::Value = serde_json::from_str(&actual.envelope).unwrap();
        match expected {
            None => {
                assert!(actual.returned);
                assert_eq!(wire["payload"]["outcome"]["value"], "29");
            }
            Some((domain, code)) => {
                assert!(!actual.returned);
                assert_eq!(wire["payload"]["outcome"]["status"]["domain_id"], domain);
                assert_eq!(wire["payload"]["outcome"]["status"]["code"], code);
            }
        }
    }
    interpret(&corpus::evaluation_order_source(), None);
    for (ty, value, expected) in corpus::cases() {
        interpret(&corpus::source(ty, value, expected), None);
        interpret(&corpus::multi_source(ty, value, expected), None);
    }
    let legacy = corpus::source("i64", "-9", "got==-9")
        .replace("title:string", "title:Bytes")
        .replace("title:\"a\\u{0}é\"", "title:bytes_zeroed(4usize)")
        .replace("title:\"second\"", "title:bytes_zeroed(6usize)")
        .replace("str_len_bytes(title)==4", "byte_len(title)==4usize");
    interpret(&legacy, None);
    interpret(&corpus::oracle_source(), None);
    interpret(&corpus::empty_failure(), Some(("semaprax.vec.v1", 2)));
    for index in ["1usize", "18446744073709551615usize"] {
        interpret(
            &corpus::failure(index, "9", ""),
            Some(("semaprax.vec.v1", 2)),
        );
    }
    interpret(
        &corpus::failure("1usize", "rhs()", "@id(\"field.rhs\") fn rhs()->i64 {1/0}"),
        Some(("semaprax.arithmetic.v1", 4)),
    );
    interpret(
        &corpus::failure(
            "index()",
            "9",
            "@id(\"field.index\") fn index()->usize {1usize/0usize}",
        ),
        Some(("semaprax.arithmetic.v1", 4)),
    );
}
