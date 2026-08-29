use wasmparser::{ExternalKind, Parser, Payload, TypeRef};

use super::support::*;

#[test]
fn emitted_module_has_the_exact_runtime_import_and_selected_export_inventory() {
    let fixture = fixture();
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&fixture.build.module_wasm)
        .expect("package module is structurally valid Core Wasm");

    let mut imports = Vec::new();
    let mut exports = Vec::new();
    for payload in Parser::new(0).parse_all(&fixture.build.module_wasm) {
        match payload.unwrap() {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.unwrap();
                    assert_eq!(import.module, "env");
                    assert!(matches!(import.ty, TypeRef::Func(_)));
                    imports.push(import.name.to_owned());
                }
            }
            Payload::ExportSection(section) => {
                exports.extend(section.into_iter().map(|export| {
                    let export = export.unwrap();
                    (export.name.to_owned(), export.kind)
                }));
            }
            _ => {}
        }
    }
    assert_eq!(
        imports,
        [
            "spx_add",
            "spx_sub",
            "spx_mul",
            "spx_div",
            "spx_rem",
            "spx_neg",
            "spx_contract_fail",
        ]
    );
    let expected_exports = vec![
        (raw_scalar_symbol("calculator.add"), ExternalKind::Func),
        (raw_scalar_symbol("calculator.not"), ExternalKind::Func),
    ];
    assert_eq!(exports, expected_exports);

    let manifest: serde_json::Value = serde_json::from_str(&fixture.build.manifest_json).unwrap();
    assert_eq!(
        manifest["runtime_imports"],
        serde_json::json!([
            {"module":"env","name":"spx_add","kind":"function"},
            {"module":"env","name":"spx_sub","kind":"function"},
            {"module":"env","name":"spx_mul","kind":"function"},
            {"module":"env","name":"spx_div","kind":"function"},
            {"module":"env","name":"spx_rem","kind":"function"},
            {"module":"env","name":"spx_neg","kind":"function"},
            {"module":"env","name":"spx_contract_fail","kind":"function"}
        ])
    );
}

#[test]
fn artifacts_from_different_authenticated_builds_cannot_be_cross_paired() {
    let first = fixture_from_source(
        "pair",
        "1.0.0",
        &simple_source("pair", 41),
        &["pair.answer"],
    );
    let second = fixture_from_source(
        "pair",
        "1.0.0",
        &simple_source("pair", 42),
        &["pair.answer"],
    );
    assert_ne!(first.build.module_wasm, second.build.module_wasm);

    let mut foreign_module = copied_build(&first.build);
    foreign_module.module_wasm = second.build.module_wasm.clone();
    assert_eq!(verify_error(&foreign_module, &first), "SPX-PB507");

    let mut foreign_manifest = copied_build(&first.build);
    foreign_manifest.manifest_json = second.build.manifest_json.clone();
    assert_eq!(verify_error(&foreign_manifest, &first), "SPX-PB507");

    let mut foreign_evidence = copied_build(&first.build);
    foreign_evidence.evidence_json = second.build.evidence_json.clone();
    assert_eq!(verify_error(&foreign_evidence, &first), "SPX-PB507");
}
