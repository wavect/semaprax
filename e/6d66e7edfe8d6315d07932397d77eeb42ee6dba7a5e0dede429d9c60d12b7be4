//! Paid ordinary frontier controls and transport of the existing allocation.
use super::*;
use std::mem::size_of;

// These banks cover the new directory, selector, iterator, partial-Drop and
// call receiving roles before they are formed. The complete original backing
// and all original work remain separately charged by their existing owners.
pub(super) fn reserve_directory(
    function: &ResolvedFunction,
    count: usize,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    budget.reserve_bytes(
        function,
        size_of::<(
            &ResolvedFunction,
            usize,
            &mut ReplayBudget,
            Result<(), Diagnostic>,
            Vec<frontier::Incoming<PathState>>,
            &mut Vec<frontier::Incoming<PathState>>,
            fn() -> frontier::Incoming<PathState>,
        )>() + HELD_EDGE_TERMINAL,
        "ordinary frontier directory receiving",
    )?;
    budget.reserve_array::<(
        frontier::Incoming<PathState>,
        frontier::Incoming<PathState>,
        frontier::Incoming<PathState>,
        &mut fn() -> frontier::Incoming<PathState>,
        // Both None field aggregates form on every cold default. No empty
        // BTreeSet constructor is executed or credited for a singleton.
        Option<Box<PathState>>,
        Option<Box<BTreeSet<Box<PathState>>>>,
        // A validated acyclic CFG reaches each block at most once. These
        // original block selector loans/scalars are moved ahead of selection,
        // not refunded; all directory slots retain their unused allowance.
        &Option<Box<PathState>>,
        &Option<Box<PathState>>,
        usize,
        usize,
        usize,
        usize,
        bool,
        bool,
        // One selector per acyclic block, including unused directory slots.
        // Queue actual/formal receivers, field place loans, indices and all
        // short-circuit predicates are paid before the first selection.
        &VecDeque<BlockId>,
        &VecDeque<BlockId>,
        &Option<Box<PathState>>,
        &Option<Box<BTreeSet<Box<PathState>>>>,
        &usize,
        &usize,
        usize,
        usize,
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
        bool,
    )>(
        function,
        count,
        "ordinary frontier repeated default receiving",
    )
}

// Direct lexical producers remove the two forwarding Result wrappers and
// owned Box/Result<Box> edge boundary. Direct singleton fields also remove the
// frontier iterator/forwarded next and consuming IntoIter carriers. Every
// remaining Box, Incoming, authority, selector, and terminal bank is retained.
// All non-frontier old block roles survive. The per-directory selector bank
// precedes the nonbinding One field pattern and is retained for every slot.
const BLOCK_COMMON: usize = 2 * size_of::<&BTreeSet<Box<PathState>>>()
    + 8 * size_of::<Box<PathState>>()
    + 6 * size_of::<&Box<PathState>>()
    + 4 * size_of::<&PathState>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 4 * size_of::<bool>()
    + size_of::<std::iter::Empty<&PathState>>()
    + size_of::<BlockId>()
    + size_of::<(
        &ResolvedFunction,
        &EdgeCondition,
        &BTreeMap<ExpressionId, StatusSourceId>,
        &mut ReplayBudget,
        &ExpressionId,
        &StatusSourceId,
        StatusSourceId,
        Option<&StatusSourceId>,
        bool,
    )>();

// Same local Box, complete unchanged non-frontier block controls. No
// incoming Option take or mutable Incoming receiver exists on this route.
pub(super) const CARRY_BLOCK: usize =
    BLOCK_COMMON + 2 * size_of::<BlockId>() + 2 * size_of::<bool>() + SINGLE_HISTORY_SELECT;

// One consumes only its Option field. The nonbinding Some pattern removes
// expect's owning formal and returned Box; its Box binding uses the unchanged
// Box bank above. The old whole-Incoming take/default/result carriers are gone.
pub(super) const BLOCK_ONE: usize = BLOCK_COMMON
    + SINGLE_HISTORY_SELECT
    + 4 * size_of::<Option<Box<PathState>>>()
    + 2 * size_of::<&mut Option<Box<PathState>>>()
    + size_of::<&mut frontier::Incoming<PathState>>();
