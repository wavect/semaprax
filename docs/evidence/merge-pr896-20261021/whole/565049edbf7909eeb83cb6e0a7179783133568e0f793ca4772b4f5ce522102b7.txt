//! Current slot receiver mirrors the original query corpus; old tests stay intact.
use super::tests::{acquire, arena, program};
use super::*;
use crate::cleanup_plan::replay::ordinary_receiving::{
    prepare_into, refuse_into, reserve_into, Failure,
};
use crate::test_allocation_observer::measure;

fn prepay_query(
    session: &mut Session<'_, '_, '_>,
    function: &ResolvedFunction,
    slot: &mut Failure<'_>,
) -> bool {
    reserve_into!(
        session.budget,
        function,
        slot,
        Session::into_query_bytes(),
        "ordinary fact query actual and formal receiving"
    )
}

#[test]
fn current_slot_zero_one_many_utf8_nul_hit_miss_noncall_and_exact_old_work() {
    let program = program();
    let function = &program.functions[0];
    let mut keys = ["", "\0", "a", "a\0", "λ", "識別", "🙂"]
        .into_iter()
        .map(|text| ExpressionId::from_owned(text.into()))
        .collect::<Vec<_>>();
    keys.extend((0..35).map(|n| ExpressionId::from_owned(format!("prefix.λ\0.{n:03}"))));
    for count in [0, 1, 15, 16, keys.len()] {
        for reverse in [false, true] {
            let mut order = keys[..count].to_vec();
            if reverse {
                order.reverse();
            }
            let arena = arena(function, &order);
            let queries = keys
                .iter()
                .rev()
                .map(|id| ExpressionId::from_owned(id.as_str().to_owned()))
                .collect::<Vec<_>>();
            let mut budget = ReplayBudget::unobserved();
            let mut slot = Failure {
                error: None,
                function,
                budget: &budget,
            };
            let mut session = acquire(&arena, function, &mut budget).unwrap();
            let mut output = None;
            for query in &queries {
                let mut old_budget = ReplayBudget::unobserved();
                let old = prefix::retained_before::find_receiving(
                    &arena,
                    function,
                    query,
                    &mut old_budget,
                )
                .unwrap();
                let before = session.budget.remaining;
                assert!(prepay_query(&mut session, function, &mut slot));
                assert!(session.fact_into(query, &mut output, &mut slot));
                assert_eq!(output.is_some(), old.is_some());
                if let (Some(current), Some(old)) = (output, old) {
                    assert!(std::ptr::eq(current, old));
                }
                assert!(slot.error.is_none());
                assert_eq!(
                    before - session.budget.remaining,
                    MAX_REPLAY_WORK_UNITS - old_budget.remaining
                );
            }
        }
    }
}

#[test]
fn every_current_query_work_cut_matches_all_six_old_fields_and_sticky_slot() {
    let program = program();
    let function = &program.functions[0];
    let keys = (0..33)
        .map(|n| ExpressionId::from_owned(format!("prefix.{n:03}")))
        .collect::<Vec<_>>();
    let arena = arena(function, &keys);
    for query in [&keys[0], &keys[17], &keys[32]] {
        let mut complete = ReplayBudget::unobserved();
        prefix::retained_before::find_receiving(&arena, function, query, &mut complete).unwrap();
        for work in 0..=MAX_REPLAY_WORK_UNITS - complete.remaining {
            let mut old_budget = ReplayBudget::unobserved();
            old_budget.remaining = work;
            let old =
                prefix::retained_before::find_receiving(&arena, function, query, &mut old_budget);
            let mut budget = ReplayBudget::unobserved();
            let mut slot = Failure {
                error: None,
                function,
                budget: &budget,
            };
            let mut session = acquire(&arena, function, &mut budget).unwrap();
            session.budget.remaining = work;
            let mut output = None;
            assert!(prepay_query(&mut session, function, &mut slot));
            let accepted = session.fact_into(query, &mut output, &mut slot);
            assert_eq!(accepted, old.is_ok());
            assert_eq!(slot.error.as_ref(), old.as_ref().err());
            assert_eq!(session.budget.remaining, old_budget.remaining);
            if !accepted {
                let first = slot.error.as_ref().unwrap().message.as_ptr();
                let remaining = session.budget.remaining;
                assert!(!session.fact_into(query, &mut output, &mut slot));
                assert_eq!(session.budget.remaining, remaining);
                assert_eq!(slot.error.as_ref().unwrap().message.as_ptr(), first);
            }
        }
    }
}

