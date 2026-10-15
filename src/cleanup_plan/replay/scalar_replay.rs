//! Physically metered proof replay for independently source-bound scalar helpers.
//! Full structure and path states precede exact observation equality.
use super::*;
pub(super) mod records;
#[cfg(test)]
pub(super) fn validate(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    validate_core(program, function, budget, |function, budget| {
        // This facade compares error policies on the same genuine Arena route.
        // The complete independent Tree driver remains compiled for its own
        // semantic and representation-accounting controls.
        if budget.physical.is_none() {
            return Err(replay_error(
                function,
                "selected scalar helper lacks backing",
            ));
        }
        expression_facts::with_budget(function, budget)
    })
}

pub(super) fn validate_paid(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), expression_facts::errors::EngineError> {
    validate_core(program, function, budget, expression_facts::paid_arena)
}

fn validate_core<'hir, E: structural_control::call_receiving::Policy>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    build_facts: impl FnOnce(
        &'hir ResolvedFunction,
        &mut ReplayBudget,
    ) -> Result<expression_facts::Facts<'hir>, E>,
) -> Result<(), E> {
    if !records::signature(program, function, budget)? {
        return Err(replay_error(
            function,
            "selected helper signature is outside Copy scalars",
        )
        .into());
    }
    #[cfg(test)]
    let borrowed = program.borrowed_helper_lease(function);
    #[cfg(test)]
    let has_borrowed = borrowed.is_some();
    #[cfg(not(test))]
    let has_borrowed = false;
    if !has_borrowed {
        scalar_nodes(program, function, budget)?;
    }
    selected_profile::validate(program, function, budget)?;
    selected_owner::owner(program, function, budget)?;
    if !has_borrowed
        && (function.loan_plan.schema != crate::loan_plan::LOAN_PLAN_SCHEMA_V1
            || !function.loan_plan.loans.is_empty()
            || !function.loan_plan.endpoints.is_empty()
            || !function.loan_plan.edges.is_empty())
    {
        return Err(replay_error(
            function,
            "pure scalar helper carries an unexpected loan proof",
        )
        .into());
    }
    #[cfg(test)]
    let selected_schema = match &borrowed {
        Some(lease) => lease.schema(program, function, budget)?,
        None => schema::selected_schema_with_index(program, function, None)?,
    };
    #[cfg(not(test))]
    let selected_schema = schema::selected_schema_with_index(program, function, None)?;
    let plan = &function.cleanup_plan;
    if !budget.selected() || plan.lexical_control.is_some() || plan.schema != selected_schema {
        return Err(replay_error(
            function,
            "selected scalar helper requires its exact nonlexical schema",
        )
        .into());
    }
    budget.charge(
        function,
        plan_structure_units(plan),
        "selected scalar helper structure",
    )?;
    let ordinary_facts;
    #[cfg(test)]
    let borrowed_facts;
    let facts: &dyn expression_facts::Lookup;
    #[cfg(test)]
    if has_borrowed {
        borrowed_facts = expression_facts::BorrowedFacts::build(function, budget)?;
        facts = &borrowed_facts;
    } else {
        ordinary_facts = build_facts(function, budget)?;
        facts = &ordinary_facts;
    }
    #[cfg(not(test))]
    {
        ordinary_facts = build_facts(function, budget)?;
        facts = &ordinary_facts;
    }
    inventory_coverage::validate_inventory_coverage(program, function, budget)?;
    status_construction::validate_required_status_sources(program, function, budget)?;
    let storage = structural_maps::validate_slots_core::<E>(function, budget)?;
    let leaves = structural_maps::collect_leaves(function, budget)?;
    status_validation::validate_core::<E>(function, facts, budget)?;
    structural_maps::validate_regions(function, &storage, budget)?;
    structural_control::validate_entry(function, &storage, &leaves, budget)?;
    let renewals = program
        .primitive_renewals(function)
        .ok_or_else(|| replay_error(function, "selected scalar helper lacks its function lease"))?;
    structural_control::validate_blocks_and_edges_paid::<E>(
        program, function, facts, &storage, &leaves, renewals, budget,
    )?;
    structural_control::validate_exits(
        program,
        function,
        &storage,
        &leaves,
        &BTreeSet::new(),
        budget,
    )?;
    structural_maps::validate_reference_coverage(function, budget)?;
    structural_control::validate_reachable_acyclic_cfg(function, budget)?;
    validate_path_states::<E>(program, function, &storage, &leaves, budget, None, None)?;
    match factored::compare_with_index(program, function, budget, None)? {
        Some(true) => Ok(()),
        Some(false) => Err(replay_error(
            function,
            "selected scalar helper control trace disagrees with typed HIR",
        )
        .into()),
        None => Err(replay_error(
            function,
            "selected scalar helper trace multiplicity overflowed",
        )
        .into()),
    }
}

fn scalar_nodes(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    type Frame<'a> = Option<(crate::hir::lexical_exit::ChildCursor<'a>, bool)>;
    budget.reserve_array::<Frame<'_>>(function, 514, "selected scalar body profile cursor")?;
    let mut stack = [None; 514];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        stack[0] = Some((crate::hir::lexical_exit::ChildCursor::new(root), false));
        let mut depth = 1;
        while depth != 0 {
            budget.charge(function, 1, "selected scalar body profile node")?;
            let (mut cursor, entered) = stack[depth - 1]
                .take()
                .ok_or_else(|| replay_error(function, "scalar profile cursor is empty"))?;
            let record = if !entered {
                records::expression(program, function, cursor.expression(), budget)?
            } else {
                None
            };
            if !entered
                && !match record {
                    Some(admitted) => admitted,
                    None => crate::hir::borrowed_scalar::expression(
                        function,
                        cursor.expression(),
                        &mut |units| {
                            budget.charge(function, units, "selected borrowed helper root identity")
                        },
                    )?,
                }
            {
                return Err(replay_error(
                    function,
                    "selected scalar helper body contains non-Copy storage",
                ));
            }
            if !entered
                && matches!(
                    &cursor.expression().kind,
                    ResolvedExprKind::LexicalExit { .. }
                )
            {
                return Err(replay_error(
                    function,
                    "selected scalar helper contains lexical control",
                ));
            }
            if let Some(child) = cursor.next() {
                if depth == 514 {
                    return Err(replay_error(function, "scalar helper exceeds HIR depth"));
                }
                stack[depth - 1] = Some((cursor, true));
                stack[depth] = Some((crate::hir::lexical_exit::ChildCursor::new(child), false));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    Ok(())
}
