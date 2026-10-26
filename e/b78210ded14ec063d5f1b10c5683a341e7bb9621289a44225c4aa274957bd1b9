//! Whole old producer and caller-funded loans; no shipping selector opt-in.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use sha2::Digest;

fn budget(program: &ResolvedProgram, function: &ResolvedFunction) -> ReplayBudget {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget).unwrap();
    budget
}

fn visit(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    expected: &[SkeletonPath],
) -> Result<(), Diagnostic> {
    paid!(
        budget,
        function,
        PRODUCE_CALL,
        "PLAN view test produce caller"
    )?;
    let produced = produce(function, budget)?;
    paid!(budget, function, COUNT_CALL, "PLAN view test count caller")?;
    assert_eq!(path_count(&produced), expected.len());
    for (index, old) in expected.iter().enumerate() {
        paid!(
            budget,
            function,
            PREPARE_CALL,
            "PLAN view test prepare caller"
        )?;
        let mut cursor = prepare(&produced, index, budget)?;
        for observation in old.observations.iter() {
            paid!(budget, function, NEXT_CALL, "PLAN view test next caller")?;
            assert_eq!(next(&mut cursor, budget)?.unwrap(), observation.as_ref());
        }
        paid!(budget, function, NEXT_CALL, "PLAN view test end caller")?;
        assert!(next(&mut cursor, budget)?.is_none());
        paid!(
            budget,
            function,
            TERMINAL_CALL,
            "PLAN view test terminal caller"
        )?;
        assert_eq!(terminal(&cursor), old.terminal);
    }
    Ok(())
}

#[test]
fn genuine_public_and_builder_walks_retain_every_ordered_old_path() {
    let program = super::super::super::tests::program();
    for function in &program.functions {
        let mut old_budget = ReplayBudget::unobserved();
        reserve_program_skeleton_work(&program, std::iter::once(function), &mut old_budget)
            .unwrap();
        let expected = before::produce(function, &mut old_budget).unwrap();
        let mut public = budget(&program, function);
        assert_eq!(public.payer, AllocationPayer::LegacyUnobserved);
        visit(function, &mut public, &expected).unwrap();
        assert_eq!(public.skeleton_spent, old_budget.skeleton_spent);
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut funded = budget(&program, function);
                assert_eq!(funded.payer, AllocationPayer::ActiveBuilder);
                visit(function, &mut funded, &expected).unwrap();
                assert_eq!(funded.skeleton_spent, old_budget.skeleton_spent);
            })
        });
        assert!(!refused && first.is_none() && !allocation.invalid);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(paid >= allocation.allocated);
    }
}

#[test]
fn full_loan_drop_exact_short_zero_and_first_refusal() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    let mut old = ReplayBudget::unobserved();
    reserve_program_skeleton_work(&program, std::iter::once(function), &mut old).unwrap();
    let expected = before::produce(function, &mut old).unwrap();
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result =
                    reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget)
                        .and_then(|_reserved| visit(function, &mut budget, &expected));
                let success = result.is_ok();
                if !success {
                    assert!(budget
                        .reserve_bytes(function, 0, "PLAN view sticky zero")
                        .is_err());
                }
                success
            })
        })
    };
    let ((success, allocation), refused, paid, first) = run(usize::MAX);
    assert!(success && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    let ((success, allocation), refused, exact, first) = run(paid);
    assert!(success && !refused && first.is_none());
    assert_eq!(exact, paid);
    assert!(!allocation.invalid && allocation.live == 0);
    for limit in [0, paid - 1] {
        let ((success, allocation), refused, _, first) = run(limit);
        assert!(!success && refused && first.is_some());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn unsupported_admissions_and_semantic_indices_do_not_mint_a_lease() {
    let program = super::super::super::tests::program();
    let function = &program.functions[0];
    let mut public = budget(&program, function);
    assert!(super::super::admission::eligible(function, &public));
    assert!(!super::super::super::eligible(function, &public));
    assert!(!select(function, &mut public).unwrap());
    for variant in 0..3 {
        let mut excluded = budget(&program, function);
        match variant {
            0 => excluded.admission = AdmissionMode::SelectedRetained,
            1 => excluded.lent_skeleton = true,
            _ => excluded.merge_paths = true,
        }
        let error = produce(function, &mut excluded).err().unwrap();
        assert_eq!(error.code, "SPX-H006");
    }
    let produced = produce(function, &mut public).unwrap();
    assert!(prepare(&produced, usize::MAX, &mut public).is_err());
    // Semantic index refusal does not invent a sticky physical exhaustion.
    public
        .reserve_bytes(function, 0, "PLAN view after semantic index")
        .unwrap();
    assert_eq!(CURSOR_PREPARE_CALL, super::super::super::PREPARE_CALL);
    assert_eq!(CURSOR_NEXT_CALL, super::super::super::NEXT_CALL);
}

#[test]
fn complete_immediate_old_producer_and_pending_shipping_gate_remain_exact() {
    let current = include_str!("producer.rs");
    let registration = "#[path = \"producer_admission.rs\"]\nmod admission;\n#[path = \"producer_view.rs\"]\npub(in crate::cleanup_plan::replay::ordinary_plan_history) mod view;\n";
    assert_eq!(current.matches(registration).count(), 1);
    let restored = current.replace(registration, "");
    let changed = "if !admission::eligible(function, budget) {";
    assert_eq!(restored.matches(changed).count(), 1);
    let restored = restored.replace(changed, "if !eligible(function, budget) {");
    let before = include_str!("producer_admission/producer.before.txt");
    assert_eq!(restored, before);
    assert_eq!(
        &sha2::Sha256::digest(before.as_bytes())[..],
        &OLD_PRODUCER_SHA
    );
    let owner = include_str!("../ordinary_plan_history.rs");
    assert!(owner.contains("if budget.payer != AllocationPayer::ActiveBuilder {"));
    assert!(owner.contains("#[cfg(not(test))]\n    {\n        Ok(false)\n    }"));
}

const OLD_PRODUCER_SHA: [u8; 32] = [
    22, 38, 164, 189, 40, 66, 108, 193, 231, 152, 229, 147, 70, 204, 214, 246, 206, 27, 145, 159,
    113, 180, 104, 93, 91, 71, 126, 199, 55, 240, 236, 46,
];
