use super::saved::{ObservationDag, ObservationDagKind, Observations, SkeletonWork};
use super::*;
use crate::test_allocation_observer::measure;

fn fixture() -> ResolvedProgram {
    let source = crate::check(
        "module ordinary.dag; @id(\"main\") fn main()->i64 {0}",
        "ordinary-dag.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}

#[test]
fn ordinary_empty_union_preserves_saved_multiset_and_literal_node_storage_through_drop() {
    let program = fixture();
    let function = &program.functions[0];
    let required = 5 * physical_clone::rc_backing_bytes::<ObservationDag>()
        + 4 * std::mem::size_of::<Rc<ObservationDag>>();
    let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let setup = budget.physical.as_ref().unwrap().charged();
    let mut work = SkeletonWork::new(&program, function, &mut budget);
    let members = || (0..4).map(|_| Observations::default()).collect::<Vec<_>>();
    let old_members = members();
    let new_members = members();
    let (_, old) = measure(|| drop(Observations::union_for_test(old_members)));
    let ((), new) = measure(|| {
        let value = work.ordinary_union(new_members).unwrap();
        let root = value.prefix.as_ref().unwrap();
        assert_eq!(root.ordinal, Some(4));
        let ObservationDagKind::Alt(parts) = &root.kind else {
            panic!("saved union shape");
        };
        assert_eq!(parts.len(), 4);
        for (ordinal, part) in parts.iter().enumerate() {
            assert_eq!(part.ordinal, Some(ordinal));
            assert!(matches!(&part.kind, ObservationDagKind::Seq(None, items) if items.is_empty()));
        }
        drop(value);
    });
    assert_eq!(old.allocations, 6);
    assert_eq!(new.allocations, 6);
    assert_eq!(old.allocated, required);
    assert_eq!(new.allocated, required);
    assert_eq!(
        work.budget.physical.as_ref().unwrap().charged() - setup,
        required
    );
    assert_eq!(work.next_ordinary_dag, 5);
    assert_eq!(work.budget.skeleton_spent, 5);
    for observed in [old, new] {
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    #[allow(
        clippy::drop_non_drop,
        reason = "End this borrowed proof at the measured lifetime boundary."
    )]
    drop(work);
    for (extra, accepted) in [(required, true), (required - 1, false)] {
        let mut budget = ReplayBudget::with_physical(function, setup + extra).unwrap();
        budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let input = members();
        let (result, observed) = measure(|| {
            let result = work.ordinary_union(input);
            let accepted = result.is_ok();
            drop(result);
            accepted
        });
        assert_eq!(result, accepted);
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        if !accepted {
            let used = work.budget.physical.as_ref().unwrap().charged();
            assert!(work
                .budget
                .reserve_bytes(function, 0, "sticky ordinary union")
                .is_err());
            assert_eq!(work.budget.physical.as_ref().unwrap().charged(), used);
        }
    }
    // Five new ordinals are five actual metadata publications. This is a
    // literal work boundary, independent of measured storage or implementation
    // counters, and it does not claim old low-budget admission is unchanged.
    for (units, accepted, nodes) in [(5, true, 5), (4, false, 4)] {
        let mut budget = ReplayBudget::with_physical(function, setup + required).unwrap();
        budget.skeleton_remaining = units;
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let result = work.ordinary_union(members());
        assert_eq!(result.is_ok(), accepted);
        drop(result);
        assert_eq!(work.next_ordinary_dag, nodes);
        assert_eq!(work.budget.skeleton_spent, nodes);
        if !accepted {
            let charged = work.budget.physical.as_ref().unwrap().charged();
            assert!(work.charge_for_test(0, "sticky ordinal work").is_err());
            assert_eq!(work.budget.physical.as_ref().unwrap().charged(), charged);
        }
    }
}
