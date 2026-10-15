//! Independent original queue and literal allocator-layout Drop controls.
use super::*;
use crate::test_allocation_observer::{current_metrics, measure};
use std::fmt::Write;

fn fixture(fields: usize) -> DeclarationIndex {
    let mut source = String::from("module leaf.drop; @id(\"leaf\") record Leaf {");
    for field in 0..fields {
        write!(source, "@id(\"field.{field}\") field_{field}:i64,").unwrap();
    }
    source.push_str("} @id(\"main\") fn main()->i64 {0}");
    let program = crate::check(&source, "leaf-drop.spx").unwrap();
    DeclarationIndex::from_verified_with_prelude(&program, &[]).unwrap()
}
// Build the old queue's owned representation independently. Clone now shares
// nominal owners, whose nonunique drain deliberately cannot expose their roots.
// DerefMut detaches the legacy wrapper without changing any field/type value.
fn old_fixture(fields: usize) -> DeclarationIndex {
    let mut index = fixture(fields);
    for fields in index.record_fields.values_mut() {
        let _ = std::ops::DerefMut::deref_mut(fields);
        assert!(!fields.has_shared_owner());
    }
    index
}
fn nominal_leaves(index: &mut DeclarationIndex) {
    for fields in index.record_fields.values_mut() {
        for field in fields {
            field.ty = ResolvedType::Nominal {
                declaration: DeclarationId::new("leaf"),
                arguments: Vec::new(),
            };
        }
    }
}
// Also observe construction, representation materialization and destruction in
// one uninterrupted interval. The snapshots isolate real Drop allocations;
// every construction allocation must still settle in the enclosing observer.
fn whole_lifetime_control(nominal: bool) {
    for old in [true, false] {
        let ((), whole) = measure(|| {
            let mut index = if old { old_fixture(16) } else { fixture(16) };
            if nominal {
                nominal_leaves(&mut index);
            }
            let before = current_metrics();
            if old {
                old_drop(index);
            } else {
                drop(index);
            }
            let after = current_metrics();
            let queue = (4 + 8 + 16) * std::mem::size_of::<ResolvedType>();
            assert_eq!(
                after.allocations - before.allocations,
                if old { 3 } else { 0 }
            );
            assert_eq!(
                after.reallocations - before.reallocations,
                if old { 2 } else { 0 }
            );
            assert_eq!(
                after.allocated - before.allocated,
                if old { queue } else { 0 }
            );
        });
        assert!(!whole.invalid);
        assert_eq!(whole.live, 0);
        assert_eq!(whole.freed, whole.allocated);
    }
}
fn old_drop(mut index: DeclarationIndex) {
    let mut pending = Vec::new();
    // These fixtures own ordinary record vectors. Reproduce the original
    // complete leaf queue directly; the current disposer intentionally omits
    // leaf roots and cannot serve as the independent old control.
    assert!(index.variant_cases.is_empty() && index.case_fields.is_empty());
    for (_, mut fields) in std::mem::take(&mut index.record_fields) {
        assert!(!fields.has_shared_owner());
        for field in fields.iter_mut() {
            pending.push(std::mem::replace(&mut field.ty, ResolvedType::I64));
        }
    }
    while let Some(ty) = pending.pop() {
        if let ResolvedType::Nominal { arguments, .. } = ty {
            pending.extend(arguments);
        }
    }
}
#[test]
fn sixteen_actual_leaf_fields_remove_only_the_original_literal_drop_queue() {
    let current = fixture(16);
    let old = old_fixture(16);
    let roots: usize = current
        .record_fields
        .values()
        .map(|fields| fields.len())
        .sum();
    assert_eq!(roots, 16);
    assert!(current.variant_cases.is_empty() && current.case_fields.is_empty());
    let ((), old_metrics) = measure(|| old_drop(old));
    let ((), new_metrics) = measure(|| drop(current));
    for metrics in [old_metrics, new_metrics] {
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.freed, metrics.allocated);
    }
    // Literal capacities of the original pinned Vec growth: 4, 8, 16.
    let original_queue = (4 + 8 + 16) * std::mem::size_of::<ResolvedType>();
    assert_eq!(old_metrics.allocations, 3);
    assert_eq!(old_metrics.reallocations, 2);
    assert_eq!(old_metrics.allocated, original_queue);
    assert_eq!(new_metrics.allocations, 0);
    assert_eq!(new_metrics.reallocations, 0);
    assert_eq!(new_metrics.allocated, 0);
    whole_lifetime_control(false);
}
#[test]
fn zero_argument_nominal_fields_do_not_allocate_drop_scratch() {
    let mut current = fixture(16);
    nominal_leaves(&mut current);
    let mut old = old_fixture(16);
    nominal_leaves(&mut old);
    let ((), old_metrics) = measure(|| old_drop(old));
    let ((), current_metrics) = measure(|| drop(current));
    assert!(old_metrics.allocated > 0);
    for metrics in [old_metrics, current_metrics] {
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.freed, metrics.allocated);
    }
    assert_eq!(current_metrics.allocated, 0);
    whole_lifetime_control(true);
}
#[test]
fn nested_nominal_payloads_keep_iterative_depth_512_disposal_and_complete_settlement() {
    let mut current = fixture(1);
    let mut ty = ResolvedType::I64;
    for _ in 0..512 {
        ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("deep.nominal"),
            arguments: vec![ty],
        };
    }
    current.record_fields.values_mut().next().unwrap()[0].ty = ty;
    // Construct an independent twin iteratively; recursive Clone is not the
    // operation this hostile depth disposal contract admits or measures.
    let mut old = fixture(1);
    let mut ty = ResolvedType::I64;
    for _ in 0..512 {
        ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("deep.nominal"),
            arguments: vec![ty],
        };
    }
    old.record_fields.values_mut().next().unwrap()[0].ty = ty;
    let ((), old_metrics) = measure(|| old_drop(old));
    let ((), current_metrics) = measure(|| drop(current));
    for metrics in [old_metrics, current_metrics] {
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.freed, metrics.allocated);
    }
}
