//! Independent replay driver for cleanup-plan v18 lexical-profile functions.
//!
//! The order is deliberate. The HIR lexical contour is replayed first, so no
//! certificate is read before its meaning is independently known. Structural
//! identity, region, status and call-commit validation then reuses the legacy
//! validators unchanged, certificates are authenticated against both the HIR
//! facts and the plan, and only then do the all-path ownership replay and the
//! typed control skeleton run over the acyclic proof CFG. A failed or stale
//! certificate never reaches a backend.
use super::*;
use crate::ast::LexicalExitKind;
#[cfg(test)]
#[allow(
    dead_code,
    reason = "complete independently compiled original lexical caller"
)]
mod before_operands;
#[cfg(test)]
mod operand_tests;
mod ordinary;

pub(super) fn validate_with_index(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<(), Diagnostic> {
    validate_core(
        program,
        function,
        budget,
        helpers,
        expression_facts::with_budget,
    )
}

pub(super) fn validate_paid(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), expression_facts::errors::EngineError> {
    if !budget.selected() && budget.pays_allocations() {
        return ordinary::validate_core(
            program,
            function,
            budget,
            None,
            expression_facts::paid_arena,
        );
    }
    let plan = &function.cleanup_plan;
    selected_profile::paid::validate(program, function, budget)?;
    let owner = selected_owner::paid::owner(program, function, budget)?;
    crate::hir::lexical_exit::validate_function(program, function, &owner)?;
    // The lexical resource plan was deferred before enumerating path refusal.
    // Bound structure now, but construct no plan automaton until every exact
    // lexical/certificate/ownership check below has authenticated its meaning.
    let structure_units = plan_structure_units(plan);
    if structure_units > MAX_REPLAY_WORK_UNITS {
        return Err(replay_error(
            function,
            "cleanup replay structure exceeds the global work budget",
        )
        .into());
    }
    budget.charge(function, structure_units, "structural validation")?;
    let expression_facts = expression_facts::paid_arena(function, budget)?;
    let selected = budget.selected();
    let (storage, leaves) = if selected {
        inventory_coverage::validate_inventory_coverage(program, function, budget)?;
        status_construction::validate_required_status_sources(program, function, budget)?;
        let storage = structural_maps::validate_slots_core::<expression_facts::errors::EngineError>(
            function, budget,
        )?;
        let leaves = structural_maps::collect_leaves(function, budget)?;
        status_validation::validate_core::<expression_facts::errors::EngineError>(
            function,
            &expression_facts,
            budget,
        )?;
        structural_maps::validate_regions(function, &storage, budget)?;
        structural_control::validate_entry(function, &storage, &leaves, budget)?;
        let renewals = program.primitive_renewals(function).ok_or_else(|| {
            replay_error(
                function,
                "selected replay lacks its independently rebuilt renewal lease",
            )
        })?;
        structural_control::validate_blocks_and_edges_paid::<expression_facts::errors::EngineError>(
            program,
            function,
            &expression_facts,
            &storage,
            &leaves,
            renewals,
            budget,
        )?;
        (storage, leaves)
    } else {
        ordinary_inventory::validate_inventory_coverage(program, function, budget)?;
        validate_required_status_sources(program, function)?;
        let storage =
            validate_slots_paid_core::<expression_facts::errors::EngineError>(function, budget)?;
        let leaves = collect_leaves_paid(function, budget)?;
        expression_facts::receiving::validate_status_sources_core::<
            expression_facts::errors::EngineError,
        >(function, &expression_facts, budget)?;
        expression_facts::receiving::validate_regions(function, &storage, budget)?;
        expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;
        expression_facts::receiving::validate_blocks_and_edges(
            program,
            function,
            &expression_facts,
            &storage,
            &leaves,
            None,
            budget,
        )?;
        (storage, leaves)
    };

    let facts = lexical_hir::collect(function, budget)?;
    let return_operands = if selected {
        let mut operands = BTreeSet::new();
        for operand in facts
            .exits
            .iter()
            .filter(|exit| exit.kind == LexicalExitKind::Return)
            .filter_map(|exit| exit.value)
        {
            if !operands.contains(&operand.id) {
                budget.reserve_btree_node::<ExpressionId, ()>(
                    function,
                    "return operand identity index",
                )?;
            }
            operands.insert(operand.id.clone());
        }
        structural_control::validate_exits(
            program, function, &storage, &leaves, &operands, budget,
        )?;
        operands
    } else {
        let operands = facts
            .exits
            .iter()
            .filter(|exit| exit.kind == LexicalExitKind::Return)
            .filter_map(|exit| exit.value.map(|operand| operand.id.clone()))
            .collect::<BTreeSet<_>>();
        expression_facts::receiving::validate_exits(
            program, function, &storage, &leaves, &operands, budget,
        )?;
        operands
    };
    // Keep the independently validated operand inventory live through certificate authentication.
    let _return_operands = return_operands;
    let auth = lexical_cert::authenticate(program, function, &owner, &facts, budget)?;
    if selected {
        structural_maps::validate_reference_coverage(function, budget)?;
        structural_control::validate_reachable_acyclic_cfg(function, budget)?;
    } else {
        structural_maps::validate_reference_coverage(function, budget)?;
        structural_control::validate_ordinary_cfg(function, budget)?;
    }
    // Lexical functions always replay every path: no summary shortcut applies.
    validate_path_states::<expression_facts::errors::EngineError>(
        program,
        function,
        &storage,
        &leaves,
        budget,
        Some(&auth),
        None,
    )?;
    factored::strategy::validate_authenticated(
        program,
        function,
        budget,
        None,
        expression_facts.len(),
    )
    .map_err(expression_facts::errors::EngineError::from)
}

#[cfg(test)]
pub(super) fn validate_paid_before_local_receivers(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), expression_facts::errors::EngineError> {
    validate_core(
        program,
        function,
        budget,
        None,
        expression_facts::paid_arena,
    )
}

fn validate_core<'hir, E: structural_control::call_receiving::Policy>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    build_facts: impl FnOnce(
        &'hir ResolvedFunction,
        &mut ReplayBudget,
    ) -> Result<expression_facts::Facts<'hir>, E>,
) -> Result<(), E> {
    if !budget.selected() && budget.pays_allocations() {
        return ordinary::validate_core(program, function, budget, helpers, build_facts);
    }
    let plan = &function.cleanup_plan;
    selected_profile::validate(program, function, budget)?;
    let owner = selected_owner::owner(program, function, budget)?;
    crate::hir::lexical_exit::validate_function(program, function, &owner)?;
    // The lexical resource plan was deferred before enumerating path refusal.
    // Bound structure now, but construct no plan automaton until every exact
    // lexical/certificate/ownership check below has authenticated its meaning.
    let structure_units = plan_structure_units(plan);
    if structure_units > MAX_REPLAY_WORK_UNITS {
        return Err(replay_error(
            function,
            "cleanup replay structure exceeds the global work budget",
        )
        .into());
    }
    budget.charge(function, structure_units, "structural validation")?;
    let expression_facts = build_facts(function, budget)?;
    let selected = budget.selected();
    let (storage, leaves) = if selected {
        inventory_coverage::validate_inventory_coverage(program, function, budget)?;
        status_construction::validate_required_status_sources(program, function, budget)?;
        let storage = structural_maps::validate_slots_core::<E>(function, budget)?;
        let leaves = structural_maps::collect_leaves(function, budget)?;
        status_validation::validate_core::<E>(function, &expression_facts, budget)?;
        structural_maps::validate_regions(function, &storage, budget)?;
        structural_control::validate_entry(function, &storage, &leaves, budget)?;
        let renewals = program.primitive_renewals(function).ok_or_else(|| {
            replay_error(
                function,
                "selected replay lacks its independently rebuilt renewal lease",
            )
        })?;
        structural_control::validate_blocks_and_edges_paid::<E>(
            program,
            function,
            &expression_facts,
            &storage,
            &leaves,
            renewals,
            budget,
        )?;
        (storage, leaves)
    } else {
        ordinary_inventory::validate_inventory_coverage(program, function, budget)?;
        validate_required_status_sources(program, function)?;
        let storage = validate_slots_paid_core::<E>(function, budget)?;
        let leaves = collect_leaves_paid(function, budget)?;
        expression_facts::receiving::validate_status_sources_core::<E>(
            function,
            &expression_facts,
            budget,
        )?;
        expression_facts::receiving::validate_regions(function, &storage, budget)?;
        expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;
        expression_facts::receiving::validate_blocks_and_edges(
            program,
            function,
            &expression_facts,
            &storage,
            &leaves,
            helpers,
            budget,
        )?;
        (storage, leaves)
    };

    let facts = lexical_hir::collect(function, budget)?;
    let return_operands = if selected {
        let mut operands = BTreeSet::new();
        for operand in facts
            .exits
            .iter()
            .filter(|exit| exit.kind == LexicalExitKind::Return)
            .filter_map(|exit| exit.value)
        {
            if !operands.contains(&operand.id) {
                budget.reserve_btree_node::<ExpressionId, ()>(
                    function,
                    "return operand identity index",
                )?;
            }
            operands.insert(operand.id.clone());
        }
        structural_control::validate_exits(
            program, function, &storage, &leaves, &operands, budget,
        )?;
        operands
    } else {
        let operands = facts
            .exits
            .iter()
            .filter(|exit| exit.kind == LexicalExitKind::Return)
            .filter_map(|exit| exit.value.map(|operand| operand.id.clone()))
            .collect::<BTreeSet<_>>();
        expression_facts::receiving::validate_exits(
            program, function, &storage, &leaves, &operands, budget,
        )?;
        operands
    };
    // Keep the independently validated operand inventory live through certificate authentication.
    let _return_operands = return_operands;
    let auth = lexical_cert::authenticate(program, function, &owner, &facts, budget)?;
    if selected {
        structural_maps::validate_reference_coverage(function, budget)?;
        structural_control::validate_reachable_acyclic_cfg(function, budget)?;
    } else {
        structural_maps::validate_reference_coverage(function, budget)?;
        structural_control::validate_ordinary_cfg(function, budget)?;
    }
    // Lexical functions always replay every path: no summary shortcut applies.
    validate_path_states::<E>(
        program,
        function,
        &storage,
        &leaves,
        budget,
        Some(&auth),
        helpers,
    )?;
    factored::strategy::validate_authenticated(
        program,
        function,
        budget,
        helpers,
        expression_facts.len(),
    )
    .map_err(E::from)
}
