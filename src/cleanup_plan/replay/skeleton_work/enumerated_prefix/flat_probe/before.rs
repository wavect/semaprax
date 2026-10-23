//! Narrow transport for the complete ordinary HIR flat-representation probe.
//! Traversal, work units, sticky failures and all dynamic refusal text are unchanged.
use super::*;
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
const PAYMENT: usize = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * size_of::<Result<(), ByteFailure>>();
pub(super) const CALL: usize = 2
    * size_of::<(&[SkeletonPath], &ResolvedFunction, &mut ReplayBudget)>()
    + 2 * size_of::<Result<bool, ByteFailure>>()
    + 2 * size_of::<std::slice::Iter<'static, SkeletonPath>>();
pub(super) fn all_flat(
    paths: &[SkeletonPath],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, ByteFailure> {
    // Root dispatch excludes borrowed-work ownership; a direct caller must
    // likewise reject it before spending a new metadata traversal debit.
    if budget.lent_skeleton {
        return Err(budget.compact_refusal(
            function,
            "enumerated HIR flat probe cannot spend a lent skeleton allowance",
            "",
        ));
    }
    let mut pending = paths.iter();
    loop {
        budget.reserve_compact_bytes(
            function,
            2 * PAYMENT
                + 2 * size_of::<&mut std::slice::Iter<'static, SkeletonPath>>()
                + 2 * size_of::<Option<&SkeletonPath>>()
                + 2 * size_of::<&SkeletonPath>()
                + 2 * size_of::<&Option<NodeRc>>()
                + 2 * size_of::<bool>(),
            "enumerated HIR flat representation probe receivers",
        )?;
        budget.charge_compact(function, 1, "enumerated HIR flat representation probe")?;
        let Some(path) = pending.next() else {
            return Ok(true);
        };
        if path.observations.prefix.is_some() {
            return Ok(false);
        }
    }
}

const _: () = assert!(
    super::PAYMENT - PAYMENT
        == 2 * (size_of::<Result<(), Diagnostic>>() - size_of::<Result<(), ByteFailure>>())
);
const _: () = assert!(
    super::FLAT_CALL - CALL
        == 2 * (size_of::<Result<bool, Diagnostic>>() - size_of::<Result<bool, ByteFailure>>())
);
