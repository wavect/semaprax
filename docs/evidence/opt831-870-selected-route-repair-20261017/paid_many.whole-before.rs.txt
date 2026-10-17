//! Inline first/repeated targets; only distinct paid targets own flat backing.
use super::{BTreeMap, Diagnostic, ReplayBudget, ResolvedFunction, Weighted};
use std::mem::size_of;

pub(super) enum Targets {
    One { child: usize, weight: u64 },
    Many(BTreeMap<usize, u64>),
    PaidMany(Weighted),
}
// Caller admission precedes argument copying, helper entry, Vec construction,
// return/receiving and enum publication. Old bills provide no credit.
pub(super) const PROMOTE_RECEIVING: usize = 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 4 * size_of::<usize>()
    + 5 * size_of::<u64>()
    + 2 * size_of::<Targets>()
    + 2 * size_of::<Result<Targets, Diagnostic>>()
    + 2 * size_of::<Weighted>()
    + 4 * size_of::<(usize, u64)>()
    + size_of::<Option<u64>>()
    + 3 * size_of::<bool>() // distinct assertion, payer branch, sorted order
    + 2 * size_of::<&usize>() // assert_ne borrowed arguments
    + size_of::<(&usize, &usize)>()
    + 2 * size_of::<&mut Weighted>() // fallible growth owner argument/receiving
    + 5 * size_of::<Result<(), Diagnostic>>();
pub(super) const ADD_RECEIVING: usize = 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&mut Weighted>()
    + 6 * size_of::<usize>() // child argument/receiving, bound, index, len, shifts
    + 2 * size_of::<u32>() // leading-zero input/result
    + 3 * size_of::<u64>()
    + 2 * size_of::<bool>()
    + 2 * size_of::<Result<bool, Diagnostic>>()
    + 2 * size_of::<Result<usize, usize>>()
    + size_of::<Option<u64>>()
    + 2 * size_of::<fn(&(usize, u64)) -> usize>()
    + 2 * size_of::<&[(usize, u64)]>() // binary-search borrowed receiver
    + size_of::<&usize>()
    + size_of::<(usize, u64)>()
    + 6 * size_of::<Result<(), Diagnostic>>();
pub(super) const FINISH_RECEIVING: usize = 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<Weighted>()
    + size_of::<Result<super::SharedWeighted, Diagnostic>>()
    + size_of::<Result<(), Diagnostic>>();

