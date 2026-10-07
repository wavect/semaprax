//! Admission and proof controls for the explicit additive selector.
use super::*;

const SOURCE: &str = r#"module test.toolkit_proof;
@id("choice") variant Choice { @id("choice.empty") Empty, @id("choice.text") Text { @id("choice.text.value") value: string, }, }
@id("consume") fn consume(value: own Choice) -> i64 { match own value { Choice::Empty {} => 0, Choice::Text { value: text } => string_len(text), } }
@id("app.main") fn main() -> i64 { let value=Choice::Text { value: string_trim(" text ") }; consume(value) }
"#;

#[test]
fn toolkit_selector_validates_owned_variant_call_and_every_canonical_exit() {
    let ast = program(SOURCE);
    let resolved = crate::hir::resolve(&ast).unwrap();
    let ids = ["app.main".to_owned()];
    assert!(admission::prepare_toolkit(&resolved, &ids).is_ok());
    assert!(emit_module(&ast, &ids, InternalStringOptions::default()).is_err());
    let mut forged = resolved.clone();
    let function = forged
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "consume")
        .unwrap();
    function.params[0].ownership = crate::hir::OwnershipMode::Value;
    assert!(admission::prepare_toolkit(&forged, &ids).is_err());
    let mut forged = resolved.clone();
    let function = forged
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let exit = function
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| !exit.finalize_in_order.is_empty())
        .unwrap();
    exit.finalize_in_order.clear();
    assert!(admission::prepare_toolkit(&forged, &ids).is_err());
}

#[test]
fn toolkit_runtime_source_authenticates_exact_optional_import_tail() {
    let artifact = emit_text_toolkit_module(
        &program(SOURCE),
        &["app.main".to_owned()],
        InternalStringOptions::default(),
    )
    .unwrap();
    assert!(artifact
        .runtime_source()
        .contains("\"drop\",\"from_i64\",\"from_usize\",\"compare\",\"spx_string_trim_v2\"]"));
    assert!(artifact.runtime_source().contains("toolkit.begin()"));
    assert!(artifact
        .runtime_source()
        .contains("Object.assign(operations,toolkit.operations)"));
    assert!(artifact.runtime_source().contains("semaprax.filesystem.v1"));
    let imports = wasmparser::Parser::new(0)
        .parse_all(artifact.wasm_bytes())
        .filter_map(|payload| match payload.unwrap() {
            wasmparser::Payload::ImportSection(section) => Some(section.count()),
            _ => None,
        })
        .sum::<u32>();
    assert_eq!(imports, 14);
}

#[test]
fn toolkit_owned_variant_layout_rejects_copy_and_byte_carrier_forgeries() {
    use crate::variant_layout::{VariantFieldValueKind, VariantLayout, VariantTarget};
    let resolved = crate::hir::resolve(&program(SOURCE)).unwrap();
    for target in [VariantTarget::Native64, VariantTarget::Wasm32] {
        let layout =
            VariantLayout::for_variant(&resolved, target, &DeclarationId::new("choice")).unwrap();
        assert_eq!(
            layout.cases[1].fields[0].value_kind,
            VariantFieldValueKind::OwnedString
        );
        assert_eq!(
            (
                layout.cases[1].fields[0].size,
                layout.cases[1].fields[0].align
            ),
            (8, 8)
        );
        for kind in [
            VariantFieldValueKind::Copy,
            VariantFieldValueKind::OwnedBytes,
        ] {
            let mut forged = layout.clone();
            forged.cases[1].fields[0].value_kind = kind;
            assert!(forged.validate(&resolved).is_err());
        }
    }
}

#[test]
fn toolkit_collection_selector_authenticates_exact_optional_import_tail() {
    let ast = program(
        r#"module test.toolkit_map_proof;
@id("count") fn count(value: borrow Map<i64,i64>) -> i64 { i64_from_usize(map_len<i64,i64>(value)) }
@id("app.main") fn main() -> i64 { let value=map_new<i64,i64>(1usize);count(value) }
"#,
    );
    let resolved = crate::hir::resolve(&ast).unwrap();
    let ids = ["app.main".to_owned()];
    assert!(admission::prepare_toolkit(&resolved, &ids).is_ok());
    for emitter in [
        emit_module,
        emit_copy_variant_module,
        emit_general_loop_match_module,
    ] {
        assert!(emitter(&ast, &ids, InternalStringOptions::default()).is_err());
    }
    let mut forged = resolved.clone();
    forged
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "count")
        .unwrap()
        .params[0]
        .ownership = crate::hir::OwnershipMode::Value;
    assert!(admission::prepare_toolkit(&forged, &ids).is_err());
    let artifact = emit_text_toolkit_module(&ast, &ids, InternalStringOptions::default()).unwrap();
    let mut imports = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(artifact.wasm_bytes()) {
        if let wasmparser::Payload::ImportSection(section) = payload.unwrap() {
            for item in section {
                let item = item.unwrap();
                imports.push((item.module.to_owned(), item.name.to_owned()));
            }
        }
    }
    assert_eq!(imports.len(), 15);
    assert_eq!(
        &imports[13..],
        &[
            ("env".to_owned(), "spx_collection_checked_v2".to_owned()),
            ("env".to_owned(), "spx_collection_drop_v2".to_owned())
        ]
    );
    assert!(artifact
        .runtime_source()
        .contains("\"compare\",\"spx_collection_checked_v2\",\"spx_collection_drop_v2\"]"));
    assert!(artifact
        .runtime_source()
        .contains("i<IMPORT_NAMES.length-2?\"semaprax.internal-strings.v1\":\"env\""));
    assert!(artifact.runtime_source().contains("collections.settle()"));
    assert!(!artifact.runtime_source().contains("collections.clear("));
}
