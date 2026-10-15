//! Warm held-slot controls; complete immediate old receiver stays independent.
use super::*;
use crate::bounded_output::{active_remaining, reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};
fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}

#[test]
fn held_warm_authentication_removes_only_real_carriers_at_zero_exact_and_short_frontiers() {
    let function = fixture();
    let fee = std::mem::size_of::<Result<(), Diagnostic>>();
    for available in [0, fee - 1, fee] {
        let (((), allocation), refused, _, first) = with_limit_usage_refusal(1 << 20, || measure(|| {
            let mut current = memo::RetainedMemo::<usize>::new();
            let mut budget = ReplayBudget::new();
            current.authenticate_retained(&function, &mut budget);
            assert!(!current.failed());
            let left = active_remaining().unwrap();
            assert!(reserve_active_required(left - available));
            let at = current_metrics().allocated;
            for _ in 0..8 {
                current.authenticate_retained(&function, &mut budget);
                assert!(!current.failed());
                assert_eq!(active_remaining(), Some(available));
                assert_eq!(current_metrics().allocated, at);
            }
            drop(current); drop(budget);
        }));
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed);
    }
}

#[test]
fn held_complete_immediate_receiver_keeps_ids_signatures_overflow_and_every_work_frontier() {
    let function = fixture();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    let mut probe = ReplayBudget::unobserved();
    let before = probe.remaining;
    before_held::Canonicalizer::new(&automaton, sink)
        .id(&function, vec![(roots[3], 1)], &mut probe).unwrap();
    for units in 0..=before - probe.remaining {
        for input in [vec![(roots[3], 1)], vec![(roots[3], u64::MAX)], vec![(roots[6], 1)]] {
            let mut old = before_held::Canonicalizer::new(&automaton, sink);
            let mut current = Canonicalizer::new(&automaton, sink);
            let mut a = ReplayBudget::unobserved(); let mut b = ReplayBudget::unobserved();
            a.remaining = units; b.remaining = units;
            assert_eq!(old.id(&function, input.clone(), &mut a), current.id(&function, input, &mut b));
            assert_eq!((a.remaining,a.skeleton_remaining),(b.remaining,b.skeleton_remaining));
            assert_eq!(old.signatures(),&current.ids); super::inline_weighted::assert_states(&current.states, old.states());
        }
    }
}

#[test]
fn held_immediate_old_and_current_cold_cuts_and_whole_drop_remain_separate() {
    let function = fixture();
    let observe = |old: bool, limit| with_limit_usage_refusal(limit, || {
        let mut budget = ReplayBudget::new();
        if old {
            let mut memo = before_held::memo::RetainedMemo::<usize>::new();
            memo.authenticate_retained(&function, &mut budget);
            let failed = memo.failed();
            let first = if failed { Some(memo.failure()) } else { None };
            let at = current_metrics().allocated;
            if failed { memo.authenticate_retained(&function,&mut budget); assert!(memo.failed()); assert_eq!(current_metrics().allocated,at); }
            drop(memo); drop(budget); first
        } else {
            let mut memo = memo::RetainedMemo::<usize>::new();
            memo.authenticate_retained(&function, &mut budget);
            let failed = memo.failed();
            let first = if failed { Some(memo.failure()) } else { None };
            let at = current_metrics().allocated;
            if failed { memo.authenticate_retained(&function,&mut budget); assert!(memo.failed()); assert_eq!(current_metrics().allocated,at); }
            drop(memo); drop(budget); first
        }
    });
    let (upper, _) = measure(|| observe(true, 1 << 20).2);
    for limit in 0..=upper {
        let ((),allocation)=measure(|| {
            let old=observe(true,limit); let new=observe(false,limit);
            assert_eq!(old.0,new.0);assert_eq!((old.1,old.2,old.3),(new.1,new.2,new.3));
            drop(old);drop(new);
        });
        assert!(!allocation.invalid && allocation.live==0 && allocation.allocated==allocation.freed);
    }
}

#[test]
fn held_foreign_function_budget_nested_and_restored_owner_refuse_without_foreign_debit() {
    let function = fixture();
    let foreign_function = fixture();
    for kind in 0..3 {
        let (((), allocation), _, _, _) = with_limit_usage_refusal(1 << 20, || measure(|| {
            let mut current = memo::RetainedMemo::<usize>::new();
            let mut budget = ReplayBudget::new();
            current.authenticate_retained(&function,&mut budget);
            assert!(!current.failed());
            if kind == 2 {
                let ((), refused, used, first) = with_limit_usage_refusal(0, || {
                    let at = current_metrics().allocated;
                    current.authenticate_retained(&function,&mut budget);
                    assert!(current.failed()); assert_eq!(current_metrics().allocated,at);
                });
                assert!(!refused && used==0 && first.is_none());
            } else {
                let mut foreign = ReplayBudget::new();
                let left=active_remaining();let at=current_metrics().allocated;
                if kind==0 { current.authenticate_retained(&foreign_function,&mut budget); }
                else { current.authenticate_retained(&function,&mut foreign); }
                assert_eq!(active_remaining(),left);assert_eq!(current_metrics().allocated,at);
            }
            assert!(current.failed());
            assert_eq!(current.failure(),Diagnostic::io("SPX-H006",String::new()));
            let at=current_metrics().allocated;let left=active_remaining();
            current.authenticate_retained(&function,&mut budget);
            assert!(current.failed());assert!(current.get(&0).is_none());
            assert_eq!(current.failure(),Diagnostic::io("SPX-H006",String::new()));
            assert_eq!(active_remaining(),left);assert_eq!(current_metrics().allocated,at);
            drop(current);drop(budget);
        }));
        assert!(!allocation.invalid && allocation.live==0 && allocation.allocated==allocation.freed);
    }
}

