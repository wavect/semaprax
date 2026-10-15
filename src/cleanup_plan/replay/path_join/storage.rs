//! Precharge actual owned clone carriers. Shared identity Arcs allocate nothing
//! when cloned; projection/type/case containers retain their actual backing.
use super::*;
use std::mem::size_of;
pub(super) mod empty_renewals;
fn array<T>(count: usize) -> Option<usize> {
    count.checked_mul(size_of::<T>())
}
fn place(place: &CleanupPlace) -> Option<usize> {
    array::<DeclarationId>(place.projections.len())
}
fn ty(ty: &ResolvedType) -> Option<usize> {
    let mut stack = [None; crate::cache_codec::MAX_DEPTH + 1];
    stack[0] = Some((ty, 0usize));
    let mut len = 1;
    let mut bytes = 0usize;
    let mut work = 65_536usize;
    while len != 0 {
        let (value, next) = stack[len - 1]?;
        if next == 0 {
            work = work.checked_sub(1)?;
            let owned = match value {
                ResolvedType::Function { parameters, .. } => {
                    array::<ResolvedType>(parameters.len())?
                        .checked_add(size_of::<ResolvedType>())?
                }
                ResolvedType::Nominal { arguments, .. } => array::<ResolvedType>(arguments.len())?,
                ResolvedType::TypeParameter { .. } => 0,
                ResolvedType::Never
                | ResolvedType::Unit
                | ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool
                | ResolvedType::String
                | ResolvedType::ArrayU8(_)
                | ResolvedType::Bytes
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::StringMap
                | ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair
                | ResolvedType::MutFunctionI64 => 0,
            };
            bytes = bytes.checked_add(owned)?;
        }
        let child = match value {
            ResolvedType::Function { parameters, result } => parameters
                .get(next)
                .or_else(|| (next == parameters.len()).then_some(result.as_ref())),
            ResolvedType::Nominal { arguments, .. } => arguments.get(next),
            _ => None,
        };
        if let Some(child) = child {
            if len == stack.len() {
                return None;
            }
            stack[len - 1] = Some((value, next + 1));
            stack[len] = Some((child, 0));
            len += 1;
        } else {
            len -= 1;
        }
    }
    Some(bytes)
}
fn staged(source: &StagedCopyResultSource) -> Option<usize> {
    match source {
        StagedCopyResultSource::Body { instance, .. } => ty(instance),
        StagedCopyResultSource::TryResidual {
            source_instance,
            target_instance,
            ..
        }
        | StagedCopyResultSource::TryOptionNone {
            source_instance,
            target_instance,
            ..
        } => ty(source_instance)?.checked_add(ty(target_instance)?),
    }
}
fn returned(source: &ReturnedSource) -> Option<usize> {
    match source {
        ReturnedSource::Scalar(_) => Some(0),
        ReturnedSource::Owned(value) => place(value),
    }
}
fn variant(value: &ReplayConditionalVariant) -> Option<usize> {
    value.cases.iter().try_fold(
        place(&value.root)?.checked_add(array::<(DeclarationId, Vec<LivenessFlagId>)>(
            value.cases.len(),
        )?)?,
        |n, (_, flags)| n.checked_add(array::<LivenessFlagId>(flags.len())?),
    )
}
fn state_bytes(state: &PathState) -> Option<usize> {
    let node = super::super::physical_storage::btree_node_upper::<
        ExpressionId,
        crate::cleanup_plan::renewal_history::RenewalHistory,
    >()?;
    let mut bytes = node
        .checked_add(array::<LivenessFlagId>(state.live_order.len())?)?
        .checked_add(array::<ReplayConditionalVariant>(
            state.conditional_variants.len(),
        )?)?;
    for history in state.renewals.values() {
        bytes = bytes
            .checked_add(node)?
            .checked_add(history.clone_payload_bytes()?)?;
    }
    for value in &state.conditional_variants {
        bytes = bytes.checked_add(variant(value)?)?;
    }
    if let Some(value) = &state.staged_copy_result {
        bytes = bytes.checked_add(staged(value)?)?;
    }
    if let Some(value) = &state.returned {
        bytes = bytes.checked_add(returned(value)?)?;
    }
    Some(bytes)
}
pub(in crate::cleanup_plan::replay) fn reserve_btree_node<K, V>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(());
    }
    budget.reserve_bytes(
        function,
        super::super::physical_storage::btree_node_upper::<K, V>().unwrap_or(usize::MAX),
        phase,
    )
}
pub(in crate::cleanup_plan::replay) fn reserve_state_clone(
    function: &ResolvedFunction,
    state: &PathState,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(());
    }
    if state.staged_copy_result.is_some() {
        budget.reserve_array::<Option<(&ResolvedType, usize)>>(
            function,
            crate::cache_codec::MAX_DEPTH + 1,
            "clone type scratch",
        )?;
    }
    budget.reserve_bytes(function, state_bytes(state).unwrap_or(usize::MAX), phase)
}
pub(super) fn reserve_ownership_clone(
    function: &ResolvedFunction,
    state: &PathState,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(());
    }
    let bytes = (|| {
        state.conditional_variants.iter().try_fold(
            array::<LivenessFlagId>(state.live_order.len())?.checked_add(array::<
                ReplayConditionalVariant,
            >(
                state.conditional_variants.len(),
            )?)?,
            |n, value| n.checked_add(variant(value)?),
        )
    })();
    budget.reserve_bytes(
        function,
        bytes.unwrap_or(usize::MAX),
        "joined ownership clone",
    )
}
pub(super) fn reserve_control_clone(
    function: &ResolvedFunction,
    state: &PathState,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(());
    }
    if state.staged_copy_result.is_some() {
        budget.reserve_array::<Option<(&ResolvedType, usize)>>(
            function,
            crate::cache_codec::MAX_DEPTH + 1,
            "control clone type scratch",
        )?;
    }
    let bytes = (|| {
        let mut bytes = 0usize;
        if let Some(value) = &state.staged_copy_result {
            bytes = bytes.checked_add(staged(value)?)?;
        }
        if let Some(value) = &state.returned {
            bytes = bytes.checked_add(returned(value)?)?;
        }
        Some(bytes)
    })();
    budget.reserve_bytes(
        function,
        bytes.unwrap_or(usize::MAX),
        "ownership group control clone",
    )
}

