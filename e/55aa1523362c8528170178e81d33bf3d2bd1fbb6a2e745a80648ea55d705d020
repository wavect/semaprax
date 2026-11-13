//! Complete independent paid receivers; scratch remains bound to one payer.
use super::*;

fn receiver_result(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    saved: bool,
) -> (Result<(), Diagnostic>, usize, usize, usize) {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget).unwrap();
    let result = if saved {
        before::validate_inventory_coverage(program, function, &mut budget)
    } else {
        validate_inventory_coverage(program, function, &mut budget)
    };
    (
        result,
        budget.remaining,
        budget.skeleton_remaining,
        budget.skeleton_spent,
    )
}

#[test]
fn complete_paid_before_and_reused_inventory_preserve_every_shape_flag_and_work() {
    for program in fixtures() {
        for function in &program.functions {
            assert_eq!(
                receiver_result(&program, function, true),
                receiver_result(&program, function, false)
            );
            let mut old_budget = ReplayBudget::new();
            let mut new_budget = ReplayBudget::new();
            let mut receiver = nested_shape::PaidShapeReceiver::new(&mut new_budget);
            let mut old_flags = 0;
            let mut new_flags = 0;
            for slot in &function.cleanup.slots {
                let old = nested_shape::expected_shape_for_type_paid_before(
                    &program,
                    function,
                    &slot.ty,
                    &mut old_flags,
                    &mut old_budget,
                );
                let current = receiver.derive(&program, function, &slot.ty, &mut new_flags);
                assert_eq!(old, current);
                assert_eq!(old_flags, new_flags);
                assert!(receiver.settled());
            }
            let current = receiver.budget();
            assert_eq!(
                (
                    old_budget.remaining,
                    old_budget.skeleton_remaining,
                    old_budget.skeleton_spent
                ),
                (
                    current.remaining,
                    current.skeleton_remaining,
                    current.skeleton_spent
                )
            );
        }
    }
}

#[test]
fn full_late_malformed_duplicate_and_flag_census_diagnostics_keep_original_priority() {
    let programs = fixtures();
    let program = &programs[0];
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "envelope.consume")
        .unwrap();
    assert!(!function.cleanup.slots.is_empty());
    let mut attacks = Vec::new();
    let mut malformed = function.clone();
    malformed.cleanup.slots.last_mut().unwrap().ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("shape.unknown.late"),
        arguments: Vec::new(),
    };
    attacks.push(malformed);
    let mut duplicate = function.clone();
    let mut slot = duplicate.cleanup.slots[0].clone();
    slot.discovery_index = slot.discovery_index.wrapping_add(1);
    duplicate.cleanup.slots.push(slot);
    attacks.push(duplicate);
    let mut flags = function.clone();
    flags
        .cleanup
        .flags
        .pop()
        .expect("owned fixture has a liveness flag");
    attacks.push(flags);
    let mut entries = function.clone();
    entries
        .cleanup_plan
        .entry_state
        .live_owned_parameters
        .clear();
    entries
        .cleanup
        .entry_state
        .live_owned_parameters
        .push(crate::cleanup::CleanupStorageId(u32::MAX));
    attacks.push(entries);
    for attack in attacks {
        let old = receiver_result(program, &attack, true);
        let current = receiver_result(program, &attack, false);
        assert!(old.0.is_err());
        assert_eq!(old, current);
    }
}

fn scalar_observation(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    limit: usize,
    saved: bool,
    admitted: bool,
) {
    let (((), metrics), refused, used, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut receiver = nested_shape::PaidShapeReceiver::new(&mut budget);
            let mut flags = 0;
            for _ in 0..17 {
                let result = if saved {
                    nested_shape::expected_shape_for_type_paid_before(
                        program,
                        function,
                        &ResolvedType::I64,
                        &mut flags,
                        receiver.budget(),
                    )
                } else {
                    receiver.derive(program, function, &ResolvedType::I64, &mut flags)
                };
                assert!(receiver.settled());
                if crate::bounded_output::active_refused() {
                    assert!(result.is_err());
                    let before = crate::test_allocation_observer::current_metrics();
                    let remaining = active_remaining();
                    drop(if saved {
                        nested_shape::expected_shape_for_type_paid_before(
                            program,
                            function,
                            &ResolvedType::I64,
                            &mut flags,
                            receiver.budget(),
                        )
                    } else {
                        receiver.derive(program, function, &ResolvedType::I64, &mut flags)
                    });
                    let after = crate::test_allocation_observer::current_metrics();
                    assert_eq!(
                        (before.allocations, before.allocated),
                        (after.allocations, after.allocated)
                    );
                    assert_eq!(remaining, active_remaining());
                    assert!(!reserve_active_required(0));
                    break;
                }
                assert_eq!(result.unwrap(), FieldLivenessShape::NoDrop);
            }
            drop(receiver);
            drop(budget);
        })
    });
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
    if admitted {
        assert!(!refused && used == limit && first.is_none());
    } else {
        assert!(refused && first.is_some());
    }
    if limit == 0 {
        assert!(refused && used == 0 && first.is_some());
        assert_eq!(metrics.allocated, 0);
    }
}

