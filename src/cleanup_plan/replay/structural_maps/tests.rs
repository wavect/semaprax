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

fn insert_owned_status_before(
    function: &ResolvedFunction,
    statuses: &mut BTreeSet<StatusSourceId>,
    source: &StatusSourceId,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !statuses.contains(source) {
        budget.reserve_btree_node::<StatusSourceId, ()>(function, "status reference set")?;
        budget.reserve_clone(function, source, "status reference identity")?;
        statuses.insert(source.clone());
    }
    Ok(())
}

#[test]
fn borrowed_status_coverage_still_rejects_missing_and_changed_declarations() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "structural.main")
        .unwrap();
    assert!(!function.cleanup_plan.status_sources.is_empty());
    validate_reference_coverage(function, &mut ReplayBudget::new()).unwrap();
    let mut missing = function.clone();
    missing.cleanup_plan.status_sources.pop().unwrap();
    let mut changed = function.clone();
    let id = &mut changed.cleanup_plan.status_sources[0].id;
    id.expression = ExpressionId::from_owned(format!("{}:forged", id.expression.as_str()));
    for forged in [&missing, &changed] {
        let error = validate_reference_coverage(forged, &mut ReplayBudget::new()).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error
            .message
            .contains("declared and referenced cleanup status sources differ"));
    }
}

#[test]
fn borrowed_status_sets_keep_full_identity_order_and_pay_backing_through_drop() {
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::measure;
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "structural.main")
        .unwrap();
    let mut sources = Vec::new();
    for text in ["", "a", "a\0", "a\0b", "é", "z", "long-status-expression"] {
        for lane in [
            StatusLane::OwnerAdmission,
            StatusLane::OperationFailure,
            StatusLane::AllocationFailure,
            StatusLane::ContractFalse,
        ] {
            sources.push(StatusSourceId {
                expression: ExpressionId::from_owned(text.into()),
                lane,
            });
        }
    }
    // The original owned-key set is an independent full-field/order oracle.
    let expected: BTreeSet<_> = sources.iter().cloned().collect();
    assert_eq!(expected.len(), sources.len());
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut statuses = BTreeSet::new();
                let result = (|| {
                    for source in sources.iter().rev().chain(&sources) {
                        insert_status(function, &mut statuses, source, &mut budget)?;
                    }
                    assert_eq!(statuses.len(), expected.len());
                    assert!(statuses.iter().copied().eq(expected.iter()));
                    let remaining = crate::bounded_output::active_remaining();
                    for source in &sources {
                        insert_status(function, &mut statuses, source, &mut budget)?;
                    }
                    assert_eq!(crate::bounded_output::active_remaining(), remaining);
                    Ok::<_, Diagnostic>(())
                })();
                let accepted = result.is_ok();
                drop(result);
                drop(statuses);
                drop(budget);
                accepted
            })
        })
    };
    let ((accepted, allocation), refused, exact, first) = observe(usize::MAX);
    assert!(accepted && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= exact && exact > 0);
    let ((), refused, original, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        let mut statuses = BTreeSet::new();
        for source in sources.iter().rev().chain(&sources) {
            insert_owned_status_before(function, &mut statuses, source, &mut budget).unwrap();
        }
        assert_eq!(statuses, expected);
    });
    assert!(!refused && first.is_none());
    assert!(exact < original);
    for limit in [0, exact - 1, exact] {
        let ((accepted, allocation), refused, paid, first) = observe(limit);
        assert_eq!(accepted, limit == exact);
        assert_eq!(refused, limit != exact);
        assert_eq!(first.is_some(), limit != exact);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
        if limit == 0 {
            assert_eq!(allocation.allocated, 0);
        }
    }
}
