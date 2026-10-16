//! Authentication of cleanup-plan v18 lexical-control certificates.
//!
//! A certificate is proof data and carries no authority. Every field is
//! recomputed from independently replayed typed HIR (`lexical_hir`, the HIR
//! lexical validators) and from the plan's own blocks, edges and exits. Plan
//! vectors keep their authored runtime order; they are compared, never sorted
//! or repaired. The proof CFG stays acyclic: a loop latch is a terminal exit,
//! and the only edge-free continuation is a generated advancement fragment
//! entered from a certified `continue`.
use super::*;
pub(super) mod storage;
use crate::ast::LexicalExitKind;
use crate::cleanup_plan::{CleanupPlan, ExitTargetId};
use crate::hir::{ControlTarget, FunctionExecutionId, LoopLatchKind};

/// Facts the flow replay needs after the certificates are authenticated.
#[cfg_attr(test, derive(Eq, PartialEq))]
pub(super) struct LexicalAuth {
    /// Loop certificate index by the condition expression its target names.
    pub(super) loops: BTreeMap<ExpressionId, usize>,
    /// Blocks where an owning-break fragment rejoins ordinary flow. Every
    /// incoming state must agree exactly; no merge may repair history.
    pub(super) exact_joins: BTreeSet<BlockId>,
    /// The fragment exit that leaves the wrapper must prove this generated
    /// conditional Step assignment has no authored renewal reservation.
    pub(super) wrapper_cancels: BTreeMap<ExitTargetId, ExpressionId>,
    /// Owned item storage an owning `continue` disposes beside its retained
    /// remainder (outside the exit's leaves regions).
    pub(super) item_disposals: BTreeMap<ExitTargetId, BTreeSet<StorageId>>,
    /// Execution-boundary certificate index by its exact block.
    pub(super) boundaries: BTreeMap<BlockId, super::lexical_boundary::BoundaryAuth>,
}

/// `true` when typed HIR contains a lexical exit. A tree deeper than the HIR
/// lexical scan ceiling cannot be resolver-admitted lexical output; it keeps its
/// legacy classification, and every legacy check still fails closed on it: a
/// lexical exit needs certificate-only continuations a legacy plan cannot hold.
pub(super) fn requires_lexical(function: &ResolvedFunction) -> bool {
    crate::hir::lexical_exit::requires(function).unwrap_or(false)
}

/// Entry of the generated advancement fragment a certified `continue` hands to.
pub(super) fn latch_resume(plan: &CleanupPlan, exit: ExitTargetId) -> Option<BlockId> {
    let control = plan.lexical_control.as_deref()?;
    control
        .exits
        .iter()
        .find(|certificate| certificate.exit == exit)?
        .resume
}

/// Region that owns the loop a target names.
pub(super) fn loop_region(plan: &CleanupPlan, target: &ControlTarget) -> Option<CleanupRegionId> {
    let ControlTarget::Loop { site, .. } = target else {
        return None;
    };
    let control = plan.lexical_control.as_deref()?;
    control
        .loops
        .iter()
        .find(|certificate| &certificate.condition == site)
        .map(|certificate| certificate.region)
}

fn reach(
    function: &ResolvedFunction,
    start: BlockId,
    avoid: Option<BlockId>,
    stop: &BTreeSet<BlockId>,
    excluded: Option<&[bool]>,
) -> Vec<bool> {
    let mut seen = vec![false; function.cleanup_plan.blocks.len()];
    if Some(start) == avoid {
        return seen;
    }
    seen[start.0 as usize] = true;
    let mut stack = vec![start];
    while let Some(block) = stack.pop() {
        for next in block_successors(function, block) {
            let index = next.0 as usize;
            if seen[index]
                || Some(next) == avoid
                || stop.contains(&next)
                || excluded.is_some_and(|excluded| excluded[index])
            {
                continue;
            }
            seen[index] = true;
            stack.push(next);
        }
    }
    seen
}

fn chain_contains(
    function: &ResolvedFunction,
    block: BlockId,
    region: CleanupRegionId,
) -> Result<bool, Diagnostic> {
    let start = function.cleanup_plan.blocks[block.0 as usize].region;
    Ok(region_chain_to_root(function, start)?.contains(&region))
}

