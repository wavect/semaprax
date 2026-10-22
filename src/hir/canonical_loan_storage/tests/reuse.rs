//! Complete old/current append parity and literal reusable backing through Drop.
use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};
use std::mem::size_of;

fn equal(current: &Storage<'_>, saved: &Storage<'_>) {
    assert_eq!(current.ids, saved.ids);
    assert_eq!(current.liveness, saved.liveness);
    assert_eq!(current.origins, saved.origins);
}

#[test]
fn complete_saved_append_and_current_reuse_keep_all_original_attack_priorities_and_origins() {
    let program = fixture();
    for attack in 0..7 {
        let mut first = plan("attack");
        let mut second = plan("later");
        match attack {
            0 => {}
            1 => {
                first.endpoints[0].live_before = vec![LoanId(99)].into_boxed_slice();
                first.loans[0].start.phase = LoanPointPhase::After;
            }
            2 => {
                first.endpoints[0].live_before =
                    vec![LoanId(0), LoanId(0), LoanId(99)].into_boxed_slice();
            }
            3 => {
                first.loans[0].start.phase = LoanPointPhase::After;
                second.endpoints[0].live_before = vec![LoanId(99)].into_boxed_slice();
            }
            4 => second.loans[0].site = first.loans[0].site.clone(),
            5 => {
                first.loans[1].id = LoanId(0);
                first.endpoints[0].live_before = vec![LoanId(0)].into_boxed_slice();
            }
            6 => second.endpoints[0] = first.endpoints[0].clone(),
            _ => unreachable!(),
        }
        let plans = vec![first, second];
        let saved = caller_before::from_plans(&program, plans.clone());
        let current = caller::from_plans_reused(&program, plans);
        match (saved, current) {
            (Ok(saved), Ok(current)) => equal(&current, &saved),
            (Err(saved), Err(current)) => assert_eq!(current, saved),
            _ => panic!("complete receiver outcome differs"),
        }
    }
    let plans = vec![plan("first"), plan("second")];
    let expected = old::from_plans(plans.clone()).unwrap();
    let pointers = plans
        .iter()
        .flat_map(|p| p.loans.iter().map(|l| l.origin.projections.as_ptr()))
        .collect::<Vec<_>>();
    let current = caller::from_plans_reused(&program, plans).unwrap();
    same(&current, &expected);
    for (origin, pointer) in current.origins.iter().zip(pointers) {
        assert_eq!(origin.projections.as_ptr(), pointer);
    }
    let empty = LoanPlan {
        schema: "test-storage-only",
        loans: Vec::new(),
        endpoints: Vec::new(),
        edges: Vec::new(),
    };
    let saved = caller_before::from_plans(&program, vec![empty.clone()]).unwrap();
    let current = caller::from_plans_reused(&program, vec![empty]).unwrap();
    equal(&current, &saved);
}

