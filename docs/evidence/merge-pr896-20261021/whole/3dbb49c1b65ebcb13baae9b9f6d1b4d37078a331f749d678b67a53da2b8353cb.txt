use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

fn function(keys: &[(&str, StatusLane)]) -> ResolvedFunction {
    let mut program = crate::hir::resolve(&crate::check(
        "module prefix.status; @id(\"status.f\") fn f(x:i64)->i64{x+1} @id(\"status.main\") fn main()->i64{0}",
        "status-prefix.spx").unwrap()).unwrap();
    let index = program
        .functions
        .iter()
        .position(|f| f.id.as_str() == "status.f")
        .unwrap();
    let mut function = program.functions.swap_remove(index);
    function.cleanup_plan.status_sources = keys
        .iter()
        .map(|(key, lane)| StatusSource {
            id: StatusSourceId {
                expression: ExpressionId::from_owned((*key).to_owned()),
                lane: *lane,
            },
            producer: StatusProducer::ContractFalse {
                phase: crate::cleanup_plan::ContractPhase::Requires,
                ordinal: 0,
            },
        })
        .collect();
    function
}
#[test]
fn reused_search_matches_every_old_link_and_preserves_all_original_work() {
    let names = (0..512)
        .map(|i| format!("opaque.λ\0.{:04}", i * 197 % 512))
        .collect::<Vec<_>>();
    let mut keys = names
        .iter()
        .map(|n| (n.as_str(), StatusLane::OperationFailure))
        .collect::<Vec<_>>();
    keys.extend_from_within(..8);
    let function = function(&keys);
    let (_, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut old_budget = ReplayBudget::new();
        let mut new_budget = ReplayBudget::new();
        let mut old = Directory::new(&function, &mut old_budget, "status cursor parity").unwrap();
        let mut new = Directory::new(&function, &mut new_budget, "status cursor parity").unwrap();
        let mut before = old.held_prefix(&function, &mut old_budget).unwrap();
        let mut current = new.held_prefix(&function, &mut new_budget).unwrap();
        drop(before.index.as_mut().unwrap().cursor.take().unwrap());
        let address =
            &**current.index.as_ref().unwrap().cursor.as_ref().unwrap() as *const IndexSearch;
        for source in &function.cleanup_plan.status_sources {
            assert_eq!(
                before
                    .insert(&function, &source.id, &mut old_budget)
                    .unwrap(),
                current
                    .insert(&function, &source.id, &mut new_budget)
                    .unwrap()
            );
            assert_eq!(
                (before.directory.root, before.directory.first),
                (current.directory.root, current.directory.first)
            );
            assert!(before.directory.balanced() && current.directory.balanced());
            assert!(before.directory.keys().eq(current.directory.keys()));
            for (a, b) in before.directory.nodes.iter().zip(&current.directory.nodes) {
                assert!(std::ptr::eq(a.key, b.key));
                assert_eq!(
                    (a.left, a.right, a.parent, a.next, a.height),
                    (b.left, b.right, b.parent, b.next, b.height)
                );
            }
            assert_eq!(
                address,
                &**current.index.as_ref().unwrap().cursor.as_ref().unwrap() as *const IndexSearch
            );
        }
        assert_eq!(
            old_budget.remaining - new_budget.remaining,
            3 * (keys.len() - 1)
        );
    });
    assert!(!refused && first.is_none());
}

#[test]
fn reused_search_removes_owning_carriers_and_releases_all_backing() {
    let names = (0..512)
        .map(|i| format!("a.{}.z", i * 197 % 512))
        .collect::<Vec<_>>();
    let keys = names
        .iter()
        .map(|n| (n.as_str(), StatusLane::ContractFalse))
        .collect::<Vec<_>>();
    let function = function(&keys);
    let run = |reuse: bool| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut directory =
                    Directory::new(&function, &mut budget, "status cursor accounting").unwrap();
                let mut held = directory.held_prefix(&function, &mut budget).unwrap();
                if !reuse {
                    drop(held.index.as_mut().unwrap().cursor.take().unwrap());
                }
                for source in &function.cleanup_plan.status_sources {
                    assert!(held.insert(&function, &source.id, &mut budget).unwrap());
                }
                drop(held);
                assert_eq!(directory.len(), 512);
                assert!(directory.balanced());
            })
        })
    };
    let (((), old), old_refused, old_paid, old_first) = run(false);
    let (((), new), new_refused, new_paid, new_first) = run(true);
    assert!(!old_refused && !new_refused && old_first.is_none() && new_first.is_none());
    assert!(old_paid - new_paid > 512 * (INDEX_SEARCH_BYTES + COMPARISON_BYTES) / 2);
    for (allocation, paid) in [(old, old_paid), (new, new_paid)] {
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        assert!(allocation.allocated <= paid && allocation.highwater <= paid);
    }
    eprintln!(
        "status-cursor complete512 old={old_paid} new={new_paid} saved={}",
        old_paid - new_paid
    );
}

#[test]
fn reused_search_backing_is_paid_before_allocation_at_exact_short_zero_and_floor() {
    let function = function(&[("x", StatusLane::OwnerAdmission)]);
    let run = |available, floor| {
        with_limit_usage_refusal(available + floor, || {
            assert!(crate::bounded_output::set_active_floor(floor));
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = allocate(&function, &mut budget, "status cursor backing");
                let ok = result.is_ok();
                drop(result);
                if !ok {
                    assert!(budget
                        .reserve_bytes(&function, 0, "sticky cursor backing")
                        .is_err());
                }
                ok
            })
        })
    };
    let ((ok, _), refused, exact, first) = run(usize::MAX, 0);
    assert!(ok && !refused && first.is_none());
    for floor in [0, 37] {
        for available in [0, exact - 1, exact] {
            let ((ok, metrics), refused, spent, first) = run(available, floor);
            assert_eq!(ok, available == exact);
            assert_eq!(refused, !ok);
            assert_eq!(first.is_some(), !ok);
            if let Some(first) = first {
                assert_eq!(first.floor, floor);
            }
            assert!(spent <= available && metrics.allocated <= spent);
            assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
            if available == 0 {
                assert_eq!(metrics.allocated, 0);
            }
        }
    }
}
