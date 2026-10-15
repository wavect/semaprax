use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;

fn semantic(result: Result<LoanPlan, storage::PlannerError>) -> Result<LoanPlan, Diagnostic> {
    match result {
        Ok(plan) => Ok(plan),
        Err(storage::PlannerError::Semantic(error)) => Err(error),
        Err(storage::PlannerError::Physical(_)) => {
            panic!("ample receiver unexpectedly refused physically")
        }
    }
}

fn prior(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    owner: &CfgReservation,
) -> Result<LoanPlan, storage::PlannerError> {
    // Complete prior facade, not a reconstruction from the new route.
    funded_flat::before_inventory_for_test(program, function, owner)
}

#[test]
fn complete_old_and_retained_plans_diagnostics_allocation_and_drop_match() {
    let (program, index) = boundary_tests::fixture_with_uses(8, true);
    let mut hostiles = vec![program.clone()];
    let mut duplicate = program.clone();
    let function = &mut duplicate.functions[index];
    let original = function.body.id.clone();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("fixture block")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("fixture let")
    };
    value.id = original;
    hostiles.push(duplicate);
    let mut saved = false;
    for program in &hostiles {
        let function = &program.functions[index];
        let (expected, refused, _) = with_limit_usage(1 << 26, || {
            let owner = CfgReservation::acquire().unwrap();
            semantic(prior(program, function, &owner))
        });
        assert!(!refused);
        let mut observations = Vec::new();
        for retained in [false, true] {
            let (((), allocation), refused, debit) = with_limit_usage(1 << 26, || {
                measure(|| {
                    let owner = CfgReservation::acquire().unwrap();
                    let result = if retained {
                        funded_flat::build(program, function, &owner)
                    } else {
                        prior(program, function, &owner)
                    };
                    let result = semantic(result);
                    assert_eq!(result, expected);
                    drop(result);
                    drop(owner);
                })
            });
            assert!(!refused && !allocation.invalid);
            assert_eq!(allocation.live, 0);
            assert_eq!(allocation.allocated, allocation.freed);
            observations.push((allocation.allocated, debit));
        }
        if expected.is_ok() {
            assert!(
                observations[1].0 < observations[0].0,
                "actual second map nodes must disappear"
            );
            assert!(
                observations[1].1 < observations[0].1,
                "all new carrier payments must still yield net debit reduction here"
            );
            saved = true;
        }
    }
    let scalar = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let (_, refused, _) = with_limit_usage(1 << 26, || {
        let owner = CfgReservation::acquire().unwrap();
        let expected = semantic(prior(&program, scalar, &owner));
        let current = semantic(funded_flat::build(&program, scalar, &owner));
        assert_eq!(current, expected);
        assert!(current.unwrap().loans.is_empty());
    });
    assert!(!refused);
    assert!(saved);
}

#[test]
fn actual_retained_work_and_byte_routes_have_exact_short_zero_and_sticky_boundaries() {
    let (program, index) = boundary_tests::fixture_with_uses(2, true);
    let function = &program.functions[index];
    let (exact_work, refused, _) = with_limit_usage(1 << 26, || {
        let owner = CfgReservation::acquire().unwrap();
        let subject = funded_flat::subject_for_test(&program, function, &owner).unwrap();
        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        work.allocation = Some(&owner);
        let inventory = inspect(subject, &owner, &mut work).unwrap().unwrap();
        let plan = paid_plan::build_retained(&program, function, &mut work, inventory).unwrap();
        assert_eq!(plan, function.loan_plan);
        work.used
    });
    assert!(!refused && exact_work > 0);
    for limit in [0, exact_work - 1, exact_work] {
        let (_, refused, _) = with_limit_usage(1 << 26, || {
            let owner = CfgReservation::acquire().unwrap();
            let result = funded_flat::with_inventory_work_limit(limit, || {
                funded_flat::build(&program, function, &owner)
            });
            assert!(owner.first().is_none());
            match result {
                Ok(plan) => {
                    assert_eq!(limit, exact_work);
                    assert_eq!(plan, function.loan_plan);
                }
                Err(storage::PlannerError::Semantic(error)) => {
                    assert!(limit < exact_work);
                    assert_eq!(error.code, "SPX-H006");
                    assert!(error.message.contains("work"));
                }
                Err(storage::PlannerError::Physical(_)) => {
                    panic!("work refusal must stay semantic")
                }
            }
        });
        assert!(!refused);
    }
    let (_, refused, exact, _) = with_limit_usage_refusal(1 << 26, || {
        let owner = CfgReservation::acquire().unwrap();
        let result = funded_flat::build(&program, function, &owner);
        assert!(result.is_ok());
        drop(result);
        drop(owner);
    });
    assert!(!refused);
    for limit in [0, exact - 1, exact] {
        let ((success, allocation), refused, debit, first) =
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let owner = match CfgReservation::acquire() {
                        Ok(owner) => owner,
                        Err(_) => return false,
                    };
                    let result = funded_flat::build(&program, function, &owner);
                    let success = result.is_ok();
                    drop(result);
                    if !success {
                        let original = owner.first();
                        assert!(original.is_some());
                        assert!(funded_flat::build(&program, function, &owner).is_err());
                        assert_eq!(owner.first(), original);
                    }
                    drop(owner);
                    success
                })
            });
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        if limit == exact {
            assert!(success && !refused && first.is_none());
            assert_eq!(debit, exact);
        } else {
            assert!(!success && refused && first.is_some());
        }
    }
}