#[test]
fn held_cached_closure_keeps_original_one_work_unit_and_same_result_without_auth_debit() {
    let function=fixture();let (automaton,sink,roots)=super::tests::automaton(&function);
    let (((),allocation),refused,_,first)=with_limit_usage_refusal(1<<20,||measure(||{
        let mut c=Canonicalizer::new(&automaton,sink);let mut budget=ReplayBudget::new();
        let held=c.closure(&function,roots[3],&mut budget).unwrap().unwrap();
        let work=budget.remaining;let paid=active_remaining();let at=current_metrics().allocated;
        for offset in 1..=8 {
            let again=c.closure(&function,roots[3],&mut budget).unwrap().unwrap();
            assert!(Rc::ptr_eq(&held,&again)); assert_eq!(budget.remaining,work-offset);
            assert_eq!(active_remaining(),paid);assert_eq!(current_metrics().allocated,at);
        }
        drop(held);drop(c);drop(budget);
    }));
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live==0 && allocation.allocated==allocation.freed);
}

#[test]
fn held_foreign_oracle_never_queries_foreign_or_restored_payer() {
    use crate::cleanup_plan::replay::allocation::OracleScope;
    let function=fixture();let foreign_function=fixture();
    let scope=OracleScope::open_with_work(1<<20,1<<20).unwrap();let lease=scope.lease();
    for foreign_budget in [false,true] {
        let mut current=memo::RetainedMemo::<usize>::new();
        let mut budget=ReplayBudget::with_skeleton_limit_oracle(0,lease);
        current.authenticate_retained(&function,&mut budget);assert!(!current.failed());
        let mut other=ReplayBudget::with_skeleton_limit_oracle(0,lease);
        let before=scope.receipt();
        if foreign_budget { current.authenticate_retained(&function,&mut other); }
        else { current.authenticate_retained(&foreign_function,&mut budget); }
        assert_eq!(scope.receipt(),before);assert!(current.failed());
        assert_eq!(current.failure(),Diagnostic::io("SPX-H006",String::new()));
        current.authenticate_retained(&function,&mut budget);
        assert_eq!(scope.receipt(),before);assert!(current.failed());
    }
}

#[test]
fn held_warm_floor_equality_and_first_required_refusal_remain_sticky_after_floor_clear() {
    use crate::bounded_output::{set_active_floor,clear_active_floor};
    let function=fixture();
    let (((),allocation),refused,_,first)=with_limit_usage_refusal(1<<20,||measure(||{
        let mut memo=memo::RetainedMemo::<usize>::new();let mut budget=ReplayBudget::new();
        memo.authenticate_retained(&function,&mut budget);assert!(!memo.failed());
        let left=active_remaining().unwrap();assert!(set_active_floor(left));
        let at=current_metrics().allocated;
        memo.authenticate_retained(&function,&mut budget);assert!(!memo.failed());
        assert_eq!(active_remaining(),Some(left));assert_eq!(current_metrics().allocated,at);
        assert!(!reserve_active_required(1));
        memo.authenticate_retained(&function,&mut budget);assert!(memo.failed());
        let error=memo.failure();assert_eq!(error.code,"SPX-H006");
        assert!(error.message.contains("ordinary builder already failed"));
        clear_active_floor();
        memo.authenticate_retained(&function,&mut budget);assert!(memo.failed());
        assert_eq!(memo.failure(),Diagnostic::io("SPX-H006",String::new()));
        assert_eq!(current_metrics().allocated,at);
        drop(error);drop(memo);drop(budget);
    }));
    assert!(refused);let first=first.unwrap();assert_eq!(first.requested,1);assert!(first.floor>0);
    assert!(!allocation.invalid && allocation.live==0 && allocation.allocated==allocation.freed);
}

#[test]
fn held_oracle_old_current_actual_warm_requests_are_not_padded_and_whole_owners_drop() {
    use crate::cleanup_plan::replay::allocation::OracleScope;
    let function=fixture();let fee=std::mem::size_of::<Result<(),Diagnostic>>();
    for old in [false,true] {
        let scope=OracleScope::open_with_work(1<<20,1<<20).unwrap();let lease=scope.lease();
        let ((),allocation)=measure(|| {
            let mut budget=ReplayBudget::with_skeleton_limit_oracle(0,lease);
            if old {
                let mut memo=before_held::memo::RetainedMemo::<usize>::new();
                memo.authenticate_retained(&function,&mut budget);assert!(!memo.failed());
                let before=scope.receipt();let at=current_metrics().allocated;
                for _ in 0..8 { memo.authenticate_retained(&function,&mut budget);assert!(!memo.failed()); }
                let after=scope.receipt();assert_eq!(after.charged-before.charged,8*fee);
                assert_eq!(after.calls-before.calls,16);assert_eq!(after.work,before.work);
                assert_eq!(current_metrics().allocated,at);drop(memo);
            } else {
                let mut memo=memo::RetainedMemo::<usize>::new();
                memo.authenticate_retained(&function,&mut budget);assert!(!memo.failed());
                let before=scope.receipt();let at=current_metrics().allocated;
                for _ in 0..8 { memo.authenticate_retained(&function,&mut budget);assert!(!memo.failed()); }
                let after=scope.receipt();assert_eq!(after.charged,before.charged);
                assert_eq!(after.calls-before.calls,8);assert_eq!(after.work,before.work);
                assert_eq!(current_metrics().allocated,at);drop(memo);
            }
            drop(budget);
        });
        assert!(!allocation.invalid && allocation.live==0 && allocation.allocated==allocation.freed);
    }
}
