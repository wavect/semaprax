//! Test-only Oracle-payer construction of `SkeletonWork`. Chosen only when the
//! budget's payer is an explicit oracle lease; every ordinary constructor is
//! unchanged. The live program/function pair must hold the scoped invocation
//! authority for that same lease, otherwise construction fails closed. The
//! normal refusal text and the 514-slot clone cursor are prepaid on the account
//! first; each inventory is then built by its funded twin on that account. Any
//! account, physical or auth refusal returns as the replay refusal: it is never
//! `None`, `false` or an empty inventory, and nothing reads the ACTIVE ledger.
use super::*;
use crate::cleanup_plan::replay::allocation::{authority, OracleLease, OracleRefusal};
use crate::hir::funded::FundedError;

fn refused(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    error: FundedError<OracleRefusal>,
) -> Diagnostic {
    let reason = match error {
        FundedError::Refused(OracleRefusal::Exhausted { .. }) => {
            "oracle account exhausted at skeleton entry"
        }
        FundedError::Refused(OracleRefusal::Sticky) => {
            "oracle account sticky refusal at skeleton entry"
        }
        FundedError::Refused(_) => "oracle account refused at skeleton entry",
        FundedError::Layout(reason)
        | FundedError::Census(reason)
        | FundedError::Unfunded(reason) => reason,
    };
    budget.budget_refusal(function, reason, "")
}

impl<'a, 'b> SkeletonWork<'a, 'b> {
    pub(super) fn oracle_with_index(
        program: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        budget: &'b mut ReplayBudget,
        index: Option<&'b crate::loop_calls::byte_writer::ModuleIndex<'a>>,
        lease: OracleLease,
    ) -> Result<Self, Diagnostic> {
        if !matches!(authority(program, function), Ok(Some(held)) if held == lease) {
            return Err(budget.budget_refusal(
                function,
                "oracle invocation authority differs from the live subject",
                "",
            ));
        }
        budget.prepare_builder(function)?;
        budget.prepare_clone_cursor(function)?;
        let mut pay = |bytes: usize| lease.charge(bytes);
        let mut units = |work: usize| lease.charge_work(work);
        let renewal_bindings = if index.is_some() {
            crate::cleanup_plan::renewal_profile::RenewalBindings::empty()
        } else {
            match crate::cleanup_plan::renewal_profile::bindings_with_index_funded_with_work(
                program, function, None, &mut pay, &mut units,
            ) {
                Ok(bindings) => bindings,
                Err(error) => return Err(refused(budget, function, error)),
            }
        };
        let string_owner_moves =
            match crate::literal_format::checked_moving_string_operands_funded_with_work(
                function, &mut units, &mut pay,
            ) {
                Ok(moves) => moves,
                Err(error) => return Err(refused(budget, function, error)),
            };
        let string_condition_reads =
            match crate::string_ops::conditions::function_reads_funded_with_work(
                function, &mut units, &mut pay,
            ) {
                Ok(reads) => reads,
                Err(error) => return Err(refused(budget, function, error)),
            };
        let requires = match crate::hir::lexical_exit::requires_funded_with_work(
            function, &mut pay, &mut units,
        ) {
            Ok(requires) => requires,
            Err(error) => return Err(refused(budget, function, error)),
        };
        // A semantic missing or ambiguous owner is the original absence; a
        // payer refusal never is.
        let owner = if requires {
            match crate::hir::lexical_exit::owner_funded_with_work(
                program, function, &mut pay, &mut units,
            ) {
                Ok(Ok(owner)) => Some(owner),
                Ok(Err(_)) => None,
                Err(error) => return Err(refused(budget, function, error)),
            }
        } else {
            None
        };
        Ok(Self {
            function,
            budget,
            continuations: None,
            active_copy_region: None,
            dag_scratch: None,
            ordinary_scratch: None,
            next_ordinary_dag: 0,
            held_renewals: None,
            ordinary_index: index,
            ordinary_program: Some(program),
            renewal_bindings,
            string_owner_moves,
            string_condition_reads,
            owner,
        })
    }
}
