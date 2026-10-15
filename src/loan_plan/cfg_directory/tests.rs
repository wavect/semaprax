//! Independent old BTree carriers and complete saved planner remain the oracle.
//! Inputs and the active Budget/Rc are prepared outside the observer; owner,
//! construction, lookups, replacements, canonical drain and Drop are inside.
use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};

fn orders(count: usize) -> [Vec<usize>; 5] {
    [
        (0..count).collect(),
        (0..count).rev().collect(),
        (0..count).step_by(2).chain((1..count).step_by(2)).collect(),
        if count >= 3 {
            [2, 0, 1].into_iter().chain(3..count).collect()
        } else {
            (0..count).collect()
        },
        if count >= 3 {
            [0, 2, 1].into_iter().chain(3..count).collect()
        } else {
            (0..count).collect()
        },
    ]
}

#[test]
fn whole_ordered_directory_ascending_descending_interleaved_receivers_keep_log_height() {
    for count in [0, 1, 2, 17, 128, MAX_LOAN_ENDPOINTS_V1] {
        for order in orders(count) {
            let ((), old) = measure(|| {
                let mut map = BTreeMap::new();
                let mut set = BTreeSet::new();
                for key in &order {
                    assert_eq!(map.insert(*key, *key + 1), None);
                    assert!(set.insert(*key));
                }
                for key in 0..count {
                    assert_eq!(map.get(&key), Some(&(key + 1)));
                    assert_eq!(map.insert(key, key + 2), Some(key + 1));
                    assert!(!set.insert(key));
                }
                assert_eq!(map.len(), count);
                assert_eq!(set.len(), count);
                // Original ordered extraction, no sorted-vector insertion oracle.
                let values: Vec<_> = set.into_iter().collect();
                assert!(values.iter().copied().eq(0..count));
                drop(values);
                drop(map);
            });
            let (((), current), refused, paid) = with_limit_usage(usize::MAX, || {
                measure(|| {
                    let owner = CfgReservation::acquire().unwrap();
                    let mut map = Map::new(&owner).unwrap();
                    let mut set = Set::new(&owner).unwrap();
                    for key in &order {
                        assert_eq!(map.insert(*key, *key + 1, &owner).unwrap(), None);
                        assert!(set_insert(&mut set, *key, Some(&owner)).unwrap());
                    }
                    for key in 0..count {
                        assert_eq!(map.get(&key), Some(&(key + 1)));
                        assert_eq!(map.insert(key, key + 2, &owner).unwrap(), Some(key + 1));
                        assert!(!set_insert(&mut set, key, Some(&owner)).unwrap());
                        assert_eq!(map.get(&key), Some(&(key + 2)));
                    }
                    assert_eq!(map.len, count);
                    assert_eq!(set.0.len, count);
                    // AVL permits at most twice the binary height. This is an
                    // insertion-work control, not a new production quota.
                    let bound = if count == 0 {
                        0
                    } else {
                        2 * (usize::BITS - count.leading_zeros()) as usize
                    };
                    assert!(tree::height(&map.root) <= bound);
                    assert!(tree::height(&set.0.root) <= bound);
                    let values = set.into_vec(&owner).unwrap();
                    assert!(values.iter().copied().eq(0..count));
                    drop(values);
                    drop(map);
                    drop(owner);
                })
            });
            assert!(!refused);
            assert!(current.allocated <= paid);
            assert!(current.highwater <= paid);
            for observed in [old, current] {
                assert_eq!(observed.freed, observed.allocated);
                assert_eq!(observed.live, 0);
                assert!(!observed.invalid);
            }
            // Report both actual whole receivers. No relative savings assertion
            // hides per-node AVL overhead or substitutes an owned-type oracle.
            eprintln!("CFG directories count={count} old={old:?} current={current:?} paid={paid}");
        }
    }
}

