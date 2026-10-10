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
    clone_capacity_bytes(plan)?.checked_add(shared_identity_bytes(plan, covered_hir_keys)?)
}

/// Capacity that can shrink when cloning a plan. ExpressionId backing is
/// immutable and shared by Clone, so its physical debit cancels in a clone
/// difference. This census allocates no scratch and never changes the full
/// physical-storage census used for retained proofs or snapshot reconstruction.
pub(crate) fn clone_capacity_bytes(plan: &LoanPlan) -> Option<usize> {
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
    Some(bytes)
}

/// Count one backing allocation per pointer, including Arc headers and the
/// moved String carrier. Separate allocations with equal bytes remain distinct.
/// The temporary inventory is charged before allocation and is not refunded.
fn shared_identity_bytes(plan: &LoanPlan, covered_hir_keys: &[usize]) -> Option<usize> {
    let mut count = 0usize;
    // Validate every physical backing before allocating scratch. The caller
    // already owns and charged its sorted HIR keys; covered references need
    // neither a second physical debit nor an inventory entry here.
    for identity in proof_identities(plan) {
        let key = identity.shared_allocation_key()?;
        identity.shared_allocation_bytes()?;
        if covered_hir_keys.binary_search(&key).is_err() {
            count = count.checked_add(1)?;
        }
    }
    if count == 0 {
        return Some(0);
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
    for identity in proof_identities(plan) {
        if covered_hir_keys
            .binary_search(&identity.shared_allocation_key()?)
            .is_err()
        {
            identities.push(identity);
        }
    }
    debug_assert_eq!(identities.len(), count);
    identities.sort_unstable_by_key(|identity| identity.shared_allocation_key());
    let mut previous = None;
    let mut bytes = 0usize;
    for identity in identities {
        let key = identity.shared_allocation_key()?;
        if previous != Some(key) {
            // Equal bytes in independent allocations still own separate bytes.
            bytes = bytes.checked_add(identity.shared_allocation_bytes()?)?;
            previous = Some(key);
        }
    }
    Some(bytes)
}

fn proof_identities(plan: &LoanPlan) -> impl Iterator<Item = &ExpressionId> {
    plan.loans
        .iter()
        .flat_map(|loan| {
            [&loan.site, &loan.start.expression]
                .into_iter()
                .chain(loan.ends.iter().map(|point| &point.expression))
        })
        .chain(
            plan.endpoints
                .iter()
                .map(|endpoint| &endpoint.point.expression),
        )
}
