//! Actual paid constructors and complete independent original node destruction.
use super::*;
use crate::test_allocation_observer::{measure, watch_free_order, Metrics};

fn fixture() -> ResolvedProgram {
    let parsed = crate::check(
        "module dag.frontier; @id(\"main\") fn main()->i64 {0}",
        "dag-frontier.spx",
    )
    .unwrap();
    crate::hir::resolve(&parsed).unwrap()
}
fn clean(observed: Metrics) {
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}
fn input_tail(
    work: &mut SkeletonWork<'_, '_>,
    item: &Observation,
) -> Result<Vec<Observation>, Diagnostic> {
    work.budget
        .reserve_array::<Observation>(work.function, 1, "frontier fixture owned tail")?;
    Ok(vec![item.clone()])
}
fn union_probe(
    program: &ResolvedProgram,
    limit: usize,
    units: usize,
) -> (Result<(usize, usize), ()>, Metrics) {
    let function = &program.functions[0];
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    budget.skeleton_remaining = units;
    let mut work = SkeletonWork::new(program, function, &mut budget);
    let input = (0..4).map(|_| Observations::default()).collect();
    let before = work.budget.physical.as_ref().unwrap().charged();
    measure(|| {
        let result = work.ordinary_union(input);
        if let Ok(value) = &result {
            let root = value.prefix.as_ref().unwrap();
            assert_eq!(root.ordinal, Some(4));
            assert_eq!(root.pending_bound, Some(4));
            let ObservationDagKind::Alt(parts) = &root.kind else {
                panic!("union");
            };
            assert_eq!(parts.len(), 4);
            for (i, part) in parts.iter().enumerate() {
                assert_eq!(part.ordinal, Some(i));
                assert!(
                    matches!(&part.kind, ObservationDagKind::Seq(None, tail) if tail.is_empty())
                );
            }
        }
        let success = result.is_ok();
        drop(result);
        let spent = work.budget.skeleton_spent;
        let charged = work.budget.physical.as_ref().unwrap().charged() - before;
        if !success {
            let paid = work.budget.physical.as_ref().unwrap().charged();
            let sticky = work.budget.reserve_bytes(function, 0, "frontier sticky");
            assert!(sticky.is_err());
            drop(sticky);
            assert_eq!(work.budget.physical.as_ref().unwrap().charged(), paid);
        }
        // This receiving owner includes the reusable backing retained by Work.
        drop(work);
        if success {
            Ok((charged, spent))
        } else {
            Err(())
        }
    })
}
#[test]
fn actual_empty_union_prepays_literal_frontier_node_backing_and_complete_work_drop() {
    let program = fixture();
    let function = &program.functions[0];
    let setup = ReplayBudget::with_physical(function, 1 << 20)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let capacity = physical_storage::replay_growth_capacity(0, 4).unwrap();
    let required = 5 * physical_clone::rc_backing_bytes::<ObservationDag>()
        + 4 * std::mem::size_of::<Rc<ObservationDag>>()
        + physical_clone::rc_backing_bytes::<std::cell::RefCell<dag_storage::DestructionScratch>>()
        + capacity * std::mem::size_of::<Rc<ObservationDag>>();
    let (result, observed) = union_probe(&program, setup + required, 18);
    assert_eq!(result.unwrap(), (required, 18));
    assert_eq!(observed.allocations, 8);
    assert_eq!(observed.allocated, required);
    clean(observed);
    for (bytes, units) in [(required - 1, 18), (0, 18), (required, 0), (required, 17)] {
        let (result, observed) = union_probe(&program, setup + bytes, units);
        assert!(result.is_err());
        clean(observed);
    }
}
fn marker(function: &ResolvedFunction) -> Observation {
    Rc::new(SkeletonObservation::Boolean {
        expression: function.body.id.clone(),
        value: true,
    })
}
fn backing<T>(node: &Rc<T>) -> usize {
    // Same two-count Rc header layout used by the physical Rc backing owner.
    // The watch validates this exact address against an actual live allocation.
    (Rc::as_ptr(node) as usize) - 2 * std::mem::size_of::<usize>()
}
fn current_order(program: &ResolvedProgram, item: &Observation) -> ([usize; 32], Metrics) {
    let function = &program.functions[0];
    let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut work = SkeletonWork::new(program, function, &mut budget);
    let before = work.budget.physical.as_ref().unwrap().charged();
    let ((order, paid), metrics) = measure(|| {
        let tail = input_tail(&mut work, item).unwrap();
        let leaf = work
            .ordinary_dag(ObservationDagKind::Seq(None, tail))
            .unwrap();
        let leaf_tail = match &leaf.kind {
            ObservationDagKind::Seq(_, t) => t.as_ptr() as usize,
            _ => unreachable!(),
        };
        let leaf_backing = backing(&leaf);
        let tail = input_tail(&mut work, item).unwrap();
        let middle = work
            .ordinary_dag(ObservationDagKind::Seq(Some(leaf), tail))
            .unwrap();
        let middle_tail = match &middle.kind {
            ObservationDagKind::Seq(_, t) => t.as_ptr() as usize,
            _ => unreachable!(),
        };
        let middle_backing = backing(&middle);
        let weak = Rc::downgrade(&middle);
        let copied_weak = weak.clone();
        let tail = input_tail(&mut work, item).unwrap();
        let root = work
            .ordinary_dag(ObservationDagKind::Seq(Some(middle), tail))
            .unwrap();
        assert_eq!(root.pending_bound, Some(1));
        let root_tail = match &root.kind {
            ObservationDagKind::Seq(_, t) => t.as_ptr() as usize,
            _ => unreachable!(),
        };
        let root_weak = Rc::downgrade(&root);
        let pointers = [
            root_tail,
            middle_tail,
            leaf_tail,
            backing(&root),
            middle_backing,
            leaf_backing,
        ];
        assert!(root_weak.upgrade().is_some());
        assert!(weak.upgrade().is_some() && copied_weak.upgrade().is_some());
        let ((), report) = watch_free_order(&pointers, || {
            drop(root);
            assert!(
                root_weak.upgrade().is_none()
                    && weak.upgrade().is_none()
                    && copied_weak.upgrade().is_none()
            );
            drop(copied_weak);
            drop(weak);
            drop(root_weak);
        });
        assert!(!report.invalid);
        assert_eq!(report.count, 6);
        let mut tails = [usize::MAX; 3];
        for (count, ordinal) in report.order[..6]
            .iter()
            .copied()
            .filter(|i| *i < 3)
            .enumerate()
        {
            tails[count] = ordinal;
        }
        assert_eq!(tails, [1, 2, 0]);
        let paid = work.budget.physical.as_ref().unwrap().charged() - before;
        drop(work);
        (report.order, paid)
    });
    assert!(metrics.allocated <= paid);
    (order, metrics)
}
fn saved_order(program: &ResolvedProgram, item: &Observation) -> ([usize; 32], Metrics) {
    let function = &program.functions[0];
    let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut work = saved::SkeletonWork::new(program, function, &mut budget);
    let (order, metrics) = measure(|| {
        let make_tail = || vec![item.clone()];
        let leaf = work
            .ordinary_dag(saved::ObservationDagKind::Seq(None, make_tail()))
            .unwrap();
        let leaf_tail = match &leaf.kind {
            saved::ObservationDagKind::Seq(_, t) => t.as_ptr() as usize,
            _ => unreachable!(),
        };
        let leaf_backing = backing(&leaf);
        let middle = work
            .ordinary_dag(saved::ObservationDagKind::Seq(Some(leaf), make_tail()))
            .unwrap();
        let middle_tail = match &middle.kind {
            saved::ObservationDagKind::Seq(_, t) => t.as_ptr() as usize,
            _ => unreachable!(),
        };
        let middle_backing = backing(&middle);
        let weak = Rc::downgrade(&middle);
        let copied_weak = weak.clone();
        let root = work
            .ordinary_dag(saved::ObservationDagKind::Seq(Some(middle), make_tail()))
            .unwrap();
        let root_tail = match &root.kind {
            saved::ObservationDagKind::Seq(_, t) => t.as_ptr() as usize,
            _ => unreachable!(),
        };
        let root_weak = Rc::downgrade(&root);
        let pointers = [
            root_tail,
            middle_tail,
            leaf_tail,
            backing(&root),
            middle_backing,
            leaf_backing,
        ];
        assert!(root_weak.upgrade().is_some());
        assert!(weak.upgrade().is_some() && copied_weak.upgrade().is_some());
        let ((), report) = watch_free_order(&pointers, || {
            drop(root);
            assert!(
                root_weak.upgrade().is_none()
                    && weak.upgrade().is_none()
                    && copied_weak.upgrade().is_none()
            );
            drop(copied_weak);
            drop(weak);
            drop(root_weak);
        });
        assert!(!report.invalid);
        assert_eq!(report.count, 6);
        #[allow(
            clippy::drop_non_drop,
            reason = "End this borrowed proof at the measured lifetime boundary."
        )]
        drop(work);
        report.order
    });
    (order, metrics)
}
#[test]
fn actual_weak_headers_and_nonempty_sequence_tails_keep_independent_original_drop_order() {
    let program = fixture();
    let item = marker(&program.functions[0]);
    let (old, old_metrics) = saved_order(&program, &item);
    let (current, current_metrics) = current_order(&program, &item);
    assert_eq!(current, old);
    clean(old_metrics);
    clean(current_metrics);
    assert_eq!(Rc::strong_count(&item), 1);
}
fn deep_probe(
    program: &ResolvedProgram,
    depth: usize,
    limit: usize,
) -> (Result<(usize, usize), ()>, Metrics) {
    let function = &program.functions[0];
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut work = SkeletonWork::new(program, function, &mut budget);
    let before = work.budget.physical.as_ref().unwrap().charged();
    measure(|| {
        let result = (|| {
            let mut root = work.ordinary_dag(ObservationDagKind::Seq(None, Vec::new()))?;
            for _ in 0..depth {
                work.budget.reserve_array::<Rc<ObservationDag>>(
                    function,
                    2,
                    "duplicate fixture members",
                )?;
                root = work.ordinary_dag(ObservationDagKind::Alt(vec![root.clone(), root]))?;
            }
            assert_eq!(
                root.pending_bound,
                Some(if depth == 0 { 0 } else { depth + 1 })
            );
            let capacity = work
                .ordinary_scratch
                .as_ref()
                .map_or(0, |s| s.borrow().pending.capacity());
            let weak = Rc::downgrade(&root);
            drop(root);
            assert!(weak.upgrade().is_none());
            drop(weak);
            Ok::<usize, Diagnostic>(capacity)
        })();
        let charged = work.budget.physical.as_ref().unwrap().charged() - before;
        let result = match result {
            Ok(capacity) => Ok((charged, capacity)),
            Err(error) => {
                drop(error);
                Err(())
            }
        };
        drop(work);
        result
    })
}
#[test]
fn duplicate_deep_dag_frontier_is_linear_and_all_publication_failures_drop_paid_children() {
    let program = fixture();
    let function = &program.functions[0];
    let setup = ReplayBudget::with_physical(function, 1 << 24)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    for depth in [0, 1, 17, 1024] {
        let (result, observed) = deep_probe(&program, depth, 1 << 24);
        let (paid, capacity) = result.unwrap();
        assert!(observed.allocated <= paid);
        assert!(capacity <= 2 * (depth + 1).max(4));
        clean(observed);
        let (exact, observed) = deep_probe(&program, depth, setup + paid);
        assert_eq!(exact.unwrap(), (paid, capacity));
        assert!(observed.allocated <= paid);
        clean(observed);
        for extra in [0, paid - 1] {
            let (result, observed) = deep_probe(&program, depth, setup + extra);
            assert!(result.is_err());
            clean(observed);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Tree {
    Seq(Option<usize>, Option<Box<Tree>>, Vec<Observation>),
    Cat(Option<usize>, Vec<Tree>),
    Alt(Option<usize>, Vec<Tree>),
}
fn tree(node: &ObservationDag) -> Tree {
    match &node.kind {
        ObservationDagKind::Seq(prefix, items) => Tree::Seq(
            node.ordinal,
            prefix.as_ref().map(|p| Box::new(tree(p))),
            items.clone(),
        ),
        ObservationDagKind::Cat(parts) => {
            Tree::Cat(node.ordinal, parts.iter().map(|p| tree(p)).collect())
        }
        ObservationDagKind::Alt(parts) => {
            Tree::Alt(node.ordinal, parts.iter().map(|p| tree(p)).collect())
        }
    }
}
fn old_tree(node: &saved::ObservationDag) -> Tree {
    match &node.kind {
        saved::ObservationDagKind::Seq(prefix, items) => Tree::Seq(
            node.ordinal,
            prefix.as_ref().map(|p| Box::new(old_tree(p))),
            items.clone(),
        ),
        saved::ObservationDagKind::Cat(parts) => {
            Tree::Cat(node.ordinal, parts.iter().map(|p| old_tree(p)).collect())
        }
        saved::ObservationDagKind::Alt(parts) => {
            Tree::Alt(node.ordinal, parts.iter().map(|p| old_tree(p)).collect())
        }
    }
}
#[test]
fn complete_cat_alt_owned_tails_duplicate_and_external_owner_keep_saved_values() {
    let program = fixture();
    let function = &program.functions[0];
    let item = marker(function);
    let mut old_budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    let mut new_budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    old_budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    new_budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut old = saved::SkeletonWork::new(&program, function, &mut old_budget);
    let mut new = SkeletonWork::new(&program, function, &mut new_budget);
    for alternatives in [false, true] {
        let old_head = old
            .ordinary_dag(saved::ObservationDagKind::Seq(None, vec![item.clone()]))
            .unwrap();
        let tail = input_tail(&mut new, &item).unwrap();
        let new_head = new
            .ordinary_dag(ObservationDagKind::Seq(None, tail))
            .unwrap();
        let old_tail = old
            .ordinary_dag(saved::ObservationDagKind::Seq(
                Some(old_head.clone()),
                vec![item.clone(), item.clone()],
            ))
            .unwrap();
        new.budget
            .reserve_array::<Observation>(function, 2, "paired owned tail")
            .unwrap();
        let new_tail = new
            .ordinary_dag(ObservationDagKind::Seq(
                Some(new_head.clone()),
                vec![item.clone(), item.clone()],
            ))
            .unwrap();
        let old_parts = vec![old_head.clone(), old_tail.clone(), old_tail];
        new.budget
            .reserve_array::<Rc<ObservationDag>>(function, 3, "paired owned parts")
            .unwrap();
        let new_parts = vec![new_head.clone(), new_tail.clone(), new_tail];
        let old_root = old
            .ordinary_dag(if alternatives {
                saved::ObservationDagKind::Alt(old_parts)
            } else {
                saved::ObservationDagKind::Cat(old_parts)
            })
            .unwrap();
        let new_root = new
            .ordinary_dag(if alternatives {
                ObservationDagKind::Alt(new_parts)
            } else {
                ObservationDagKind::Cat(new_parts)
            })
            .unwrap();
        assert_eq!(tree(&new_root), old_tree(&old_root));
        assert_eq!(new_root.pending_bound, Some(3));
        let weak = Rc::downgrade(&new_head);
        let retained = new_root.clone();
        drop(new_root);
        assert!(weak.upgrade().is_some());
        let ((), drop_metrics) = measure(|| {
            drop(retained);
            drop(new_head);
        });
        assert_eq!(drop_metrics.allocated, 0);
        clean(drop_metrics);
        assert!(weak.upgrade().is_none());
        drop(weak);
        drop(old_root);
        drop(old_head);
    }
}
#[test]
fn foreign_work_and_ledger_children_need_only_immutable_structural_bounds() {
    let program = fixture();
    let function = &program.functions[0];
    let mut first_budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    let mut second_budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    first_budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    second_budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut first = SkeletonWork::new(&program, function, &mut first_budget);
    let mut second = SkeletonWork::new(&program, function, &mut second_budget);
    let before_first = first.budget.physical.as_ref().unwrap().charged();
    let before_second = second.budget.physical.as_ref().unwrap().charged();
    let (paid, observed) = measure(|| {
        let leaf = first
            .ordinary_dag(ObservationDagKind::Seq(None, Vec::new()))
            .unwrap();
        let child = first
            .ordinary_dag(ObservationDagKind::Seq(Some(leaf), Vec::new()))
            .unwrap();
        let parent = second
            .ordinary_dag(ObservationDagKind::Seq(Some(child), Vec::new()))
            .unwrap();
        assert_eq!(parent.pending_bound, Some(1));
        drop(parent);
        let paid = first.budget.physical.as_ref().unwrap().charged() - before_first
            + second.budget.physical.as_ref().unwrap().charged()
            - before_second;
        drop(first);
        drop(second);
        paid
    });
    assert!(observed.allocated <= paid);
    clean(observed);
}
#[test]
fn successful_paid_parent_drains_legacy_children_but_zero_refusal_preserves_legacy_boundary() {
    let program = fixture();
    let function = &program.functions[0];
    let legacy = || {
        let leaf = Rc::new(ObservationDag::legacy(ObservationDagKind::Seq(
            None,
            Vec::new(),
        )));
        let child = Rc::new(ObservationDag::legacy(ObservationDagKind::Seq(
            Some(leaf),
            Vec::new(),
        )));
        ObservationDagKind::Seq(Some(child), Vec::new())
    };
    let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut work = SkeletonWork::new(&program, function, &mut budget);
    let parent = work.ordinary_dag(legacy()).unwrap();
    let ((), observed) = measure(|| drop(parent));
    assert_eq!(observed.allocated, 0);
    clean(observed);
    drop(work);
    let setup = ReplayBudget::with_physical(function, 1 << 20)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let mut budget = ReplayBudget::with_physical(function, setup).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut work = SkeletonWork::new(&program, function, &mut budget);
    let input = legacy();
    let ((), observed) = measure(|| {
        let result = work.ordinary_dag(input);
        assert!(result.is_err());
        drop(result);
        drop(work);
    });
    // This is deliberately the unchanged unfunded compatibility receiver.
    // Admission failed before a parent frontier existed: do not count it paid.
    assert!(observed.allocations > 0);
    clean(observed);
}
#[test]
fn checked_bound_overflow_refuses_before_node_publication_and_preserves_sticky_payer() {
    let program = fixture();
    let function = &program.functions[0];
    let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut work = SkeletonWork::new(&program, function, &mut budget);
    let first = Rc::new(ObservationDag::legacy(ObservationDagKind::Seq(
        None,
        Vec::new(),
    )));
    let mut forged = ObservationDag::legacy(ObservationDagKind::Seq(None, Vec::new()));
    forged.pending_bound = Some(usize::MAX);
    let input = ObservationDagKind::Alt(vec![first, Rc::new(forged)]);
    let before = work.budget.physical.as_ref().unwrap().charged();
    let result = work.ordinary_dag(input);
    assert!(result.is_err());
    drop(result);
    assert_eq!(work.next_ordinary_dag, 0);
    assert!(work.ordinary_scratch.is_none());
    assert_eq!(work.budget.physical.as_ref().unwrap().charged(), before);
    work.budget
        .reserve_bytes(function, usize::MAX, "physical sticky control")
        .unwrap_err();
    let charged = work.budget.physical.as_ref().unwrap().charged();
    assert!(work
        .ordinary_dag(ObservationDagKind::Seq(None, Vec::new()))
        .is_err());
    assert_eq!(work.budget.physical.as_ref().unwrap().charged(), charged);
    assert_eq!(work.next_ordinary_dag, 0);
}
#[test]
fn independent_original_type_drop_builders_and_complete_receiver_are_byte_locked() {
    let original =
        include_str!("../../../../../docs/evidence/ordinary-dag-drop/original-skeleton-work.txt");
    let saved = include_str!("saved.rs");
    // Audit the actual compiled receiver, including the complete frozen file
    // and the only appended access bridges, rather than an unused archive.
    use sha2::Digest;
    assert_eq!(saved.len(), 14172);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(sha2::Sha256::digest(saved.as_bytes()))
        ),
        "fd8afd796f7f05555b304fed01cdf92fd1fafa069f99c6b20f371fd3ffc43223"
    );
    let compiled = include_str!("saved_receiver.rs");
    let (compiled_original, compiled_bridges) = compiled.split_at(saved.len());
    assert_eq!(compiled_original.as_bytes(), saved.as_bytes());
    assert_eq!(
        compiled_bridges,
        r#"
// Test-only lexical access bridges; the complete saved receiver above is verbatim.
impl Observations {
    pub(super) fn union_for_test(members: Vec<Observations>) -> Self {
        Self::union(members)
    }
}

impl SkeletonWork<'_, '_> {
    pub(super) fn charge_for_test(&mut self, units: usize, phase: &str) -> Result<(), Diagnostic> {
        self.charge(units, phase)
    }
}
"#
    );
    let start = original
        .find("#[derive(Clone, Default)]\npub(super) struct Observations")
        .unwrap();
    let end = original[start..].find("impl SkeletonWork").unwrap() + start;
    assert!(saved.contains(&original[start..end]));
    let current = include_str!("../../skeleton_work.rs");
    let old_drop_start = original.find("impl Drop for ObservationDag {").unwrap();
    let old_drop_end = original[old_drop_start..]
        .find("\nimpl ObservationDag {")
        .unwrap()
        + old_drop_start;
    let current_drop_start = current.find("impl Drop for ObservationDag {").unwrap();
    let current_drop_end = current[current_drop_start..]
        .find("\nimpl ObservationDag {")
        .unwrap()
        + current_drop_start;
    assert_eq!(
        &current[current_drop_start..current_drop_end],
        &original[old_drop_start..old_drop_end]
    );
    let original_builders =
        include_str!("../../../../../docs/evidence/ordinary-dag-drop/original-ordinary-dag.txt");
    let start = original_builders.find("impl SkeletonWork").unwrap();
    let end = original_builders.find("#[cfg(test)]\nmod tests;").unwrap();
    assert!(saved.contains(&original_builders[start..end]));
    let receiver_source = include_str!("tests.rs");
    assert_eq!(
        receiver_source
            .matches("Observations::union_for_test(old_members)")
            .count(),
        1
    );
    assert_eq!(
        receiver_source
            .matches("work.charge_for_test(0, \"sticky ordinal work\")")
            .count(),
        1
    );
    let receiver = receiver_source
        .replace(
            "use super::saved::{ObservationDag, ObservationDagKind, Observations, SkeletonWork};\n",
            "",
        )
        .replace(
            "Observations::union_for_test(old_members)",
            "Observations::union(old_members)",
        )
        .replace(
            "work.charge_for_test(0, \"sticky ordinal work\")",
            "work.charge(0, \"sticky ordinal work\")",
        );
    let storage_source = include_str!("../dag_storage/tests.rs");
    assert_eq!(
        storage_source
            .matches("drop(work.ordinary_scratch.take());")
            .count(),
        2
    );
    let storage = storage_source.replace("        drop(work.ordinary_scratch.take());\n", "");
    assert_eq!(
        storage,
        include_str!(
            "../../../../../docs/evidence/ordinary-dag-drop/original-dag-storage-tests.txt"
        )
    );
    let tails_source = include_str!("../dag_storage/tests/tail_moves.rs");
    assert_eq!(
        tails_source
            .matches("drop(work.ordinary_scratch.take());")
            .count(),
        4
    );
    let tails = tails_source
        .replace("            drop(work.ordinary_scratch.take());\n", "")
        .replace("        drop(work.ordinary_scratch.take());\n", "");
    assert_eq!(
        tails,
        include_str!("../../../../../docs/evidence/ordinary-dag-drop/original-tail-moves.txt")
    );
    assert_eq!(
        receiver,
        include_str!(
            "../../../../../docs/evidence/ordinary-dag-drop/original-ordinary-dag-tests.txt"
        )
    );
}

#[test]
fn foreign_selected_conservative_bound_is_paid_in_full_without_new_session_policy() {
    let program = fixture();
    let function = &program.functions[0];
    let mut selected_budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    let mut ordinary_budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
    selected_budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    ordinary_budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut selected = SkeletonWork::new(&program, function, &mut selected_budget);
    let mut ordinary = SkeletonWork::new(&program, function, &mut ordinary_budget);
    let before_selected = selected.budget.physical.as_ref().unwrap().charged();
    let before_ordinary = ordinary.budget.physical.as_ref().unwrap().charged();
    let (paid, observed) = measure(|| {
        let leaf = selected
            .selected_dag_for_foreign_control(ObservationDagKind::Seq(None, Vec::new()))
            .unwrap();
        assert_eq!(leaf.pending_bound, Some(0));
        let mut unrelated = selected
            .selected_dag_for_foreign_control(ObservationDagKind::Seq(None, Vec::new()))
            .unwrap();
        for _ in 0..8 {
            selected
                .budget
                .reserve_array::<Rc<ObservationDag>>(
                    function,
                    2,
                    "selected foreign fixture members",
                )
                .unwrap();
            unrelated = selected
                .selected_dag_for_foreign_control(ObservationDagKind::Alt(vec![
                    unrelated.clone(),
                    unrelated,
                ]))
                .unwrap();
        }
        let child = selected
            .selected_dag_for_foreign_control(ObservationDagKind::Seq(Some(leaf), Vec::new()))
            .unwrap();
        assert_eq!(child.pending_bound, Some(17));
        let parent = ordinary
            .ordinary_dag(ObservationDagKind::Seq(Some(child), Vec::new()))
            .unwrap();
        assert_eq!(parent.pending_bound, Some(17));
        assert!(
            ordinary
                .ordinary_scratch
                .as_ref()
                .unwrap()
                .borrow()
                .pending
                .capacity()
                >= 17
        );
        drop(unrelated);
        drop(parent);
        let paid = selected.budget.physical.as_ref().unwrap().charged() - before_selected
            + ordinary.budget.physical.as_ref().unwrap().charged()
            - before_ordinary;
        drop(selected);
        drop(ordinary);
        paid
    });
    assert!(observed.allocated <= paid);
    clean(observed);
}

#[test]
fn cat_alt_actual_member_backing_and_rc_release_order_pin_the_physical_frontier_difference() {
    use super::ObservationDagKind as NewKind;
    use saved::ObservationDagKind as OldKind;
    let program = fixture();
    let function = &program.functions[0];
    let item = marker(function);
    macro_rules! receiver {
        ($entry:expr, $kind:ident, $node:ty, $alternatives:expr) => {{
            let mut budget = ReplayBudget::with_physical(function, 1 << 20).unwrap();
            budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
            let mut work = ($entry)(&program, function, &mut budget);
            let before = work.budget.physical.as_ref().unwrap().charged();
            let ((report, paid), metrics) = measure(|| {
                let mut leaves = Vec::new();
                // Fixture metadata is outside the DAG ownership proof but is
                // itself explicitly paid before its actual Vec acquisition.
                work.budget
                    .reserve_array::<Rc<$node>>(function, 5, "order fixture temporary leaves")
                    .unwrap();
                leaves.try_reserve_exact(5).unwrap();
                let mut tails = [0usize; 5];
                let mut headers = [0usize; 5];
                for i in 0..5 {
                    work.budget
                        .reserve_array::<Observation>(function, 1, "order fixture tail")
                        .unwrap();
                    let leaf = work
                        .ordinary_dag($kind::Seq(None, vec![item.clone()]))
                        .unwrap();
                    tails[i] = match &leaf.kind {
                        $kind::Seq(_, tail) => tail.as_ptr() as usize,
                        _ => unreachable!(),
                    };
                    headers[i] = backing(&leaf);
                    leaves.push(leaf);
                }
                work.budget
                    .reserve_array::<Rc<$node>>(function, 2, "order fixture inner parts")
                    .unwrap();
                let fourth = leaves.pop().unwrap();
                let third = leaves.pop().unwrap();
                let inner = work
                    .ordinary_dag(if $alternatives {
                        $kind::Alt(vec![third, fourth])
                    } else {
                        $kind::Cat(vec![third, fourth])
                    })
                    .unwrap();
                let inner_parts = match &inner.kind {
                    $kind::Cat(p) | $kind::Alt(p) => p.as_ptr() as usize,
                    _ => unreachable!(),
                };
                let inner_header = backing(&inner);
                work.budget
                    .reserve_array::<Rc<$node>>(function, 4, "order fixture root parts")
                    .unwrap();
                let first = leaves.remove(0);
                let second = leaves.remove(0);
                let third = leaves.remove(0);
                let root = work
                    .ordinary_dag(if $alternatives {
                        $kind::Cat(vec![first, second, third, inner])
                    } else {
                        $kind::Alt(vec![first, second, third, inner])
                    })
                    .unwrap();
                let root_parts = match &root.kind {
                    $kind::Cat(p) | $kind::Alt(p) => p.as_ptr() as usize,
                    _ => unreachable!(),
                };
                let pointers = [
                    root_parts,
                    inner_parts,
                    tails[0],
                    tails[1],
                    tails[2],
                    tails[3],
                    tails[4],
                    backing(&root),
                    inner_header,
                    headers[0],
                    headers[1],
                    headers[2],
                    headers[3],
                    headers[4],
                ];
                let ((), report) = watch_free_order(&pointers, || drop(root));
                assert!(!report.invalid);
                assert_eq!(report.count, pointers.len());
                drop(leaves);
                let paid = work.budget.physical.as_ref().unwrap().charged() - before;
                #[allow(
                    clippy::drop_non_drop,
                    reason = "End this borrowed proof at the measured lifetime boundary."
                )]
                drop(work);
                (report, paid)
            });
            (report, paid, metrics)
        }};
    }
    for alternatives in [false, true] {
        let (old, _, old_metrics) = receiver!(
            saved::SkeletonWork::new,
            OldKind,
            saved::ObservationDag,
            alternatives
        );
        let (current, paid, current_metrics) =
            receiver!(SkeletonWork::new, NewKind, ObservationDag, alternatives);
        let old_at = |ordinal| {
            old.order[..old.count]
                .iter()
                .position(|i| *i == ordinal)
                .unwrap()
        };
        let current_at = |ordinal| {
            current.order[..current.count]
                .iter()
                .position(|i| *i == ordinal)
                .unwrap()
        };
        // Legacy root member storage doubles as the frontier and is released
        // on its actual realloc before the inner member carrier is released.
        assert!(old_at(0) < old_at(1));
        // The prepaid route retains root member backing until field Drop.
        assert!(current_at(1) < current_at(0));
        for tail in 2..7 {
            assert!(current_at(tail) < current_at(0));
        }
        let mut old_tails = [usize::MAX; 5];
        let mut current_tails = [usize::MAX; 5];
        for (oi, ordinal) in old.order[..old.count]
            .iter()
            .copied()
            .filter(|i| (2..7).contains(i))
            .enumerate()
        {
            old_tails[oi] = ordinal;
        }
        for (ci, ordinal) in current.order[..current.count]
            .iter()
            .copied()
            .filter(|i| (2..7).contains(i))
            .enumerate()
        {
            current_tails[ci] = ordinal;
        }
        assert_eq!(current_tails, old_tails);
        assert_eq!(current_tails, [6, 5, 4, 3, 2]);
        assert!(old_metrics.reallocations > 0);
        assert_eq!(current_metrics.reallocations, 0);
        assert!(current_metrics.allocated <= paid);
        clean(old_metrics);
        clean(current_metrics);
    }
}
