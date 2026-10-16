//! Full immediate boundary authentication, affine failures expanded lexically.
use super::{
    BTreeMap, BlockId, BoundaryAuth, EdgeCondition, ExecutionBoundaryKind, LexicalFacts,
    ReplayBudget, ResolvedFunction,
};
use crate::cleanup_plan::renewal_profile::{
    boundary_refusal::BoundaryRefusal, child_refusal::Refusal,
};
use crate::cleanup_plan::replay::selected_context::child::Permit;
type Error = crate::cleanup_plan::replay::expression_facts::errors::EngineError;
macro_rules! required {
    ($permit:ident,$value:expr,$reason:ident) => {
        match $value {
            Some(value) => value,
            None => return Err($permit.refuse(Refusal::Boundary(BoundaryRefusal::$reason))),
        }
    };
}
pub(in crate::cleanup_plan::replay) fn authenticate(
    function: &ResolvedFunction,
    facts: &LexicalFacts<'_>,
    domains: &[Option<usize>],
    budget: &mut ReplayBudget,
) -> Result<BTreeMap<BlockId, BoundaryAuth>, Error> {
    let permit = Permit::acquire::<BTreeMap<BlockId, BoundaryAuth>>(function, budget)?;
    let plan = &function.cleanup_plan;
    let control = required!(
        permit,
        plan.lexical_control.as_deref(),
        ExecutionBoundariesHaveNoLexicalControlCertificate
    );
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
        return Err(permit.refuse(Refusal::Boundary(
            BoundaryRefusal::ExecutionBoundariesDoNotCoverExactlyTheEntryAndEveryTypedPhaseLoop,
        )));
    }
    let mut by_block = BTreeMap::new();
    let mut index = 0usize;
    macro_rules! admit { ($site:expr,$domain:expr,$returned:expr,$entry:expr)=>{{
let site=$site;let domain=$domain;let returned=$returned;let entry=$entry;
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
                            return Err(permit.refuse(Refusal::Boundary(BoundaryRefusal::PhaseLoopConditionRepeatsABooleanEdge)));
                        }
                    }
                }
            }
            let (Some(yes), Some(no)) = (yes, no) else {
                return Err(permit.refuse(Refusal::Boundary(BoundaryRefusal::PhaseLoopLacksComplementaryBooleanEdges)));
            };
            if yes.from != no.from
                || domains[yes.to.0 as usize] != domain
                || domains[no.to.0 as usize] != domain
            {
                return Err(permit.refuse(Refusal::Boundary(BoundaryRefusal::PhaseLoopConditionCrossesItsExactProofDomain)));
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
                != crate::cleanup_plan::replay::lexical_cert::paid::storage::region_chain(function, region, budget)?
            || domains[block.0 as usize] != domain
            || crate::cleanup_plan::replay::lexical_cert::paid::storage::insert(
                function,
                budget,
                &mut by_block,
                block,
                BoundaryAuth { index, returned },
            )?
            .is_some()
        {
            return Err(permit.refuse(Refusal::Boundary(BoundaryRefusal::ExecutionBoundaryDiffersFromItsTypedPhaseBlockOrCanonicalContour)));
        }
        index += 1;

}};}
    admit!(&function.body.id, None, None, true);
    for site in &facts.requires_boundaries {
        admit!(site, None, None, false);
    }
    for site in &facts.boundaries {
        admit!(site, None, None, false);
    }
    if ordinary {
        for site in &facts.ensures_boundaries {
            admit!(site, None, None, false);
        }
    }
    // Builder-independent authored Return order. Each return certificate was
    // already checked against this exact typed exit and operand by lexical_cert.
    for (fact, certificate) in facts.exits.iter().zip(&control.exits) {
        if fact.kind != crate::ast::LexicalExitKind::Return {
            continue;
        }
        let entry = required!(
            permit,
            certificate.resume,
            ReturnPostconditionsHaveNoExactDomain
        );
        let domain = required!(
            permit,
            domains[entry.0 as usize],
            ReturnPostconditionsBelongToTheOrdinaryDomain
        );
        for site in &facts.ensures_boundaries {
            admit!(site, Some(domain), Some(certificate.exit), false);
        }
    }
    Ok(by_block)
}