#[test]
fn literal_reusable_scratch_is_paid_once_and_retained_through_exact_short_receiver_drop() {
    let program = fixture();
    // One retained local2/IDs2/live3 scratch. Persistent buffers still grow
    // IDs2→4, live3→6 and origins2→4 with full replacement backing charges.
    let bytes = 2 * size_of::<(LoanId, usize)>()
        + 2 * size_of::<(IdKey, LoanId, usize)>()
        + 3 * size_of::<(LiveKey, Option<usize>, usize)>()
        + 6 * size_of::<(IdKey, LoanId)>()
        + 9 * size_of::<(LiveKey, usize)>()
        + 6 * size_of::<Place>();
    for allowance in [bytes, bytes - 1, 0] {
        let plans = vec![plan("first"), plan("second")];
        // Held input plan/payload and the existing Budget are outside this
        // storage-only observation, identically to the complete saved control.
        let (((), observed), refused, used, receipt) = with_limit_usage_refusal(allowance, || {
            measure(|| {
                let result = caller::from_plans_reused(&program, plans);
                assert_eq!(result.is_ok(), allowance == bytes);
                if let Ok(storage) = result {
                    assert_eq!(storage.origins.len(), 4);
                    assert_eq!(storage._scratch.local.capacity(), 2);
                    assert_eq!(storage._scratch.ids.capacity(), 2);
                    assert_eq!(storage._scratch.liveness.capacity(), 3);
                    assert!(
                        storage._scratch.ids.is_empty() && storage._scratch.liveness.is_empty()
                    );
                    assert!(current_metrics().live > 0);
                    drop(storage);
                }
                if allowance < bytes {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
            })
        });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(observed.allocated <= used && observed.highwater <= used);
        assert_eq!(refused, allowance < bytes);
        if allowance == bytes {
            assert_eq!(used, bytes);
            assert_eq!(observed.allocated, bytes);
            assert!(receipt.is_none());
        } else {
            assert!(receipt.is_some());
        }
    }
}

#[test]
fn full_saved_and_current_batch_observers_include_all_probe_backing_and_actual_drop() {
    let program = fixture();
    let run = |saved| {
        let plans = (0..8)
            .map(|i| plan(&format!("batch{i}")))
            .collect::<Vec<_>>();
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let result = if saved {
                    caller_before::from_plans(&program, plans)
                } else {
                    caller::from_plans_reused(&program, plans)
                };
                let storage = result.unwrap();
                assert_eq!(storage.origins.len(), 16);
                drop(storage);
            })
        })
    };
    let (((), saved), refused, saved_paid, first) = run(true);
    assert!(!refused && first.is_none() && !saved.invalid);
    assert!(saved.allocated <= saved_paid && saved.highwater <= saved_paid);
    assert_eq!(saved.allocated, saved.freed);
    assert_eq!(saved.live, 0);
    let (((), current), refused, paid, first) = run(false);
    assert!(!refused && first.is_none() && !current.invalid);
    assert!(current.allocated <= paid && current.highwater <= paid);
    assert_eq!(current.allocated, current.freed);
    assert_eq!(current.live, 0);
    assert!(current.allocated < saved.allocated && paid < saved_paid);
}

#[test]
fn poisoned_receiving_owner_cannot_reuse_paid_spare_capacity_or_replace_first_refusal() {
    let program = fixture();
    let plans = vec![plan("first")];
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut storage = caller::from_plans_reused(&program, plans).unwrap();
        assert!(!crate::bounded_output::reserve_active_required(usize::MAX));
        let empty = LoanPlan {
            schema: "test-storage-only",
            loans: Vec::new(),
            endpoints: Vec::new(),
            edges: Vec::new(),
        };
        let (error, after) =
            measure(|| caller::append_for_receiving(&mut storage, empty).unwrap_err());
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.is_empty());
        assert_eq!(after.allocated, 0);
        assert!(!crate::bounded_output::reserve_active_required(0));
        assert_eq!(storage.origins.len(), 2);
        drop(storage);
    });
    assert!(refused);
    assert_eq!(first.unwrap().requested, usize::MAX);
}

#[test]
fn complete_old_current_constructor_and_validation_keep_exact_subject_and_full_outcomes() {
    let program = fixture();
    let directory = crate::hir::function_directory::caller::prepare(&program).unwrap();
    let seed = ConstructorSeed::authenticate(&program, Some(&directory)).unwrap();
    let saved = caller_before::prepare(&seed).unwrap();
    let current = caller::prepare(&seed).unwrap();
    equal(&current, &saved);
    assert_eq!(
        caller_before::validate(&program, None, Some(&directory)),
        caller::validate(&program, None, Some(&directory))
    );
    let other = program.clone();
    assert!(!current.is_current(&other));
    let mut hostile = program.clone();
    hostile.entrypoint = crate::hir::DeclarationId::new("bad\0entry");
    hostile.functions[0].body.id = ExpressionId::from_owned("bad\0body".into());
    assert_eq!(
        caller_before::validate(&hostile, None, None),
        caller::validate(&hostile, None, None)
    );
    let capacity_error = |saved| {
        with_limit_usage(7, || {
            assert!(crate::bounded_output::reserve_active_required(2));
            if saved {
                caller_before::capacity_failure(5, 1)
            } else {
                caller::capacity_failure(5, 1)
            }
        })
    };
    assert_eq!(capacity_error(true), capacity_error(false));
}

