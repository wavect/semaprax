use super::*;
use crate::test_allocation_observer::measure;

fn program(source: &str) -> ResolvedProgram {
    let parsed = crate::check(source, "ordinary-receiving.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    let parsed = crate::check(&canonical, "ordinary-receiving.spx").unwrap();
    assert_eq!(crate::format::canonical(&parsed), canonical);
    crate::hir::resolve(&parsed).unwrap()
}

#[test]
fn lexical_work_producer_keeps_complete_old_first_error_and_sticky_payload() {
    let program = program("module receiving.work; @id(\"main\") fn main()->i64 {0}");
    let function = &program.functions[0];
    for lent in [false, true] {
        for limit in 0..=17 {
            for units in 0..=17 {
                let mut old = ReplayBudget::unobserved();
                old.remaining = limit;
                old.skeleton_remaining = limit;
                old.lent_skeleton = lent;
                let expected = old.charge(function, units, "receiving work control");
                let mut current = ReplayBudget::unobserved();
                current.remaining = limit;
                current.skeleton_remaining = limit;
                current.lent_skeleton = lent;
                let mut slot = Failure {
                    error: None,
                    function,
                    budget: &current,
                };
                let budget = &mut current;
                let accepted =
                    charge_into!(budget, function, slot, units, "receiving work control");
                assert_eq!(accepted, expected.is_ok());
                assert_eq!(slot.error.as_ref(), expected.as_ref().err()); // all six fields
                assert_eq!(budget.remaining, old.remaining);
                assert_eq!(budget.skeleton_remaining, old.skeleton_remaining);
                assert_eq!(budget.skeleton_spent, old.skeleton_spent);
                if !accepted {
                    let first = slot.error.as_ref().unwrap().message.as_ptr();
                    let before = (budget.remaining, budget.skeleton_remaining);
                    assert!(!reserve_into!(budget, function, slot, 0, "sticky retry"));
                    assert_eq!(slot.error.as_ref().unwrap().message.as_ptr(), first);
                    assert_eq!((budget.remaining, budget.skeleton_remaining), before);
                }
            }
        }
    }
}

#[test]
fn complete_direct_collectors_match_full_immediate_constructor_and_drop_at_all_cuts() {
    for source in [
        "module receiving.scalar; @id(\"main\") fn main()->i64 {0}",
        include_str!("../../../../tests/owned_data/byte_helper_renewal.spx"),
        r#"module receiving.lexical;
@id("probe") fn probe(flag:bool)->i64 {
 let key=str"x";let view=str_as_bytes(key);let mut next=0;
 while next<3 {next=next+1;if flag {continue;};if next==2 {return if byte_len(view)==1usize {7}else{0};};0}
 9
}
@id("main") fn main()->i64 {probe(false)}
"#,
        r#"module receiving.string;
@id("main") fn main()->i64 {
 let mut text=string_from_i64(1); let mut count=0;
 while count<2 {text=string_concat(text,"x"); count=count+1; 0}
 string_len(text)
}"#,
    ] {
        let program = program(source);
        let held = crate::loop_calls::byte_writer::ModuleIndex::new(&program).unwrap();
        for function in &program.functions {
            for index in [None, Some(&held)] {
                let mut old_budget = ReplayBudget::unobserved();
                // Full immediate funded constructor, not pre-funding predecessor.
                let old =
                    SkeletonWork::with_index(&program, function, &mut old_budget, index).unwrap();
                let expected_requires = crate::hir::lexical_exit::requires(function).unwrap();
                if function.id.as_str() == "probe" {
                    assert!(
                        expected_requires,
                        "genuine admitted LexicalExit reaches owner_into"
                    );
                    assert!(matches!(
                        &old.owner,
                        Some(crate::hir::FunctionExecutionId::Monomorphic(_))
                    ));
                }
                let mut expression_ids = Vec::new();
                crate::hir::function_value::walk(function, |expression| {
                    expression_ids.push(expression.id.clone())
                });
                let observe = |limit| {
                    crate::bounded_output::with_limit_usage_refusal(limit, || {
                        measure(|| {
                            let mut budget =
                                ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                            let result =
                                (|| {
                                    budget.prepare_builder(function)?;
                                    budget.prepare_clone_cursor(function)?;
                                    budget.reserve_bytes(
                                        function,
                                        skeleton_work::receiving::acquisition_bytes(),
                                        "ordinary skeleton receiving owner",
                                    )?;
                                    let mut slot = Failure {
                                        error: None,
                                        function,
                                        budget: &budget,
                                    };
                                    let mut prepared = skeleton_work::receiving::Prepared {
            owner: None, requires: false, reads: BTreeSet::new(), moves: BTreeSet::new(),
            renewals: crate::cleanup_plan::renewal_profile::RenewalBindings::empty(),
        };
                                    if !skeleton_work::receiving::prepare_into(
                                        &program,
                                        function,
                                        &mut budget,
                                        index.is_some(),
                                        &mut prepared,
                                        &mut slot,
                                    ) {
                                        return Err(slot.error.take().unwrap());
                                    }
                                    assert_eq!(prepared.requires, expected_requires);
                                    if expected_requires {
                                        assert!(
                                            prepared.owner.is_some(),
                                            "owner_into writes the actual execution owner"
                                        );
                                    }
                                    let current = SkeletonWork {
                                        function,
                                        budget: &mut budget,
                                        continuations: None,
                                        active_copy_region: None,
                                        dag_scratch: None,
                                        ordinary_scratch: None,
                                        next_ordinary_dag: 0,
                                        held_renewals: None,
                                        ordinary_index: index,
                                        ordinary_program: Some(&program),
                                        renewal_bindings: prepared.renewals,
                                        string_owner_moves: prepared.moves,
                                        string_condition_reads: prepared.reads,
                                        owner: prepared.owner,
                                    };
                                    assert_eq!(current.string_owner_moves, old.string_owner_moves);
                                    assert_eq!(
                                        current.string_condition_reads,
                                        old.string_condition_reads
                                    );
                                    assert_eq!(current.owner, old.owner);
                                    assert_eq!(
                                        current.renewal_bindings.appends(),
                                        old.renewal_bindings.appends()
                                    );
                                    assert_eq!(
                                        current.renewal_bindings.has_helpers(),
                                        old.renewal_bindings.has_helpers()
                                    );
                                    for id in &expression_ids {
                                        assert_eq!(
                                            current.renewal_bindings.get(id),
                                            old.renewal_bindings.get(id)
                                        );
                                        if let (Some(current), Some(old)) = (
                                            current.renewal_bindings.get(id),
                                            old.renewal_bindings.get(id),
                                        ) {
                                            assert!(std::ptr::eq(current, old));
                                        }
                                        assert_eq!(
                                            current.renewal_bindings.is_helper(id),
                                            old.renewal_bindings.is_helper(id)
                                        );
                                    }
                                    assert_eq!(current.budget.remaining, old.budget.remaining);
                                    assert_eq!(
                                        current.budget.skeleton_remaining,
                                        old.budget.skeleton_remaining
                                    );
                                    drop(current);
                                    Ok::<_, Diagnostic>(())
                                })();
                            let ok = result.is_ok();
                            drop(result);
                            drop(budget);
                            ok
                        })
                    })
                };
                let ((ok, heap), refused, exact, first) = observe(usize::MAX);
                assert!(ok && !refused && first.is_none());
                assert_eq!(heap.live, 0);
                assert_eq!(heap.allocated, heap.freed);
                assert!(!heap.invalid);
                for limit in [0, exact - 1, exact] {
                    let ((ok, heap), refused, paid, first) = observe(limit);
                    assert_eq!(ok, limit == exact);
                    assert_eq!(refused, !ok);
                    assert_eq!(first.is_some(), !ok);
                    assert!(paid <= limit);
                    assert_eq!(heap.live, 0);
                    assert_eq!(heap.allocated, heap.freed);
                    assert!(!heap.invalid);
                    assert!(heap.allocated <= paid);
                    if limit == 0 {
                        assert_eq!(heap.allocated, 0);
                    }
                }
            }
        }
    }

    // Typed-HIR adversarial owner contexts: no frontend admission is asserted.
    let lexical = program(
        r#"module receiving.lexical;
@id("probe") fn probe(flag:bool)->i64 {
 let key=str"x";let view=str_as_bytes(key);let mut next=0;
 while next<3 {next=next+1;if flag {continue;};if next==2 {return if byte_len(view)==1usize {7}else{0};};0}
 9
}
@id("main") fn main()->i64 {probe(false)}
"#,
    );
    let function = lexical
        .functions
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap()
        .clone();
    assert!(crate::hir::lexical_exit::requires(&function).unwrap());
    for ambiguous in [false, true] {
        let mut context = lexical.clone();
        if ambiguous {
            context.functions.push(function.clone());
        } else {
            context.functions.retain(|f| f.id != function.id);
        }
        let mut old_requests = Vec::new();
        let old = crate::hir::lexical_exit::owner_funded(&context, &function, &mut |bytes| {
            old_requests.push(bytes);
            Ok::<_, ()>(())
        })
        .unwrap();
        let verdict = old.as_ref().unwrap_err();
        let expected = verdict.diagnostic();
        assert_eq!(
            crate::hir::lexical_exit::owner(&context, &function).unwrap_err(),
            expected
        );
        let mut requests = Vec::new();
        let mut output = None;
        crate::hir::lexical_exit::owner_into(&context, &function, &mut output, &mut |bytes| {
            requests.push(bytes);
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(output, old.ok());
        assert_eq!(
            requests, old_requests,
            "ordered zero/payload guard frontiers"
        );
        for cut in 0..old_requests.len() {
            let mut old_at = 0;
            let old = crate::hir::lexical_exit::owner_funded(&context, &function, &mut |_| {
                let at = old_at;
                old_at += 1;
                if at == cut {
                    Err(())
                } else {
                    Ok(())
                }
            });
            let mut current_at = 0;
            let mut output = None;
            let current =
                crate::hir::lexical_exit::owner_into(&context, &function, &mut output, &mut |_| {
                    let at = current_at;
                    current_at += 1;
                    if at == cut {
                        Err(())
                    } else {
                        Ok(())
                    }
                });
            assert!(matches!(
                old,
                Err(crate::hir::funded::FundedError::Refused(()))
            ));
            assert!(matches!(
                current,
                Err(crate::hir::funded::FundedError::Refused(()))
            ));
            assert_eq!(current_at, old_at);
        }
    }
}

#[test]
fn different_budget_and_source_cannot_rebind_a_receiving_slot() {
    let program = program(
        "module receiving.foreign; @id(\"a\") fn a()->i64 {0} @id(\"main\") fn main()->i64 {0}",
    );
    let function = &program.functions[0];
    let original = ReplayBudget::unobserved();
    let mut slot = Failure {
        error: None,
        function,
        budget: &original,
    };
    let mut other = ReplayBudget::unobserved();
    let budget = &mut other;
    assert!(!reserve_into!(budget, function, slot, 0, "foreign"));
    let expected = replay_error(
        function,
        "ordinary replay receiving source or payer is outside its current invocation",
    );
    assert_eq!(slot.error.as_ref(), Some(&expected));
    let first = slot.error.as_ref().unwrap().message.as_ptr();
    let function = &program.functions[1];
    assert!(!charge_into!(budget, function, slot, 0, "foreign source"));
    assert_eq!(slot.error.as_ref().unwrap().message.as_ptr(), first);

    // A fresh same-payer slot must reach the source check, not the sticky guard.
    let owner = &program.functions[0];
    let foreign = &program.functions[1];
    let mut current = ReplayBudget::with_physical(owner, 64 * 1024 * 1024).unwrap();
    let original_charged = current.physical.as_ref().unwrap().charged();
    let text_pointer = current.refusal_text.as_ref().unwrap().as_ptr();
    let text_capacity = current.refusal_text.as_ref().unwrap().capacity();
    let mut fresh = Failure {
        error: None,
        function: owner,
        budget: &current,
    };
    let expected = replay_error(
        foreign,
        "ordinary replay receiving source or payer is outside its current invocation",
    );
    let (accepted, heap) = measure(|| {
        let budget = &mut current;
        let function = foreign;
        charge_into!(budget, function, fresh, 0, "fresh foreign source")
    });
    assert!(!accepted);
    assert_eq!(fresh.error.as_ref(), Some(&expected)); // complete six fields
    assert_eq!(fresh.error.as_ref().unwrap().message.as_ptr(), text_pointer);
    assert!(fresh.error.as_ref().unwrap().message.len() <= text_capacity);
    assert_eq!(
        current.physical.as_ref().unwrap().charged(),
        original_charged
    );
    assert_eq!((current.remaining, current.skeleton_remaining), (0, 0));
    assert!(current.refusal_spent);
    assert!(current.physical.as_deref_mut().unwrap().reserve(0).is_err());
    assert!(current.refusal_text.is_none());
    assert!(!heap.invalid && heap.allocated == 0 && heap.live == 0);
}
