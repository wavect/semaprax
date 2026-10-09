//! Physical retained loan-proof capacities; byte equality never implies shared storage.
use super::{Loan, LoanEdge, LoanEndpoint, LoanId, LoanPlan, LoanProgramPoint};
use crate::hir::{ExpressionId, Place, PlaceProjection};

pub(super) fn owned_capacity_bytes(plan: &LoanPlan) -> Option<usize> {
    owned_capacity_bytes_excluding(plan, &[])
}

/// Physical union with HIR backing already covered by the caller's retained
/// HIR prebound. Keys must come from live authoritative HIR, never byte equality.
/// Empty exclusions preserve the independent full-proof census used by caches.
pub(crate) fn owned_capacity_bytes_excluding(
    plan: &LoanPlan,
    covered_hir_keys: &[usize],
) -> Option<usize> {
    if covered_hir_keys.windows(2).any(|pair| pair[0] >= pair[1]) {
        return None;
    }
    fn add(total: &mut usize, bytes: usize) -> Option<()> {
        *total = total.checked_add(bytes)?;
        Some(())
    }
    fn place_bytes(place: &Place) -> Option<usize> {
        let mut bytes = place.root.as_str().len();
        add(
            &mut bytes,
            place
                .projections
                .capacity()
                .checked_mul(std::mem::size_of::<PlaceProjection>())?,
        )?;
        for projection in &place.projections {
            match projection {
                PlaceProjection::Field(field) => add(&mut bytes, field.as_str().len())?,
                PlaceProjection::VariantField { case, field } => {
                    add(&mut bytes, case.as_str().len())?;
                    add(&mut bytes, field.as_str().len())?;
                }
            }
        }
        Some(bytes)
    }

    let mut bytes = plan
        .loans
        .capacity()
        .checked_mul(std::mem::size_of::<Loan>())?;
    add(
        &mut bytes,
        plan.endpoints
            .capacity()
            .checked_mul(std::mem::size_of::<LoanEndpoint>())?,
    )?;
    add(
        &mut bytes,
        plan.edges
            .capacity()
            .checked_mul(std::mem::size_of::<LoanEdge>())?,
    )?;
    for loan in &plan.loans {
        add(&mut bytes, place_bytes(&loan.origin)?)?;
        add(
            &mut bytes,
            loan.ends
                .capacity()
                .checked_mul(std::mem::size_of::<LoanProgramPoint>())?,
        )?;
        add(
            &mut bytes,
            loan.end_edges
                .capacity()
                .checked_mul(std::mem::size_of::<u16>())?,
        )?;
    }
    for endpoint in &plan.endpoints {
        for ids in [
            &endpoint.live_before,
            &endpoint.starts,
            &endpoint.kills,
            &endpoint.live_after,
        ] {
            add(
                &mut bytes,
                ids.capacity().checked_mul(std::mem::size_of::<LoanId>())?,
            )?;
        }
    }
    for edge in &plan.edges {
        add(
            &mut bytes,
            edge.live
                .capacity()
                .checked_mul(std::mem::size_of::<LoanId>())?,
        )?;
    }
    add(&mut bytes, shared_identity_bytes(plan, covered_hir_keys)?)?;
    Some(bytes)
}

/// Count one backing allocation per pointer, including Arc headers and the
/// moved String carrier. Separate allocations with equal bytes remain distinct.
/// The temporary inventory is charged before allocation and is not refunded.
fn shared_identity_bytes(plan: &LoanPlan, covered_hir_keys: &[usize]) -> Option<usize> {
    let mut count = plan.endpoints.len();
    for loan in &plan.loans {
        count = count.checked_add(2)?.checked_add(loan.ends.len())?;
    }
    let inventory_bytes = count.checked_mul(std::mem::size_of::<&ExpressionId>())?;
    if !crate::bounded_output::reserve_active_required(inventory_bytes) {
        return None;
    }
    let mut identities = Vec::with_capacity(count);
    let excess = identities
        .capacity()
        .checked_sub(count)?
        .checked_mul(std::mem::size_of::<&ExpressionId>())?;
    if !crate::bounded_output::reserve_active_required(excess) {
        return None;
    }
    for loan in &plan.loans {
        identities.push(&loan.site);
        identities.push(&loan.start.expression);
        identities.extend(loan.ends.iter().map(|point| &point.expression));
    }
    identities.extend(
        plan.endpoints
            .iter()
            .map(|endpoint| &endpoint.point.expression),
    );
    debug_assert_eq!(identities.len(), count);
    identities.sort_unstable_by_key(|identity| identity.shared_allocation_key());
    let mut previous = None;
    let mut bytes = 0usize;
    for identity in identities {
        let key = identity.shared_allocation_key()?;
        if previous != Some(key) {
            // Validate backing even when its physical allocation is shared
            // with charged HIR. Unmatched equal text still owns separate bytes.
            let owned = identity.shared_allocation_bytes()?;
            if covered_hir_keys.binary_search(&key).is_err() {
                bytes = bytes.checked_add(owned)?;
            }
            previous = Some(key);
        }
    }
    Some(bytes)
}