#[test]
fn growing_probe_replacements_use_exact_needed_backing_and_clear_all_entries() {
    let program = fixture();
    let mut smaller = plan("small");
    smaller.loans.truncate(1);
    smaller.endpoints[0].live_before = vec![LoanId(0)].into_boxed_slice();
    let larger = plan("larger");
    let live_count = |plan: &LoanPlan| {
        plan.endpoints
            .iter()
            .map(|endpoint| match endpoint.point.phase {
                LoanPointPhase::Before => endpoint.live_before.len(),
                LoanPointPhase::After => endpoint.live_after.len(),
            })
            .sum::<usize>()
    };
    assert_eq!(live_count(&smaller), 2);
    assert_eq!(live_count(&larger), 3);
    assert!(smaller
        .endpoints
        .iter()
        .flat_map(|endpoint| match endpoint.point.phase {
            LoanPointPhase::Before => endpoint.live_before.iter(),
            LoanPointPhase::After => endpoint.live_after.iter(),
        })
        .all(|loan| *loan == LoanId(0)));
    let plans = vec![smaller, larger];
    let saved = caller_before::from_plans(&program, plans.clone()).unwrap();
    let current = caller::from_plans_reused(&program, plans).unwrap();
    equal(&current, &saved);
    // live2→live3 uses exact3, rather than a fresh doubled4 reservation.
    assert_eq!(current._scratch.local.capacity(), 2);
    assert_eq!(current._scratch.ids.capacity(), 2);
    assert_eq!(current._scratch.liveness.capacity(), 3);
    assert!(current._scratch.local.is_empty());
    assert!(current._scratch.ids.is_empty());
    assert!(current._scratch.liveness.is_empty());
}

#[test]
fn complete_old_callers_storage_and_tests_remain_exact_and_all_current_hooks_are_bound() {
    use sha2::{Digest, Sha256};
    let digest = |text: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
        )
    };
    let old = include_str!("../../../../docs/evidence/canonical-loan-scratch/caller_before.txt");
    assert_eq!(
        digest(old),
        "47bec8003e7c5e7056edd1a3beeadddc355c9d422fdf184df71d1d3f0b9e7f0b"
    );
    let hooks = include_str!("../../../../docs/evidence/canonical-loan-scratch/caller-hooks.json");
    assert_eq!(
        digest(hooks),
        "e368abf524b313f744b7ae4d7b8aeacae3e3d0079cc67a0af300b036c95a6b13"
    );
    let hooks: serde_json::Value = serde_json::from_str(hooks).unwrap();
    let mut projected = include_str!("../caller.rs").to_owned();
    // Unregistered compiled source: authenticate the ENTIRE current input,
    // reverse only these exact optional observer adapters, then preserve every
    // original whole-before assertion and old inverse below. No page admission.
    assert_eq!(projected.len(), 17695);
    assert_eq!(
        digest(&projected),
        "b9e11a029bbc1252cc80bd83eaae627bca19c3ca12b399ff2796b445b67f95c0"
    );
    for (start, end, current, previous) in [
        (
            7870,
            8100,
            r########"    );
    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::CanonicalLoanStorage,
"########,
            r########""########,
        ),
        (
            6007,
            6253,
            r########"        );
        #[cfg(feature = "unstable-workflow-profiling")]
        let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
            crate::bounded_output::exclusive_debits::Operation::ValidatorConstructor,
"########,
            r########""########,
        ),
        (
            5072,
            5731,
            r########"    let helpers = {
        #[cfg(feature = "unstable-workflow-profiling")]
        let _exclusive_constructor = crate::bounded_output::exclusive_debits::operation_guard(
            crate::bounded_output::exclusive_debits::Operation::ModuleIndex,
        );
        crate::loop_calls::byte_writer::ModuleIndex::new(program)?
    };
    let seed = {
        #[cfg(feature = "unstable-workflow-profiling")]
        let _exclusive_constructor = crate::bounded_output::exclusive_debits::operation_guard(
            crate::bounded_output::exclusive_debits::Operation::ConstructorSeed,
        );
        ConstructorSeed::authenticate(program, directory)?
    };
"########,
            r########"    let helpers = crate::loop_calls::byte_writer::ModuleIndex::new(program)?;
    let seed = ConstructorSeed::authenticate(program, directory)?;
"########,
        ),
        (
            4607,
            4831,
            r########"    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::CoreValidation,
    );