#[test]
fn exact_short_and_sticky_directory_payment_preserves_prior_publication() {
    let setup =
        std::mem::size_of::<CfgReservation>() + 46 + std::mem::size_of::<Map<usize, usize>>();
    let node = 2 * std::mem::size_of::<tree::Node<usize, usize>>();
    // Every denied byte of a prepared missing leaf retains the whole old map.
    // No directory allocation or partial mutation precedes that reservation.
    for available in 0..node {
        let (((), observed), refused, used, first) =
            with_limit_usage_refusal(setup + node + available, || {
                measure(|| {
                    let owner = CfgReservation::acquire().unwrap();
                    let mut map = Map::<usize, usize>::new(&owner).unwrap();
                    assert_eq!(map.insert(7, 11, &owner).unwrap(), None);
                    assert!(map.insert(3, 5, &owner).is_err());
                    let selected = owner.first().unwrap();
                    assert_eq!(owner.reserve(0), Err(selected));
                    assert_eq!(owner.reserve(1), Err(selected));
                    assert!(map.insert(7, 99, &owner).is_err());
                    assert_eq!(map.len, 1);
                    assert_eq!(map.get(&7), Some(&11));
                    assert_eq!(map.get(&3), None);
                    drop(map);
                    let pointer = owner.text_pointer();
                    let report = owner.diagnostic();
                    assert_eq!(report.message.as_ptr(), pointer);
                    drop(report);
                })
            });
        assert!(refused);
        assert!(first.is_some());
        assert_eq!(used, setup + node);
        assert_eq!(observed.freed, observed.allocated);
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
        assert!(observed.allocated <= used);
    }
    let (((), observed), refused, used) = with_limit_usage(setup + 2 * node, || {
        measure(|| {
            let owner = CfgReservation::acquire().unwrap();
            let mut map = Map::<usize, usize>::new(&owner).unwrap();
            map.insert(7, 11, &owner).unwrap();
            map.insert(3, 5, &owner).unwrap();
            let before = current_metrics();
            assert_eq!(map.insert(7, 99, &owner).unwrap(), Some(11));
            let after = current_metrics();
            assert_eq!(after.allocations, before.allocations);
            assert_eq!(after.reallocations, before.reallocations);
            assert_eq!(map.len, 2);
            assert_eq!(map.get(&7), Some(&99));
            assert_eq!(map.get(&3), Some(&5));
            drop(map);
            drop(owner);
        })
    });
    assert!(!refused);
    assert_eq!(used, setup + 2 * node);
    assert!(observed.allocated <= used);
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
}

#[test]
fn cfg_carriers_preserve_complete_canonical_graph_and_original_work_equations() {
    let (program, index) = boundary_tests::fixture_with_uses(17, true);
    let function = &program.functions[index];
    let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
    let old = build_cfg(function, &mut old_work).unwrap();
    let ((), refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let mut current_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        current_work.allocation = Some(&owner);
        let current = build_cfg(function, &mut current_work).unwrap();
        assert_eq!(current_work.used, old_work.used);
        assert_eq!(current.points, old.points);
        assert_eq!(current.edges, old.edges);
        assert_eq!(current.expressions.len(), old.expressions.len());
        for (actual, expected) in current.expressions.iter().zip(&old.expressions) {
            assert!(std::ptr::eq(*actual, *expected));
        }
        for (ordinal, point) in old.points.iter().enumerate() {
            assert_eq!(current.nodes.get(point), old.nodes.get(point));
            assert_eq!(
                current.successor_edges(ordinal),
                old.successor_edges(ordinal)
            );
            assert_eq!(current.predecessors(ordinal), old.predecessors(ordinal));
            assert!(current.successors(ordinal).eq(old.successors(ordinal)));
        }
    });
    assert!(!refused);
    // The original first charged-work failure is unchanged, including the
    // diagnostic's complete span/help. Physical refusal is tested separately.
    let mut old_work = WorkCounter::new(0);
    let old_error = build_cfg(function, &mut old_work).err().unwrap();
    let ((), refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let mut work = WorkCounter::new(0);
        work.allocation = Some(&owner);
        assert_eq!(build_cfg(function, &mut work).err().unwrap(), old_error);
        assert_eq!(work.used, old_work.used);
        assert_eq!(owner.first(), None);
    });
    assert!(!refused);
    let mut forged = function.clone();
    let ResolvedExprKind::Block { statements, .. } = &forged.body.kind else {
        panic!("original nested body");
    };
    let duplicate = statement_entry(&statements[0]).id.clone();
    forged.body.id = duplicate;
    let old_error = build_cfg(&forged, &mut WorkCounter::new(MAX_LOAN_PLAN_WORK_V1))
        .err()
        .unwrap();
    assert_eq!(
        old_error.message,
        "CFG contains a duplicate expression identity"
    );
    let ((), refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        work.allocation = Some(&owner);
        assert_eq!(build_cfg(&forged, &mut work).err().unwrap(), old_error);
        assert_eq!(owner.first(), None);
    });
    assert!(!refused);
}

