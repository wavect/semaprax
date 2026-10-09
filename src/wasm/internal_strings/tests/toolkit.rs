//! Admission and proof controls for the explicit additive selector.
use super::*;

const SOURCE: &str = r#"module test.toolkit_proof;
@id("choice") variant Choice { @id("choice.empty") Empty, @id("choice.text") Text { @id("choice.text.value") value: string, }, }
@id("consume") fn consume(value: own Choice) -> i64 { match own value { Choice::Empty {} => 0, Choice::Text { value: text } => string_len(text), } }
@id("app.main") fn main() -> i64 { let value=Choice::Text { value: string_trim(" text ") }; consume(value) }
"#;

#[test]
fn toolkit_owned_string_byte_views_require_a_replayed_full_root_loan() {
    let source = program(
        r#"module test.toolkit_owned_string_byte_view;
@id("app.fused") fn fused() -> i64 {
    let text = "h\u{0}é";
    let bytes = str_as_bytes(string_as_str(text));
    if byte_len(bytes) == 4usize { 1 } else { 0 }
}
@id("app.named") fn named() -> i64 {
    let text = "h\u{0}é";
    let text_view = string_as_str(text);
    let bytes = str_as_bytes(text_view);
    if byte_len(bytes) == 4usize { 1 } else { 0 }
}
@id("app.main") fn main() -> i64 { fused() + named() }
"#,
    );
    let resolved = crate::hir::resolve(&source).unwrap();
    let ids = ["app.main".to_owned()];
    assert!(admission::prepare_toolkit(&resolved, &ids).is_ok());
    let artifact =
        emit_text_toolkit_module(&source, &ids, InternalStringOptions::default()).unwrap();
    let mut imports = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(artifact.wasm_bytes()) {
        if let wasmparser::Payload::ImportSection(section) = payload.unwrap() {
            for import in section.into_imports() {
                let import = import.unwrap();
                imports.push((import.module.to_owned(), import.name.to_owned()));
            }
        }
    }
    assert!(imports.contains(&("env".to_owned(), "spx_bytes_get".to_owned())));
    assert_eq!(
        imports.last().unwrap(),
        &("env".to_owned(), "spx_bytes_get".to_owned())
    );
    assert!(artifact.runtime_source().contains("spx_bytes_get(carrier,index)"));
    assert!(artifact
        .runtime_source()
        .contains("ENV_IMPORT_NAMES.includes(item.name)?\"env\":\"semaprax.internal-strings.v1\""));

    let mut forged = resolved.clone();
    let producer = forged
        .declarations
        .byte_slice_provenances()
        .find(|(_, provenance)| provenance.root_kind == crate::hir::ByteSliceRootKind::OwnedString)
        .and_then(|(_, provenance)| provenance.producer.clone())
        .unwrap();
    let main = forged
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.fused")
        .unwrap();
    let loan = main
        .loan_plan
        .loans
        .iter_mut()
        .find(|loan| loan.site == producer && loan.cause == crate::loan_plan::LoanCause::SliceView)
        .unwrap();
    loan.origin.root = crate::hir::ValueId::intrinsic_parameter("forged.byte.root", 0);
    assert!(admission::prepare_toolkit(&forged, &ids).is_err());
}

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
@id("byte-view") fn byte_view() -> i64 { let text="h\u{0}é"; let view=str_as_bytes(string_as_str(text)); match byte_get(view,1usize) { Option::Some { value: zero } => if zero==0u8 { 1 } else { 0 }, Option::None {} => 0, } }
@id("app.main") fn main() -> i64 { let value=map_new<i64,i64>(1usize);count(value)+byte_view() }
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
            for item in section.into_imports() {
                let item = item.unwrap();
                imports.push((item.module.to_owned(), item.name.to_owned()));
            }
        }
    }
    assert_eq!(imports.len(), 16);
    assert_eq!(
        &imports[13..],
        &[
            ("env".to_owned(), "spx_collection_checked_v2".to_owned()),
            ("env".to_owned(), "spx_collection_drop_v2".to_owned()),
            ("env".to_owned(), "spx_bytes_get".to_owned())
        ]
    );
    assert!(artifact
        .runtime_source()
        .contains("\"compare\",\"spx_collection_checked_v2\",\"spx_collection_drop_v2\",\"spx_bytes_get\"]"));
    assert!(artifact
        .runtime_source()
        .contains("ENV_IMPORT_NAMES.includes(item.name)?\"env\":\"semaprax.internal-strings.v1\""));
    assert!(artifact.runtime_source().contains("item.name!==IMPORT_NAMES[i]"));
    assert!(artifact.runtime_source().contains("item.kind!==\"function\""));
    assert!(artifact.runtime_source().contains("imports.length!==IMPORT_NAMES.length"));
    assert!(artifact.runtime_source().contains("collections.settle()"));
    assert!(!artifact.runtime_source().contains("collections.clear("));
}
