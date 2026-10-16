//! Execution-guard cleanup boundaries of cleanup plan v18.
//!
//! A boundary certificate selects no status, admits no owner or ABI and cannot
//! make an allocation failure recoverable. Replay reconstructs the exact
//! positions from typed HIR (function entry before `requires`, and each loop's
//! iteration entry after its condition's cleanup and before its body, for
//! ordinary and lexical loops alike) and the exact finalizer contour from the
//! replayed ownership state at that block. Metadata is never compared against a
//! builder; a forged, missing, duplicate or out-of-domain boundary refuses.
use super::lexical_cert::LexicalAuth;
use super::lexical_hir::LexicalFacts;
use super::*;
use crate::cleanup_plan::{ExecutionBoundaryKind, ExitTargetId};

/// An exact boundary and the independently authenticated return that owns its
/// postcondition domain; no return state is admitted in any other phase.
#[cfg_attr(test, derive(Eq, PartialEq))]
pub(super) struct BoundaryAuth {
    index: usize,
    returned: Option<ExitTargetId>,
}

/// Structural authentication: authored phase/order, exact proof domain,
/// complementary condition edges, iteration block and region chain.
pub(super) fn authenticate(
    function: &ResolvedFunction,
    facts: &LexicalFacts<'_>,
    domains: &[Option<usize>],
    budget: &mut ReplayBudget,
) -> Result<BTreeMap<BlockId, BoundaryAuth>, Diagnostic> {
    let plan = &function.cleanup_plan;
    let fail = |message: &str| replay_error(function, message);
    let control = plan
        .lexical_control
        .as_deref()
        .ok_or_else(|| fail("execution boundaries have no lexical-control certificate"))?;
    let returns = facts
        .exits
        .iter()
        .filter(|fact| fact.kind == crate::ast::LexicalExitKind::Return)
        .count();
    let ordinary = facts.body_completes || facts.tries > 0;
    let expected = 1usize
        .saturating_add(facts.requires_boundaries.len())
        .saturating_add(facts.boundaries.len())
        .saturating_add(
            facts
                .ensures_boundaries
                .len()
                .saturating_mul(returns + usize::from(ordinary)),
        );
    budget.charge(
        function,
        expected.saturating_mul(
            plan.edges
                .len()
                .saturating_add(plan.exits.len())
                .saturating_add(plan.regions.len()),
        ),
        "lexical execution boundary phase replay",
    )?;
    if control.execution_boundaries.len() != expected {
        return Err(fail(
            "execution boundaries do not cover exactly the entry and every typed phase loop",
        ));
    }
    let mut by_block = BTreeMap::new();
    let mut index = 0usize;
    let mut admit = |site: &ExpressionId,
                     domain: Option<usize>,
                     returned: Option<ExitTargetId>,
                     entry: bool|
     -> Result<(), Diagnostic> {
        let (kind, block) = if entry {
            (ExecutionBoundaryKind::FunctionEntry, plan.entry)
        } else {
            let mut yes = None;
            let mut no = None;
            for edge in &plan.edges {
                if domains[edge.from.0 as usize] != domain {
                    continue;
                }
                if let EdgeCondition::BooleanResult(expression, value) = &edge.condition {
                    if expression == site {
                        let target = if *value { &mut yes } else { &mut no };
                        if target.replace(edge).is_some() {
                            return Err(fail("phase loop condition repeats a Boolean edge"));
                        }
                    }
                }
            }
            let (Some(yes), Some(no)) = (yes, no) else {
                return Err(fail("phase loop lacks complementary Boolean edges"));
            };
            if yes.from != no.from
                || domains[yes.to.0 as usize] != domain
                || domains[no.to.0 as usize] != domain
            {
                return Err(fail("phase loop condition crosses its exact proof domain"));
            }
            (ExecutionBoundaryKind::LoopIteration, yes.to)
        };
        let item = &control.execution_boundaries[index];
        let region = plan.blocks[block.0 as usize].region;
        if item.kind != kind
            || &item.site != site
            || item.block != block
            || item.region != region
            || item.leaves_regions
                != super::lexical_cert::storage::region_chain(function, region, budget)?
            || domains[block.0 as usize] != domain
            || super::lexical_cert::storage::insert(
                function,
                budget,
                &mut by_block,
                block,
                BoundaryAuth { index, returned },
            )?
            .is_some()
        {
            return Err(fail(
                "execution boundary differs from its typed phase, block or canonical contour",
            ));
        }
        index += 1;
        Ok(())
    };
    admit(&function.body.id, None, None, true)?;
    for site in &facts.requires_boundaries {
        admit(site, None, None, false)?;
    }
    for site in &facts.boundaries {
        admit(site, None, None, false)?;
    }
    if ordinary {
        for site in &facts.ensures_boundaries {
            admit(site, None, None, false)?;
        }
    }
    // Builder-independent authored Return order. Each return certificate was
    // already checked against this exact typed exit and operand by lexical_cert.
    for (fact, certificate) in facts.exits.iter().zip(&control.exits) {
        if fact.kind != crate::ast::LexicalExitKind::Return {
            continue;
        }
        let entry = certificate
            .resume
            .ok_or_else(|| fail("return postconditions have no exact domain"))?;
        let domain = domains[entry.0 as usize]
            .ok_or_else(|| fail("return postconditions belong to the ordinary domain"))?;
        for site in &facts.ensures_boundaries {
            admit(site, Some(domain), Some(certificate.exit), false)?;
        }
    }
    Ok(by_block)
}

