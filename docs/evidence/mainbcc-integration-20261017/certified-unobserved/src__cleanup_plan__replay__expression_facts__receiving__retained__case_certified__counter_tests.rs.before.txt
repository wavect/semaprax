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
