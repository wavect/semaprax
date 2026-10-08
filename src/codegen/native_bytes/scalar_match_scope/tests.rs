use super::*;

fn function(source: &str) -> ResolvedFunction {
    let source = format!("module test.runtime_anchors; {source} @id(\"main\") fn main()->i64{{0}}");
    let parsed = crate::parse(&source, std::path::Path::new("runtime-anchors.spx")).unwrap();
    let resolved = crate::hir::resolve(&parsed).unwrap();
    crate::hir::validate(&resolved).unwrap();
    resolved
        .functions
        .into_iter()
        .find(|f| f.id.as_str() == "read")
        .unwrap()
}

#[test]
fn dormant_named_string_read_keeps_inventory_without_a_runtime_anchor() {
    let function = function(
        "@id(\"read\") fn read(text:string)->i64{let mut i=0; while i<string_len(text){i=i+1;0} i}",
    );
    let plan = NativeBytesPlan::build(&function).unwrap().unwrap();
    let dormant = function
        .cleanup_plan
        .slots
        .iter()
        .find(|slot| {
            slot.ty == crate::hir::ResolvedType::String
                && matches!(slot.storage, StorageId::Temporary(_))
        })
        .unwrap();
    assert!(
        plan.value(&dormant.storage).is_ok(),
        "the structural slot remains"
    );
    assert!(plan.is_region_slot(&dormant.storage));
    assert!(!plan.has_runtime_lifecycle(&dormant.storage));
    assert_eq!(function.cleanup_plan.regions.len(), 2, "root and body only");
    assert!(function.cleanup_plan.regions[0]
        .slots
        .contains(&dormant.storage));
}

#[test]
fn active_string_condition_temporary_still_requires_its_canonical_parent() {
    let function =
        function("@id(\"read\") fn read()->i64{let mut i=0; while string_len(\"x\")>i{i=i+1;0} i}");
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("block fixture");
    };
    let crate::hir::ResolvedStatement::While { condition, .. } = &statements[1] else {
        panic!("while fixture");
    };
    let mut plan = NativeBytesPlan::build(&function).unwrap().unwrap();
    let storage = function
        .cleanup_plan
        .slots
        .iter()
        .find(|slot| {
            slot.ty == crate::hir::ResolvedType::String
                && matches!(slot.storage, StorageId::Temporary(_))
                && plan.has_runtime_lifecycle(&slot.storage)
        })
        .unwrap()
        .storage
        .clone();
    let anchors = BTreeSet::from([storage.clone()]);
    assert!(!plan
        .scalar_match_guard_scope_exit(&condition.id, &anchors)
        .unwrap()
        .is_empty());
    let scope = plan
        .scope_exits
        .iter_mut()
        .find(|scope| scope.storage.contains(&storage))
        .unwrap();
    scope.guard_branch = None;
    scope.parent = None;
    assert!(plan.has_runtime_lifecycle(&storage));
    let diagnostic = plan
        .scalar_match_guard_scope_exit(&condition.id, &anchors)
        .unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert_eq!(
        diagnostic.message,
        "String scalar-match region parent is not canonical"
    );
}
