use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleReceipt, OracleRefusal, OracleScope};
use crate::test_allocation_observer::measure;

fn control(program: &ResolvedProgram, ty: &ResolvedType) {
    let function = &program.functions[0];
    let expected_flags = program.declarations.type_flags(ty);
    let expected_drop = crate::cleanup::type_needs_resource_cleanup(program, ty)
        .map_err(|message| replay_error(function, message));
    let run = |bytes, work| {
        let ((matched, retry, receipt), metrics) = measure(|| {
            let scope = OracleScope::open_with_work(bytes, work).unwrap();
            let lease = scope.lease();
            let invocation = enter(program, function, lease).unwrap();
            let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
            let classified = super::super::flags(&program.declarations, function, ty, &mut budget);
            let cleanup = super::super::needs_drop(program, function, ty, &mut budget);
            let matched = matches!(&classified, Ok(value) if value == &expected_flags)
                && cleanup == expected_drop;
            drop(classified);
            drop(cleanup);
            drop(budget);
            let retry = lease.charge(0);
            assert_eq!(invocation.exit(), Ok(()));
            (matched, retry, scope.finish())
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(
            metrics.allocated <= receipt.charged,
            "{metrics:?} {receipt:?}"
        );
        (matched, retry, receipt)
    };
    let (matched, retry, baseline) = run(1 << 20, 1 << 25);
    assert!(matched && retry == Ok(()));
    assert!(baseline.charged > 0 && baseline.work > 0);
    assert!(!refused(baseline));
    let (matched, retry, exact) = run(baseline.charged, baseline.work);
    assert!(matched && retry == Ok(()));
    assert_eq!(exact.charged, baseline.charged);
    assert_eq!(exact.work, baseline.work);
    for (bytes, work) in [
        (baseline.charged - 1, baseline.work),
        (0, baseline.work),
        (baseline.charged, baseline.work - 1),
        (baseline.charged, 0),
    ] {
        let (matched, retry, receipt) = run(bytes, work);
        assert!(
            !matched,
            "physical refusal cannot publish semantic equality"
        );
        assert_eq!(retry, Err(OracleRefusal::Sticky));
        assert!(refused(receipt));
    }
}
fn refused(receipt: OracleReceipt) -> bool {
    receipt.first_refusal.is_some()
        || receipt.first_work_refusal.is_some()
        || receipt.physical_failure.is_some()
}

#[test]
fn leased_full_inventory_flags_and_cleanup_preserve_original_receivers_and_exact_paid_drop() {
    for mut program in super::super::tests::fixtures() {
        // Exact full original uncached classifier and outer receiver remain
        // independent; current Oracle routing changes only their payer/fold.
        program.declarations = program.declarations.classifier_uncached_for_test();
        let mut roots = Vec::new();
        for item in &program.types {
            roots.push(ResolvedType::Nominal {
                declaration: item.id.clone(),
                arguments: Vec::new(),
            });
        }
        for f in &program.functions {
            roots.push(f.return_type.clone());
            roots.extend(f.params.iter().map(|p| p.ty.clone()));
        }
        roots.extend([
            ResolvedType::Never,
            ResolvedType::I64,
            ResolvedType::Bytes,
            ResolvedType::Str,
            ResolvedType::MutFunctionI64,
            ResolvedType::ArrayU8(17),
        ]);
        for root in roots {
            control(&program, &root);
        }
    }
}

#[test]
fn leased_inventory_keeps_hostile_primitive_cache_precedence_and_full_generic_membership() {
    let base = super::super::tests::fixtures().into_iter().next().unwrap();
    for root in [
        ResolvedType::I64,
        ResolvedType::Bytes,
        ResolvedType::Str,
        ResolvedType::MutFunctionI64,
        ResolvedType::Never,
    ] {
        let mut program = base.clone();
        program.declarations.inventory_insert_fact_for_test(
            &root,
            crate::hir::TypeFacts {
                copy: false,
                contains_resource: true,
                sized: false,
                needs_drop: true,
                layout_key: "hostile original cache layout".to_owned(),
            },
        );
        control(&program, &root);
    }
    for width in [0, 1, 17] {
        let root = ResolvedType::Nominal {
            declaration: DeclarationId::new("unknown.oracle.é\0full"),
            arguments: vec![
                ResolvedType::Function {
                    parameters: vec![ResolvedType::I64],
                    result: Box::new(ResolvedType::Bytes),
                };
                width
            ],
        };
        control(&base, &root);
    }
}
