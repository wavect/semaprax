//! Exact single-epsilon closure receiver, authenticated by the enclosing memo.
use super::{BTreeMap, Diagnostic, Rc, ReplayBudget, ResolvedFunction};
use std::mem::size_of;

type Closure = Rc<BTreeMap<usize, u64>>;

pub(super) fn share(
    function: &ResolvedFunction,
    inner: &Closure,
    budget: &mut ReplayBudget,
) -> Result<Closure, Diagnostic> {
    // The general producer starts an empty map and inserts each unique child
    // entry once: 0 + routes cannot overflow u64. Retain EACH old work request
    // and its phase/order; do not aggregate away partial exhaustion.
    for _ in inner.iter() {
        budget.charge(function, 1, "factored closure route addition")?;
    }
    budget.charge(
        function,
        inner.len().saturating_add(1),
        "factored skeleton closure",
    )?;
    // The callee's named Rc initializer, caller's named receiving Rc local,
    // and this helper's Result carrier are distinct source constructions. Its
    // child map and Rc allocation remain owned by the already-paid memo entry;
    // Rc::clone acquires no new backing. No absent map/header debit or credit.
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            2 * size_of::<Closure>() + size_of::<Result<Closure, Diagnostic>>(),
            "factored epsilon closure receiving",
        )?;
    }
    budget.reserve_bytes(function, 0, "factored epsilon closure payer authentication")?;
    let shared = Rc::clone(inner);
    Ok(shared)
}

#[cfg(test)]
mod tests;
