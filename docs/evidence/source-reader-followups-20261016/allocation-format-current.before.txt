//! The active builder pays storage; it never authenticates selected replay.
use super::*;
mod old;
mod shape_before;
mod shape_cursor_tests;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{current_metrics, measure};
fn program() -> ResolvedProgram {
    let source = crate::check(
        r#"module replay.payer;
@id("payer.main") fn main()->i64 {
 let data=bytes_zeroed(1usize); let text="x";
 if byte_len(bytes_as_slice(data))==1usize && str_len_bytes(string_as_str(text))==1 {7}else{0}
}"#,
        "ordinary-replay-payer.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
fn indexes(function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<(), Diagnostic> {
    let facts = expression_facts::with_budget(function, budget)?;
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    assert_eq!(facts.len(), expression_facts(function)?.len());
    assert_eq!(storage, old::validate_slots(function)?);
    let old = old::collect_leaves(function)?;
    assert_eq!(leaves.len(), old.len());
    for (flag, leaf) in leaves.iter() {
        assert_eq!(leaf.place, old[flag].place);
        assert_eq!(leaf.lifecycle, old[flag].lifecycle);
    }
    Ok(())
}
#[test]
fn ordinary_builder_payment_never_grants_selected_admission_and_covers_index_drop() {
    let program = program();
    let function = &program.functions[0];
    // Reference collections are constructed outside both the builder and the
    // observer. Their original arbitrary-shape semantics remain independent.
    let old_facts = expression_facts(function).unwrap();
    let old_storage = old::validate_slots(function).unwrap();
    let old_leaves = old::collect_leaves(function).unwrap();
    let (((), metrics), refused, charged, receipt) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            assert_eq!(budget.admission, AdmissionMode::Ordinary);
            assert_eq!(budget.payer, AllocationPayer::ActiveBuilder);
            assert!(budget.physical.is_none());
            let facts = expression_facts::with_budget(function, &mut budget).unwrap();
            let storage = validate_slots_paid(function, &mut budget).unwrap();
            let leaves = collect_leaves_paid(function, &mut budget).unwrap();
            assert_eq!(facts.len(), old_facts.len());
            assert_eq!(storage, old_storage);
            for (flag, leaf) in leaves.iter() {
                assert_eq!(leaf.place, old_leaves[flag].place);
                assert_eq!(leaf.lifecycle, old_leaves[flag].lifecycle);
            }
            assert!(!budget.selected());
            drop((facts, storage, leaves, budget));
        })
    });
    assert!(!refused && receipt.is_none() && !metrics.invalid);
    assert!(metrics.allocations > 0);
    assert!(metrics.allocated <= charged);
    assert!(metrics.highwater <= charged);
    assert_eq!(metrics.allocated, metrics.freed);
    assert_eq!(metrics.live, 0);
    // The actual ordinary replay entry keeps its complete authority checks.
    let mut legacy = ReplayBudget::unobserved();
    // Use the real caller's capped independent skeleton census. Directly
    // calling the member receiver starts with no authorized skeleton work.
    let upper =
        reserve_program_skeleton_work(&program, std::iter::once(function), &mut legacy).unwrap();
    assert!(upper > 0 && upper <= MAX_REPLAY_WORK_UNITS);
    validate_structure_with_budget(&program, function, &mut legacy, None).unwrap();
    let (checked, refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut active = ReplayBudget::new();
        let active_upper =
            reserve_program_skeleton_work(&program, std::iter::once(function), &mut active)
                .unwrap();
        assert_eq!(active_upper, upper);
        let result = validate_structure_with_budget(&program, function, &mut active, None);
        // The current Arena prepays search/rotation/receiving visits that the
        // independent Tree route does not charge. Exact original Tree parity
        // remains compiled in ordinary_before; skeleton parity stays exact.
        assert!(active.remaining < legacy.remaining);
        assert!(active.remaining > 0);
        assert_eq!(active.skeleton_spent, legacy.skeleton_spent);
        assert!(!active.selected() && active.physical.is_none());
        result
    });
    checked.unwrap();
    assert!(!refused);
    let mut hostile = function.clone();
    hostile.cleanup_plan.slots[0].id.0 += 1;
    let old = old::validate_slots(&hostile).err().unwrap();
    let (new, refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        validate_slots_paid(&hostile, &mut ReplayBudget::new())
            .err()
            .unwrap()
    });
    assert_eq!(new, old);
    assert!(!refused);
}
#[test]
fn active_builder_zero_exact_short_refusal_is_sticky_and_moves_paid_text() {
    let program = program();
    let function = &program.functions[0];
    // Independent literal: one prepaid String request, then a u64 array.
    let text_bytes = function.id.as_str().len() + 512;
    let full = text_bytes + 3 * std::mem::size_of::<u64>();
    for limit in [0, text_bytes - 1, full - 1, full] {
        let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                // Save the actual paid backing before its first failure moves
                // it into the diagnostic. This adds no second reservation.
                let paid_text = if limit >= text_bytes {
                    budget.prepare_builder(function).unwrap();
                    Some(budget.refusal_text.as_ref().unwrap().as_ptr())
                } else {
                    None
                };
                let result = budget.reserve_array::<u64>(function, 3, "ordinary literal array");
                if limit == full {
                    result.unwrap();
                    let values = vec![0u64; 3];
                    drop(values);
                } else {
                    let error = result
                        .err()
                        .unwrap_or_else(|| panic!("exactly short builder refuses"));
                    assert_eq!(error.code, "SPX-H006");
                    if limit >= text_bytes {
                        assert!(!error.message.is_empty());
                        assert_eq!(Some(error.message.as_ptr()), paid_text);
                    } else {
                        assert!(error.message.is_empty());
                    }
                    let before = current_metrics();
                    assert!(budget.reserve_bytes(function, 0, "sticky zero").is_err());
                    assert!(budget
                        .reserve_vec(function, &mut Vec::<u64>::new(), 0, "sticky spare")
                        .is_err());
                    assert_eq!(current_metrics(), before);
                    assert!(!budget.selected());
                    drop(error);
                }
                drop(budget);
            })
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(refused, limit != full);
        if limit == full {
            assert_eq!(used, full);
            assert!(receipt.is_none());
        } else {
            assert!(receipt.is_some());
            assert!(used <= limit);
        }
    }
    // Work can refuse before facts or slots allocate. Its first message must
    // retain the complete legacy wording on the one paid backing.
    for mode in 0..3 {
        let attempt = |budget: &mut ReplayBudget| {
            budget.remaining = 2;
            budget.skeleton_remaining = 2;
            match mode {
                0 => budget.charge(function, 3, "literal early work"),
                1 => budget.reserve_skeleton(function, 3),
                _ => budget.charge_skeleton(function, 3, "literal early skeleton"),
            }
        };
        let expected = attempt(&mut ReplayBudget::unobserved()).unwrap_err();
        let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(text_bytes, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let error = attempt(&mut budget).unwrap_err();
                assert_eq!(error, expected);
                assert!(!budget.selected());
                let before = current_metrics();
                assert!(budget.charge(function, 0, "retry").is_err());
                assert_eq!(current_metrics(), before);
                drop((error, budget));
            })
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(metrics.allocated, text_bytes);
        assert_eq!(used, text_bytes);
        // Local work refusal is sticky in ReplayBudget; it does not fabricate
        // a physical enclosing-ledger refusal while that ledger still fits.
        assert!(!refused && receipt.is_none());
    }
    // Independent literal backing inventory for the first type clone census:
    // one refusal String and one 514-entry fixed Option<(type-ref,ordinal)>.
    let cursor_bytes = 514 * std::mem::size_of::<Option<(&ResolvedType, usize)>>();
    for limit in [text_bytes + cursor_bytes - 1, text_bytes + cursor_bytes] {
        let (((), metrics), refused, used, receipt) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = budget.reserve_clone(function, &ResolvedType::I64, "literal type");
                if limit == text_bytes + cursor_bytes {
                    result.unwrap();
                    let before = current_metrics();
                    budget
                        .reserve_clone(function, &ResolvedType::I64, "reuse cursor")
                        .unwrap();
                    assert_eq!(current_metrics(), before);
                    assert!(budget.builder_clone_cursor_paid);
                } else {
                    let error = result.unwrap_err();
                    assert_eq!(error.code, "SPX-H006");
                    assert!(!budget.builder_clone_cursor_paid);
                    let before = current_metrics();
                    assert!(budget
                        .reserve_clone(function, &ResolvedType::I64, "retry")
                        .is_err());
                    assert_eq!(current_metrics(), before);
                    drop(error);
                }
                drop(budget);
            })
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(refused, limit < text_bytes + cursor_bytes);
        if refused {
            assert_eq!(used, text_bytes);
            assert!(receipt.is_some());
        } else {
            assert_eq!(used, text_bytes + cursor_bytes);
            assert!(receipt.is_none());
        }
    }
    // Complete ordinary verdict comparison remains outside allocation evidence.
    let mut unobserved = ReplayBudget::unobserved();
    indexes(function, &mut unobserved).unwrap();
}

mod accounting_scope;
mod ordinary_before;
