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

// One consumes only its Option field. The nonbinding Some pattern removes
// expect's owning formal and returned Box; its Box binding uses the unchanged
// Box bank above. The old whole-Incoming take/default/result carriers are gone.
pub(super) const BLOCK_ONE: usize = BLOCK_COMMON
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
