//! Actual private planner backing constructors. Legacy builds retain their
//! original unobserved allocations; caller-funded builds never create a quota.
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::diagnostic::Diagnostic;

pub(super) fn marker() -> Diagnostic {
    Diagnostic::io("SPX-H006", String::new())
}

pub(super) fn vector<T>(
    capacity: usize,
    payer: Option<&CfgReservation>,
) -> Result<Vec<T>, Diagnostic> {
    let Some(payer) = payer else {
        return Ok(Vec::with_capacity(capacity));
    };
    let _allocation_stage = crate::bounded_output::loan_allocation_stage_guard(
        crate::bounded_output::ReservationStage::LoanVectorBacking,
    );
    payer.array::<T>(capacity).map_err(|_| marker())?;
    let requested = std::alloc::Layout::array::<T>(capacity)
        .expect("array layout checked by payer")
        .size();
    let mut values = Vec::new();
    if values.try_reserve_exact(capacity).is_err() {
        payer.allocation(requested);
        return Err(marker());
    }
    if values.capacity() != capacity {
        let excess = values
            .capacity()
            .saturating_sub(capacity)
            .saturating_mul(std::mem::size_of::<T>());
        let _ = payer.capacity(requested, excess);
        return Err(marker());
    }
    Ok(values)
}

pub(super) fn filled<T: Clone>(
    count: usize,
    value: T,
    payer: Option<&CfgReservation>,
) -> Result<Vec<T>, Diagnostic> {
    let mut values = vector(count, payer)?;
    values.resize(count, value);
    Ok(values)
}

// This tag is selected exclusively from the actual affine owner's first
// failure, never from Diagnostic code/text (including a semantic empty H006).
pub(crate) enum PlannerError {
    Physical(crate::bounded_output::cfg_reservation::Refused),
    Semantic(Diagnostic),
}
pub(super) fn classify<T>(
    result: Result<T, Diagnostic>,
    payer: &CfgReservation,
) -> Result<T, PlannerError> {
    if let Some(reason) = payer.first() {
        return Err(PlannerError::Physical(reason));
    }
    result.map_err(PlannerError::Semantic)
}

pub(super) fn push<T>(
    values: &mut Vec<T>,
    value: T,
    payer: Option<&CfgReservation>,
) -> Result<(), Diagnostic> {
    let Some(payer) = payer else {
        values.push(value);
        return Ok(());
    };
    payer.reserve(0).map_err(|_| marker())?;
    if values.len() == values.capacity() {
        let Some(target) = values
            .capacity()
            .checked_mul(2)
            .and_then(|n| values.len().checked_add(1).map(|need| n.max(need).max(4)))
        else {
            payer.arithmetic();
            return Err(marker());
        };
        let _allocation_stage = crate::bounded_output::loan_allocation_stage_guard(
            crate::bounded_output::ReservationStage::LoanVectorBacking,
        );
        payer.array::<T>(target).map_err(|_| marker())?;
        let requested = std::alloc::Layout::array::<T>(target)
            .expect("array layout checked by payer")
            .size();
        if values.try_reserve_exact(target - values.len()).is_err() {
            payer.allocation(requested);
            return Err(marker());
        }
        if values.capacity() != target {
            let excess = values
                .capacity()
                .saturating_sub(target)
                .saturating_mul(std::mem::size_of::<T>());
            let _ = payer.capacity(requested, excess);
            return Err(marker());
        }
    }
    values.push(value);
    Ok(())
}

pub(super) fn extend<T>(
    values: &mut Vec<T>,
    iter: impl IntoIterator<Item = T>,
    payer: Option<&CfgReservation>,
) -> Result<(), Diagnostic> {
    if payer.is_none() {
        values.extend(iter);
        return Ok(());
    }
    for value in iter {
        push(values, value, payer)?;
    }
    Ok(())
}

