//! Reached-capacity backing for the ordinary unobserved continuation owner.
//! The old 516-frame reservation/depth and every old traversal debit remain.
//! No observed/selected/oracle payer enters this additional physical strategy.
use super::*;
use std::mem::size_of;

type Acquisition<'a> = Result<(Lease, Vec<Frame<'a>>), Diagnostic>;
// Fresh helper actual/formal loans and returned/received forwarded sum. The
// existing outer acquisition's own sum and original owner/body roles remain.
pub(super) const ACQUIRE_CALL: usize =
    2 * size_of::<(&mut Storage<'static>, &ResolvedFunction, &mut ReplayBudget)>()
        + 2 * size_of::<Acquisition<'static>>()
        + 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
        + 2 * size_of::<Result<(), Diagnostic>>();
// This renewed bank precedes every new grow helper's actual arguments. Its
// payment/charge/backing calls, arithmetic/borrowed Vec::append receivers,
// moved owner, fallible sums, allocation/deallocation layout/pointer roles are
// distinct from the immutable original full reservation retained by fresh().
pub(super) const GROW_CALL: usize = 2 * size_of::<(
    &mut Vec<Frame<'static>>,
    &ResolvedFunction,
    &mut ReplayBudget,
)>() + 2 * size_of::<Result<(), Diagnostic>>()
    + 8 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 10 * size_of::<Result<(), Diagnostic>>()
    + 5 * size_of::<Vec<Frame<'static>>>()
    + 8 * size_of::<&mut Vec<Frame<'static>>>()
    + 4 * size_of::<&Vec<Frame<'static>>>()
    + 32 * size_of::<usize>() // length/capacity and saturating/max/clamp arithmetic carriers
    + 8 * size_of::<bool>()
    + 4 * size_of::<std::alloc::Layout>()
    + 8 * size_of::<std::ptr::NonNull<Frame<'static>>>();

impl<'hir> Storage<'hir> {
    // The original gateway already performed retained-scope authentication.
    // Only a current lent LegacyUnobserved place enters, after ACQUIRE_CALL admits
    // its new forwarding carriers; no borrowed program is authority by itself.
    pub(super) fn acquire_reached(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Acquisition<'hir> {
        if self.frames.is_some() {
            let frames = self.frames.as_mut().unwrap();
            debug_assert!(frames.is_empty());
            if frames.capacity() == 0 {
                *frames = if self.receiving_owner.is_some() && !budget.lent_skeleton {
                    fresh_receiving(function, budget)?
                } else {
                    fresh(function, budget)?
                };
            } else {
                budget.reserve_bytes(function, 0, "typed HIR continuation stack")?;
            }
            return Ok((Lease::Top, self.frames.take().unwrap()));
        }
        #[cfg(test)]
        if legacy_nested() {
            return Ok((Lease::Local, fresh(function, budget)?));
        }
        if self.nested.is_none() {
            budget.reserve_array::<Nested<'hir>>(function, 1, "nested continuation carrier")?;
            self.nested = Some(Box::new(Nested {
                depth: 0,
                slots: Vec::new(),
            }));
        }
        let nested = self.nested.as_mut().unwrap();
        let depth = nested.depth;
        let next = depth
            .checked_add(1)
            .ok_or_else(|| replay_error(function, "nested continuation depth overflowed"))?;
        if depth == nested.slots.len() {
            // Replacement is allocated while the old directory remains live.
            // Pay its complete layout, not merely its capacity delta.
            budget.reserve_array::<Option<Vec<Frame<'hir>>>>(
                function,
                next,
                "nested continuation directory",
            )?;
            let mut replacement = Vec::with_capacity(next);
            replacement.append(&mut nested.slots);
            replacement.push(None);
            nested.slots = replacement;
        }
        debug_assert!(nested.slots[depth].as_ref().is_none_or(Vec::is_empty));
        if nested.slots[depth].is_none() {
            nested.slots[depth] =
                Some(if self.receiving_owner.is_some() && !budget.lent_skeleton {
                    fresh_receiving(function, budget)?
                } else {
                    fresh(function, budget)?
                });
        } else {
            budget.reserve_bytes(function, 0, "typed HIR continuation stack")?;
        }
        let frames = nested.slots[depth].take().unwrap();
        nested.depth = next;
        #[cfg(test)]
        record_nested_entry(depth, frames.as_ptr() as usize);
        Ok((Lease::Nested(depth), frames))
    }
}

pub(super) fn grow(
    frames: &mut Vec<Frame<'_>>,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    // Same live-frame relocation debit as the current observed strategy. The
    // budget's existing charge() routes a lent allowance to charge_skeleton;
    // no old debit, scope or allowance is reclassified or replenished.
    let units = 8 + 2 * frames.len();
    budget.charge(function, units, "held continuation relocation")?;
    let target = frames
        .capacity()
        .saturating_mul(2)
        .clamp((frames.len() + 1).max(2), FRAME_CAPACITY);
    budget.reserve_array::<Frame<'_>>(function, target, "held continuation replacement backing")?;
    #[cfg(test)]
    note_growth(units, target);
    let mut replacement = Vec::with_capacity(target);
    replacement.append(frames);
    *frames = replacement;
    Ok(())
}

#[cfg(test)]
#[derive(Clone, Copy, Default, Debug)]
pub(super) struct Receipt {
    pub(super) growths: usize,
    pub(super) work: usize,
    pub(super) allocated_slots: usize,
}
#[cfg(test)]
thread_local! {
    static RECEIPT: std::cell::Cell<Receipt> = const { std::cell::Cell::new(Receipt {growths:0, work:0, allocated_slots:0}) };
}
#[cfg(test)]
fn note_growth(work: usize, slots: usize) {
    RECEIPT.with(|cell| {
        let mut r = cell.get();
        r.growths = r.growths.saturating_add(1);
        r.work = r.work.saturating_add(work);
        r.allocated_slots = r.allocated_slots.saturating_add(slots);
        cell.set(r);
    });
}
#[cfg(test)]
pub(super) fn receipt() -> Receipt {
    RECEIPT.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(super) fn reset() {
    RECEIPT.with(|r| r.set(Receipt::default()));
}
