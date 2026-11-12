//! Replay the closed IterStep<Bytes> epochs admitted by retained static preflight.
use super::*;
use crate::cleanup_plan::FinalizeAction;
use crate::hir::CompilerDeclaration as C;
use crate::iterator_ops as iter;

type Case = (DeclarationId, Vec<LivenessFlagId>);

macro_rules! refuses {
    ($permit:ident) => {
        path_error!(
            $permit,
            Reason::Finite(Kind::SelectedPrimitiveReplayExcludesConditionalAggregateTransitions)
        )
    };
}

#[allow(clippy::too_many_arguments)]
pub(super) fn initialize(
    function: &ResolvedFunction,
    destination: &CleanupPlace,
    variant: &DeclarationId,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
    permit: &Permit<'_>,
) -> Result<(), Error> {
    const PHASE: &str = "selected iterator conditional initialization";
    budget.reserve_array::<(
        Vec<LivenessFlagId>,
        &LivenessFlagId,
        &LivenessFlagId,
        &Leaf,
        &Leaf,
        Vec<Case>,
        DeclarationId,
        DeclarationId,
        ReplayConditionalVariant,
    )>(function, 1, PHASE)?;
    budget.charge(function, 9, PHASE)?;
    let yielded = vacant(
        function,
        destination,
        state,
        storage,
        leaves,
        budget,
        permit,
    )?;
    let [item, rest] = yielded.as_slice() else {
        return Err(refuses!(permit));
    };
    let item_leaf = leaf_at!(budget, function, leaves, item);
    let rest_leaf = leaf_at!(budget, function, leaves, rest);
    if !step_field(item_leaf, C::IterItem, C::BytesDrop)
        || !step_field(rest_leaf, C::IterRest, C::IterDrop)
    {
        return Err(refuses!(permit));
    }
    budget.reserve_array::<Case>(function, 2, PHASE)?;
    budget.reserve_clone(function, destination, PHASE)?;
    budget.reserve_clone(function, variant, PHASE)?;
    budget.reserve_vec(function, &mut state.conditional_variants, 1, PHASE)?;
    let done = DeclarationId::compiler(C::IterDone);
    let yield_case = DeclarationId::compiler(C::IterYield);
    let cases = vec![(done, Vec::new()), (yield_case, yielded)];
    state.conditional_variants.push(ReplayConditionalVariant {
        root: clone_value!(budget, function, destination, CleanupPlace),
        variant: clone_value!(budget, function, variant, DeclarationId),
        cases,
    });
    Ok(())
}

