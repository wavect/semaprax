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