// Many takes only its optional boxed tree and moves the tree into the
// original IntoIter. The Box allocation was paid at promotion and is retired
// here; no fresh empty tree is constructed during take.
pub(super) const BLOCK_MANY: usize = BLOCK_COMMON
    // join_units and both possible incoming_len expressions: four optional
    // field/pattern loans; three Box deref actual/formal pairs plus two named
    // pattern bindings. The exact-join second len remains conservatively paid.
    + 4 * size_of::<&Option<Box<BTreeSet<Box<PathState>>>>>()
    + 8 * size_of::<&Box<BTreeSet<Box<PathState>>>>()
    + 4 * size_of::<Option<Box<BTreeSet<Box<PathState>>>>>()
    + 2 * size_of::<&mut Option<Box<BTreeSet<Box<PathState>>>>>()
    + 2 * size_of::<Box<BTreeSet<Box<PathState>>>>()
    + 4 * size_of::<BTreeSet<Box<PathState>>>()
    + size_of::<&mut frontier::Incoming<PathState>>();
// Group replay is unchanged, so retain its ENTIRE original block price,
// including its conservative four Incoming carriers. This exact old enum
// declaration is a layout-only price anchor; it is never constructed at runtime
// and cannot grant storage or a credit to the new directory representation.
#[allow(dead_code, reason = "retain the exact unchanged group tariff layout")]
enum GroupPrice<T> {
    Empty,
    One(Box<T>),
    Many(BTreeSet<Box<T>>),
}
pub(super) const BLOCK_CONTROLS: usize = BLOCK_COMMON
    + 4 * size_of::<GroupPrice<PathState>>()
    + 2 * size_of::<bool>()
    + 2 * size_of::<&GroupPrice<PathState>>()
    + 4 * size_of::<usize>();

// One terminal local Diagnostic and its conversion operand, plus the actual
// and formal Box of explicit drop before converting the selected error.
pub(super) const HELD_EDGE_TERMINAL: usize =
    2 * size_of::<Diagnostic>() + 2 * size_of::<Box<PathState>>();

macro_rules! held_edge {
    ($function:expr, $state:ident, $condition:expr, $sources:ident) => {{
        if $state.pending_failure.is_some() || $state.selected_failure.is_some() || $state.published
        {
            let error = replay_error(
                $function,
                "conditional edge follows pending, selected, or published state",
            );
            // The removed callee dropped its owned Box before returning its
            // Diagnostic to the caller for E conversion. Keep that exact order.
            drop($state);
            return Err(error.into());
        }
        match $condition {
            EdgeCondition::Always => {}
            EdgeCondition::BooleanResult(expression, false) => {
                if let Some(source) = $sources.get(expression) {
                    $state.pending_failure = Some(source.clone());
                }
            }
            EdgeCondition::BooleanResult(_, true)
            | EdgeCondition::VariantCase { .. }
            | EdgeCondition::ArmSelected { .. }
            | EdgeCondition::StatusZero(_) => {}
            EdgeCondition::StatusNonzero(source) => {
                $state.pending_failure = Some(source.clone());
            }
        }
    }};
}
pub(super) use held_edge;

