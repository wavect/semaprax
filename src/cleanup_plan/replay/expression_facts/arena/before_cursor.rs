use super::producer_before_cursor as producer;
use super::*;

pub(super) fn build<'hir, P: errors::Policy>(
    function: &'hir ResolvedFunction,
    payer: capacity::Payer<'_>,
    budget: &mut ReplayBudget,
) -> Result<Arena<'hir>, P::Error> {
    let mut capacity = capacity::FactCapacity::with_payer(function, payer)
        .map_err(|failure| P::capacity(function, failure))?;
    budget.charge(function, 1, "expression-fact arena owner construction")?;
    let mut facts = Arena::new(&mut capacity).map_err(|failure| P::capacity(function, failure))?;
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        producer::collect_arena_facts::<P>(
            function,
            expression,
            &mut facts,
            Some(&mut capacity),
            budget,
        )?;
    }
    Ok(facts)
}