#[test]
fn retained_scalar_backing_exact_short_sticky_zero_and_full_drop_are_literal() {
    let program = program("module inventory.scratch; @id(\"main\") fn main()->i64 {0}");
    let function = &program.functions[0];
    let observe = |saved| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut receiver = nested_shape::PaidShapeReceiver::new(&mut budget);
                let mut flags = 0;
                let mut warm_capacity = None;
                for _ in 0..17 {
                    let result = if saved {
                        nested_shape::expected_shape_for_type_paid_before(
                            &program,
                            function,
                            &ResolvedType::I64,
                            &mut flags,
                            receiver.budget(),
                        )
                    } else {
                        receiver.derive(&program, function, &ResolvedType::I64, &mut flags)
                    };
                    assert_eq!(result.unwrap(), FieldLivenessShape::NoDrop);
                    assert!(receiver.settled());
                    if !saved {
                        let capacity = receiver.capacities();
                        assert!(capacity.0 > 0 && capacity.1 > 0);
                        if let Some(warm) = warm_capacity {
                            assert_eq!(capacity, warm);
                        }
                        warm_capacity = Some(capacity);
                    }
                }
                drop(receiver);
                drop(budget);
            })
        })
    };
    let (((), old), old_refused, old_required, _) = observe(true);
    let (((), current), refused, required, _) = observe(false);
    assert!(!old_refused && !refused && required > 0);
    assert!(!old.invalid && old.live == 0 && !current.invalid && current.live == 0);
    assert_eq!(old.allocated, old.freed);
    assert_eq!(current.allocated, current.freed);
    assert!(old.allocated > current.allocated);
    assert!(current.allocated <= required && current.highwater <= required);
    assert!(old_required > 0);
    for (saved, debit) in [(true, old_required), (false, required)] {
        scalar_observation(&program, function, debit, saved, true);
        scalar_observation(&program, function, debit - 1, saved, false);
        scalar_observation(&program, function, 0, saved, false);
    }
}

#[test]
fn warm_success_late_unknown_and_retry_clear_payloads_without_losing_backing() {
    let programs = fixtures();
    let program = &programs[0];
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "envelope.consume")
        .unwrap();
    let ty = &function.cleanup.slots[0].ty;
    let mut budget = ReplayBudget::new();
    let mut receiver = nested_shape::PaidShapeReceiver::new(&mut budget);
    let mut flags = 0;
    receiver.derive(program, function, ty, &mut flags).unwrap();
    assert!(receiver.settled());
    let capacity = receiver.capacities();
    let unknown = ResolvedType::Nominal {
        declaration: DeclarationId::new("unknown.scratch.child"),
        arguments: Vec::new(),
    };
    let current = receiver.derive(program, function, &unknown, &mut flags);
    let old = nested_shape::expected_shape_for_type_paid_before(
        program,
        function,
        &unknown,
        &mut flags,
        &mut ReplayBudget::new(),
    );
    assert_eq!(old, current);
    assert!(current.is_err() && receiver.settled());
    assert_eq!(capacity, receiver.capacities());
    receiver.derive(program, function, ty, &mut flags).unwrap();
    assert!(receiver.settled());
    assert_eq!(capacity, receiver.capacities());
}