#[test]
fn complete_saved_planner_constructor_receiving_and_drop_have_exact_short_sticky_payment() {
    for count in [1, 3, 17] {
        let (mut program, index) = boundary_tests::fixture_with_uses(count, true);
        let expected = saved_planner::build_plan(&program, &program.functions[index]).unwrap();
        assert!(!expected.loans.is_empty());
        assert_eq!(
            build_plan(&program, &program.functions[index]).unwrap(),
            expected
        );
        assert_whole_plan(&program, &program.functions[index], &expected);
        // Attached metadata gives neither sizing nor canonical-order authority.
        program.functions[index].loan_plan.loans.clear();
        program.functions[index].loan_plan.endpoints.reverse();
        program.functions[index].loan_plan.edges.reverse();
        assert_eq!(
            saved_planner::build_plan(&program, &program.functions[index]).unwrap(),
            expected
        );
        assert_whole_plan(&program, &program.functions[index], &expected);
    }
    // Preserve the complete original renewal application, with all functions,
    // fresh scopes and physical plan identities rather than a simplified loop.
    let source = include_str!("../../../tests/language/while_loops/record_borrow_renewal.spx");
    let ast = crate::check(source, "cfg-directory-renewal.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    for function in &program.functions {
        let expected = saved_planner::build_plan(&program, function).unwrap();
        assert_eq!(build_plan(&program, function).unwrap(), expected);
        assert_whole_plan(&program, function, &expected);
    }
}

fn assert_whole_plan(program: &ResolvedProgram, function: &ResolvedFunction, expected: &LoanPlan) {
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let owner = match CfgReservation::acquire() {
                    Ok(owner) => owner,
                    Err(_) => return false,
                };
                match funded_flat::build(program, function, &owner) {
                    Ok(plan) => {
                        assert_eq!(&plan, expected);
                        drop(plan);
                        drop(owner);
                        true
                    }
                    Err(storage::PlannerError::Physical(reason)) => {
                        assert_eq!(owner.reserve(0), Err(reason));
                        assert_eq!(owner.reserve(1), Err(reason));
                        let pointer = owner.text_pointer();
                        let report = owner.diagnostic();
                        assert_eq!(report.message.as_ptr(), pointer);
                        drop(report);
                        false
                    }
                    Err(storage::PlannerError::Semantic(error)) => {
                        panic!("original positive failed: {error:?}")
                    }
                }
            })
        })
    };
    let ((success, observed), refused, exact, first) = run(usize::MAX);
    assert!(success);
    assert!(!refused);
    assert_eq!(first, None);
    assert!(observed.allocated <= exact);
    assert!(observed.highwater <= exact);
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
    let ((success, repeated), refused, used, first) = run(exact);
    assert!(success);
    assert!(!refused);
    assert_eq!(used, exact);
    assert_eq!(first, None);
    assert_eq!(repeated, observed);
    let ((success, short), refused, used, first) = run(exact - 1);
    assert!(!success);
    assert!(refused);
    assert!(first.is_some());
    assert!(used < exact);
    assert_eq!(short.freed, short.allocated);
    assert_eq!(short.live, 0);
    assert!(!short.invalid);
    assert!(short.allocated <= used);
}

#[derive(Debug)]
struct ByOrdinal(usize, &'static str);
impl PartialEq for ByOrdinal {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for ByOrdinal {}
impl PartialOrd for ByOrdinal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ByOrdinal {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

#[test]
fn duplicate_keys_retain_original_key_and_detached_payer_cannot_publish() {
    let (((), observed), refused, paid) = with_limit_usage(usize::MAX, || {
        measure(|| {
            let owner = CfgReservation::acquire().unwrap();
            let mut set = Set::new(&owner).unwrap();
            assert!(set_insert(&mut set, ByOrdinal(7, "first"), Some(&owner)).unwrap());
            let before = current_metrics();
            assert!(!set_insert(&mut set, ByOrdinal(7, "replacement"), Some(&owner)).unwrap());
            assert_eq!(current_metrics(), before);
            let values = set.into_vec(&owner).unwrap();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0].0, 7);
            assert_eq!(values[0].1, "first");
            drop(values);
            drop(owner);
        })
    });
    assert!(!refused);
    assert!(observed.allocated <= paid);
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
    let ((owner, mut map), refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let mut map = Map::<usize, usize>::new(&owner).unwrap();
        map.insert(7, 11, &owner).unwrap();
        (owner, map)
    });
    assert!(!refused);
    // A new invocation supplies no authority to the retained prior payer.
    // This receiving-only window excludes prior storage, which drops below.
    let (((), receiving), refused, paid) = with_limit_usage(usize::MAX, || {
        measure(|| {
            assert!(map.insert(7, 99, &owner).is_err());
            assert!(map.insert(3, 5, &owner).is_err());
            assert_eq!(
                owner.first(),
                Some(crate::bounded_output::cfg_reservation::Refused::Detached)
            );
            assert_eq!(map.len, 1);
            assert_eq!(map.get(&7), Some(&11));
            assert_eq!(map.get(&3), None);
        })
    });
    assert!(!refused);
    assert_eq!(paid, 0);
    assert_eq!(receiving.allocations, 0);
    assert_eq!(receiving.reallocations, 0);
    assert_eq!(receiving.allocated, 0);
    assert_eq!(receiving.freed, 0);
    assert_eq!(receiving.live, 0);
    assert!(!receiving.invalid);
    drop(map);
    drop(owner);
}