#[test]
fn current_query_exact_short_zero_floor_and_unwind_release_all_storage() {
    let program = program();
    let function = &program.functions[0];
    let keys = [function.body.id.clone()];
    let arena = arena(function, &keys);
    let observe = |bytes, count, floor| {
        crate::bounded_output::with_limit_usage_refusal(bytes + floor, || {
            assert!(crate::bounded_output::set_active_floor(floor));
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = (|| {
                    budget.prepare_builder(function)?;
                    budget.reserve_bytes(
                        function,
                        Session::OLD_SCRATCH
                            + Session::acquisition_bytes()
                            + Failure::acquisition_bytes(),
                        "retained expression-fact owner acquisition",
                    )?;
                    budget.charge(
                        function,
                        12,
                        "retained expression-fact owner initialization",
                    )?;
                    let mut slot = Failure {
                        error: None,
                        function,
                        budget: &budget,
                    };
                    let mut session = Session::new_prepaid(&arena, function, &mut budget);
                    let mut output = None;
                    for _ in 0..count {
                        if !prepay_query(&mut session, function, &mut slot)
                            || !session.fact_into(&function.body.id, &mut output, &mut slot)
                        {
                            let first = slot.error.as_ref().unwrap().message.as_ptr();
                            let remaining = crate::bounded_output::active_remaining();
                            assert!(!session.fact_into(&function.body.id, &mut output, &mut slot));
                            assert_eq!(slot.error.as_ref().unwrap().message.as_ptr(), first);
                            assert_eq!(crate::bounded_output::active_remaining(), remaining);
                            return Err(slot.error.take().unwrap());
                        }
                    }
                    Ok::<_, Diagnostic>(())
                })();
                let accepted = result.is_ok();
                drop(result);
                drop(budget);
                accepted
            })
        })
    };
    for count in [0, 1, 7] {
        let ((ok, heap), refused, exact, _) = observe(usize::MAX, count, 0);
        assert!(ok && !refused && !heap.invalid && heap.live == 0);
        assert_eq!(heap.allocated, heap.freed);
        for floor in [0, 37] {
            for bytes in [0, exact - 1, exact] {
                let ((ok, heap), refused, paid, _) = observe(bytes, count, floor);
                assert_eq!(ok, bytes == exact);
                assert_eq!(refused, !ok);
                assert!(paid <= bytes && !heap.invalid && heap.live == 0);
                assert_eq!(heap.allocated, heap.freed);
                if bytes == 0 {
                    assert_eq!(heap.allocated, 0);
                }
            }
        }
    }
    let mut budget = ReplayBudget::unobserved();
    let (_, heap) = measure(|| {
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut slot = Failure {
                error: None,
                function,
                budget: &budget,
            };
            let mut session = acquire(&arena, function, &mut budget).unwrap();
            let mut output = None;
            assert!(prepay_query(&mut session, function, &mut slot));
            assert!(session.fact_into(&function.body.id, &mut output, &mut slot));
            std::panic::resume_unwind(Box::new(()));
        }));
        assert!(unwound.is_err());
        drop(unwound);
        budget.reserve_bytes(function, 0, "after unwind").unwrap();
    });
    assert!(!heap.invalid && heap.live == 0);
    assert_eq!(heap.allocated, heap.freed);
}

#[test]
fn genuine_arena_producer_keeps_call_and_noncall_payload_addresses() {
    let program = crate::hir::resolve(&crate::check(
        "module receiving.calls; @id(\"add\") fn add(x:i64)->i64 {x+1} @id(\"main\") fn main()->i64 {add(0)}",
        "receiving-calls.spx").unwrap()).unwrap();
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    let mut physical = physical_storage::PhysicalReservation::new(usize::MAX);
    let mut build_budget = ReplayBudget::unobserved();
    let arena = super::super::build::<errors::Legacy>(
        function,
        capacity::Payer::Selected(&mut physical),
        &mut build_budget,
    )
    .unwrap();
    let mut queries = Vec::new();
    crate::hir::function_value::walk(function, |expression| queries.push(expression.id.clone()));
    let mut budget = ReplayBudget::unobserved();
    let mut slot = Failure {
        error: None,
        function,
        budget: &budget,
    };
    let mut session = acquire(&arena, function, &mut budget).unwrap();
    let mut output = None;
    let mut call = false;
    let mut noncall = false;
    for query in &queries {
        let mut old_budget = ReplayBudget::unobserved();
        let old = prefix::retained_before::find_receiving(&arena, function, query, &mut old_budget)
            .unwrap();
        assert!(prepay_query(&mut session, function, &mut slot));
        assert!(session.fact_into(query, &mut output, &mut slot));
        let old = old.unwrap();
        let current = output.unwrap();
        assert!(std::ptr::eq(current, old));
        if current.is_some() {
            call = true;
        } else {
            noncall = true;
        }
    }
    assert!(call && noncall);
}
