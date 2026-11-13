//! Direct typed-HIR materializations that precede skeleton path construction.
use super::*;
mod collections;
pub(super) use collections::{field_params, vector_params};

pub(super) fn call_root(
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<CallSkeletonState>, Diagnostic> {
    work.budget
        .reserve_array::<CallSkeletonState>(work.function, 1, "initial call state slots")?;
    Ok(vec![(empty_expr_path(), Vec::new())])
}
pub(super) fn push_projection(
    place: &mut CleanupPlace,
    field: &DeclarationId,
    phase: &str,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<(), Diagnostic> {
    work.budget.reserve_vec(
        work.function,
        &mut place.projections,
        1,
        "owned projection slots",
    )?;
    let field = work.clone_owned(field, phase)?;
    place.projections.push(field);
    Ok(())
}
/// Retain the legacy uncharged observation's exact sequence and work count;
/// shared insertion reserves the actual RC payload before converting it.
pub(super) fn push_observation(
    observations: &mut Observations,
    observation: SkeletonObservation,
    work: &mut SkeletonWork<'_, '_>,
    phase: &str,
) -> Result<(), Diagnostic> {
    if work.budget.selected() && observations.prefix.is_some() {
        return Err(replay_error(
            work.function,
            "selected lexical replay requires flat observations",
        ));
    }
    work.budget
        .reserve_vec(work.function, &mut observations.items, 1, phase)?;
    work.budget
        .reserve_bytes(work.function, physical_clone::observation_rc_bytes(), phase)?;
    observations.push(observation.into());
    Ok(())
}

impl ClonePayload for crate::hir::ResolvedParam {
    fn clone_payload_bytes(&self) -> Option<usize> {
        self.name.len().checked_add(self.ty.clone_payload_bytes()?)
    }
    #[cfg(test)]
    fn clone_payload_bytes_with_work(&self, work: &mut impl FnMut(usize) -> bool) -> Option<usize> {
        if !work(2) {
            return None;
        }
        if !work(self.name.len()) {
            return None;
        }
        self.name
            .len()
            .checked_add(self.ty.clone_payload_bytes_with_work(work)?)
    }
}
fn digits(mut index: usize) -> usize {
    let mut count = 1;
    while index >= 10 {
        index /= 10;
        count += 1;
    }
    count
}
fn parameter_id(
    operation: &str,
    index: usize,
    work: &mut SkeletonWork<'_, '_>,
    metered: bool,
) -> Result<crate::hir::ValueId, Diagnostic> {
    if metered && !work.budget.selected() {
        return Err(replay_error(
            work.function,
            "borrowed parameter constructor requires backing reservation",
        ));
    }
    if !work.budget.pays_allocations() {
        if metered {
            return Err(replay_error(
                work.function,
                "borrowed parameter constructor requires backing reservation",
            ));
        }
        return Ok(crate::hir::ValueId::intrinsic_parameter(operation, index));
    }
    let decimal_len = if metered {
        work.charge(1, "borrowed intrinsic identity length arithmetic")?;
        index.checked_ilog10().map_or(1, |n| n as usize + 1)
    } else {
        digits(index)
    };
    let len = operation
        .len()
        .checked_add(7)
        .and_then(|n| n.checked_add(decimal_len))
        .ok_or_else(|| replay_error(work.function, "intrinsic parameter identity overflow"))?;
    // A temporary exact String overlaps the new Arc<str> allocation. Round its
    // two-counter header + trailing text to usize alignment before allocation.
    let alignment = std::mem::align_of::<usize>();
    let arc = len
        .checked_add(2 * std::mem::size_of::<usize>())
        .and_then(|n| n.checked_add(alignment - 1))
        .map(|n| n / alignment * alignment);
    let bytes = arc.and_then(|n| n.checked_add(len)).unwrap_or(usize::MAX);
    work.budget
        .reserve_bytes(work.function, bytes, "intrinsic parameter identity")?;
    #[cfg(test)]
    if metered {
        return crate::hir::ValueId::intrinsic_parameter_with_work(
            operation,
            index,
            &mut |units| {
                work.charge(
                    units,
                    "borrowed intrinsic identity actual constructor visit",
                )
            },
        );
    }
    #[cfg(not(test))]
    let _ = metered;
    Ok(crate::hir::ValueId::intrinsic_parameter_exact(
        operation, index,
    ))
}
pub(super) fn static_params(
    operation: &str,
    types: &[ResolvedType],
    names: &[&str],
    ownership: impl Fn(usize, &ResolvedType) -> OwnershipMode,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<crate::hir::ResolvedParam>, Diagnostic> {
    static_params_impl(operation, types, names, ownership, work, false)
}
#[cfg(test)]
pub(super) fn borrowed_static_params(
    operation: &str,
    types: &[ResolvedType],
    names: &[&str],
    ownership: impl Fn(usize, &ResolvedType) -> OwnershipMode,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<crate::hir::ResolvedParam>, Diagnostic> {
    static_params_impl(operation, types, names, ownership, work, true)
}
fn static_params_impl(
    operation: &str,
    types: &[ResolvedType],
    names: &[&str],
    ownership: impl Fn(usize, &ResolvedType) -> OwnershipMode,
    work: &mut SkeletonWork<'_, '_>,
    metered: bool,
) -> Result<Vec<crate::hir::ResolvedParam>, Diagnostic> {
    work.budget.reserve_array::<crate::hir::ResolvedParam>(
        work.function,
        types.len().min(names.len()),
        "intrinsic parameter slots",
    )?;
    let mut params = Vec::with_capacity(types.len().min(names.len()));
    for (index, (ty, name)) in types.iter().zip(names).enumerate() {
        if metered {
            work.charge(1, "borrowed intrinsic actual parameter header")?;
            for _ in name.bytes() {
                work.charge(1, "borrowed intrinsic parameter name byte")?;
            }
            work.charge(1, "borrowed intrinsic scalar or view type clone")?;
        }
        let id = parameter_id(operation, index, work, metered)?;
        work.budget
            .reserve_bytes(work.function, name.len(), "intrinsic parameter name")?;
        let ty = work.clone_payload(ty, "intrinsic parameter type")?;
        params.push(crate::hir::ResolvedParam {
            id,
            name: (*name).to_owned(),
            ownership: ownership(index, &ty),
            ty,
            span: crate::ast::Span::default(),
        });
    }
    Ok(params)
}

/// The first selected backing profile does not admit nominal/callable provider
/// construction. Refuse before catalog helpers can create uncharged types,
/// names or identities; ordinary replay retains the existing provider path.
pub(super) fn unsupported_provider(work: &mut SkeletonWork<'_, '_>) -> Result<(), Diagnostic> {
    if work.budget.selected() {
        Err(replay_error(
            work.function,
            "selected lexical replay parameter provider is outside the metered primitive profile",
        ))
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intrinsic_parameter_exact_preserves_unicode_and_decimal_identity_bytes() {
        for operation in ["core.bytes.zeroed", "unicode.é.控制"] {
            for index in [0usize, 9, 10, 999, usize::MAX] {
                assert_eq!(
                    crate::hir::ValueId::intrinsic_parameter(operation, index),
                    crate::hir::ValueId::intrinsic_parameter_exact(operation, index)
                );
                // Decimal emission and UTF8 check, each of the three actual byte passes
                // (String write, fingerprint, Arc transfer). This count is
                // independent of the receiving constructor's callback.
                let decimal = index.to_string();
                let units = 2 * decimal.len() + 3 * (operation.len() + 7 + decimal.len());
                for allowance in [units, units - 1] {
                    let mut remaining = allowance;
                    let result = crate::hir::ValueId::intrinsic_parameter_with_work(
                        operation,
                        index,
                        &mut |count| {
                            remaining = remaining.checked_sub(count).ok_or(())?;
                            Ok::<(), ()>(())
                        },
                    );
                    assert_eq!(remaining, 0);
                    if allowance == units {
                        assert_eq!(
                            result.unwrap(),
                            crate::hir::ValueId::intrinsic_parameter(operation, index)
                        );
                    } else {
                        assert!(result.is_err());
                    }
                }
            }
        }
    }
    #[test]
    fn parameter_clone_payload_counts_type_and_name_but_shares_value_identity() {
        let parameter = crate::hir::ResolvedParam {
            id: crate::hir::ValueId::intrinsic_parameter("core.bytes.zeroed", 0),
            name: "requested".to_owned(),
            ownership: OwnershipMode::Value,
            ty: ResolvedType::Usize,
            span: crate::ast::Span::default(),
        };
        assert_eq!(parameter.clone_payload_bytes(), Some("requested".len()));
        let params = vec![parameter.clone(), parameter];
        assert_eq!(
            params.clone_payload_bytes(),
            Some(2 * std::mem::size_of::<crate::hir::ResolvedParam>() + 2 * "requested".len())
        );
    }
}
