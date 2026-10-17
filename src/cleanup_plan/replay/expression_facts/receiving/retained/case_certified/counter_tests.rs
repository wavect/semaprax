//! Complete immediate prior certified receiver, unchanged counter fees/work.
use super::{counter_before, validate_status_sources_certified};
use crate::cleanup_plan::replay::expression_facts::receiving::retained::{Facts, StatusCases};
use crate::cleanup_plan::replay::{ReplayBudget, MAX_REPLAY_WORK_UNITS};

#[test]
fn certified_while_counter_preserves_complete_before_fields_work_and_receiving_debit() {
    let ast = crate::check(
        r#"module counter.status;
@id("counter.add") fn add(x:i64)->i64 requires x>=0 ensures result>=0 {x+1}
@id("counter.main") fn main()->i64 {let a=add(1)+3-2*4/2%3;let b=add(2)+3;-1}
"#,
        "counter-status.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        crate::cleanup_plan::replay::expression_facts::arena::build::<
            crate::cleanup_plan::replay::expression_facts::errors::Legacy,
        >(
            function,
            crate::cleanup_plan::replay::expression_facts::capacity::Payer::Selected(
                &mut crate::cleanup_plan::replay::physical_storage::PhysicalReservation::new(
                    usize::MAX,
                ),
            ),
            &mut preparation,
        )
        .unwrap(),
    );
    let observe = |old, work, bytes| {
        let mut budget = match ReplayBudget::with_physical(function, bytes) {
            Ok(budget) => budget,
            Err(error) => return (Err(error), None),
        };
        budget.remaining = work;
        let result = (|| {
            let proof = StatusCases::validate(&program, function, &mut budget)?;
            let sources = if old {
                counter_before::validate_status_sources_certified(
                    function,
                    &facts,
                    &mut budget,
                    &proof,
                )?
            } else {
                validate_status_sources_certified(function, &facts, &mut budget, &proof)?
            };
            drop(sources);
            drop(proof);
            Ok::<_, crate::diagnostic::Diagnostic>(())
        })();
        (
            result,
            Some((
                budget.remaining,
                budget.skeleton_remaining,
                budget.physical.as_ref().unwrap().charged(),
            )),
        )
    };
    let complete = observe(true, MAX_REPLAY_WORK_UNITS, 1 << 24);
    assert!(complete.0.is_ok());
    let (remaining, _, paid) = complete.1.unwrap();
    let spent = MAX_REPLAY_WORK_UNITS - remaining;
    for work in 0..=spent {
        assert_eq!(
            observe(false, work, 1 << 24),
            observe(true, work, 1 << 24),
            "all six error fields and exact byte/work counters at each work cut"
        );
    }
    for bytes in 0..=paid {
        assert_eq!(
            observe(false, MAX_REPLAY_WORK_UNITS, bytes),
            observe(true, MAX_REPLAY_WORK_UNITS, bytes),
            "all six error fields and exact counters at every physical cut"
        );
    }
    let saved = include_str!("counter_before.rs")
        .replacen("use super::super::{", "use super::{", 1)
        .replacen(
            "super::super::super::super::super::validate_status_sources",
            "super::super::super::super::validate_status_sources",
            1,
        );
    assert_eq!(saved, include_str!("../../../../../../../docs/evidence/status-case-counter-828f/complete-certified-before.rs.txt"));
}


#[test]
fn certified_unobserved_wide_dispatch_keeps_complete_rows_and_original_work_ceiling() {
    // Real HIR discovery must authenticate these rows: synthetic status tables
    // would not exercise the current certified production receiver.
    let prefix = "ordinary.shared.".repeat(4);
    let main_id = format!("{prefix}main");
    let mut source = format!(
        "module ordinary.wide;\n@id(\"wide.add\") fn add(x:i64)->i64 requires x>=0 ensures result>=0 {{x+1}}\n@id(\"{main_id}\") fn main()->i64 {{\n"
    );
    for n in 0..512 {
        source.push_str(&format!("let v{n}=add(0)+3-2*4/2%3;\n"));
    }
    source.push_str("-1}\n");
    let ast = crate::check(&source, "ordinary-wide-certified.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(function.cleanup_plan.status_sources.len() >= 3000);
    let mut expected = function
        .cleanup_plan
        .status_sources
        .iter()
        .map(|source| &source.id)
        .collect::<Vec<_>>();
    expected.sort();
    assert!(expected
        .iter()
        .all(|id| id.expression.as_str().len() > prefix.len()));
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        crate::cleanup_plan::replay::expression_facts::arena::build::<
            crate::cleanup_plan::replay::expression_facts::errors::Legacy,
        >(
            function,
            crate::cleanup_plan::replay::expression_facts::capacity::Payer::Selected(
                &mut crate::cleanup_plan::replay::physical_storage::PhysicalReservation::new(
                    usize::MAX,
                ),
            ),
            &mut preparation,
        )
        .unwrap(),
    );
    let observe = |old| {
        let ((ok, remaining), allocation) = crate::test_allocation_observer::measure(|| {
            let mut budget = ReplayBudget::unobserved();
            assert!(matches!(
                budget.payer,
                crate::cleanup_plan::replay::allocation::AllocationPayer::LegacyUnobserved
            ));
            let result = (|| {
                let proof = StatusCases::validate(&program, function, &mut budget)?;
                let sources = if old {
                    counter_before::validate_status_sources_certified(
                        function,
                        &facts,
                        &mut budget,
                        &proof,
                    )?
                } else {
                    validate_status_sources_certified(function, &facts, &mut budget, &proof)?
                };
                let ids = sources.ids.as_ref().unwrap();
                assert_eq!(ids.len(), expected.len());
                assert!(ids.keys().copied().eq(expected.iter().copied()));
                drop(sources);
                drop(proof);
                Ok::<_, crate::diagnostic::Diagnostic>(())
            })();
            let ok = result.is_ok();
            drop(result);
            let remaining = budget.remaining;
            drop(budget);
            (ok, remaining)
        });
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(
            allocation.allocated, allocation.freed,
            "actual receiver, proof, diagnostic and last Drop stay observed"
        );
        (ok, remaining)
    };
    let before = observe(true);
    let current = observe(false);
    assert!(
        current.0,
        "actual certified unobserved route fits the unchanged ceiling"
    );
    assert!(current.1 > 0 && current.1 < MAX_REPLAY_WORK_UNITS);
    assert!(
        current.1 > before.1,
        "the actual held-prefix dispatch saves work against the complete counter receiver"
    );
}
