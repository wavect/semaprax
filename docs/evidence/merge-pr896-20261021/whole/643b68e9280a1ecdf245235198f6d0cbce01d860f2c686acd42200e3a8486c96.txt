//! Direct ordinary constructor outputs. Collector order is the immediate old
//! constructor's; no common traversal or producer verdict is reused.
use super::*;
use crate::cleanup_plan::replay::ordinary_receiving::{
    prepare_into, refuse_into, reserve_into, Failure,
};
use crate::hir::funded::FundedError;
use std::mem::size_of;

// On failure, struct fields drop in the legacy locals' reverse declaration
// order. Successful assembly moves them into the unchanged SkeletonWork order.
pub(in crate::cleanup_plan::replay) struct Prepared<'a> {
    pub(in crate::cleanup_plan::replay) owner: Option<crate::hir::FunctionExecutionId>,
    pub(in crate::cleanup_plan::replay) requires: bool,
    pub(in crate::cleanup_plan::replay) reads: BTreeSet<ExpressionId>,
    pub(in crate::cleanup_plan::replay) moves: BTreeSet<ExpressionId>,
    pub(in crate::cleanup_plan::replay) renewals:
        crate::cleanup_plan::renewal_profile::RenewalBindings<'a>,
}
// Complete new output/error owners, construction return/receive, actual/formal
// output loans, explicit callback captures and small funded result transports.
// No original Result or local-header fee is subtracted from this expression.
pub(in crate::cleanup_plan::replay) const fn acquisition_bytes() -> usize {
    size_of::<Prepared<'_>>()
        + crate::cleanup_plan::renewal_profile::RenewalBindings::empty_receiving_bytes()
        + size_of::<Failure<'_>>()
        + Failure::terminal_bytes()
        + 2 * size_of::<(
            &dyn crate::cleanup::ReplayContext,
            &ResolvedFunction,
            &mut ReplayBudget,
            bool,
            &mut Prepared<'_>,
            &mut Failure<'_>,
            bool,
        )>()
        + 2 * size_of::<(
            &mut ReplayBudget,
            &ResolvedFunction,
            &mut Failure<'_>,
            &crate::bounded_output::classifier_string::ClassifierLedger,
        )>()
        + size_of::<(
            &mut ReplayBudget,
            &mut Failure<'_>,
            &crate::bounded_output::classifier_string::ClassifierLedger,
        )>()
        + 2 * size_of::<&Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<crate::bounded_output::classifier_string::ClassifierLedger>()
        + 10 * size_of::<Result<(), FundedError<()>>>()
        // Frozen c86 conservative reserve-callback invoice, retained unchanged.
        // Not ten extra physically formed refs; the following new actual/formal
        // collector tuples separately describe their12 callback reference roles.
        + 10 * size_of::<&mut dyn FnMut(usize) -> Result<(), ()>>()
        + 2 * size_of::<(
            &dyn crate::cleanup::ReplayContext,
            &ResolvedFunction,
            Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
            &mut crate::cleanup_plan::renewal_profile::RenewalBindings<'_>,
            &mut dyn FnMut(usize) -> Result<(), ()>,
            &mut dyn FnMut(usize) -> Result<(), ()>,
        )>()
        + 4 * size_of::<(
            &ResolvedFunction,
            &mut BTreeSet<ExpressionId>,
            &mut dyn FnMut(usize) -> Result<(), ()>,
        )>()
        + 2 * size_of::<(
            &ResolvedFunction,
            &mut bool,
            &mut dyn FnMut(usize) -> Result<(), ()>,
        )>()
        + 2 * size_of::<(
            &dyn crate::cleanup::ReplayContext,
            &ResolvedFunction,
            &mut Option<crate::hir::FunctionExecutionId>,
            &mut dyn FnMut(usize) -> Result<(), ()>,
        )>()
}

pub(in crate::cleanup_plan::replay) fn prepare_into<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    indexed: bool,
    output: &mut Prepared<'hir>,
    slot: &mut Failure<'_>,
) -> bool {
    if slot.error.is_some() {
        return false;
    }
    // Same one captured ledger and same per-callback current() frontier as the
    // immediate old constructor. The lookup successor instead retains its
    // existing HeldPrefix/frozen-directory guard and exclusive budget loan.
    let ledger = crate::bounded_output::classifier_string::ClassifierLedger::capture();
    let payer = &mut *budget;
    let destination = &mut *slot;
    let held_ledger = &ledger;
    let mut reserve = move |bytes| {
        if !held_ledger.current() {
            refuse_into!(
                payer,
                function,
                destination,
                "ordinary replay setup payer is outside its current invocation",
                ""
            );
            return Err(());
        }
        if reserve_into!(
            payer,
            function,
            destination,
            bytes,
            "ordinary skeleton constructor inventory"
        ) {
            Ok(())
        } else {
            Err(())
        }
    };

    macro_rules! inventory {
        ($operation:expr) => {{
            let result = $operation;
            match result {
                Ok(()) => {}
                Err(FundedError::Refused(())) => return false,
                Err(
                    FundedError::Layout(reason)
                    | FundedError::Census(reason)
                    | FundedError::Unfunded(reason),
                ) => {
                    return refuse_into!(budget, function, slot, reason, "");
                }
            }
        }};
    }
    if !indexed {
        inventory!(crate::cleanup_plan::renewal_profile::bindings_into(
            program,
            function,
            None,
            &mut output.renewals,
            &mut reserve,
            &mut |_| Ok(())
        ));
    }
    inventory!(crate::literal_format::moving_into(
        function,
        &mut output.moves,
        &mut reserve
    ));
    inventory!(crate::string_ops::conditions::reads_into(
        function,
        &mut output.reads,
        &mut reserve
    ));
    inventory!(crate::hir::lexical_exit::requires_into(
        function,
        &mut output.requires,
        &mut reserve
    ));
    if output.requires {
        inventory!(crate::hir::lexical_exit::owner_into(
            program,
            function,
            &mut output.owner,
            &mut reserve
        ));
    }
    true
}