/// How a certified exit enters its finite proof fragment.
#[derive(Clone, Copy)]
enum Entry {
    /// A lexical `return` hands its result to a per-return postcondition domain.
    Return,
    /// An owning source-For `break`, by loop index: wrapper completion fragment.
    Break(usize),
    /// A generated-advancement `continue`, by loop index.
    Latch(usize),
}

enum Domain {
    Ordinary,
    Return,
    Break(usize),
    Latch(usize, usize),
}

/// A certified owning `continue` advances while retaining the Yield child, arm
/// and body scopes that bind the remainder. Its leaves regions are exactly the
/// regions strictly inside the Yield child scope, derived from the plan's own
/// region tree and the remainder's owning region.
pub(super) fn owned_continue_leaves(
    function: &ResolvedFunction,
    exit: ExitTargetId,
    chain: &[CleanupRegionId],
) -> Result<Option<Vec<CleanupRegionId>>, Diagnostic> {
    let plan = &function.cleanup_plan;
    let Some(control) = plan.lexical_control.as_deref() else {
        return Ok(None);
    };
    let Some(certificate) = control
        .exits
        .iter()
        .find(|certificate| certificate.exit == exit)
    else {
        return Ok(None);
    };
    if certificate.kind != LexicalExitKind::Continue {
        return Ok(None);
    }
    let Some(lp) = control
        .loops
        .iter()
        .find(|lp| lp.target == certificate.target)
    else {
        return Ok(None);
    };
    let LoopLatchKind::OwnedIteratorStep { remainder, .. } = &lp.latch else {
        return Ok(None);
    };
    let storage = StorageId::Value(remainder.clone());
    let mut owners = chain.iter().enumerate().filter(|(_, region)| {
        plan.regions
            .get(region.0 as usize)
            .is_some_and(|region| region.slots.contains(&storage))
    });
    let fail = |message: &str| replay_error(function, message);
    let (index, arm) = owners
        .next()
        .map(|(index, region)| (index, *region))
        .ok_or_else(|| fail("owning continue remainder has no active Yield scope"))?;
    if owners.next().is_some() || index == 0 {
        return Err(fail("owning continue remainder scope is ambiguous"));
    }
    let body = plan.regions[arm.0 as usize]
        .parent
        .ok_or_else(|| fail("owning continue Yield scope has no body parent"))?;
    if plan.regions[body.0 as usize].parent != Some(lp.region) {
        return Err(fail("owning continue retained scope chain differs"));
    }
    Ok(Some(chain[..index - 1].to_vec()))
}

