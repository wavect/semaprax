//! Complete old/current shape semantics and literal inline/spill backing.
use super::super::super::shape_cursor::{Item, Pending};
use super::*;
use crate::cleanup::{FieldLiveness, VariantCaseLiveness};
fn equal_leaves(a: &Leaves, b: &Leaves) {
    assert_eq!(a.len(), b.len());
    for (flag, leaf) in a.iter() {
        assert_eq!(leaf.place, b[flag].place);
        assert_eq!(leaf.lifecycle, b[flag].lifecycle);
        assert_eq!(a.under(&leaf.place), b.under(&leaf.place));
    }
}
fn shapes() -> Vec<ResolvedFunction> {
    let p = program();
    let original = p.functions[0].clone();
    let mut plain = original.clone();
    for slot in &mut plain.cleanup_plan.slots {
        slot.field_liveness_shape = FieldLivenessShape::NoDrop;
    }
    let mut wide = plain.clone();
    let fields = (0..80)
        .map(|i| FieldLiveness {
            field: DeclarationId::new(format!("shape.field.{i}")),
            field_index: i,
            shape: FieldLivenessShape::NoDrop,
        })
        .collect();
    wide.cleanup_plan.slots[0].field_liveness_shape = FieldLivenessShape::Record {
        declaration: DeclarationId::new("shape.record"),
        fields,
    };
    let mut variant = plain.clone();
    let declaration = DeclarationId::new("shape.variant");
    variant.cleanup_plan.slots[0].ty = ResolvedType::Nominal {
        declaration: declaration.clone(),
        arguments: vec![],
    };
    variant.cleanup_plan.slots[0].field_liveness_shape = FieldLivenessShape::Variant {
        declaration,
        cases: vec![VariantCaseLiveness {
            case: DeclarationId::new("shape.case"),
            case_index: 0,
            fields: vec![FieldLiveness {
                field: DeclarationId::new("shape.leaf"),
                field_index: 0,
                shape: FieldLivenessShape::Leaf {
                    flag: LivenessFlagId(0),
                    lifecycle: DeclarationId::new("shape.drop"),
                },
            }],
        }],
    };
    let mut late = wide.clone();
    if let FieldLivenessShape::Record { fields, .. } =
        &mut late.cleanup_plan.slots[0].field_liveness_shape
    {
        fields[79].field_index = 0;
    }
    let mut duplicate = variant.clone();
    if let FieldLivenessShape::Variant { cases, .. } =
        &mut duplicate.cleanup_plan.slots[0].field_liveness_shape
    {
        cases.push(cases[0].clone());
    }
    vec![original, plain, wide, variant, late, duplicate]
}
#[test]
fn complete_old_paid_shape_receiver_keeps_all_ordered_leaves_and_late_failures() {
    for function in shapes() {
        let original =
            shape_before::collect_leaves_paid(&function, &mut ReplayBudget::unobserved());
        let legacy = old::collect_leaves(&function);
        let current = collect_leaves_paid(&function, &mut ReplayBudget::unobserved());
        match (current, original, legacy) {
            (Ok(a), Ok(b), Ok(c)) => {
                equal_leaves(&a, &b);
                equal_leaves(&a, &c);
            }
            (Err(a), Err(b), Err(c)) => {
                assert_eq!(a, b);
                assert_eq!(a, c);
            }
            _ => panic!("complete shape receivers disagree"),
        }
    }
}
#[test]
fn inline_shape_roots_remove_only_real_worklist_backing_through_drop() {
    let mut function = program().functions.remove(0);
    for slot in &mut function.cleanup_plan.slots {
        slot.field_liveness_shape = FieldLivenessShape::NoDrop;
    }
    let (((), before), refused, old_paid, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let leaves = shape_before::collect_leaves_paid(&function, &mut budget).unwrap();
            assert!(leaves.is_empty());
            drop((leaves, budget));
        })
    });
    assert!(!refused && !before.invalid && before.live == 0);
    assert_eq!(before.allocated, before.freed);
    assert!(before.allocated <= old_paid && before.highwater <= old_paid);
    let (((), after), refused, paid, _) = with_limit_usage_refusal(0, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let leaves = collect_leaves_paid(&function, &mut budget).unwrap();
            assert!(leaves.is_empty());
            drop((leaves, budget));
        })
    });
    assert!(!refused && !after.invalid && after.live == 0);
    assert_eq!(after.allocated, after.freed);
    assert_eq!(after.allocated, 0);
    assert_eq!(paid, 0);
    assert!(before.allocated > after.allocated && old_paid > paid);
}
#[test]
fn shape_spill_exact_short_sticky_reuse_and_drop_use_literal_backing() {
    let function = program().functions.remove(0);
    let shape = FieldLivenessShape::NoDrop;
    // One complete authored group: 32 inline events and exactly 33 heap events.
    let full = function.id.as_str().len() + 512 + 33 * std::mem::size_of::<Item<'_>>();
    for limit in [full - 1, full] {
        let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut pending = Pending::new();
                let result = pending.reserve(&function, &mut budget, 65, "literal shape events");
                if limit == full {
                    result.unwrap();
                    for _ in 0..65 {
                        pending.push(Item::Shape(&shape));
                    }
                    for _ in 0..65 {
                        assert!(matches!(pending.pop(), Some(Item::Shape(_))));
                    }
                    assert!(pending.pop().is_none());
                    let before = current_metrics();
                    pending
                        .reserve(&function, &mut budget, 65, "reused shape events")
                        .unwrap();
                    for _ in 0..65 {
                        pending.push(Item::Shape(&shape));
                    }
                    assert_eq!(current_metrics(), before);
                } else {
                    let error = result.unwrap_err();
                    let before = current_metrics();
                    assert!(pending
                        .reserve(&function, &mut budget, 1, "sticky inline root")
                        .is_err());
                    assert_eq!(current_metrics(), before);
                    drop(error);
                }
                drop((pending, budget));
            })
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= used && metrics.highwater <= used);
        assert_eq!(refused, limit < full);
        if limit == full {
            assert_eq!(used, full);
            assert!(receipt.is_none());
        } else {
            assert!(used <= limit && receipt.is_some());
        }
    }
}
#[test]
fn full_paid_shape_source_snapshot_keeps_original_body_and_funding() {
    use sha2::{Digest, Sha256};
    let frozen = include_str!(
        "../../../../../docs/evidence/cleanup-shape-cursor/full-paid-shape-before.txt"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(frozen.as_bytes()))
        ),
        "b254906a5f694b156e3632d3bb7d72aa5beb4d27b5f7a929cbd225bdd9ac5736"
    );
    let compiled = include_str!("shape_before.rs")
        .split_once("use super::*;\n")
        .unwrap()
        .1;
    assert_eq!(
        compiled.replacen(
            "pub(super) fn collect_leaves_paid(",
            "fn collect_leaves_paid(",
            1
        ),
        frozen.strip_suffix("\n").unwrap()
    );
}