#[test]
fn retained_inventory_foreign_subject_reservation_detached_and_sticky_owners_fail_closed() {
    let (program, index) = boundary_tests::fixture_with_uses(2, true);
    let function = &program.functions[index];
    let foreign_program = program.clone();
    let foreign_function = function.clone();
    for attack in 0..3 {
        let (((), allocation), refused, _) = with_limit_usage(1 << 26, || {
            measure(|| {
                let owner = CfgReservation::acquire().unwrap();
                let other = CfgReservation::acquire().unwrap();
                let subject = funded_flat::subject_for_test(&program, function, &owner).unwrap();
                let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
                work.allocation = Some(&owner);
                let inventory = inspect(subject, &owner, &mut work).unwrap().unwrap();
                let before = crate::bounded_output::active_remaining();
                let result = match attack {
                    0 => inventory.into_parts(&foreign_program, function, Some(&owner)),
                    1 => inventory.into_parts(&program, &foreign_function, Some(&owner)),
                    _ => inventory.into_parts(&program, function, Some(&other)),
                };
                let error = result.err().unwrap();
                assert_eq!(
                    error,
                    Diagnostic::io_static(
                        "SPX-H006",
                        "candidate inventories belong to another subject or reservation"
                    )
                );
                assert_eq!(crate::bounded_output::active_remaining(), before);
                assert!(owner.first().is_none() && other.first().is_none());
                drop(error);
                drop(other);
                drop(owner);
            })
        });
        assert!(
            !refused
                && !allocation.invalid
                && allocation.live == 0
                && allocation.allocated == allocation.freed
        );
    }
    let (((), allocation), refused, _, first) = with_limit_usage_refusal(1 << 26, || {
        measure(|| {
            let owner = CfgReservation::acquire().unwrap();
            let subject = funded_flat::subject_for_test(&program, function, &owner).unwrap();
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            work.allocation = Some(&owner);
            let inventory = inspect(subject, &owner, &mut work).unwrap().unwrap();
            let (_, nested_refused, nested_debit) = with_limit_usage(1 << 26, || {
                let error = inventory
                    .into_parts(&program, function, Some(&owner))
                    .err()
                    .unwrap();
                assert_eq!(error, storage::marker());
                assert_eq!(
                    owner.first(),
                    Some(crate::bounded_output::cfg_reservation::Refused::Detached)
                );
                drop(error);
            });
            assert!(!nested_refused && nested_debit == 0);
            assert_eq!(
                owner.reserve(0),
                Err(crate::bounded_output::cfg_reservation::Refused::Detached)
            );
            drop(owner);
        })
    });
    assert!(refused && first.is_some());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
}

#[test]
fn complete_original_receivers_and_all_source_joins_are_byte_exact() {
    use sha2::{Digest, Sha256};
    let sources = [
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/paid_candidate.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/loan-candidate-inventory-reuse/paid_candidate.rs.before.txt")), "1e0b84ebde6ce7c110238cfb35f68f5daf1befbc2a2648e1f49203a302aa1f22"),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/paid_plan.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/loan-candidate-inventory-reuse/paid_plan.rs.before.txt")), "98db139e9b14abb2126a887f760e4db2bbbd095a9a2b89df924bef71e68732f3"),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/funded_flat.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/loan-candidate-inventory-reuse/funded_flat.rs.before.txt")), "3c4084c88c10993169b65b9bf3a16de490f14b98754cba4966b5f7af56df7d99"),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/funded_flat/work_boundary.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/loan-candidate-inventory-reuse/work_boundary.rs.before.txt")), "706a4b53dbe3e980470a6156d06a6ef4f31fedcb357a61b7491b6a24c08b7733"),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/loan_plan/paid_cursor/tests.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/loan-candidate-inventory-reuse/paid_cursor_tests.rs.before.txt")), "36d74075307777d67fcbcc5694c32d7d553fdb8ef0c208cb42e93bba3fb87202"),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/loan-candidate-inventory-reuse/tests__hir_module_boundaries.rs.before.txt")), "4ba11a086ed1efbd2211d5b21e334316b0a3c91ccd66b2cf75677d4eb76c12bf"),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/loan-candidate-inventory-reuse/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt")), "a36ff7c4241aaa4ecc8698b3ae64d5bef7c9ec0ecef208d5c612deb9e0e66718"),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/loan-candidate-inventory-reuse/source_custody_b.rs.before.txt")), "b85fd51439e81d83b5a776244dc17f7f9ce83b8704040407fcfddefdb8f665c6"),
    ];
    for (current, original, fixed) in sources {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            fixed
        );
        // Resolve only the exact recorded complete source; never compare a reduced span.
        let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/loan-candidate-inventory-reuse/exact-source-inverses.json"
        )))
        .unwrap();
        let record = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["original_sha256"] == fixed)
            .unwrap();
        assert_eq!(
            before_inventory_source(record["source"].as_str().unwrap(), current),
            original
        );
    }
    let prior = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/loan-candidate-inventory-reuse/funded_flat.rs.before.txt"
    ));
    let start = prior.find("pub(super) fn build(").unwrap();
    let end = prior.find("\nfn lexical_preflight(").unwrap();
    assert_eq!(include_str!("../../funded_flat/inventory_before.rs"),
        format!("//! Complete pre-reuse funded facade; original helpers remain in its parent.\nuse super::*;\n\n{}\n", &prior[start..end]));
}