#[test]
fn full_archived_paid_and_inventory_sources_bind_every_current_hook() {
    use sha2::{Digest, Sha256};
    let proof: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/nested-shape-scratch/source-provenance.json"
    )))
    .unwrap();
    let files = [
        (
            "src/cleanup_plan/replay/nested_shape/paid.rs",
            include_str!("../../nested_shape/paid.rs"),
            "ebcd2e4ac0668c7f0117aae32c0900721d24719e425c7ced3cd93da1c69dd422",
        ),
        (
            "src/cleanup_plan/replay/nested_shape.rs",
            include_str!("../../nested_shape.rs"),
            "b7875ad1c94cf9934621511d27ed0c54e8c02c591af9c5c98f4e8debeda5d702",
        ),
        (
            "src/cleanup_plan/replay/ordinary_inventory.rs",
            include_str!("../../ordinary_inventory.rs"),
            "5049f4d8b103626ebaf1034150850c10d8dba41e74f1fb75912a331df572a82b",
        ),
        (
            "src/cleanup_plan/replay/ordinary_inventory/tests.rs",
            include_str!("../tests.rs"),
            "c76079bac3a7d71784be9600f6e31355b4b541e8b2505f3fc5b23fdbaf75f28d",
        ),
    ];
    for (path, current, archived_sha) in files {
        let entry = &proof["archived_sources"][path];
        let (archive_name, archive) = match path {
            "src/cleanup_plan/replay/nested_shape/paid.rs" => ("src__cleanup_plan__replay__nested_shape__paid.rs.before.txt", include_str!("../../../../../docs/evidence/nested-shape-scratch/src__cleanup_plan__replay__nested_shape__paid.rs.before.txt")),
            "src/cleanup_plan/replay/nested_shape.rs" => ("src__cleanup_plan__replay__nested_shape.rs.before.txt", include_str!("../../../../../docs/evidence/nested-shape-scratch/src__cleanup_plan__replay__nested_shape.rs.before.txt")),
            "src/cleanup_plan/replay/ordinary_inventory.rs" => ("src__cleanup_plan__replay__ordinary_inventory.rs.before.txt", include_str!("../../../../../docs/evidence/nested-shape-scratch/src__cleanup_plan__replay__ordinary_inventory.rs.before.txt")),
            "src/cleanup_plan/replay/ordinary_inventory/tests.rs" => ("src__cleanup_plan__replay__ordinary_inventory__tests.rs.before.txt", include_str!("../../../../../docs/evidence/nested-shape-scratch/src__cleanup_plan__replay__ordinary_inventory__tests.rs.before.txt")),
            _ => unreachable!("complete fixed archived-source inventory"),
        };
        assert_eq!(entry["archive"].as_str().unwrap(), archive_name);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(archive.as_bytes()))
            ),
            archived_sha
        );
        assert_eq!(entry["sha256"].as_str().unwrap(), archived_sha);
        let mut projected = current.to_owned();
        for hook in proof["current_projection"][path].as_array().unwrap() {
            let from = hook["from"].as_str().unwrap();
            let to = hook["to"].as_str().unwrap();
            assert_eq!(
                projected.matches(from).count(),
                hook["count"].as_u64().unwrap() as usize
            );
            projected = projected.replace(from, to);
        }
        assert_eq!(projected, archive);
    }
    assert_eq!(include_str!("../../nested_shape/paid_before.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/nested-shape-scratch/src__cleanup_plan__replay__nested_shape__paid.rs.before.txt")));
    let original = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/nested-shape-scratch/src__cleanup_plan__replay__ordinary_inventory.rs.before.txt"));
    let compiled = original.split("\n#[cfg(test)]\nthread_local!").next().unwrap()
        .replace("#[cfg(test)]\npub(super) mod owned;\npub(super) mod storage;\npub(super) mod substitute;\n", "")
        .replace("nested_shape::expected_shape_for_type_paid(", "nested_shape::expected_shape_for_type_paid_before(");
    assert_eq!(compiled, include_str!("../before.rs"));
}

#[test]
fn late_child_after_valid_owned_fields_preserves_full_error_flags_and_settlement() {
    let programs = fixtures();
    let program = &programs[0];
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "envelope.consume")
        .unwrap();
    let ty = &function.cleanup.slots[0].ty;
    let mut changed = program.clone();
    let envelope = changed
        .types
        .iter_mut()
        .find(|item| item.id.as_str() == "envelope.type")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut envelope.kind else {
        panic!("record fixture");
    };
    assert_eq!(fields.last().unwrap().ty, ResolvedType::I64);
    fields.last_mut().unwrap().ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("late.unknown.after.owned.fields"),
        arguments: Vec::new(),
    };
    let mut budget = ReplayBudget::new();
    let mut receiver = nested_shape::PaidShapeReceiver::new(&mut budget);
    let mut flags = 0;
    drop(receiver.derive(program, function, ty, &mut flags).unwrap());
    let capacities = receiver.capacities();
    let mut old_flags = 0;
    let old = nested_shape::expected_shape_for_type_paid_before(
        &changed,
        function,
        ty,
        &mut old_flags,
        &mut ReplayBudget::new(),
    );
    flags = 0;
    let current = receiver.derive(&changed, function, ty, &mut flags);
    assert!(old.is_err());
    assert_eq!(old, current);
    assert_eq!(old_flags, flags);
    assert!(
        flags > 0,
        "earlier owned children really ran before the late malformed field"
    );
    assert!(receiver.settled());
    assert_eq!(capacities, receiver.capacities());
    drop(current);
    flags = 0;
    drop(receiver.derive(program, function, ty, &mut flags).unwrap());
    assert!(receiver.settled());
    assert_eq!(capacities, receiver.capacities());
}