#[inline]
fn step_field(leaf: &Leaf, field: C, lifecycle: C) -> bool {
    matches!(leaf.place.projections.as_slice(), [case, member]
        if case.as_str() == iter::YIELD_ID && member.as_str() == field.text())
        && leaf.lifecycle.as_str() == lifecycle.text()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn vacant(
    function: &ResolvedFunction,
    place: &CleanupPlace,
    state: &PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
    permit: &Permit<'_>,
) -> Result<Vec<LivenessFlagId>, Error> {
    const PHASE: &str = "selected iterator vacant epoch";
    budget.reserve_array::<(
        &ResolvedFunction,
        &CleanupPlace,
        &PathState,
        &BTreeSet<StorageId>,
        &Leaves,
        &mut ReplayBudget,
        &Permit<'_>,
        Result<Vec<LivenessFlagId>, Error>,
        Vec<LivenessFlagId>,
    )>(function, 1, PHASE)?;
    budget.charge(function, 9, PHASE)?;
    if !place.projections.is_empty() || position(function, place, state, budget)?.is_some() {
        return Err(refuses!(permit));
    }
    let flags = path_child!(
        budget,
        function,
        PlaceCall,
        6,
        structural_control::selected::helpers::validate_place(
            function, place, storage, leaves, budget
        )
    )?;
    budget.charge(
        function,
        state.live_order.len().saturating_mul(flags.len()),
        PHASE,
    )?;
    if flags.iter().any(|flag| state.live_order.contains(flag)) {
        return Err(refuses!(permit));
    }
    Ok(flags)
}

pub(super) fn position(
    function: &ResolvedFunction,
    place: &CleanupPlace,
    state: &PathState,
    budget: &mut ReplayBudget,
) -> Result<Option<usize>, Error> {
    const PHASE: &str = "selected iterator epoch lookup";
    budget.reserve_array::<(
        &ResolvedFunction,
        &CleanupPlace,
        &PathState,
        &mut ReplayBudget,
        Result<Option<usize>, Error>,
        usize,
    )>(function, 1, PHASE)?;
    let id_len = match &place.storage {
        StorageId::Value(id) => id.as_str().len(),
        StorageId::Temporary(id) => id.as_str().len(),
        StorageId::CallArgument { .. } | StorageId::ProvisionalResult => return Ok(None),
    };
    budget.charge(
        function,
        id_len
            .saturating_add(1)
            .saturating_mul(state.conditional_variants.len())
            .saturating_add(6),
        PHASE,
    )?;
    Ok(variant_position!(
        budget,
        function,
        state.conditional_variants,
        variant,
        variant.variant.as_str() == iter::STEP_ID && variant.root.storage == place.storage
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn authenticate(
    function: &ResolvedFunction,
    source: &CleanupPlace,
    case: &DeclarationId,
    state: &mut PathState,
    budget: &mut ReplayBudget,
    permit: &Permit<'_>,
) -> Result<(), Error> {
    const PHASE: &str = "selected iterator case authentication";
    budget.reserve_array::<(
        usize,
        &ReplayConditionalVariant,
        &Case,
        &Case,
        ReplayConditionalVariant,
        std::vec::IntoIter<Case>,
        Case,
        Vec<LivenessFlagId>,
    )>(function, 1, PHASE)?;
    budget.charge(function, 8, PHASE)?;
    if !source.projections.is_empty() {
        return Err(refuses!(permit));
    }
    let Some(index) = position(function, source, state, budget)? else {
        return Err(refuses!(permit));
    };
    let epoch = index!(
        budget,
        function,
        state.conditional_variants,
        index,
        ReplayConditionalVariant
    );
    let [done, yielded] = epoch.cases.as_slice() else {
        return Err(refuses!(permit));
    };
    if done.0.as_str() != iter::DONE_ID
        || !done.1.is_empty()
        || yielded.0.as_str() != iter::YIELD_ID
        || yielded.1.len() != 2
        || !matches!(case.as_str(), iter::DONE_ID | iter::YIELD_ID)
    {
        return Err(refuses!(permit));
    }
    if case.as_str() == iter::DONE_ID {
        state.conditional_variants.remove(index);
        return Ok(());
    }
    budget.reserve_vec(function, &mut state.live_order, 2, PHASE)?;
    let mut cases = state.conditional_variants.remove(index).cases.into_iter();
    let (_, flags) = cases.next_back().expect("checked Yield case");
    path_child!(
        budget,
        function,
        AppendFlagsCall,
        6,
        finish::append_flags(
            function,
            state,
            flags,
            Kind::SelectedPrimitiveReplayExcludesConditionalAggregateTransitions,
            budget,
        )
    )
}

macro_rules! next_guarded {
    ($budget:ident, $function:ident, $actual:ident) => {{
        let mut guarded = None;
        while let Some(action) = dynamic_next!(
            $budget,
            $function,
            $actual,
            std::slice::Iter<'_, FinalizeAction>,
            &FinalizeAction,
            (&FinalizeAction),
            3
        ) {
            if action.active_case.is_some() {
                guarded = Some(action);
                break;
            }
        }
        guarded
    }};
}

type Departure<'a> = (
    &'a ExitTarget,
    &'a BTreeMap<StorageId, CleanupRegionId>,
    &'a BTreeSet<StorageId>,
);

fn leaves_scope(
    function: &ResolvedFunction,
    epoch: &ReplayConditionalVariant,
    departure: Departure<'_>,
    budget: &mut ReplayBudget,
) -> Result<bool, Error> {
    const PHASE: &str = "selected iterator conditional departure";
    budget.reserve_array::<(
        &ResolvedFunction,
        &ReplayConditionalVariant,
        Departure<'_>,
        &mut ReplayBudget,
        Result<bool, Error>,
        &CleanupRegionId,
        &ExitTarget,
        &BTreeMap<StorageId, CleanupRegionId>,
        &BTreeSet<StorageId>,
    )>(function, 1, PHASE)?;
    let (exit, storage_regions, extra_disposed) = departure;
    let region = map_index!(
        budget,
        function,
        storage_regions,
        &epoch.root.storage,
        StorageId,
        CleanupRegionId
    );
    budget.charge(function, exit.leaves_regions.len().saturating_add(9), PHASE)?;
    Ok(exit.leaves_regions.contains(region) || extra_disposed.contains(&epoch.root.storage))
}

pub(in crate::cleanup_plan::replay) fn matches_finalizers(
    function: &ResolvedFunction,
    actions: &[FinalizeAction],
    variants: &[ReplayConditionalVariant],
    departure: Option<Departure<'_>>,
    budget: &mut ReplayBudget,
) -> Result<bool, Error> {
    const PHASE: &str = "selected iterator conditional finalizer history";
    budget.reserve_array::<(
        &ResolvedFunction,
        &[FinalizeAction],
        &[ReplayConditionalVariant],
        Option<Departure<'_>>,
        Departure<'_>,
        &mut ReplayBudget,
        Result<bool, Error>,
        &ReplayConditionalVariant,
        &Case,
        &Case,
        &LivenessFlagId,
        &FinalizeAction,
        &crate::cleanup_plan::VariantCaseGuard,
        std::slice::Iter<'_, FinalizeAction>,
        Option<&FinalizeAction>,
        std::iter::Rev<std::slice::Iter<'_, ReplayConditionalVariant>>,
        usize,
    )>(function, 1, PHASE)?;
    budget.charge(function, 17, PHASE)?;
    let mut actual = actions.iter();
    let mut epochs = variants.iter().rev();
    while let Some(epoch) = dynamic_next!(
        budget,
        function,
        epochs,
        std::iter::Rev<std::slice::Iter<'_, ReplayConditionalVariant>>,
        &ReplayConditionalVariant,
        (&ReplayConditionalVariant),
        3
    ) {
        if let Some(departure) = departure {
            if !leaves_scope(function, epoch, departure, budget)? {
                continue;
            }
        }
        let [done, yielded] = epoch.cases.as_slice() else {
            return Ok(false);
        };
        if epoch.variant.as_str() != iter::STEP_ID
            || done.0.as_str() != iter::DONE_ID
            || !done.1.is_empty()
            || yielded.0.as_str() != iter::YIELD_ID
            || yielded.1.len() != 2
        {
            return Ok(false);
        }
        let id_len = match &epoch.root.storage {
            StorageId::Value(id) => id.as_str().len(),
            StorageId::Temporary(id) => id.as_str().len(),
            StorageId::CallArgument { .. } | StorageId::ProvisionalResult => return Ok(false),
        };
        budget.charge(function, id_len.saturating_add(1).saturating_mul(2), PHASE)?;
        for flag in yielded.1.iter().rev() {
            let Some(action) = next_guarded!(budget, function, actual) else {
                return Ok(false);
            };
            let guard = action
                .active_case
                .as_ref()
                .expect("selected guarded action");
            if action.guard_flag != *flag
                || guard.storage != epoch.root.storage
                || guard.variant.as_str() != iter::STEP_ID
                || guard.case.as_str() != iter::YIELD_ID
            {
                return Ok(false);
            }
        }
    }
    Ok(next_guarded!(budget, function, actual).is_none())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn finalize(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    state: &mut PathState,
    storage_regions: &BTreeMap<StorageId, CleanupRegionId>,
    extra_disposed: &BTreeSet<StorageId>,
    budget: &mut ReplayBudget,
    permit: &Permit<'_>,
) -> Result<(), Error> {
    const PHASE: &str = "selected iterator conditional exit";
    budget.reserve_array::<(
        &ResolvedFunction,
        &ExitTarget,
        &mut PathState,
        &BTreeMap<StorageId, CleanupRegionId>,
        &BTreeSet<StorageId>,
        &mut ReplayBudget,
        &Permit<'_>,
        Result<(), Error>,
        Departure<'_>,
        Option<Departure<'_>>,
        &[FinalizeAction],
        &[ReplayConditionalVariant],
        Result<bool, Error>,
        usize,
        &ReplayConditionalVariant,
        ReplayConditionalVariant,
    )>(function, 1, PHASE)?;
    budget.charge(function, 16, PHASE)?;
    let departure = (exit, storage_regions, extra_disposed);
    if !matches_finalizers(
        function,
        &exit.finalize_in_order,
        &state.conditional_variants,
        Some(departure),
        budget,
    )? {
        return Err(path_error!(
            permit,
            Reason::Finite(Kind::ExitDoesNotPreserveExactConditionalVariantFinalizers)
        ));
    }
    let mut index = state.conditional_variants.len();
    while index > 0 {
        index -= 1;
        let epoch = index!(
            budget,
            function,
            state.conditional_variants,
            index,
            ReplayConditionalVariant
        );
        if leaves_scope(function, epoch, departure, budget)? {
            budget.charge(function, state.conditional_variants.len() - index, PHASE)?;
            state.conditional_variants.remove(index);
        }
    }
    Ok(())
}
