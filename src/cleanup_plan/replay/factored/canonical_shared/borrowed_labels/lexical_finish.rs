//! Same immutable census and target traversal; lexical paid publication only.
use super::*;
impl<'a> Directory<'a> {
    fn finish_lexical(self, function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<Vec<(Label, SharedWeighted)>, Diagnostic> {
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
                Some(paid_many::Targets::PaidMany(_)) => {
                    paid_many::finish!(function, budget, targets, optional)
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
    Ok(Some((accepted, labels.finish_lexical(function, budget)?)))
}