pub(super) fn authenticate(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    owner: &FunctionExecutionId,
    facts: &lexical_hir::LexicalFacts<'_>,
    budget: &mut ReplayBudget,
) -> Result<LexicalAuth, Diagnostic> {
    let plan = &function.cleanup_plan;
    let fail = |message: &str| replay_error(function, message);
    let control = plan
        .lexical_control
        .as_deref()
        .ok_or_else(|| fail("lexical-profile function has no lexical-control certificate"))?;
    if control.schema != crate::hir::lexical_exit::LEXICAL_CONTROL_FLOW_SCHEMA_V1 {
        return Err(fail("lexical-control certificate uses an unknown schema"));
    }
    // Charge every graph traversal below before the first one runs: two per loop
    // (dominance and reach), one per resume, one ordinary reach, each visiting
    // every block, edge and exit and scanning the exit certificates, plus the
    // per-edge region-chain walks of each abrupt-only region and the one
    // dominance walk per execution boundary.
    let graph = plan
        .blocks
        .len()
        .saturating_add(plan.edges.len())
        .saturating_add(plan.exits.len());
    let traversals = control
        .loops
        .len()
        .saturating_mul(2)
        .saturating_add(control.resumes.len().saturating_mul(2))
        .saturating_add(1);
    let units = traversals
        .saturating_mul(graph.saturating_mul(control.exits.len().saturating_add(1)))
        .saturating_add(
            control
                .abrupt_only_regions
                .len()
                .saturating_mul(plan.edges.len())
                .saturating_mul(plan.regions.len()),
        )
        .saturating_add(plan.edges.len().saturating_mul(plan.regions.len()))
        .saturating_add(
            plan.regions.len().saturating_mul(
                plan.blocks
                    .len()
                    .saturating_add(plan.exits.len().saturating_mul(plan.regions.len())),
            ),
        )
        .saturating_add(
            plan.edges
                .len()
                .saturating_mul(control.execution_boundaries.len()),
        );
    budget.charge(function, units, "lexical certificate replay")?;
    let owned_result = type_needs_drop(program, function, &function.return_type)?;

    // Every identity a certificate names must exist before any traversal.
    for certificate in &control.exits {
        if certificate
            .resume
            .is_some_and(|entry| !block_exists(function, entry))
        {
            return Err(fail(
                "lexical exit certificate names an unknown resume block",
            ));
        }
    }
    for certificate in &control.loops {
        let known_region = plan.regions.get(certificate.region.0 as usize).is_some();
        if !known_region
            || !block_exists(function, certificate.condition_entry)
            || !block_exists(function, certificate.after)
        {
            return Err(fail("loop certificate names an unknown region or block"));
        }
        // Every continue owns its own fragment; no shared latch entry exists.
        if certificate.latch_entry.is_some() {
            return Err(fail("loop certificate carries a shared latch entry"));
        }
    }
    for resume in &control.resumes {
        if !block_exists(function, resume.entry)
            || resume
                .blocks
                .iter()
                .any(|block| !block_exists(function, *block))
        {
            return Err(fail("resume certificate names an unknown block"));
        }
    }

    // Loops: independently derived order, target, latch and edge structure.
    if control.loops.len() != facts.loops.len() {
        return Err(fail(
            "loop certificates do not cover exactly the reachable HIR loops",
        ));
    }
    let mut loops = BTreeMap::new();
    for (index, (certificate, fact)) in control.loops.iter().zip(&facts.loops).enumerate() {
        budget.reserve_clone(function, owner, "lexical loop owner")?;
        let expected = ControlTarget::Loop {
            owner: owner.clone(),
            site: fact.condition.id.clone(),
        };
        if certificate.target != expected
            || certificate.condition != fact.condition.id
            || certificate.body != fact.body.id
            || &certificate.latch != fact.latch
        {
            return Err(fail("loop certificate disagrees with its typed HIR loop"));
        }
        if storage::insert(
            function,
            budget,
            &mut loops,
            certificate.condition.clone(),
            index,
        )?
        .is_some()
        {
            return Err(fail("loop certificate condition is repeated"));
        }
        let (branch, true_edge, false_edge) = loop_branch(function, &certificate.condition)?;
        if plan.edges[false_edge.0 as usize].to != certificate.after
            || plan.blocks[certificate.after.0 as usize].region != certificate.region
            || plan.blocks[certificate.condition_entry.0 as usize].region != certificate.region
            || plan.blocks[plan.edges[true_edge.0 as usize].to.0 as usize].region
                != certificate.region
        {
            return Err(fail(
                "loop certificate blocks do not match the condition branch and loop region",
            ));
        }
        // The concrete head must dominate the branch and reach it.
        let none = BTreeSet::new();
        if storage::reach(
            function,
            plan.entry,
            Some(certificate.condition_entry),
            &none,
            None,
            budget,
        )?[branch.0 as usize]
            || !storage::reach(
                function,
                certificate.condition_entry,
                None,
                &none,
                None,
                budget,
            )?[branch.0 as usize]
        {
            return Err(fail(
                "loop condition entry does not dominate its condition branch",
            ));
        }
    }

    // Exits: independently derived order, site, kind, target and continuation.
    if control.exits.len() != facts.exits.len() {
        return Err(fail(
            "exit certificates do not cover exactly the reachable HIR exits",
        ));
    }
    let mut certified = BTreeSet::new();
    let mut entries = BTreeMap::<BlockId, Entry>::new();
    let mut item_disposals = BTreeMap::<ExitTargetId, BTreeSet<StorageId>>::new();
    for (certificate, fact) in control.exits.iter().zip(&facts.exits) {
        if certificate.site != fact.expression.id
            || certificate.kind != fact.kind
            || &certificate.target != fact.target
        {
            return Err(fail("exit certificate disagrees with its typed HIR exit"));
        }
        let exit = plan
            .exits
            .get(certificate.exit.0 as usize)
            .filter(|exit| exit.id == certificate.exit)
            .ok_or_else(|| fail("exit certificate names an unknown exit"))?;
        if !storage::insert_set(function, budget, &mut certified, certificate.exit)? {
            return Err(fail("exit certificate names a repeated exit"));
        }
        match (fact.kind, &exit.continuation) {
            (
                LexicalExitKind::Return,
                ExitContinuation::FunctionReturn {
                    target,
                    edge,
                    source,
                },
            ) => {
                let operand = fact
                    .value
                    .ok_or_else(|| fail("function return has no operand"))?;
                budget.reserve_clone(function, owner, "lexical return owner")?;
                let expected_target = ControlTarget::Function(owner.clone());
                let source_matches = match source {
                    CleanupResultSource::Scalar { expression } => {
                        !owned_result && expression == &operand.id
                    }
                    CleanupResultSource::Owned { storage } => {
                        owned_result
                            && storage == &CleanupPlace::whole(StorageId::ProvisionalResult)
                    }
                };
                let entry = plan.edges[edge.0 as usize].to;
                if target != &expected_target
                    || certificate.target != expected_target
                    || !source_matches
                    || certificate.resume != Some(entry)
                    || storage::insert(function, budget, &mut entries, entry, Entry::Return)?
                        .is_some()
                {
                    return Err(fail(
                        "function return exit disagrees with its operand, result source or resume",
                    ));
                }
            }
            (LexicalExitKind::Break, ExitContinuation::LoopBreak { target, edge }) => {
                let index = loop_index(&loops, owner, &certificate.target)
                    .ok_or_else(|| fail("loop break names an unknown loop"))?;
                let lp = &control.loops[index];
                let to = plan.edges[edge.0 as usize].to;
                // An owning traversal completes its wrapper in a finite proof
                // fragment; every other loop joins the loop continuation.
                let owning = matches!(lp.latch, LoopLatchKind::OwnedIteratorStep { .. });
                let route_ok = if owning {
                    certificate.resume == Some(to)
                        && to != lp.after
                        && storage::insert(function, budget, &mut entries, to, Entry::Break(index))?
                            .is_none()
                } else {
                    certificate.resume.is_none() && to == lp.after
                };
                if target != &certificate.target
                    || !route_ok
                    || !storage::chain_contains(function, exit.from, lp.region, budget.selected())?
                {
                    return Err(fail("loop break exit disagrees with its loop join"));
                }
            }
            (LexicalExitKind::Continue, ExitContinuation::LoopLatch { target }) => {
                let index = loop_index(&loops, owner, &certificate.target)
                    .ok_or_else(|| fail("loop continue names an unknown loop"))?;
                if target != &certificate.target {
                    return Err(fail("loop continue exit names another target"));
                }
                let generated = lexical_hir::generated_advance(
                    facts.loops[index].body,
                    facts.loops[index].latch,
                )
                .is_some();
                match (generated, certificate.resume) {
                    (false, None) => {}
                    (true, Some(entry)) => {
                        if storage::insert(
                            function,
                            budget,
                            &mut entries,
                            entry,
                            Entry::Latch(index),
                        )?
                        .is_some()
                        {
                            return Err(fail("continue exits share one generated fragment"));
                        }
                        if let Some(item) = lexical_hir::yield_item(facts.loops[index].body) {
                            if item.ownership == OwnershipMode::Own {
                                let mut disposal = BTreeSet::new();
                                storage::insert_set(
                                    function,
                                    budget,
                                    &mut disposal,
                                    StorageId::Value(item.id.clone()),
                                )?;
                                storage::insert(
                                    function,
                                    budget,
                                    &mut item_disposals,
                                    certificate.exit,
                                    disposal,
                                )?;
                            }
                        }
                    }
                    _ => {
                        return Err(fail(
                            "continue resume disagrees with its generated advancement",
                        ));
                    }
                }
            }
            _ => {
                return Err(fail(
                    "exit certificate kind disagrees with its cleanup continuation",
                ));
            }
        }
    }

    // Normal iteration ends, per loop.
    let mut iteration_ends = BTreeSet::new();
    for (certificate, fact) in control.loops.iter().zip(&facts.loops) {
        if fact.body_completes {
            let exit = plan
                .exits
                .get(certificate.iteration_end.0 as usize)
                .filter(|exit| exit.id == certificate.iteration_end)
                .ok_or_else(|| fail("loop certificate has no normal iteration end"))?;
            if !matches!(
                &exit.continuation,
                ExitContinuation::LoopLatch { target } if target == &certificate.target
            ) || certified.contains(&certificate.iteration_end)
                || !storage::insert_set(
                    function,
                    budget,
                    &mut iteration_ends,
                    certificate.iteration_end,
                )?
            {
                return Err(fail(
                    "loop iteration end is not its own uncertified latch exit",
                ));
            }
        } else if certificate.iteration_end != UNRESOLVED_NORMAL_END {
            // The sentinel is accepted only for an independently abrupt body.
            return Err(fail(
                "loop with a normal endpoint carries the abrupt-only iteration sentinel",
            ));
        }
    }

    // Resume inventories: exact reconstructed membership, disjoint domains.
    // Correspondence is derived from the exits, never from vector order.
    let mut cert_entries = BTreeSet::new();
    for resume in &control.resumes {
        if !storage::insert_set(function, budget, &mut cert_entries, resume.entry)? {
            return Err(fail("resume certificate entry is repeated"));
        }
    }
    if cert_entries.len() != entries.len() || entries.keys().any(|e| !cert_entries.contains(e)) {
        return Err(fail(
            "resume certificates do not cover exactly the certified exit entries",
        ));
    }
    let block_count = plan.blocks.len();
    let mut stop = BTreeSet::new();
    for entry in entries.keys() {
        storage::insert_set(function, budget, &mut stop, *entry)?;
    }
    let ordinary = storage::reach(function, plan.entry, None, &stop, None, budget)?;
    budget.reserve_array::<Option<usize>>(function, block_count, "lexical proof domains")?;
    let mut domains = vec![None::<usize>; block_count];
    let mut exact_joins = BTreeSet::new();
    for (index, resume) in control.resumes.iter().enumerate() {
        let none = BTreeSet::new();
        let seen = storage::reach(function, resume.entry, None, &none, Some(&ordinary), budget)?;
        let count = 1usize.saturating_add(
            seen.iter()
                .enumerate()
                .filter(|(block, seen)| **seen && *block != resume.entry.0 as usize)
                .count(),
        );
        budget.reserve_array::<BlockId>(function, count, "lexical resume inventory")?;
        let mut inventory = Vec::with_capacity(count);
        inventory.push(resume.entry);
        inventory.extend(
            seen.iter()
                .enumerate()
                .filter(|(block, seen)| **seen && *block != resume.entry.0 as usize)
                .map(|(block, _)| BlockId(block as u32)),
        );
        if resume.blocks != inventory {
            return Err(fail(
                "resume certificate is not its entry followed by its exact reachable blocks",
            ));
        }
        for block in &inventory {
            let slot = &mut domains[block.0 as usize];
            if ordinary[block.0 as usize] || slot.replace(index).is_some() {
                return Err(fail("lexical proof domains overlap"));
            }
        }
        // A fragment is closed except one authenticated rejoin of a break.
        let leaving_edges = || {
            plan.edges
                .iter()
                .filter(|edge| seen[edge.from.0 as usize] && !seen[edge.to.0 as usize])
        };
        budget.reserve_array::<&crate::cleanup_plan::CleanupEdge>(
            function,
            leaving_edges().count(),
            "lexical leaving edges",
        )?;
        let mut leaving = Vec::with_capacity(leaving_edges().count());
        leaving.extend(leaving_edges());
        match entries[&resume.entry] {
            Entry::Break(lp_index) => {
                let [edge] = leaving.as_slice() else {
                    return Err(fail(
                        "owning break fragment does not end in exactly one parent join",
                    ));
                };
                let parent = plan.regions[control.loops[lp_index].region.0 as usize].parent;
                if !ordinary[edge.to.0 as usize]
                    || !matches!(edge.condition, EdgeCondition::Always)
                    || plan.blocks[edge.from.0 as usize].terminator
                        != CleanupTerminator::Goto(edge.id)
                    || Some(plan.blocks[edge.to.0 as usize].region) != parent
                {
                    return Err(fail(
                        "owning break fragment does not rejoin its wrapper's parent exactly",
                    ));
                }
                storage::insert_set(function, budget, &mut exact_joins, edge.to)?;
            }
            Entry::Return | Entry::Latch(_) => {
                if !leaving.is_empty() {
                    return Err(fail("terminal lexical fragment has an outgoing edge"));
                }
            }
        }
    }
    if (0..block_count).any(|block| !ordinary[block] && domains[block].is_none()) {
        return Err(fail("cleanup block belongs to no lexical proof domain"));
    }

    // Domain entry discipline: only certified exits reach a resume.
    let mut entry_edges = BTreeMap::new();
    for exit in &plan.exits {
        let edge = match &exit.continuation {
            ExitContinuation::FunctionReturn { edge, .. }
            | ExitContinuation::LoopBreak { edge, .. } => *edge,
            _ => continue,
        };
        storage::insert(function, budget, &mut entry_edges, edge, exit.from)?;
    }
    for edge in &plan.edges {
        let Some(target) = domains[edge.to.0 as usize] else {
            continue;
        };
        let inside = domains[edge.from.0 as usize] == Some(target);
        let certified_entry = control.resumes[target].entry == edge.to
            && entry_edges.get(&edge.id) == Some(&edge.from);
        if !inside && !certified_entry {
            return Err(fail(
                "an edge enters a lexical resume outside its certified exit",
            ));
        }
    }

    // Per-domain exit shapes, uncertified latches and success exits.
    let mut ordinary_commits = 0usize;
    budget.reserve_array::<usize>(function, control.resumes.len(), "lexical resume commits")?;
    let mut resume_commits = vec![0usize; control.resumes.len()];
    budget.reserve_array::<usize>(function, control.resumes.len(), "lexical resume latches")?;
    let mut resume_latches = vec![0usize; control.resumes.len()];
    budget.reserve_array::<usize>(function, control.resumes.len(), "lexical resume cancels")?;
    let mut resume_cancels = vec![0usize; control.resumes.len()];
    let mut wrapper_cancels = BTreeMap::new();
    let mut commit_exits = Vec::new();
    for exit in &plan.exits {
        let domain = match domains[exit.from.0 as usize] {
            None => Domain::Ordinary,
            Some(index) => match entries[&control.resumes[index].entry] {
                Entry::Return => Domain::Return,
                Entry::Break(_) => Domain::Break(index),
                Entry::Latch(lp) => Domain::Latch(index, lp),
            },
        };
        let accounted = certified.contains(&exit.id) || iteration_ends.contains(&exit.id);
        match (&domain, &exit.continuation) {
            (Domain::Break(index), ExitContinuation::Continue(_)) => {
                let Entry::Break(lp) = entries[&control.resumes[*index].entry] else {
                    unreachable!("break domain entry");
                };
                let region = control.loops[lp].region;
                if exit.leaves_regions == [region] {
                    let LoopLatchKind::OwnedIteratorStep { assignment, .. } =
                        &control.loops[lp].latch
                    else {
                        return Err(fail("owning break fragment has no owning latch"));
                    };
                    resume_cancels[*index] += 1;
                    storage::insert(
                        function,
                        budget,
                        &mut wrapper_cancels,
                        exit.id,
                        assignment.clone(),
                    )?;
                }
            }
            (_, ExitContinuation::Continue(_) | ExitContinuation::ReturnFailure { .. }) => {}
            (Domain::Ordinary, ExitContinuation::CommitResult { .. }) => {
                ordinary_commits += 1;
                budget.reserve_vec(
                    function,
                    &mut commit_exits,
                    1,
                    "lexical success exit inventory",
                )?;
                commit_exits.push(exit.id);
            }
            (Domain::Return, ExitContinuation::CommitResult { .. }) => {
                resume_commits[domains[exit.from.0 as usize].expect("return domain")] += 1;
                budget.reserve_vec(
                    function,
                    &mut commit_exits,
                    1,
                    "lexical success exit inventory",
                )?;
                commit_exits.push(exit.id);
            }
            (
                Domain::Ordinary,
                ExitContinuation::LoopBreak { .. }
                | ExitContinuation::FunctionReturn { .. }
                | ExitContinuation::LoopLatch { .. },
            ) if accounted => {}
            (Domain::Latch(index, lp), ExitContinuation::LoopLatch { target }) if !accounted => {
                if target != &control.loops[*lp].target {
                    return Err(fail("latch fragment ends at another loop's latch"));
                }
                resume_latches[*index] += 1;
            }
            _ => {
                return Err(fail(
                    "exit continuation is not admitted in its lexical proof domain",
                ));
            }
        }
    }
    // A terminal residual `?` path and the normal body path share one epilogue.
    if ordinary_commits != usize::from(facts.body_completes || facts.tries > 0) {
        return Err(fail(
            "ordinary success exits disagree with the body's normal completion",
        ));
    }
    for (index, resume) in control.resumes.iter().enumerate() {
        let shape_ok = match entries[&resume.entry] {
            Entry::Return => {
                resume_commits[index] == 1
                    && resume_latches[index] == 0
                    && resume_cancels[index] == 0
            }
            Entry::Latch(_) => {
                resume_latches[index] == 1
                    && resume_commits[index] == 0
                    && resume_cancels[index] == 0
            }
            Entry::Break(_) => {
                resume_cancels[index] == 1
                    && resume_commits[index] == 0
                    && resume_latches[index] == 0
            }
        };
        if !shape_ok {
            return Err(fail(
                "resume proof domain does not end in its exact terminal exit",
            ));
        }
    }
    if control.function_success_exits != commit_exits
        || !control
            .function_success_exits
            .contains(&plan.regions[0].normal_scope_end)
    {
        return Err(fail(
            "function success exits are not the exact ordinary and return commits",
        ));
    }
    if iteration_ends.len()
        != facts
            .loops
            .iter()
            .filter(|fact| fact.body_completes)
            .count()
    {
        return Err(fail(
            "normal iteration ends are not one per completing loop",
        ));
    }

    validate_normal_ends(function, control, &domains, budget)?;
    let boundaries = super::lexical_boundary::authenticate(function, facts, &domains, budget)?;
    Ok(LexicalAuth {
        loops,
        exact_joins,
        wrapper_cancels,
        item_disposals,
        boundaries,
    })
}