// Disclosed pinned alloc B-tree upper, not an exact node-layout receipt:
// B=6, eleven key/value slots, twelve child pointers, each field rounded
// separately to maximum alignment. One potentially held root plus one upper
// for each genuinely new key. Existing keys retain old replacement semantics.
fn node_upper<K, V>() -> Option<usize> {
    use std::mem::{align_of, size_of};
    let alignment = align_of::<K>()
        .max(align_of::<V>())
        .max(align_of::<usize>());
    [
        size_of::<Option<std::ptr::NonNull<()>>>(),
        size_of::<u16>(),
        size_of::<u16>(),
        size_of::<K>().checked_mul(11)?,
        size_of::<V>().checked_mul(11)?,
        size_of::<usize>().checked_mul(12)?,
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        let rounded = bytes.checked_add(alignment - 1)? / alignment * alignment;
        total.checked_add(rounded)
    })
}
pub(super) fn map_insert<K: Ord, V>(
    values: &mut std::collections::BTreeMap<K, V>,
    key: K,
    value: V,
    payer: Option<&CfgReservation>,
) -> Result<Option<V>, Diagnostic> {
    if let Some(payer) = payer {
        payer.reserve(0).map_err(|_| marker())?;
        if !values.contains_key(&key) {
            let bytes = node_upper::<K, V>()
                .and_then(|bytes| bytes.checked_mul(if values.is_empty() { 2 } else { 1 }));
            match bytes {
                Some(bytes) => payer.reserve(bytes).map_err(|_| marker())?,
                None => {
                    payer.arithmetic();
                    return Err(marker());
                }
            }
        }
    }
    Ok(values.insert(key, value))
}
pub(super) fn set_insert<K: Ord>(
    values: &mut std::collections::BTreeSet<K>,
    key: K,
    payer: Option<&CfgReservation>,
) -> Result<bool, Diagnostic> {
    if let Some(payer) = payer {
        payer.reserve(0).map_err(|_| marker())?;
        if !values.contains(&key) {
            let bytes = node_upper::<K, ()>()
                .and_then(|bytes| bytes.checked_mul(if values.is_empty() { 2 } else { 1 }));
            match bytes {
                Some(bytes) => payer.reserve(bytes).map_err(|_| marker())?,
                None => {
                    payer.arithmetic();
                    return Err(marker());
                }
            }
        }
    }
    Ok(values.insert(key))
}

pub(super) fn clone_place(
    place: &super::Place,
    payer: Option<&CfgReservation>,
) -> Result<super::Place, Diagnostic> {
    if payer.is_none() {
        return Ok(place.clone());
    }
    // Root and field/case identities are immutable shared carriers. Only the
    // projection directory is newly allocated by this clone.
    let mut projections = vector(place.projections.len(), payer)?;
    projections.extend(place.projections.iter().cloned());
    Ok(super::Place {
        root: place.root.clone(),
        projections,
    })
}

pub(super) fn expression_place(
    expression: &super::ResolvedExpr,
    payer: Option<&CfgReservation>,
) -> Result<Option<super::Place>, Diagnostic> {
    use super::ResolvedExprKind as K;
    let mut current = expression;
    loop {
        match &current.kind {
            K::Call { .. } => {
                return super::paid_predicates::string_slot(current)
                    .map(|place| clone_place(place, payer))
                    .transpose()
            }
            K::Place(place) | K::BorrowPlace { place, .. } => {
                return clone_place(place, payer).map(Some)
            }
            K::VecFieldRead { args, .. } | K::RowLoan { args, .. } => {
                let Some(source) = args.first() else {
                    return Ok(None);
                };
                current = source;
            }
            K::ByteRange { source, .. } => current = source,
            K::NativeRustImportCall(call)
                if matches!(
                    call.result,
                    crate::hir::ResolvedImportResultKind::BorrowedStr { .. }
                ) =>
            {
                let Some(source) = call.args.first() else {
                    return Ok(None);
                };
                current = source;
            }
            _ => return Ok(None),
        }
    }
}

pub(super) fn clone_vec<T: Clone>(
    values: &[T],
    payer: Option<&CfgReservation>,
) -> Result<Vec<T>, Diagnostic> {
    let mut out = vector(values.len(), payer)?;
    out.extend(values.iter().cloned());
    Ok(out)
}
#[cfg(test)]
pub(super) fn singleton_set<K: Ord>(
    value: K,
    payer: Option<&CfgReservation>,
) -> Result<std::collections::BTreeSet<K>, Diagnostic> {
    let mut out = std::collections::BTreeSet::new();
    set_insert(&mut out, value, payer)?;
    Ok(out)
}
#[cfg(test)]
pub(super) fn clone_set<K: Clone + Ord>(
    values: &std::collections::BTreeSet<K>,
    payer: Option<&CfgReservation>,
) -> Result<std::collections::BTreeSet<K>, Diagnostic> {
    if payer.is_none() {
        return Ok(values.clone());
    }
    let mut out = std::collections::BTreeSet::new();
    for value in values {
        set_insert(&mut out, value.clone(), payer)?;
    }
    Ok(out)
}