#[test]
fn warmed_scalar_then_real_nested_growth_exact_short_and_sticky_drop_keep_every_debit() {
    let programs = fixtures();
    let program = &programs[0];
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "envelope.consume")
        .unwrap();
    let ty = &function.cleanup.slots[0].ty;
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut receiver = nested_shape::PaidShapeReceiver::new(&mut budget);
                let mut flags = 0;
                let warm = receiver.derive(program, function, &ResolvedType::I64, &mut flags);
                let warmed = warm.is_ok();
                drop(warm);
                let warm_capacity = receiver.capacities();
                let result = receiver.derive(program, function, ty, &mut flags);
                assert!(receiver.settled());
                if crate::bounded_output::active_refused() {
                    assert!(result.is_err());
                    let before = crate::test_allocation_observer::current_metrics();
                    let remaining = active_remaining();
                    drop(receiver.derive(program, function, &ResolvedType::I64, &mut flags));
                    let after = crate::test_allocation_observer::current_metrics();
                    assert_eq!(
                        (before.allocations, before.allocated),
                        (after.allocations, after.allocated)
                    );
                    assert_eq!(remaining, active_remaining());
                    assert!(!reserve_active_required(0));
                } else {
                    result.as_ref().unwrap();
                    assert!(warmed && flags > 0);
                    let full_capacity = receiver.capacities();
                    assert!(
                        full_capacity.0 >= warm_capacity.0 && full_capacity.1 >= warm_capacity.1
                    );
                }
                drop(result);
                drop(receiver);
                drop(budget);
                warmed
            })
        })
    };
    let ((warmed, whole), refused, required, _) = observe(usize::MAX);
    assert!(warmed && !refused && required > 0 && !whole.invalid && whole.live == 0);
    assert_eq!(whole.allocated, whole.freed);
    for limit in [required, required - 1, 0] {
        let ((warmed, metrics), refused, used, first) = observe(limit);
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
        if limit == required {
            assert!(warmed && !refused && used == required);
        } else {
            assert!(refused && first.is_some());
            if limit > 0 {
                assert!(warmed);
            }
        }
        if limit == 0 {
            assert_eq!(metrics.allocated, 0);
        }
    }
}

#[test]
fn retained_backing_and_payloads_settle_on_unwind_without_changing_authority() {
    struct Interrupted<'a>(&'a ResolvedProgram);
    impl crate::cleanup::ReplayContext for Interrupted<'_> {
        fn declarations(&self) -> &crate::hir::DeclarationIndex {
            panic!("literal interrupted classifier");
        }
        fn types(&self) -> crate::hir::items::View<'_, crate::hir::ResolvedTypeDeclaration> {
            crate::hir::items::View::slice(&self.0.types)
        }
        fn interfaces(&self) -> &[crate::hir::ResolvedInterface] {
            &self.0.interfaces
        }
        fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
            crate::hir::items::View::slice(&self.0.functions)
        }
        fn function_templates(&self) -> &[crate::hir::ResolvedFunctionTemplate] {
            &self.0.function_templates
        }
        fn function_instances(&self) -> &[crate::hir::ResolvedFunctionInstance] {
            &self.0.function_instances
        }
    }
    let programs = fixtures();
    let program = &programs[0];
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "envelope.consume")
        .unwrap();
    let ty = &function.cleanup.slots[0].ty;
    let mut budget = ReplayBudget::new();
    let mut receiver = nested_shape::PaidShapeReceiver::new(&mut budget);
    let mut flags = 0;
    drop(receiver.derive(program, function, ty, &mut flags).unwrap());
    let capacities = receiver.capacities();
    let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        drop(receiver.derive(&Interrupted(program), function, ty, &mut flags));
    }));
    assert!(interrupted.is_err());
    assert!(receiver.settled());
    assert_eq!(capacities, receiver.capacities());
    drop(receiver.derive(program, function, ty, &mut flags).unwrap());
    assert!(receiver.settled());
}