use super::{
    BTreeSet, CleanupRegionId, CleanupResultSource, Diagnostic, ExitContinuation, Leaves,
    LexicalAuth, PathState, Policy, ResolvedType, ReturnedSource, StorageId,
};
use crate::cleanup_plan::renewal_profile::path_boundary_refusal::PathBoundaryRefusal;
macro_rules! required_path {
    ($permit:ident,$value:expr,$reason:ident) => {
        match $value {
            Some(value) => value,
            None => return Err($permit.refuse(Refusal::PathBoundary(PathBoundaryRefusal::$reason))),
        }
    };
}
impl Policy for Error {
    #[allow(clippy::too_many_arguments)]
    fn check(
        function: &ResolvedFunction,
        auth: &LexicalAuth,
        block: BlockId,
        state: &PathState,
        storage: &BTreeSet<StorageId>,
        storage_regions: &BTreeMap<StorageId, CleanupRegionId>,
        leaves: &Leaves,
        budget: &mut ReplayBudget,
    ) -> Result<(), Error> {
        if !budget.pays_allocations() {
            return <Diagnostic as super::Policy>::check(
                function,
                auth,
                block,
                state,
                storage,
                storage_regions,
                leaves,
                budget,
            )
            .map_err(Error::Owned);
        }
        let permit = Permit::acquire::<()>(function, budget)?;
        let Some(boundary) = auth.boundaries.get(&block) else {
            return Ok(());
        };
        let item = required_path!(
            permit,
            function
                .cleanup_plan
                .lexical_control
                .as_deref()
                .and_then(|control| control.execution_boundaries.get(boundary.index)),
            ExecutionBoundaryHasNoCertificate
        );
        if state.pending_failure.is_some() || state.selected_failure.is_some() || state.published {
            return Err(permit.refuse(Refusal::PathBoundary(
                PathBoundaryRefusal::ExecutionBoundaryIsReachedWithFailureOrPublicationState,
            )));
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
            return Err(permit.refuse(Refusal::PathBoundary(
                PathBoundaryRefusal::ExecutionBoundaryCarriesAnotherPhaseSReturnedResult,
            )));
        }
        finalizer_actions(function, &item.finalize_in_order, storage, leaves, budget)?;
        if budget.selected() {
            if !state.conditional_variants.is_empty()
                || item
                    .finalize_in_order
                    .iter()
                    .any(|action| action.active_case.is_some())
            {
                return Err(permit.refuse(Refusal::PathBoundary(PathBoundaryRefusal::SelectedLexicalBoundaryConditionalOwnershipIsOutsideThePrimitiveProfile)));
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
                return Err(permit.refuse(Refusal::PathBoundary(PathBoundaryRefusal::ExecutionBoundaryFinalizersDifferFromTheReplayedLiveOwnershipContour)));
            }
            return Ok(());
        }
        // Authenticate all region references before comparing either contour,
        // preserving the ordinary route's first-error order. Compare borrowed
        // case facts instead of allocating four vectors and cloning each guard.
        for variant in state.conditional_variants.iter().rev() {
            let _ = required_path!(
                permit,
                storage_regions.get(&variant.root.storage),
                ExecutionBoundaryVariantHasNoRegion
            );
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
            return Err(permit.refuse(Refusal::PathBoundary(PathBoundaryRefusal::ExecutionBoundaryFinalizersDifferFromTheReplayedLiveOwnershipContour)));
        }
        Ok(())
    }
}
pub(in crate::cleanup_plan::replay) fn finalizer_actions(
    function: &ResolvedFunction,
    actions: &[crate::cleanup_plan::FinalizeAction],
    known: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    if !budget.pays_allocations() {
        return crate::cleanup_plan::replay::validate_finalizer_actions(
            function, actions, known, leaves,
        )
        .map_err(Error::Owned);
    }
    let permit = Permit::acquire::<()>(function, budget)?;
    let mut finalized = BTreeSet::new();
    for action in actions {
        if !known.contains(&action.source.storage) {
            return Err(permit.refuse(Refusal::PathBoundary(
                PathBoundaryRefusal::CleanupPlaceReferencesUnknownStorage,
            )));
        }
        let mut under = leaves.iter().filter(|(_, leaf)| {
            leaf.place.storage == action.source.storage
                && leaf
                    .place
                    .projections
                    .starts_with(&action.source.projections)
        });
        let first = under.next();
        if first.is_none() {
            return Err(permit.refuse(Refusal::PathBoundary(
                PathBoundaryRefusal::CleanupPlaceHasNoDroppableLeaf,
            )));
        }
        let (flag, leaf) = first.expect("nonempty borrowed leaf query");
        if under.next().is_some() || *flag != action.guard_flag || leaf.place != action.source {
            return Err(permit.refuse(Refusal::PathBoundary(
                PathBoundaryRefusal::FinalizerGuardDoesNotNameItsExactCleanupLeaf,
            )));
        }
        if leaf.lifecycle != action.lifecycle_id
            || !crate::cleanup_plan::replay::lexical_cert::paid::storage::insert_set(
                function,
                budget,
                &mut finalized,
                action.guard_flag,
            )?
        {
            return Err(permit.refuse(Refusal::PathBoundary(
                PathBoundaryRefusal::FinalizerLifecycleOrGuardIsInvalidOrRepeated,
            )));
        }
        if let Some(condition) = &action.active_case {
            // Paying ordinary backing does not select the primitive profile.
            // Keep the original variant/path authentication without a cloned
            // leaf vector; insert_set above funds the only growing scratch.
            if budget.selected() {
                return Err(permit.refuse(Refusal::PathBoundary(PathBoundaryRefusal::SelectedLexicalBoundaryConditionalOwnershipIsOutsideThePrimitiveProfile)));
            }
            if condition.storage != action.source.storage
                || action.source.projections.first() != Some(&condition.case)
                || function
                    .cleanup_plan
                    .slots
                    .iter()
                    .find(|slot| slot.storage == condition.storage)
                    .is_none_or(|slot| {
                        !matches!(
                            &slot.ty,
                            ResolvedType::Nominal { declaration, .. }
                                if declaration == &condition.variant
                        )
                    })
            {
                return Err(permit.refuse(Refusal::PathBoundary(
                    PathBoundaryRefusal::ConditionalFinalizerDoesNotBindItsExactVariantCasePath,
                )));
            }
        }
    }
    Ok(())
}