pub(super) fn promote(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    first: usize,
    total: u64,
    child: usize,
    weight: u64,
) -> Result<Targets, Diagnostic> {
    assert_ne!(first, child);
    budget.reserve_bytes(
        function,
        0,
        "factored successor target promotion authentication",
    )?;
    if !budget.pays_allocations() {
        // Complete original tree construction for the ordinary legacy route.
        // Both original node acquisitions precede either insertion.
        budget.reserve_btree_node::<usize, u64>(function, "factored successor target node")?;
        budget.reserve_btree_node::<usize, u64>(function, "factored successor target node")?;
        let mut promoted = BTreeMap::new();
        promoted.insert(first, total);
        let sum = 0_u64
            .checked_add(weight)
            .expect("zero plus u64 cannot overflow");
        promoted.insert(child, sum);
        return Ok(Targets::Many(promoted));
    }
    // One real key comparison and two tuple insertions, acquired before either
    // comparison or publication. Allocation failure leaves the caller's One.
    budget.charge(function, 1, "factored flat promotion key comparison")?;
    budget.charge(function, 2, "factored flat promotion insertions")?;
    let mut promoted = Vec::new();
    // Use the existing fallible physical owner, including its exact geometric
    // capacity, allocation-failure receipt and capacity-mismatch refusal.
    budget.reserve_vec(
        function,
        &mut promoted,
        2,
        "factored flat promotion backing",
    )?;
    let sum = 0_u64
        .checked_add(weight)
        .expect("zero plus u64 cannot overflow");
    if first < child {
        promoted.push((first, total));
        promoted.push((child, sum));
    } else {
        promoted.push((child, sum));
        promoted.push((first, total));
    }
    Ok(Targets::PaidMany(promoted))
}
fn child_key(pair: &(usize, u64)) -> usize {
    pair.0
}
pub(super) fn add(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    targets: &mut Weighted,
    child: usize,
    weight: u64,
) -> Result<bool, Diagnostic> {
    budget.reserve_bytes(function, 0, "factored flat target authentication")?;
    // Same binary-search bound as the complete incoming flat receiver. Price
    // every possible callback argument/local/result slot before forming it.
    let comparisons = (usize::BITS - targets.len().leading_zeros()) as usize + 1;
    budget.reserve_bytes(
        function,
        comparisons * (size_of::<&(usize, u64)>() + 2 * size_of::<usize>()),
        "factored flat search callback receivers",
    )?;
    budget.charge(function, comparisons, "factored target search")?;
    match targets.binary_search_by_key(&child, child_key as fn(&(usize, u64)) -> usize) {
        Ok(index) => {
            let Some(sum) = targets[index].1.checked_add(weight) else {
                return Ok(false);
            };
            targets[index].1 = sum;
        }
        Err(index) => {
            budget.charge(
                function,
                targets.len() - index + 1,
                "factored target insertion and shifts",
            )?;
            budget.reserve_vec(function, targets, 1, "factored target weights")?;
            targets.insert(index, (child, weight));
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "paid_many/source.rs"]
mod source;
#[cfg(test)]
#[path = "paid_many/tests.rs"]
mod tests;

// Const TLS records actual production acquisition paths without allocating,
// selecting a payer, or changing a work/byte limit.
#[cfg(test)]
thread_local! {
    static ENTRIES: std::cell::Cell<(usize, usize)> = const {
        std::cell::Cell::new((0, 0))
    };
}
#[cfg(test)]
pub(super) fn note_branch(unit: bool) {
    ENTRIES.with(|value| {
        let (unit_count, general_count) = value.get();
        value.set((
            unit_count + usize::from(unit),
            general_count + usize::from(!unit),
        ));
    });
}

// One lexical payment replaces the owned helper and both forwarding Results.
// Two payment tuples are its actual/formal arguments; two Result<(), Diagnostic>
// slots are its returned/caller result. Remaining Vec/enum/Rc move slots are
// independently funded. No new owned Targets binding exists, even for the
// optional caller. TOTAL uses the exact existing Rc layout at compile time:
// no runtime helper return, addition or intermediate byte operand is formed.
// The caller already owns the flat backing; the same actual Rc backing bytes
// are acquired in this single request before any Vec extraction or publication.
pub(super) const LEXICAL_FINISH_RECEIVING: usize = size_of::<Weighted>()
    + 2 * size_of::<super::SmallWeighted>()
    + 2 * size_of::<super::SharedWeighted>()
    + 2 * size_of::<Result<(), Diagnostic>>()
    + 2 * size_of::<(&ResolvedFunction, &mut ReplayBudget, usize, &str)>();
pub(super) const LEXICAL_FINISH_TOTAL: usize = LEXICAL_FINISH_RECEIVING
    + super::physical_clone::rc_backing_bytes::<super::SmallWeighted>();

macro_rules! finish {
    ($function:ident, $budget:ident, $targets:ident) => {
        paid_many::finish!($function, $budget, $targets,
            paid_many::Targets::PaidMany(flat) => flat)
    };
    ($function:ident, $budget:ident, $targets:ident, optional) => {
        paid_many::finish!($function, $budget, $targets,
            Some(paid_many::Targets::PaidMany(flat)) => flat)
    };
    ($function:ident, $budget:ident, $targets:ident, $pattern:pat => $flat:ident) => {{
        $budget.reserve_bytes($function,
            paid_many::LEXICAL_FINISH_TOTAL,
            "factored lexical flat publication receivers")?;
        #[cfg(test)]
        paid_many::note_publication();
        let $pattern = $targets else { unreachable!("paid flat publication") };
        Rc::new(SmallWeighted::Many($flat))
    }};
}
pub(super) use finish;

#[cfg(test)]
#[path = "paid_many/lexical_tests.rs"]
mod lexical_tests;

#[cfg(test)]
thread_local! {
    static PUBLICATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
pub(super) fn note_publication() {
    PUBLICATIONS.with(|value| value.set(value.get() + 1));
}
