//! Paid selected path-state and transition family, never new replay authority.
use super::*;
use crate::cleanup_plan::replay::expression_facts::errors::EngineError as Error;
use crate::cleanup_plan::replay::lexical_cert::LexicalAuth;
use crate::cleanup_plan::replay::selected_context::child::Permit;
use crate::cleanup_plan::replay::structural_control::selected::{
    dynamic_get, dynamic_next, existing_structural_error,
};
use crate::cleanup_plan::{
    CLEANUP_PLAN_SCHEMA_V16, CLEANUP_PLAN_SCHEMA_V17, CLEANUP_PLAN_SCHEMA_V18,
    CLEANUP_PLAN_SCHEMA_V19,
};
macro_rules! path_child {
    ($budget:ident,$function:ident,$bank:ident,$fields:expr,$call:expr) => {
        receive!(
            $budget,
            $function,
            receiving::$bank<'_>,
            $fields,
            "selected path child call receiving",
            $call
        )
    };
}
macro_rules! retained_child {
    ($budget:ident,$function:ident,$bank:ident,$fields:expr,$call:expr) => {
        receive!(
            $budget,
            $function,
            receiving::$bank<'_>,
            $fields,
            "selected retained child call receiving",
            $call
        )
    };
}
macro_rules! retained_insert {
    ($budget:ident,$function:ident,$bank:ident,$fields:expr,$call:expr) => {
        receive!(
            $budget,
            $function,
            receiving::$bank<'_>,
            $fields,
            "selected retained insertion call receiving",
            $call
        )
    };
}
// A typed nested reason reuses the reviewed control receipt chain. This
// lexical constructor introduces no forwarding function, receiver or result ABI.
macro_rules! path_error {
    ($permit:ident,$reason:expr) => {
        $permit.control(
            crate::cleanup_plan::replay::structural_control::selected::reason::Reason::Path(
                $reason,
            ),
        )
    };
}
// Literal closure/result admission occurs before the callback is formed.
// Static variants capture only the already acquired permit reference.
macro_rules! path_refusal {
    ($budget:ident,$function:ident,$permit:ident,$value:ty,$kind:expr) => {{
        $budget.reserve_array::<(&Permit<'_>, Result<$value, Error>)>(
            $function,
            1,
            "selected path closure receiving",
        )?;
        $budget.charge($function, 2, "selected path closure receiving")?;
        || path_error!($permit, Reason::Finite($kind))
    }};
}
pub(in crate::cleanup_plan::replay) mod reason;
use crate::cleanup_plan::replay::inventory_coverage::paid::receive;
use reason::{Kind, Reason};
mod execution;
mod finish;
mod history;
mod joins;
mod lexical_flow;
mod lookup;
mod queries;
mod receiving;
use queries::{
    charge_retained_units, clone_state, clone_value, control_key, counter_at, equal_without_flag,
    exact_join, filtered_flag_next, find, flag_all, flag_any, flag_at, group_at, incoming_at,
    index, index_mut, leaf_at, lexical_lookup, map_index, map_insert, mutable_get,
    owned_provenance, pair_at, place_flags, plan_at, retain_flags, retained_units, scan,
    selected_renewals, slice_index, variant_any, variant_position,
};
mod renewal;
mod strings;
mod walk;
pub(in crate::cleanup_plan::replay) use walk::validate_path_states;

// Caller-side complete argument/result bank precedes the new selected helper.
pub(in crate::cleanup_plan::replay) type CallReceiving<'a> = (
    &'a dyn ReplayContext,
    &'a ResolvedFunction,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    &'a mut ReplayBudget,
    Option<&'a LexicalAuth>,
    Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    Result<(), Error>,
);

type Successors = std::iter::Flatten<std::array::IntoIter<Option<BlockId>, 2>>;
fn successors(
    function: &ResolvedFunction,
    block: BlockId,
    budget: &mut ReplayBudget,
) -> Result<Successors, Error> {
    // Actual body/iterator/capture roles; original nested backing and
    // explicit insertion/payload receiving banks remain separately intact.
    budget.reserve_array::<(
        &crate::cleanup_plan::CleanupPlan,
        &crate::cleanup_plan::CleanupTerminator,
        &EdgeId,
        &Vec<EdgeId>,
        &crate::cleanup_plan::ExitTargetId,
        [Option<BlockId>; 2],
        std::array::IntoIter<Option<BlockId>, 2>,
        Successors,
    )>(function, 1, "selected path successors body receiving")?;
    budget.charge(function, 8, "selected path successors body receiving")?;
    budget.reserve_array::<(
        &ResolvedFunction,
        BlockId,
        &mut ReplayBudget,
        Result<Successors, Error>,
        Diagnostic,
        Error,
        Diagnostic,
        Error,
    )>(function, 1, "selected path successors formal receiving")?;
    budget.charge(function, 8, "selected path successors formal receiving")?;

    let plan = &function.cleanup_plan;
    let values = match &plan_at!(budget, function, plan, blocks, block.0 as usize).terminator {
        CleanupTerminator::Goto(edge) => [
            Some(plan_at!(budget, function, plan, edges, edge.0 as usize).to),
            None,
        ],
        CleanupTerminator::Branch(edges) => [
            Some(
                plan_at!(
                    budget,
                    function,
                    plan,
                    edges,
                    index!(budget, function, edges, 0, EdgeId).0 as usize
                )
                .to,
            ),
            Some(
                plan_at!(
                    budget,
                    function,
                    plan,
                    edges,
                    index!(budget, function, edges, 1, EdgeId).0 as usize
                )
                .to,
            ),
        ],
        CleanupTerminator::Exit(exit) => {
            match plan_at!(budget, function, plan, exits, exit.0 as usize).continuation {
                ExitContinuation::Continue(edge)
                | ExitContinuation::LoopBreak { edge, .. }
                | ExitContinuation::FunctionReturn { edge, .. } => [
                    Some(plan_at!(budget, function, plan, edges, edge.0 as usize).to),
                    None,
                ],
                ExitContinuation::LoopLatch { .. } => [
                    receive!(
                        budget,
                        function,
                        (
                            &crate::cleanup_plan::CleanupPlan,
                            crate::cleanup_plan::ExitTargetId,
                            Option<BlockId>
                        ),
                        3,
                        "selected successor latch lookup receiving",
                        lexical_cert::latch_resume(plan, *exit)
                    ),
                    None,
                ],
                ExitContinuation::CommitResult { .. }
                | ExitContinuation::ReturnFailure { .. }
                | ExitContinuation::ReturnUnit => [None, None],
            }
        }
    };
    Ok(values.into_iter().flatten())
}

#[cfg(test)]
mod append_before_formals;
#[cfg(test)]
mod tests;