/// Dataflow authentication at the boundary block: the certificate's finalizer
/// vector must be exactly the live leaves of the replayed ownership state, in
/// canonical reverse-initialization order with exact conditional case guards.
#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    function: &ResolvedFunction,
    auth: &LexicalAuth,
    block: BlockId,
    state: &PathState,
    storage: &BTreeSet<StorageId>,
    storage_regions: &BTreeMap<StorageId, CleanupRegionId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let Some(boundary) = auth.boundaries.get(&block) else {
        return Ok(());
    };
    let item = function
        .cleanup_plan
        .lexical_control
        .as_deref()
        .and_then(|control| control.execution_boundaries.get(boundary.index))
        .ok_or_else(|| replay_error(function, "execution boundary has no certificate"))?;
    if state.pending_failure.is_some() || state.selected_failure.is_some() || state.published {
        return Err(replay_error(
            function,
            "execution boundary is reached with failure or publication state",
        ));
    }
    let return_matches = match (&state.returned, boundary.returned) {
        (None, None) => true,
        (Some(returned), Some(id)) => match (
            returned,
            &function.cleanup_plan.exits[id.0 as usize].continuation,
        ) {
            (
                ReturnedSource::Scalar(actual),
                ExitContinuation::FunctionReturn {
                    source: CleanupResultSource::Scalar { expression },
                    ..
                },
            ) => actual == expression,
            (
                ReturnedSource::Owned(actual),
                ExitContinuation::FunctionReturn {
                    source: CleanupResultSource::Owned { storage },
                    ..
                },
            ) => actual == storage,
            _ => false,
        },
        _ => false,
    };
    if !return_matches {
        return Err(replay_error(
            function,
            "execution boundary carries another phase's returned result",
        ));
    }
    super::lexical_cert::storage::finalizer_actions(
        function,
        &item.finalize_in_order,
        storage,
        leaves,
        budget,
    )?;
    if budget.selected() {
        if !state.conditional_variants.is_empty()
            || item
                .finalize_in_order
                .iter()
                .any(|action| action.active_case.is_some())
        {
            return Err(replay_error(
                function,
                "selected lexical boundary conditional ownership is outside the primitive profile",
            ));
        }
        let actual = item.finalize_in_order.iter().filter_map(|action| {
            state
                .live_order
                .contains(&action.guard_flag)
                .then_some(action.guard_flag)
        });
        if actual.ne(state.live_order.iter().rev().copied())
            || item.finalize_in_order.len() != state.live_order.len()
        {
            return Err(replay_error(
                function,
                "execution boundary finalizers differ from the replayed live ownership contour",
            ));
        }
        return Ok(());
    }
    // Authenticate all region references before comparing either contour,
    // preserving the ordinary route's first-error order. Compare borrowed
    // case facts instead of allocating four vectors and cloning each guard.
    for variant in state.conditional_variants.iter().rev() {
        let _ = storage_regions
            .get(&variant.root.storage)
            .ok_or_else(|| replay_error(function, "execution boundary variant has no region"))?;
    }
    let expected = state.live_order.iter().rev().copied();
    let expected_conditional = state.conditional_variants.iter().rev().flat_map(|variant| {
        variant.cases.iter().rev().flat_map(move |(case, flags)| {
            flags
                .iter()
                .rev()
                .map(move |flag| (*flag, &variant.root.storage, &variant.variant, case))
        })
    });
    let actual = item.finalize_in_order.iter().filter_map(|action| {
        state
            .live_order
            .contains(&action.guard_flag)
            .then_some(action.guard_flag)
    });
    let actual_conditional = item.finalize_in_order.iter().filter_map(|action| {
        action.active_case.as_ref().map(|condition| {
            (
                action.guard_flag,
                &condition.storage,
                &condition.variant,
                &condition.case,
            )
        })
    });
    // Every action must finalize a live leaf of this exact state: a stale
    // finalizer for a dead flag would double-release at a guard failure.
    let actions = item
        .finalize_in_order
        .iter()
        .filter(|action| action.active_case.is_none())
        .count();
    if actual.ne(expected)
        || actual_conditional.ne(expected_conditional)
        || actions != state.live_order.len()
    {
        return Err(replay_error(
            function,
            "execution boundary finalizers differ from the replayed live ownership contour",
        ));
    }
    Ok(())
}

pub(in crate::cleanup_plan::replay) mod paid;
