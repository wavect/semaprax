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
let site:&crate::hir::ExpressionId=$site;let domain=$domain;let returned=$returned;let entry=$entry;
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