fn loop_index(
    loops: &BTreeMap<ExpressionId, usize>,
    owner: &FunctionExecutionId,
    target: &ControlTarget,
) -> Option<usize> {
    match target {
        ControlTarget::Loop {
            owner: actual,
            site,
        } if actual == owner => loops.get(site).copied(),
        _ => None,
    }
}

/// The unique `Branch` whose Boolean edges decide the loop condition.
pub(super) fn loop_branch(
    function: &ResolvedFunction,
    condition: &ExpressionId,
) -> Result<(BlockId, EdgeId, EdgeId), Diagnostic> {
    let plan = &function.cleanup_plan;
    let mut yes = None;
    let mut no = None;
    for edge in &plan.edges {
        match &edge.condition {
            EdgeCondition::BooleanResult(expression, value) if expression == condition => {
                let slot = if *value { &mut yes } else { &mut no };
                if slot.replace(edge.id).is_some() {
                    return Err(replay_error(
                        function,
                        "loop condition has repeated Boolean edges",
                    ));
                }
            }
            _ => {}
        }
    }
    match (yes, no) {
        (Some(yes), Some(no))
            if plan.edges[yes.0 as usize].from == plan.edges[no.0 as usize].from =>
        {
            Ok((plan.edges[yes.0 as usize].from, yes, no))
        }
        _ => Err(replay_error(
            function,
            "loop condition has no complementary Boolean branch",
        )),
    }
}