"########,
            r########""########,
        ),
    ] {
        assert_eq!(projected.get(start..end), Some(current));
        projected.replace_range(start..end, previous);
    }
    assert_eq!(projected.len(), 16481);
    assert_eq!(
        digest(&projected),
        "86546a62bea117a0df58767a472d56825114a700966594acf91a65e354f3ef56"
    );
    // Exact later append-stage wrapper; preserve the complete old call and
    // every original hook/archive byte below rather than normalizing source.
    let append_stage = "            {\n                let _stage = crate::bounded_output::reservation_stage_guard(\n                    crate::bounded_output::ReservationStage::LoanCanonicalAppend,\n                );\n                append(&mut storage, plan, payer)?;\n            }";
    assert_eq!(projected.matches(append_stage).count(), 1);
    projected = projected.replacen(
        append_stage,
        "            append(&mut storage, plan, payer)?;",
        1,
    );
    // Exact additive production stage and test registration only; preserve
    // every byte of the complete original preparation receiver below.
    for addition in [
        "    let _stage = crate::bounded_output::reservation_stage_guard(\n        crate::bounded_output::ReservationStage::CanonicalLoanStorage,\n    );\n",
        "\n#[cfg(test)]\nmod stage_tests;\n",
        "\n#[cfg(test)]\nmod sealed_before_stage;\n",
    ] {
        assert_eq!(projected.matches(addition).count(), 1);
        projected = projected.replacen(addition, "", 1);
    }
    // The prior shared publication factored this exact complete body into
    // prepare_sealed. Undo only that named namespace/result adapter as well.
    for (current, original) in [
        ("fn prepare_with_comparison<'a>(\n    seed: &ConstructorSeed<'_>,\n    program: &'a ResolvedProgram,\n) -> Result<(Storage<'a>, Deferred<'a>), Diagnostic> {\n    prepare_sealed(seed, program, |storage, _| Ok(storage))\n}\n\npub(in crate::hir) fn prepare_shared<'a>(\n    seed: &ConstructorSeed<'a>,\n) -> Result<std::rc::Rc<Storage<'a>>, Diagnostic> {\n    // Compatibility construction never finishes Deferred: attached plan\n    // mismatch remains selected by the complete ordinary validator afterward.\n    prepare_sealed(seed, seed.program(), shared::seal).map(|(storage, _)| storage)\n}\n\n#[cfg(test)]\nmod owned_before;\nmod shared;\n\nfn prepare_sealed<'a, T>(\n    seed: &ConstructorSeed<'_>,\n    program: &'a ResolvedProgram,\n    seal: impl FnOnce(Storage<'a>, Payer<'_>) -> Result<T, Diagnostic>,\n) -> Result<(T, Deferred<'a>), Diagnostic> {\n", "fn prepare_with_comparison<'a>(\n    seed: &ConstructorSeed<'_>,\n    program: &'a ResolvedProgram,\n) -> Result<(Storage<'a>, Deferred<'a>), Diagnostic> {\n"),
        ("        Ok((seal(storage, payer)?, deferred))", "        Ok((storage, deferred))"),
    ] {
        assert_eq!(projected.matches(current).count(), 1);
        projected = projected.replacen(current, original, 1);
    }
    for hook in hooks["hooks"].as_array().unwrap() {
        let current = hook["current"].as_str().unwrap();
        assert_eq!(projected.matches(current).count(), 1, "{}", hook["name"]);
        projected = projected.replacen(current, hook["original"].as_str().unwrap(), 1);
    }
    // Independent saved source predates allocation-free test attribution.
    // Remove only the three exact guarded hooks; every old body byte still
    // participates in the final equality below.
    for hook in [
        "    #[cfg(test)]\n    let _operation = crate::bounded_output::stage_debits::operation_guard(\n        crate::bounded_output::stage_debits::Operation::CoreValidation,\n    );\n",
        "    #[cfg(test)]\n    let _operation = crate::bounded_output::stage_debits::operation_guard(\n        crate::bounded_output::stage_debits::Operation::CanonicalLoanStorage,\n    );\n",
    ] {
        assert_eq!(projected.matches(hook).count(), 1);
        projected = projected.replacen(hook, "", 1);
    }
    let guarded_constructor = "    let validator = {\n        #[cfg(test)]\n        let _operation = crate::bounded_output::stage_debits::operation_guard(\n            crate::bounded_output::stage_debits::Operation::ValidatorConstructor,\n        );\n        HirValidator::with_storage(seed, &storage)?\n    };";
    assert_eq!(projected.matches(guarded_constructor).count(), 1);
    projected = projected.replacen(
        guarded_constructor,
        "    let validator = HirValidator::with_storage(seed, &storage)?;",
        1,
    );
    assert_eq!(projected, old);
    let mut compiled_old = include_str!("../caller_before.rs").to_owned();
    // Remove the longer indentation first: the shorter line is its substring.
    for field in [
        "            _scratch: Scratch::default(),\n",
        "        _scratch: Scratch::default(),\n",
    ] {
        assert_eq!(compiled_old.matches(field).count(), 1);
        compiled_old = compiled_old.replacen(field, "", 1);
    }
    assert_eq!(compiled_old, old);
    let old_storage =
        include_str!("../../../../docs/evidence/canonical-loan-scratch/storage_before.txt");
    assert_eq!(
        digest(old_storage),
        "088a2d91895c12065ada643e98c96f1a1f261aa78923d24b19662cd27795e0bc"
    );
    let mut current_storage = include_str!("../../canonical_loan_storage.rs").to_owned();
    for addition in ["#[cfg(test)]\nmod caller_before;\nmod scratch;\nuse scratch::Scratch;\n", "    // Retain reusable paid append backing with this exact receiving owner.\n    _scratch: Scratch,\n"] {
        assert_eq!(current_storage.matches(addition).count(), 1);
        current_storage = current_storage.replacen(addition, "", 1);
    }
    assert_eq!(current_storage, old_storage);
    let old_tests =
        include_str!("../../../../docs/evidence/canonical-loan-scratch/tests_before.txt");
    assert_eq!(
        digest(old_tests),
        "2db71a1312772f847fe19c934ee47c54a685a6562fd59cb4090f99879a15a8e7"
    );
    let registration = "mod reuse;\n// The existing literal from_plans controls below retain the complete saved\n// allocation receiver. reuse tests exercise the actual production append path.\n";
    let tests = include_str!("../tests.rs");
    assert_eq!(tests.matches(registration).count(), 1);
    assert_eq!(tests.replacen(registration, "", 1), old_tests);
}

