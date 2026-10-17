//! A single unit-weight input already owns exactly the complete closed map.
//! Its immutable memo backing is moved into the receiver, never rebuilt.
use super::*;
use std::mem::size_of;
type Closure = Rc<BTreeMap<usize, u64>>;

pub(super) const RECEIVER_BYTES: usize =
    2 * size_of::<Option<Closure>>() + size_of::<&BTreeMap<usize, u64>>();

pub(super) fn receive(
    function: &ResolvedFunction,
    closure: Closure,
    budget: &mut ReplayBudget,
) -> Result<Closure, Diagnostic> {
    // For every unique key the old producer computes routes * 1 + 0. This is
    // exact even at u64::MAX. Keep every original work call and its phase.
    for _ in closure.iter() {
        budget.charge(function, 1, "factored weighted route addition")?;
    }
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            size_of::<Closure>() + size_of::<Result<Closure, Diagnostic>>(),
            "factored unit-weight closure receiving",
        )?;
    }
    budget.reserve_bytes(function, 0, "factored unit-weight closure authentication")?;
    Ok(closure)
}

#[cfg(test)]
mod tests;
