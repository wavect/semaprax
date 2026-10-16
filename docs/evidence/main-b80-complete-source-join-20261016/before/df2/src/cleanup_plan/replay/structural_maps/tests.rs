use super::*;

fn program() -> ResolvedProgram {
    let source = crate::check(
        r#"module replay.structural_owner;
@id("structural.main") fn main()->i64 {
 let mut data=bytes_zeroed(1usize); let mut at=0;
 while at<2 {data=bytes_set(data,0usize,65u8);at=at+1;if at==1 {continue;};0}
 let view=bytes_as_slice(data); if byte_len(view)==1usize {42}else{0}
}"#,
        "replay-structural-owner.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
fn run(function: &ResolvedFunction, limit: usize) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::with_physical(function, limit)?;
    let storage = validate_slots(function, &mut budget)?;
    let leaves = collect_leaves(function, &mut budget)?;
    validate_regions(function, &storage, &mut budget)?;
    validate_reference_coverage(function, &mut budget)?;
    assert_eq!(
        leaves.len(),
        function
            .cleanup_plan
            .slots
            .iter()
            .filter(|slot| matches!(slot.field_liveness_shape, FieldLivenessShape::Leaf { .. }))
            .count()
    );
    Ok(())
}
#[test]
fn structural_indexes_preserve_legacy_and_exact_one_short_debits() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "structural.main")
        .unwrap();
    let legacy_storage = super::super::validate_slots(function).unwrap();
    let legacy_leaves = super::super::collect_leaves(function).unwrap();
    let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
    assert_eq!(
        legacy_storage,
        validate_slots(function, &mut budget).unwrap()
    );
    let selected = collect_leaves(function, &mut budget).unwrap();
    assert_eq!(selected.len(), legacy_leaves.len());
    for (flag, leaf) in selected.iter() {
        assert_eq!(leaf.place, legacy_leaves[flag].place);
        assert_eq!(leaf.lifecycle, legacy_leaves[flag].lifecycle);
    }
    let (_, refused, bytes) =
        crate::bounded_output::with_limit_usage(usize::MAX, || run(function, usize::MAX).unwrap());
    assert!(!refused);
    let (exact, refused, used) =
        crate::bounded_output::with_limit_usage(bytes, || run(function, bytes));
    assert!(!refused);
    assert_eq!(used, bytes);
    exact.unwrap();
    let (short, refused, used) =
        crate::bounded_output::with_limit_usage(bytes - 1, || run(function, usize::MAX));
    assert!(refused);
    assert!(used < bytes);
    assert_eq!(short.unwrap_err().code, "SPX-H006");
}
#[test]
fn borrowed_plan_spare_capacity_is_not_new_constructor_backing() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "structural.main")
        .unwrap();
    let mut spare = function.clone();
    spare.cleanup_plan.slots.reserve_exact(100);
    spare.cleanup_plan.regions.reserve_exact(100);
    for region in &mut spare.cleanup_plan.regions {
        region.slots.reserve_exact(100);
    }
    let (_, refused, normal) =
        crate::bounded_output::with_limit_usage(usize::MAX, || run(function, usize::MAX).unwrap());
    assert!(!refused);
    let (_, refused, extended) =
        crate::bounded_output::with_limit_usage(usize::MAX, || run(&spare, usize::MAX).unwrap());
    assert!(!refused);
    assert_eq!(normal, extended);
}
#[test]
fn structural_forgery_stays_closed_under_physical_construction() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "structural.main")
        .unwrap();
    let mut forged = function.clone();
    let storage = forged.cleanup_plan.slots[0].storage.clone();
    forged.cleanup_plan.regions[0].slots.push(storage);
    assert_eq!(run(&forged, usize::MAX).unwrap_err().code, "SPX-H006");
    let mut forged = function.clone();
    forged.cleanup_plan.slots[0].id.0 += 1;
    assert_eq!(run(&forged, usize::MAX).unwrap_err().code, "SPX-H006");
}