#[test]
fn full_current_and_complete_saved_prepare_pay_every_actual_acquisition_through_drop() {
    let program = fixture();
    let directory = crate::hir::function_directory::caller::prepare(&program).unwrap();
    let seed = ConstructorSeed::authenticate(&program, Some(&directory)).unwrap();
    let expected = old::Inventory::rebuild(&program).unwrap();
    let nonempty = program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .filter(|function| {
            !crate::loan_plan::build_plan(&program, function)
                .unwrap()
                .loans
                .is_empty()
        })
        .count();
    assert!(
        nonempty >= 2,
        "strict scratch comparison requires two actual nonempty plans"
    );
    let run = |saved, limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = if saved {
                    caller_before::prepare(&seed)
                } else {
                    caller::prepare(&seed)
                };
                match result {
                    Ok(storage) => {
                        same(&storage, &expected);
                        assert!(storage.is_current(&program));
                        drop(storage);
                        true
                    }
                    Err(report) => {
                        assert_eq!(report.code, "SPX-H006");
                        drop(report);
                        assert!(!crate::bounded_output::reserve_active_required(0));
                        let before = current_metrics();
                        let retry = if saved {
                            caller_before::prepare(&seed)
                        } else {
                            caller::prepare(&seed)
                        };
                        let error = retry.err().unwrap();
                        assert_eq!(error.code, "SPX-H006");
                        assert!(error.message.is_empty());
                        drop(error);
                        assert_eq!(current_metrics(), before);
                        false
                    }
                }
            })
        })
    };
    let ((ok, saved), refused, saved_paid, first) = run(true, usize::MAX);
    assert!(ok && !refused && first.is_none() && !saved.invalid);
    assert!(saved.allocated <= saved_paid && saved.highwater <= saved_paid);
    assert_eq!(saved.allocated, saved.freed);
    assert_eq!(saved.live, 0);
    let ((ok, current), refused, paid, first) = run(false, usize::MAX);
    assert!(ok && !refused && first.is_none() && !current.invalid);
    assert!(current.allocated <= paid && current.highwater <= paid);
    assert_eq!(current.allocated, current.freed);
    assert_eq!(current.live, 0);
    assert!(current.allocated < saved.allocated && paid < saved_paid);
    for (saved, exact, observed) in [(true, saved_paid, saved), (false, paid, current)] {
        let ((ok, repeated), refused, used, first) = run(saved, exact);
        assert!(ok && !refused && first.is_none());
        assert_eq!(used, exact);
        assert_eq!(repeated, observed);
        let ((ok, short), refused, used, first) = run(saved, exact - 1);
        assert!(!ok && refused && first.is_some() && !short.invalid);
        assert!(short.allocated <= used && short.highwater <= used);
        assert_eq!(short.allocated, short.freed);
        assert_eq!(short.live, 0);
    }
}
