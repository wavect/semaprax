//! Complete measured status-fact component before the additive prefix route.
use super::{
    capacity, collect_leaves, errors, physical_storage, status_before, status_before_prefix,
    validate_blocks_and_edges_retained, validate_slots, BTreeSet, Diagnostic, Facts, Leaves,
    ReplayBudget, ResolvedFunction, ResolvedProgram, StorageId, MAX_REPLAY_WORK_UNITS,
};
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::measure;

#[allow(clippy::too_many_arguments)]
fn receive(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    facts: &Facts<'_>,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
    current: bool,
) -> Result<(), Diagnostic> {
    let statuses = if current {
        status_before_prefix::validate_status_sources_retained(function, facts, budget)?
    } else {
        status_before::validate_status_sources_retained(function, facts, budget)?
    };
    validate_blocks_and_edges_retained(
        program, function, facts, storage, leaves, None, budget, &statuses,
    )
}

// HIR/facts are existing immutable inputs. Every receiver allocation, result,
// retained token and budget drops while the actual observer is still active.
pub(super) fn observe(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    current: bool,
    work_limit: usize,
    byte_limit: usize,
    expected: Option<&Result<(), Diagnostic>>,
) -> (bool, usize, usize, bool, Option<ReservationRefusal>, usize) {
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        super::super::super::arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        )
        .unwrap(),
    );
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let ((value, allocation), refused, debit, first) = with_limit_usage_refusal(byte_limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            budget.remaining = work_limit;
            let result = receive(
                program,
                function,
                &facts,
                &storage,
                &leaves,
                &mut budget,
                current,
            );
            if let Some(expected) = expected {
                assert_eq!(&result, expected);
            }
            let success = result.is_ok();
            if !success && work_limit < MAX_REPLAY_WORK_UNITS && byte_limit == 1 << 26 {
                assert_eq!(result.as_ref().unwrap_err().code, "SPX-H006");
                assert_eq!(budget.remaining, 0);
            }
            let used = work_limit - budget.remaining;
            drop(result);
            if !success && (work_limit < MAX_REPLAY_WORK_UNITS || byte_limit < 1 << 26) {
                let remaining = budget.remaining;
                let bytes = crate::bounded_output::active_remaining();
                let retry = receive(
                    program,
                    function,
                    &facts,
                    &storage,
                    &leaves,
                    &mut budget,
                    current,
                );
                assert!(retry.is_err());
                assert_eq!(budget.remaining, remaining);
                assert_eq!(crate::bounded_output::active_remaining(), bytes);
                drop(retry);
            }
            drop(budget);
            (success, used)
        })
    });
    assert!(!allocation.invalid);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
    (
        value.0,
        value.1,
        debit,
        refused,
        first,
        allocation.allocated,
    )
}