// The selector and one actual zero-or-positive payment are funded by the
// block bank before forming this borrowed local, len receiver or Result.
pub(super) const SINGLE_HISTORY_SELECT: usize = size_of::<(
    &Box<PathState>, &Box<PathState>, // Box::as_ref actual/formal
    &PathState, &PathState, // returned borrow and lexical receiver
    &ResolvedFunction, &ResolvedFunction,
    &mut ReplayBudget, &mut ReplayBudget,
    &str, &str,
    &Vec<LivenessFlagId>, &Vec<LivenessFlagId>,
    usize, usize, usize, usize, usize, usize, // len, threshold, payment amount pairs
    &usize, &usize, &usize, &usize, // comparison operands actual/formal
    bool, bool,
    Result<(), Diagnostic>, Result<(), Diagnostic>,
)>();
macro_rules! singleton_history {
    ($function:ident, $incoming:expr, $block:ident, $budget:ident) => {{
        let incoming = $incoming;
        if incoming.live_order.len() <= 1 {
            // Empty or one-element histories are already unique acyclic chains.
            // No flags Set, iterator, helper arguments or result is formed.
            // The disappeared positive scratch request retains live zero auth.
            $budget.reserve_bytes($function, 0, "trivial singleton history authority")?;
            #[cfg(test)]
            tests::trivial_history(incoming.live_order.len());
        } else {
            $budget.reserve_bytes(
                $function, boxed_flow::SINGLE_HISTORY_CALL,
                "ordinary singleton history call and scratch controls",
            )?;
            #[cfg(test)]
            tests::history_call();
            boxed_flow::validate_singleton_history($function, incoming, $block, $budget)?;
        }
    }};
}
pub(super) use singleton_history;

// Full caller/payment/result carriers and owned scratch are paid before the
// exclusive helper receives them. The old block's remaining banks survive.
pub(super) const SINGLE_HISTORY_CALL: usize = size_of::<(
    &ResolvedFunction,
    &ResolvedFunction,
    &ResolvedFunction,
    &ResolvedFunction,
    &str,
    &str,
    &PathState,
    &PathState,
    &mut ReplayBudget,
    &mut ReplayBudget,
    &mut ReplayBudget,
    &mut ReplayBudget,
    usize,
    usize,
    BlockId,
    BlockId,
    Result<(), Diagnostic>,
    Result<(), Diagnostic>,
    Result<(), Diagnostic>,
    Result<(), Diagnostic>,
    BTreeSet<LivenessFlagId>,
    BTreeSet<LivenessFlagId>,
    BTreeSet<LivenessFlagId>,
    &Vec<LivenessFlagId>,
    &Vec<LivenessFlagId>,
    &Vec<LivenessFlagId>,
    &Vec<LivenessFlagId>,
    std::slice::Iter<'static, LivenessFlagId>,
    std::slice::Iter<'static, LivenessFlagId>,
    usize,
    usize,
    usize,
    usize,
    bool,
    bool,
    bool,
    bool,
)>();
type SingleHistoryStep = (
    Option<&'static LivenessFlagId>,
    Option<&'static LivenessFlagId>,
    Option<&'static LivenessFlagId>,
    Option<&'static LivenessFlagId>,
    &'static mut std::slice::Iter<'static, LivenessFlagId>,
    &'static mut std::slice::Iter<'static, LivenessFlagId>,
    &'static LivenessFlagId,
    &'static LivenessFlagId,
    LivenessFlagId,
    LivenessFlagId,
    bool,
    bool,
    bool,
    bool,
);
#[cfg(test)]
pub(super) const SINGLE_HISTORY_STEP: usize = size_of::<SingleHistoryStep>();

pub(super) fn validate_singleton_history(
    function: &ResolvedFunction,
    incoming: &PathState,
    block: BlockId,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    // Vec's valid allocation length is <= isize::MAX, so its extra terminal
    // iterator slot fits usize. All slots stay paid on an early duplicate.
    budget.reserve_array::<SingleHistoryStep>(
        function,
        incoming.live_order.len() + 1,
        "ordinary singleton history iterator controls",
    )?;
    let mut flags = BTreeSet::new();
    for flag in &incoming.live_order {
        // This is the original complete flags-set insertion and node tariff.
        // A single unique chain is already a topological initialization order:
        // no histories Vec, successor/indegree maps or ready set is formed.
        if !storage::insert_set(function, &mut flags, *flag, budget, "join union flags")? {
            // Do not repair or reinterpret a malformed repeated history. Keep
            // the original complete algorithm and diagnostic in that case.
            return validate_join_compatibility(
                function,
                std::iter::empty(),
                incoming,
                block,
                budget,
            );
        }
    }
    #[cfg(test)]
    tests::unique_history(incoming.live_order.len());
    Ok(())
}
