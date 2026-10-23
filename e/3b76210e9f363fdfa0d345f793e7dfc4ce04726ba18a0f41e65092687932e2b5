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
// The frame is held for this nonrecursive invocation. No iterator, path or
// budget reference escapes, and neither primitive calls user code or recurses.
// Each primitive result is consumed before reusing its receiving slots.
struct Frame<'a> {
    pending: std::slice::Iter<'a, SkeletonPath>,
    next: Option<&'a SkeletonPath>,
    dag: bool,
}
const FRAME: usize = 3 * PAYMENT // initial reservation, zero authentication, work
    + 2 * size_of::<Frame<'static>>() // aggregate initializer and retained frame
    + 2 * size_of::<&mut std::slice::Iter<'static, SkeletonPath>>()
    + 2 * size_of::<Option<&SkeletonPath>>() // next return and assignment
    + 2 * size_of::<&SkeletonPath>() // successful pattern and retained path
    + 2 * size_of::<&Option<NodeRc>>() // field borrow and is_some receiver
    + 4 * size_of::<bool>(); // field initializer, query return, field and branch
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
    budget.reserve_compact_bytes(
        function,
        FRAME,
        "enumerated HIR flat representation probe receivers",
    )?;
    let mut frame = Frame {
        pending: paths.iter(),
        next: None,
        dag: false,
    };
    loop {
        // Keep the same sticky payer/floor authentication before every work
        // event, even though the invocation still owns its fixed frame bank.
        budget.reserve_compact_bytes(
            function,
            0,
            "enumerated HIR flat representation probe receivers",
        )?;
        budget.charge_compact(function, 1, "enumerated HIR flat representation probe")?;
        frame.next = frame.pending.next();
        let Some(path) = frame.next else {
            return Ok(true);
        };
        frame.dag = path.observations.prefix.is_some();
        if frame.dag {
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

#[cfg(test)]
#[path = "flat_probe/receiving_tests.rs"]
mod receiving_tests;