/// Normal scope ends are real scope exits; a provisional end must be an
/// abrupt-only region whose ordinary subtree has no normal continuation or
/// escape. Proof fragments complete scopes already entered by authored flow and
/// never own a region's normal end, so only ordinary-domain flow is considered.
fn validate_normal_ends(
    function: &ResolvedFunction,
    control: &crate::cleanup_plan::LexicalControlPlan,
    domains: &[Option<usize>],
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let fail = |message: &str| replay_error(function, message);
    let mut lexical_edges = BTreeSet::new();
    for exit in &plan.exits {
        let edge = match &exit.continuation {
            ExitContinuation::LoopBreak { edge, .. }
            | ExitContinuation::FunctionReturn { edge, .. } => *edge,
            _ => continue,
        };
        storage::insert_set(function, budget, &mut lexical_edges, edge)?;
    }
    let mut listed = BTreeSet::new();
    for region in &control.abrupt_only_regions {
        if region.0 == 0
            || plan.regions.get(region.0 as usize).is_none()
            || !storage::insert_set(function, budget, &mut listed, *region)?
        {
            return Err(fail("abrupt-only region list is invalid or repeated"));
        }
    }
    for region in plan.regions.iter().skip(1) {
        let unresolved = region.normal_scope_end == UNRESOLVED_NORMAL_END;
        if unresolved != listed.contains(&region.id) {
            return Err(fail(
                "provisional normal scope end differs from the abrupt-only certificate",
            ));
        }
        if !unresolved {
            let exit = &plan.exits[region.normal_scope_end.0 as usize];
            if !matches!(exit.continuation, ExitContinuation::Continue(_))
                || exit.leaves_regions != [region.id]
            {
                return Err(fail("region normal scope end is not its own scope exit"));
            }
            if let Some(domain) = domains[exit.from.0 as usize] {
                // Authored ensures reuse their exact source-region identity in
                // every Return domain. Original body/Yield regions cannot
                // borrow a normal endpoint from any generated continuation.
                let entry = control.resumes[domain].entry;
                let is_return = |entry| {
                    control.exits.iter().any(|certificate| {
                        certificate.kind == LexicalExitKind::Return
                            && certificate.resume == Some(entry)
                    })
                };
                let typed_ensures_scope = plan.exits.iter().any(|failure| {
                    let ExitContinuation::ReturnFailure { source } = &failure.continuation else {
                        return false;
                    };
                    source.lane == StatusLane::ContractFalse
                        && function
                            .ensures
                            .iter()
                            .any(|clause| clause.id == source.expression)
                        && storage::chain_contains(
                            function,
                            exit.from,
                            plan.blocks[failure.from.0 as usize].region,
                            budget.selected(),
                        )
                        .unwrap_or(false)
                });
                if !is_return(entry)
                    || !typed_ensures_scope
                    || plan.blocks.iter().any(|block| {
                        block.region == region.id
                            && domains[block.id.0 as usize]
                                .is_some_and(|other| !is_return(control.resumes[other].entry))
                    })
                {
                    return Err(fail(
                        "region normal scope end crosses its typed ensures Return domains",
                    ));
                }
            }
            continue;
        }
        for exit in &plan.exits {
            if matches!(exit.continuation, ExitContinuation::Continue(_))
                && exit.leaves_regions == [region.id]
                && domains[exit.from.0 as usize].is_none()
            {
                return Err(fail("abrupt-only region has a normal scope exit"));
            }
        }
        for edge in &plan.edges {
            if domains[edge.from.0 as usize].is_none()
                && storage::chain_contains(function, edge.from, region.id, budget.selected())?
                && !storage::chain_contains(function, edge.to, region.id, budget.selected())?
                && !lexical_edges.contains(&edge.id)
            {
                return Err(fail(
                    "abrupt-only region flow escapes without a lexical exit",
                ));
            }
        }
    }
    if plan.regions[0].normal_scope_end == UNRESOLVED_NORMAL_END {
        return Err(fail("root region has no normal success exit"));
    }
    Ok(())
}

pub(in crate::cleanup_plan::replay) mod paid;
