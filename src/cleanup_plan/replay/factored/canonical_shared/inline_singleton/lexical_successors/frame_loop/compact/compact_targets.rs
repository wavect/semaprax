//! Complete paid target promotion and insertion; only error transport narrows.
//! The original producers and all their original control banks remain compiled.
use super::*;
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
use paid_many::Targets;
use std::mem::size_of;
pub(super) const PROMOTE_RECEIVING: usize = 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 4 * size_of::<usize>()
    + 5 * size_of::<u64>()
    + 2 * size_of::<Targets>()
    + 2 * size_of::<Result<Targets, ByteFailure>>()
    + 2 * size_of::<Weighted>()
    + 4 * size_of::<(usize, u64)>()
    + size_of::<Option<u64>>()
    + 3 * size_of::<bool>() // distinct assertion, payer branch, sorted order
    + 2 * size_of::<&usize>() // assert_ne borrowed arguments
    + size_of::<(&usize, &usize)>()
    + 2 * size_of::<&mut Weighted>() // fallible growth owner argument/receiving
    + 6 * size_of::<Result<(), ByteFailure>>();
pub(super) const ADD_RECEIVING: usize = 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&mut Weighted>()
    + 6 * size_of::<usize>() // child argument/receiving, bound, index, len, shifts
    + 2 * size_of::<u32>() // leading-zero input/result
    + 3 * size_of::<u64>()
    + 2 * size_of::<bool>()
    + 2 * size_of::<Result<bool, ByteFailure>>()
    + 2 * size_of::<Result<usize, usize>>()
    + size_of::<Option<u64>>()
    + 2 * size_of::<fn(&(usize, u64)) -> usize>()
    + 2 * size_of::<&[(usize, u64)]>() // binary-search borrowed receiver
    + size_of::<&usize>()
    + size_of::<(usize, u64)>()
    + 6 * size_of::<Result<(), ByteFailure>>();
pub(super) fn promote(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    first: usize,
    total: u64,
    child: usize,
    weight: u64,
) -> Result<Targets, ByteFailure> {
    assert_ne!(first, child);
    budget.reserve_compact_bytes(
        function,
        0,
        "factored successor target promotion authentication",
    )?;
    if !budget.pays_allocations() {
        // Complete original tree construction for the ordinary legacy route.
        // Both original node acquisitions precede either insertion.
        budget
            .reserve_compact_btree_node::<usize, u64>(function, "factored successor target node")?;
        budget
            .reserve_compact_btree_node::<usize, u64>(function, "factored successor target node")?;
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
    budget.charge_compact(function, 1, "factored flat promotion key comparison")?;
    budget.charge_compact(function, 2, "factored flat promotion insertions")?;
    let mut promoted = Vec::new();
    // Use the existing fallible physical owner, including its exact geometric
    // capacity, allocation-failure receipt and capacity-mismatch refusal.
    budget.reserve_compact_vec(
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
) -> Result<bool, ByteFailure> {
    budget.reserve_compact_bytes(function, 0, "factored flat target authentication")?;
    // Same binary-search bound as the complete incoming flat receiver. Price
    // every possible callback argument/local/result slot before forming it.
    let comparisons = (usize::BITS - targets.len().leading_zeros()) as usize + 1;
    budget.reserve_compact_bytes(
        function,
        comparisons * (size_of::<&(usize, u64)>() + 2 * size_of::<usize>()),
        "factored flat search callback receivers",
    )?;
    budget.charge_compact(function, comparisons, "factored target search")?;
    match targets.binary_search_by_key(&child, child_key as fn(&(usize, u64)) -> usize) {
        Ok(index) => {
            let Some(sum) = targets[index].1.checked_add(weight) else {
                return Ok(false);
            };
            targets[index].1 = sum;
        }
        Err(index) => {
            budget.charge_compact(
                function,
                targets.len() - index + 1,
                "factored target insertion and shifts",
            )?;
            budget.reserve_compact_vec(function, targets, 1, "factored target weights")?;
            targets.insert(index, (child, weight));
        }
    }
    Ok(true)
}

// The promotion bank conservatively includes all six lexical unit producers:
// zero authentication, two legacy tree nodes, two work calls, and vector growth.
const _: () = assert!(
    paid_many::PROMOTE_RECEIVING - PROMOTE_RECEIVING
        == 2 * (size_of::<Result<Targets, Diagnostic>>()
            - size_of::<Result<Targets, ByteFailure>>())
            + 5 * size_of::<Result<(), Diagnostic>>()
            - 6 * size_of::<Result<(), ByteFailure>>()
);
const _: () = assert!(
    paid_many::ADD_RECEIVING - ADD_RECEIVING
        == 2 * (size_of::<Result<bool, Diagnostic>>() - size_of::<Result<bool, ByteFailure>>())
            + 6 * (size_of::<Result<(), Diagnostic>>() - size_of::<Result<(), ByteFailure>>())
);

#[cfg(test)]
mod tests {
    use super::*;
    fn contents(targets: Targets) -> (u8, Vec<(usize, u64)>) {
        match targets {
            Targets::One { child, weight } => (0, vec![(child, weight)]),
            Targets::Many(map) => (1, map.into_iter().collect()),
            Targets::PaidMany(rows) => (2, rows),
        }
    }
    #[test]
    fn complete_target_producers_preserve_sorted_weights_overflow_work_and_diagnostics() {
        let checked = crate::check(
            "module compact_targets; @id(\"main\") fn main()->i64 {0}",
            "targets.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = &program.functions[0];
        for allowance in [0, 1, 2, 3, 4, 16, usize::MAX] {
            for weight in [0, 1, u64::MAX] {
                let run = |compact| {
                    let ((result, spent, remaining), refused, _, _) =
                        crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                            let mut budget = ReplayBudget::with_skeleton_limit(allowance);
                            let result = (|| {
                                let mut targets = if compact {
                                    promote(function, &mut budget, 8, weight, 2, weight)
                                        .map_err(|e| e.into_diagnostic())?
                                } else {
                                    paid_many::promote(function, &mut budget, 8, weight, 2, weight)?
                                };
                                let Targets::PaidMany(rows) = &mut targets else {
                                    panic!("actual active payer must use paid flat backing")
                                };
                                let repeated = if compact {
                                    add(function, &mut budget, rows, 2, weight)
                                        .map_err(|e| e.into_diagnostic())?
                                } else {
                                    paid_many::add(function, &mut budget, rows, 2, weight)?
                                };
                                let inserted = if compact {
                                    add(function, &mut budget, rows, 5, 1)
                                        .map_err(|e| e.into_diagnostic())?
                                } else {
                                    paid_many::add(function, &mut budget, rows, 5, 1)?
                                };
                                Ok::<_, Diagnostic>((targets, repeated, inserted))
                            })();
                            (result, budget.skeleton_spent, budget.skeleton_remaining)
                        });
                    (
                        result
                            .map(|(targets, a, b)| (contents(targets), a, b))
                            .map_err(|e| e.json()),
                        spent,
                        remaining,
                        refused,
                    )
                };
                assert_eq!(run(true), run(false), "work={allowance},weight={weight}");
            }
        }
        let mut old = ReplayBudget::unobserved();
        let mut new = ReplayBudget::unobserved();
        assert_eq!(
            contents(paid_many::promote(function, &mut old, 8, 3, 2, 4).unwrap()),
            contents(
                promote(function, &mut new, 8, 3, 2, 4)
                    .map_err(|e| e.into_diagnostic())
                    .unwrap()
            )
        );
    }
}
