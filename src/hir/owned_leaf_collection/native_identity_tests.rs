//! Hostile declaration mutations belong inside the HIR privacy boundary.
use crate as semaprax;
use crate::{codegen, graph, hir};
const SHARED: &str = include_str!("../../../tests/fixtures/owned-leaf-collections.spx");

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
    let item = hir::ResolvedType::Nominal {
        declaration: hir::DeclarationId::new("owned.leaf.entry"),
        arguments: vec![],
    };
    for target in [
        crate::variant_layout::VariantTarget::Native64,
        crate::variant_layout::VariantTarget::Wasm32,
    ] {
        let step = crate::iterator_ops::resolved_iter_step(item.clone());
        let layout =
            crate::variant_layout::VariantLayout::for_type(&resolved, target, &step).unwrap();
        let field = layout
            .case(&hir::DeclarationId::new(crate::iterator_ops::YIELD_ID))
            .unwrap()
            .field(&hir::DeclarationId::new(crate::iterator_ops::ITEM_ID))
            .unwrap();
        assert_eq!(
            field.value_kind,
            crate::variant_layout::VariantFieldValueKind::OwnedRecord
        );
    }
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