pub(in crate::cleanup_plan::replay) fn insert_set<K: Ord>(
    function: &ResolvedFunction,
    target: &mut BTreeSet<K>,
    value: K,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<bool, Diagnostic> {
    if target.contains(&value) {
        return Ok(false);
    }
    reserve_btree_node::<K, ()>(function, budget, phase)?;
    Ok(target.insert(value))
}
pub(in crate::cleanup_plan::replay) fn insert_map<K: Ord, V>(
    function: &ResolvedFunction,
    target: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<(), Diagnostic> {
    if !target.contains_key(&key) {
        reserve_btree_node::<K, V>(function, budget, phase)?;
    }
    target.insert(key, value);
    Ok(())
}

pub(super) fn reserve_variant_identity(
    function: &ResolvedFunction,
    root: &CleanupPlace,
    _id: &DeclarationId,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(());
    }
    budget.reserve_bytes(
        function,
        place(root).unwrap_or(usize::MAX),
        "joined variant identity",
    )
}

// Count through the existing ordered leaf index before its exact Vec copy.
// Preserve the unknown-storage refusal before attempting any reservation.
pub(in crate::cleanup_plan::replay) fn validate_place(
    function: &ResolvedFunction,
    place: &CleanupPlace,
    known: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<Vec<LivenessFlagId>, Diagnostic> {
    if !known.contains(&place.storage) {
        return super::super::validate_place(function, place, known, leaves);
    }
    if !budget.pays_allocations() {
        return super::super::validate_place(function, place, known, leaves);
    }
    let matches = |leaf: &super::super::Leaf| {
        leaf.place.storage == place.storage
            && leaf.place.projections.starts_with(&place.projections)
    };
    let count = leaves.values().filter(|leaf| matches(leaf)).count();
    if count == 0 {
        return Err(replay_error(
            function,
            "cleanup place has no droppable leaf",
        ));
    }
    budget.reserve_array::<LivenessFlagId>(function, count, "entry place leaves")?;
    let mut result = Vec::with_capacity(count);
    result.extend(
        leaves
            .iter()
            .filter_map(|(flag, leaf)| matches(leaf).then_some(*flag)),
    );
    Ok(result)
}
pub(super) fn storage_regions(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeMap<StorageId, CleanupRegionId>, Diagnostic> {
    let mut regions = BTreeMap::new();
    for region in &function.cleanup_plan.regions {
        for storage in &region.slots {
            if regions.contains_key(storage) {
                return Err(replay_error(
                    function,
                    "cleanup storage has multiple owning regions during replay",
                ));
            }
            reserve_btree_node::<StorageId, CleanupRegionId>(
                function,
                budget,
                "storage region index",
            )?;
            regions.insert(storage.clone(), region.id);
        }
    }
    Ok(regions)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloned_state_payload_counts_owned_buffers_but_not_shared_identity_handles() {
        let mut state = PathState {
            renewals: BTreeMap::new(),
            live_order: vec![LivenessFlagId(1), LivenessFlagId(2)],
            conditional_variants: Vec::new(),
            pending_failure: None,
            selected_failure: None,
            staged_copy_result: None,
            published: false,
            returned: None,
        };
        let root = super::super::super::physical_storage::btree_node_upper::<
            ExpressionId,
            super::super::super::super::renewal_history::RenewalHistory,
        >()
        .unwrap();
        assert_eq!(
            state_bytes(&state),
            array::<LivenessFlagId>(2).unwrap().checked_add(root)
        );
        let history = super::super::super::super::renewal_history::RenewalHistory::new(
            state.live_order.clone(),
            Some(LivenessFlagId(1)),
        );
        let payload = history.clone_payload_bytes().unwrap();
        state
            .renewals
            .insert(ExpressionId::from_owned("renewal".to_owned()), history);
        assert_eq!(
            state_bytes(&state),
            array::<LivenessFlagId>(2)
                .unwrap()
                .checked_add(root)
                .and_then(|n| n.checked_add(root))
                .and_then(|n| n.checked_add(payload))
        );
    }
    #[test]
    fn conditional_clone_literal_backing_keeps_projection_cases_and_flags() {
        let shared = DeclarationId::new("variant:é\0.long.shared");
        let value = ReplayConditionalVariant {
            root: CleanupPlace {
                storage: StorageId::ProvisionalResult,
                projections: vec![shared.clone(), shared.clone()],
            },
            variant: shared.clone(),
            cases: vec![
                (shared.clone(), vec![LivenessFlagId(1)]),
                (
                    DeclarationId::compiler(crate::hir::CompilerDeclaration::Vec),
                    vec![LivenessFlagId(2), LivenessFlagId(3)],
                ),
            ],
        };
        let expected = 2 * size_of::<DeclarationId>()
            + 2 * size_of::<(DeclarationId, Vec<LivenessFlagId>)>()
            + 3 * size_of::<LivenessFlagId>();
        let former_string_payload = 4 * shared.as_str().len() + "core.vec".len();
        assert!(former_string_payload > 0);
        assert_eq!(variant(&value), Some(expected));
        let (_, observed) = crate::test_allocation_observer::measure(|| {
            let copy = value.clone();
            assert_eq!(copy, value);
            drop(copy);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.allocations, 4);
        assert_eq!(observed.allocated, expected);
        assert_eq!(observed.freed, expected);
        assert_eq!(observed.live, 0);
    }
    #[test]
    fn exact_clone_charge_and_one_short_refusal_precede_clone_and_remain_sticky() {
        let parsed =
            crate::check("module joins; @id(\"main\") fn main()->i64 {0}", "join.spx").unwrap();
        let function = crate::hir::resolve(&parsed).unwrap().functions.remove(0);
        let state = PathState {
            renewals: BTreeMap::new(),
            live_order: vec![LivenessFlagId(1), LivenessFlagId(2)],
            conditional_variants: Vec::new(),
            pending_failure: None,
            selected_failure: None,
            staged_copy_result: None,
            published: false,
            returned: None,
        };
        let bytes = state_bytes(&state).unwrap();
        let mut exact = ReplayBudget::new();
        exact.admission = AdmissionMode::SelectedRetained;
        exact.payer = AllocationPayer::SelectedReservation;
        exact.physical = Some(Box::new(
            super::super::super::physical_storage::PhysicalReservation::new(bytes),
        ));
        reserve_state_clone(&function, &state, &mut exact, "test state clone").unwrap();
        assert_eq!(state.clone(), state);
        let mut short = ReplayBudget::new();
        short.admission = AdmissionMode::SelectedRetained;
        short.payer = AllocationPayer::SelectedReservation;
        short.physical = Some(Box::new(
            super::super::super::physical_storage::PhysicalReservation::new(bytes - 1),
        ));
        let mut destination = Vec::new();
        let first =
            reserve_state_clone(&function, &state, &mut short, "test state clone").unwrap_err();
        if reserve_state_clone(&function, &state, &mut short, "test state clone").is_ok() {
            destination.push(state.clone());
        }
        assert!(destination.is_empty());
        let later =
            reserve_state_clone(&function, &state, &mut short, "test state clone").unwrap_err();
        assert_eq!(first.code, later.code);
        assert_eq!(state.live_order, vec![LivenessFlagId(1), LivenessFlagId(2)]);
        reserve_state_clone(
            &function,
            &state,
            &mut ReplayBudget::new(),
            "legacy state clone",
        )
        .unwrap();
    }
}
