//! MAIN borrowed sorted outer labels composed with current inlineOne/paidMany.
//! Labels borrow only immutable automaton cells. Legacy outer trees remain in
//! both actual successor branches; this directory is selected only for payers.
use super::*;
use std::mem::size_of;

type Successors = (u64, Vec<(Label, SharedWeighted)>);
pub(super) trait Closed: 'static {
    type Iter<'a>: Iterator<Item = (&'a usize, &'a u64)> where Self: 'a;
    fn ordered(&self) -> Self::Iter<'_>;
}
impl Closed for ordered_weights::OrderedWeights {
    type Iter<'a> = ordered_weights::Iter<'a>;
    fn ordered(&self) -> Self::Iter<'_> { self.iter() }
}
#[cfg(test)]
impl Closed for BTreeMap<usize, u64> {
    type Iter<'a> = std::collections::btree_map::Iter<'a, usize, u64>;
    fn ordered(&self) -> Self::Iter<'_> { self.iter() }
}
// Actual forwarded call/return carriers and both real closed iterators. The
// actual unit/general callers acquire these before forming this new call.
pub(super) fn receiving<C: Closed>() -> usize {
    2 * size_of::<(&ResolvedFunction, &mut ReplayBudget, &Automaton, &C, u64)>()
        + 2 * size_of::<Result<Option<Successors>, Diagnostic>>()
        + size_of::<Option<Successors>>()
        + 2 * size_of::<C::Iter<'static>>()
        + 2 * size_of::<std::slice::Iter<'static, (Label, usize)>>()
        + 2 * size_of::<(&usize, &u64)>()
        + 2 * size_of::<(&Label, &usize)>()
        + 4 * size_of::<Result<(), Diagnostic>>()
}
struct Directory<'a> {
    keys: Vec<&'a Label>,
    values: Vec<Option<paid_many::Targets>>,
}
impl<'a> Directory<'a> {
    fn new(function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<Self, Diagnostic> {
        budget.reserve_bytes(function,
            2 * size_of::<Self>() + 2 * size_of::<Result<Self, Diagnostic>>(),
            "factored label directory headers and receiving")?;
        Ok(Self { keys: Vec::new(), values: Vec::new() })
    }
    fn key(&mut self, function: &ResolvedFunction, budget: &mut ReplayBudget, label: &'a Label) -> Result<(), Diagnostic> {
        budget.reserve_bytes(function, 0, "factored label directory authentication")?;
        let comparisons = (usize::BITS - self.keys.len().leading_zeros()) as usize + 1;
        budget.reserve_bytes(function,
            2 * size_of::<(&mut Self, &ResolvedFunction, &mut ReplayBudget, &Label)>()
                + 2 * size_of::<Result<usize, usize>>()
                + comparisons * (size_of::<&&Label>() + size_of::<std::cmp::Ordering>())
                + 3 * size_of::<usize>() + size_of::<u32>(),
            "factored borrowed label census receivers")?;
        budget.charge(function, comparisons, "factored successor label census search")?;
        if let Err(index) = self.keys.binary_search_by(|key| (*key).cmp(label)) {
            budget.charge(function, self.keys.len() - index + 1, "factored successor label census shifts")?;
            budget.reserve_vec(function, &mut self.keys, 1, "factored successor borrowed labels")?;
            self.keys.insert(index, label);
        }
        Ok(())
    }
    fn prepare(&mut self, function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<(), Diagnostic> {
        budget.reserve_bytes(function,
            2 * size_of::<(&mut Self, &ResolvedFunction, &mut ReplayBudget)>()
                + size_of::<Option<paid_many::Targets>>()
                + size_of::<std::slice::Iter<'_, &'a Label>>()
                + 2 * size_of::<Result<(), Diagnostic>>(),
            "factored successor target table receivers")?;
        budget.charge(function, self.keys.len(), "factored successor target headers")?;
        budget.reserve_vec(function, &mut self.values, self.keys.len(), "factored successor target table")?;
        for _ in &self.keys { self.values.push(None); }
        Ok(())
    }
    fn add(&mut self, function: &ResolvedFunction, budget: &mut ReplayBudget, label: &Label, child: usize, weight: u64) -> Result<bool, Diagnostic> {
        budget.reserve_bytes(function, 0, "factored label target authentication")?;
        let comparisons = (usize::BITS - self.keys.len().leading_zeros()) as usize + 1;
        budget.reserve_bytes(function,
            2 * size_of::<(&mut Self, &ResolvedFunction, &mut ReplayBudget, &Label, usize, u64)>()
                + 2 * size_of::<Result<usize, usize>>()
                + comparisons * (size_of::<&&Label>() + size_of::<std::cmp::Ordering>())
                + 3 * size_of::<usize>() + size_of::<u32>()
                + size_of::<&mut Option<paid_many::Targets>>()
                + 2 * size_of::<Option<u64>>()
                + size_of::<Option<paid_many::Targets>>() + size_of::<paid_many::Targets>(),
            "factored borrowed label target receivers")?;
        budget.charge(function, comparisons, "factored successor label search")?;
        let index = self.keys.binary_search_by(|key| (*key).cmp(label)).map_err(|_| {
            budget.budget_refusal(function, "immutable successor label census changed", "")
        })?;
        let slot = &mut self.values[index];
        match slot {
            None => {
                budget.reserve_bytes(function, 0, "factored singleton target authentication")?;
                let weight = 0_u64.checked_add(weight).expect("zero plus u64 cannot overflow");
                *slot = Some(paid_many::Targets::One { child, weight });
            }
            Some(paid_many::Targets::One { child: first, weight: total }) if *first == child => {
                // Keep repeated-child overflow before any promotion receiver.
                let Some(sum) = total.checked_add(weight) else { return Ok(false); };
                *total = sum;
            }
            Some(paid_many::Targets::One { child: first, weight: total }) => {
                budget.reserve_bytes(function, paid_many::PROMOTE_RECEIVING, "factored composed promotion receivers")?;
                #[cfg(test)]
                if budget.pays_allocations() { paid_many::note_branch(true); }
                *slot = Some(paid_many::promote(function, budget, *first, *total, child, weight)?);
            }
            Some(paid_many::Targets::PaidMany(flat)) => {
                budget.reserve_bytes(function, paid_many::ADD_RECEIVING, "factored composed flat add receivers")?;
                if !paid_many::add(function, budget, flat, child, weight)? { return Ok(false); }
            }
            Some(paid_many::Targets::Many(targets)) => {
                // Reachable only in direct unobserved compatibility controls.
                if !targets.contains_key(&child) {
                    budget.reserve_btree_node::<usize, u64>(function, "factored successor target node")?;
                }
                let target = targets.entry(child).or_insert(0);
                let Some(sum) = target.checked_add(weight) else { return Ok(false); };
                *target = sum;
            }
        }
        Ok(true)
    }
    fn finish(self, function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<Vec<(Label, SharedWeighted)>, Diagnostic> {
        budget.reserve_bytes(function,
            2 * size_of::<Self>()
                + 2 * size_of::<Result<Vec<(Label, SharedWeighted)>, Diagnostic>>()
                + size_of::<std::iter::Zip<std::vec::IntoIter<&'a Label>, std::vec::IntoIter<Option<paid_many::Targets>>>>()
                + size_of::<Vec<(Label, SharedWeighted)>>()
                + size_of::<(&Label, Option<paid_many::Targets>)>()
                + 2 * size_of::<SharedWeighted>() + size_of::<(Label, SharedWeighted)>(),
            "factored borrowed label publication receivers")?;
        let mut result = Vec::new();
        budget.reserve_vec(function, &mut result, self.keys.len(), "factored successor vector")?;
        for (label, targets) in self.keys.into_iter().zip(self.values) {
            let state = match targets {
                Some(paid_many::Targets::One { child, weight }) => {
                    budget.reserve_bytes(function,
                        2 * size_of::<SmallWeighted>() + size_of::<[(usize, u64); 1]>()
                            + size_of::<Result<(), Diagnostic>>(),
                        "factored inline weighted enum and array receiving")?;
                    budget.reserve_bytes(function, physical_clone::rc_backing_bytes::<SmallWeighted>(), "factored weighted shared header")?;
                    Rc::new(SmallWeighted::One([(child, weight)]))
                }
                Some(paid_many::Targets::PaidMany(flat)) => {
                    budget.reserve_bytes(function, paid_many::FINISH_RECEIVING, "factored composed flat finish receivers")?;
                    own_weighted(function, budget, flat)?
                }
                Some(paid_many::Targets::Many(targets)) => weighted_successor(function, budget, targets)?,
                None => return Err(budget.budget_refusal(function, "immutable successor label census changed", "")),
            };
            // Only an Rc observation or Copy terminal: no label payload clone.
            result.push((label.clone(), state));
        }
        Ok(result)
    }
}
pub(super) fn successors<C: Closed>(function: &ResolvedFunction, budget: &mut ReplayBudget, automaton: &Automaton, closed: &C, accepted: u64) -> Result<Option<Successors>, Diagnostic> {
    let mut labels = Directory::new(function, budget)?;
    for (node, _) in closed.ordered() {
        let moves = automaton.nodes[*node].moves();
        budget.charge(function, moves.len().saturating_add(1), "factored successor label census")?;
        for (label, _) in moves { labels.key(function, budget, label)?; }
    }
    labels.prepare(function, budget)?;
    for (node, weight) in closed.ordered() {
        let moves = automaton.nodes[*node].moves();
        budget.charge(function, moves.len().saturating_add(1), "factored skeleton subset construction")?;
        for (label, child) in moves {
            if !labels.add(function, budget, label, *child, *weight)? { return Ok(None); }
        }
    }
    Ok(Some((accepted, labels.finish(function, budget)?)))
}

#[test]
fn joined_outer_directory_keeps_inline_repetition_and_distinct_paid_promotion() {
    let function = super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1;
    crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        let label = Label::End(super::super::SkeletonTerminal::Success);
        let mut directory = Directory::new(&function, &mut budget).unwrap();
        directory.key(&function, &mut budget, &label).unwrap();
        directory.prepare(&function, &mut budget).unwrap();
        assert!(directory.add(&function, &mut budget, &label, 5, 0).unwrap());
        assert!(matches!(directory.values[0], Some(paid_many::Targets::One { child: 5, weight: 0 })));
        assert!(directory.add(&function, &mut budget, &label, 5, 2).unwrap());
        assert!(matches!(directory.values[0], Some(paid_many::Targets::One { child: 5, weight: 2 })));
        assert!(directory.add(&function, &mut budget, &label, 1, 3).unwrap());
        assert!(matches!(directory.values[0], Some(paid_many::Targets::PaidMany(_))));
        let result = directory.finish(&function, &mut budget).unwrap();
        assert_eq!(result[0].1.as_ref().as_slice(), &[(1, 3), (5, 2)]);
        let mut directory = Directory::new(&function, &mut budget).unwrap();
        directory.key(&function, &mut budget, &label).unwrap();
        directory.prepare(&function, &mut budget).unwrap();
        assert!(directory.add(&function, &mut budget, &label, 9, u64::MAX).unwrap());
        assert!(!directory.add(&function, &mut budget, &label, 9, 1).unwrap());
        assert!(matches!(directory.values[0], Some(paid_many::Targets::One { child: 9, weight: u64::MAX })));
    });
}
#[test]
fn joined_outer_headers_and_target_slots_use_actual_layouts() {
    struct PreviousDirectory<'a> { keys: Vec<&'a Label>, values: Vec<Weighted> }
    let previous = PreviousDirectory { keys: Vec::new(), values: Vec::new() };
    assert!(previous.keys.is_empty() && previous.values.is_empty());
    assert_eq!(size_of::<Directory<'_>>(), size_of::<PreviousDirectory<'_>>());
    assert_eq!(size_of::<&Label>(), size_of::<usize>());
    // Slots may widen relative to MAIN's eager Vec headers. prepare reserves
    // the actual Option<Targets> physical table; no old-header credit is used.
    assert!(size_of::<Option<paid_many::Targets>>() >= size_of::<paid_many::Targets>());
    assert!(receiving::<ordered_weights::OrderedWeights>() >= 2 * size_of::<ordered_weights::Iter<'_>>());
}