#[test]
fn current_shape_receiver_is_exact_full_original_with_named_cursor_hooks() {
    let frozen = include_str!(
        "../../../../../docs/evidence/cleanup-shape-cursor/full-paid-shape-before.txt"
    );
    let root = include_str!("../../../replay.rs");
    let start = root.find("fn collect_leaves_paid(").unwrap();
    let end = root.find("fn validate_status_sources(").unwrap();
    let mut projected = root[start..end].to_owned();
    for (current, original) in [
        ("    let mut pending = shape_cursor::Pending::new();\n", ""),
        ("            (budget, &mut pending),\n", "            budget,\n"),
        ("fn collect_shape<'shape>(", "fn collect_shape("),
        ("    shape: &'shape FieldLivenessShape,", "    shape: &FieldLivenessShape,"),
        ("    scratch: (&mut ReplayBudget, &mut shape_cursor::Pending<'shape>),\n", "    budget: &mut ReplayBudget,\n"),
        ("    let (budget, pending) = scratch;\n", ""),
        ("    use shape_cursor::Item;\n\n", "    enum Item<'a> {\n        Shape(&'a FieldLivenessShape),\n        Push(&'a DeclarationId),\n        Pop,\n    }\n\n"),
        ("    pending.reserve(function, budget, 1, \"shape worklist\")?;", "    let mut pending = Vec::new();\n    budget.reserve_vec(function, &mut pending, 1, \"shape worklist\")?;"),
        ("                pending.reserve(\n                    function,\n                    budget,\n                    fields.len().saturating_mul(3),\n                    \"record shape worklist\",\n                )?;\n", "                budget.reserve_vec(\n                    function,\n                    &mut pending,\n                    fields.len().saturating_mul(3),\n                    \"record shape worklist\",\n                )?;\n                pending\n                    .try_reserve(fields.len().saturating_mul(3))\n                    .map_err(|_| {\n                        replay_error(\n                            function,\n                            \"cleanup shape replay capacity exceeds address space\",\n                        )\n                    })?;\n"),
        ("                pending.reserve(function, budget, item_count, \"variant shape worklist\")?;\n", "                budget.reserve_vec(function, &mut pending, item_count, \"variant shape worklist\")?;\n                pending.try_reserve(item_count).map_err(|_| {\n                    replay_error(\n                        function,\n                        \"cleanup shape replay capacity exceeds address space\",\n                    )\n                })?;\n"),
    ] {
        assert_eq!(projected.matches(current).count(), 1, "each named cursor hook occurs exactly once");
        projected = projected.replacen(current, original, 1);
    }
    assert_eq!(projected, frozen);
}

#[test]
fn complete_shape_collectors_are_independently_paid_through_drop() {
    let mut fixtures = shapes();
    fixtures.truncate(4); // The four admitted shapes precede the two explicit hostile cases.
    for function in fixtures {
        let observe = |original| {
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let leaves = if original {
                        shape_before::collect_leaves_paid(&function, &mut budget)
                    } else {
                        collect_leaves_paid(&function, &mut budget)
                    }
                    .unwrap();
                    drop((leaves, budget));
                })
            })
        };
        let (((), old), refused, old_paid, receipt) = observe(true);
        assert!(!refused && receipt.is_none() && !old.invalid);
        assert!(old.allocated <= old_paid && old.highwater <= old_paid);
        assert_eq!(old.allocated, old.freed);
        assert_eq!(old.live, 0);
        let (((), current), refused, paid, receipt) = observe(false);
        assert!(!refused && receipt.is_none() && !current.invalid);
        assert!(current.allocated <= paid && current.highwater <= paid);
        assert_eq!(current.allocated, current.freed);
        assert_eq!(current.live, 0);
        assert!(current.allocated < old.allocated && paid < old_paid);
    }
}
