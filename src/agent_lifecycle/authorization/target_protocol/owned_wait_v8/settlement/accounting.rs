//! Accounting math only. Source provenance is supplied by the authenticated
//! inventory boundary, never by a caller ledger or a decoded evidence total.
use super::*;

/// A verified exchange total. Its private fields cannot reset a predecessor.
/// This inert proof grants no host, append, cleanup or restoration authority.
#[derive(Clone)]
pub(crate) struct CheckedTargetAccountingV8 {
    total: TargetAccounting,
}
impl CheckedTargetAccountingV8 {
    pub(crate) fn total(&self) -> &TargetAccounting {
        &self.total
    }
}

pub(super) struct ReservedTargetAccountingV8 {
    total: TargetAccounting,
}
pub(super) fn reserve_first(
    request: &[u8],
    limits: TargetLimits,
) -> Result<ReservedTargetAccountingV8, Error> {
    reserve(None, request, limits)
}
pub(super) fn reserve(
    predecessor: Option<&CheckedTargetAccountingV8>,
    request: &[u8],
    limits: TargetLimits,
) -> Result<ReservedTargetAccountingV8, Error> {
    let mut total = predecessor.map_or_else(TargetAccounting::default, |p| p.total);
    total
        .reserve(
            u64::try_from(request.len()).map_err(|_| Error::Capacity)?,
            1,
            limits,
        )
        .map_err(|_| Error::Binding)?;
    Ok(ReservedTargetAccountingV8 { total })
}

/// Uses the same remaining-total basis as the actual bounded target sink.
fn overflow_charge(total: &TargetAccounting, limits: TargetLimits) -> Result<usize, Error> {
    // Frozen TargetResponseSink computes its bound from cumulative request
    // bytes; charge_result separately enforces cumulative result/total bytes.
    let remaining = limits.max_total_bytes.saturating_sub(total.request_bytes());
    usize::try_from(
        limits
            .max_result_bytes
            .min(remaining)
            .min(MAX_CARRIER_BYTES as u64),
    )
    .map_err(|_| Error::Binding)?
    .checked_add(1)
    .ok_or(Error::Binding)
}

pub(super) fn verify(
    reserved: ReservedTargetAccountingV8,
    limits: TargetLimits,
    evidence: &TargetEvidence,
    result_wire: Option<&[u8]>,
) -> Result<CheckedTargetAccountingV8, Error> {
    if !evidence.dispatched() {
        return Err(Error::Binding);
    }
    let mut total = reserved.total;
    match evidence.settlement() {
        Settlement::Returned => {
            let bytes = result_wire.ok_or(Error::Malformed)?;
            total
                .charge_result(bytes.len(), limits)
                .map_err(|_| Error::Binding)?;
        }
        Settlement::HostFailed | Settlement::HostPanicked => {
            if result_wire.is_some() {
                return Err(Error::Malformed);
            }
            total.charge_result(0, limits).map_err(|_| Error::Binding)?;
        }
        Settlement::ResultBudget => {
            if result_wire.is_some() {
                return Err(Error::Malformed);
            }
            let sentinel = overflow_charge(&total, limits)?;
            if total.charge_result(sentinel, limits) != Err(Settlement::ResultBudget) {
                return Err(Error::Binding);
            }
        }
        // No-dispatch and discarded/raw-result charge bases remain closed.
        _ => return Err(Error::Binding),
    }
    if evidence.accounting() != total {
        return Err(Error::Binding);
    }
    Ok(CheckedTargetAccountingV8 { total })
}

#[cfg(test)]
mod tests;
