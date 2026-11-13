//! Complete certificate algorithm; legacy parent remains independent.
use super::{
    block_exists, lexical_hir, loop_index, BTreeMap, BTreeSet, BlockId, CleanupPlace,
    CleanupResultSource, CleanupTerminator, ControlTarget, Domain, EdgeCondition, Entry,
    ExitContinuation, ExitTargetId, FunctionExecutionId, LexicalAuth, LexicalExitKind,
    LoopLatchKind, OwnershipMode, ReplayBudget, ResolvedFunction, StatusLane, StorageId,
    UNRESOLVED_NORMAL_END,
};
use crate::cleanup_plan::renewal_profile::{
    certificate_refusal::CertificateRefusal, child_refusal::Refusal,
};
use crate::cleanup_plan::replay::selected_context::child::receiving::receive;
use crate::cleanup_plan::replay::selected_context::child::{
    ArrayReceiving, CloneReceiving, Permit, VecReceiving,
};
type Error = crate::cleanup_plan::replay::expression_facts::errors::EngineError;
pub(in crate::cleanup_plan::replay) mod chains;
pub(in crate::cleanup_plan::replay) mod storage;
macro_rules! required {
    ($permit:ident,$value:expr,$reason:ident) => {
        match $value {
            Some(value) => value,
            None => return Err($permit.refuse(Refusal::Certificate(CertificateRefusal::$reason))),
        }
    };
}
macro_rules! loop_branch {
    ($permit:ident,$function:ident,$condition:expr) => {{
        let condition = $condition;
        let plan = &$function.cleanup_plan;
        let mut yes = None;
        let mut no = None;
        for edge in &plan.edges {
            match &edge.condition {
                EdgeCondition::BooleanResult(expression, value) if expression == condition => {
                    let slot = if *value { &mut yes } else { &mut no };
                    if slot.replace(edge.id).is_some() {
                        return Err($permit.refuse(Refusal::Certificate(
                            CertificateRefusal::LoopConditionHasRepeatedBooleanEdges,
                        )));
                    }
                }
                _ => {}
            }
        }
        match (yes, no) {
            (Some(yes), Some(no))
                if plan.edges[yes.0 as usize].from == plan.edges[no.0 as usize].from =>
            {
                (plan.edges[yes.0 as usize].from, yes, no)
            }
            _ => {
                return Err($permit.refuse(Refusal::Certificate(
                    CertificateRefusal::LoopConditionHasNoComplementaryBooleanBranch,
                )))
            }
        }
    }};
}

pub(in crate::cleanup_plan::replay) fn authenticate(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    owner: &FunctionExecutionId,
    facts: &lexical_hir::LexicalFacts<'_>,
    budget: &mut ReplayBudget,
) -> Result<LexicalAuth, Error> {
    let permit = Permit::profile::<LexicalAuth>(function, budget)?;
    let plan = &function.cleanup_plan;
    let control = required!(
        permit,
        plan.lexical_control.as_deref(),
        LexicalProfileFunctionHasNoLexicalControlCertificate
    );
    if control.schema != crate::hir::lexical_exit::LEXICAL_CONTROL_FLOW_SCHEMA_V1 {
        return Err(permit.refuse(Refusal::Certificate(
            CertificateRefusal::LexicalControlCertificateUsesAnUnknownSchema,
        )));
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
    let owned_result = crate::cleanup_plan::replay::selected_type_drop::type_needs_drop!(
        permit,
        program,
        function,
        &function.return_type
    );

    // Every identity a certificate names must exist before any traversal.
    for certificate in &control.exits {
        if certificate
            .resume
            .is_some_and(|entry| !block_exists(function, entry))
        {
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::LexicalExitCertificateNamesAnUnknownResumeBlock,
            )));
        }
    }
    for certificate in &control.loops {
        let known_region = plan.regions.get(certificate.region.0 as usize).is_some();
        if !known_region
            || !block_exists(function, certificate.condition_entry)
            || !block_exists(function, certificate.after)
        {
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::LoopCertificateNamesAnUnknownRegionOrBlock,
            )));
        }
        // Every continue owns its own fragment; no shared latch entry exists.
        if certificate.latch_entry.is_some() {
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::LoopCertificateCarriesASharedLatchEntry,
            )));
        }
    }
    for resume in &control.resumes {
        if !block_exists(function, resume.entry)
            || resume
                .blocks
                .iter()
                .any(|block| !block_exists(function, *block))
        {
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::ResumeCertificateNamesAnUnknownBlock,
            )));
        }
    }

    // Loops: independently derived order, target, latch and edge structure.
    if control.loops.len() != facts.loops.len() {
        return Err(permit.refuse(Refusal::Certificate(
            CertificateRefusal::LoopCertificatesDoNotCoverExactlyTheReachableHirLoops,
        )));
    }
    let mut loops = BTreeMap::new();
    for (index, (certificate, fact)) in control.loops.iter().zip(&facts.loops).enumerate() {
        receive!(
            function,
            budget,
            permit,
            CloneReceiving<'_, FunctionExecutionId>,
            9,
            budget.child_reserve_clone(function, owner, "lexical loop owner")
        );
        let expected = ControlTarget::Loop {
            owner: owner.clone(),
            site: fact.condition.id.clone(),
        };
        if certificate.target != expected
            || certificate.condition != fact.condition.id
            || certificate.body != fact.body.id
            || &certificate.latch != fact.latch
        {
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::LoopCertificateDisagreesWithItsTypedHirLoop,
            )));
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
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::LoopCertificateConditionIsRepeated,
            )));
        }
        let (branch, true_edge, false_edge) =
            loop_branch!(permit, function, &certificate.condition);
        if plan.edges[false_edge.0 as usize].to != certificate.after
            || plan.blocks[certificate.after.0 as usize].region != certificate.region
            || plan.blocks[certificate.condition_entry.0 as usize].region != certificate.region
            || plan.blocks[plan.edges[true_edge.0 as usize].to.0 as usize].region
                != certificate.region
        {
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::LoopCertificateBlocksDoNotMatchTheConditionBranchAndLoopRegion,
            )));
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
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::LoopConditionEntryDoesNotDominateItsConditionBranch,
            )));
        }
    }

    // Exits: independently derived order, site, kind, target and continuation.
    if control.exits.len() != facts.exits.len() {
        return Err(permit.refuse(Refusal::Certificate(
            CertificateRefusal::ExitCertificatesDoNotCoverExactlyTheReachableHirExits,
        )));
    }
    let mut certified = BTreeSet::new();
    let mut entries = BTreeMap::<BlockId, Entry>::new();
    let mut item_disposals = BTreeMap::<ExitTargetId, BTreeSet<StorageId>>::new();
    for (certificate, fact) in control.exits.iter().zip(&facts.exits) {
        if certificate.site != fact.expression.id
            || certificate.kind != fact.kind
            || &certificate.target != fact.target
        {
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::ExitCertificateDisagreesWithItsTypedHirExit,
            )));
        }
        let exit = required!(
            permit,
            plan.exits
                .get(certificate.exit.0 as usize)
                .filter(|exit| exit.id == certificate.exit),
            ExitCertificateNamesAnUnknownExit
        );
        if !storage::insert_set(function, budget, &mut certified, certificate.exit)? {
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::ExitCertificateNamesARepeatedExit,
            )));
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
                let operand = required!(permit, fact.value, FunctionReturnHasNoOperand);
                receive!(
                    function,
                    budget,
                    permit,
                    CloneReceiving<'_, FunctionExecutionId>,
                    9,
                    budget.child_reserve_clone(function, owner, "lexical return owner")
                );
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
                    return Err(permit.refuse(Refusal::Certificate(CertificateRefusal::FunctionReturnExitDisagreesWithItsOperandResultSourceOrResume)));
                }
            }
            (LexicalExitKind::Break, ExitContinuation::LoopBreak { target, edge }) => {
                let index = required!(
                    permit,
                    loop_index(&loops, owner, &certificate.target),
                    LoopBreakNamesAnUnknownLoop
                );
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
                    || !crate::cleanup_plan::replay::lexical_cert::paid::chains::contains!(
                        permit,
                        function,
                        exit.from,
                        lp.region,
                        budget.selected()
                    )
                {
                    return Err(permit.refuse(Refusal::Certificate(
                        CertificateRefusal::LoopBreakExitDisagreesWithItsLoopJoin,
                    )));
                }
            }
            (LexicalExitKind::Continue, ExitContinuation::LoopLatch { target }) => {
                let index = required!(
                    permit,
                    loop_index(&loops, owner, &certificate.target),
                    LoopContinueNamesAnUnknownLoop
                );
                if target != &certificate.target {
                    return Err(permit.refuse(Refusal::Certificate(
                        CertificateRefusal::LoopContinueExitNamesAnotherTarget,
                    )));
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
                            return Err(permit.refuse(Refusal::Certificate(
                                CertificateRefusal::ContinueExitsShareOneGeneratedFragment,
                            )));
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
                        return Err(permit.refuse(Refusal::Certificate(
                            CertificateRefusal::ContinueResumeDisagreesWithItsGeneratedAdvancement,
                        )));
                    }
                }
            }
            _ => {
                return Err(permit.refuse(Refusal::Certificate(
                    CertificateRefusal::ExitCertificateKindDisagreesWithItsCleanupContinuation,
                )));
            }
        }
    }

    // Normal iteration ends, per loop.
    let mut iteration_ends = BTreeSet::new();
    for (certificate, fact) in control.loops.iter().zip(&facts.loops) {
        if fact.body_completes {
            let exit = required!(
                permit,
                plan.exits
                    .get(certificate.iteration_end.0 as usize)
                    .filter(|exit| exit.id == certificate.iteration_end),
                LoopCertificateHasNoNormalIterationEnd
            );
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
                return Err(permit.refuse(Refusal::Certificate(
                    CertificateRefusal::LoopIterationEndIsNotItsOwnUncertifiedLatchExit,
                )));
            }
        } else if certificate.iteration_end != UNRESOLVED_NORMAL_END {
            // The sentinel is accepted only for an independently abrupt body.
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::LoopWithANormalEndpointCarriesTheAbruptOnlyIterationSentinel,
            )));
        }
    }

    // Resume inventories: exact reconstructed membership, disjoint domains.
    // Correspondence is derived from the exits, never from vector order.
    let mut cert_entries = BTreeSet::new();
    for resume in &control.resumes {
        if !storage::insert_set(function, budget, &mut cert_entries, resume.entry)? {
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::ResumeCertificateEntryIsRepeated,
            )));
        }
    }
    if cert_entries.len() != entries.len() || entries.keys().any(|e| !cert_entries.contains(e)) {
        return Err(permit.refuse(Refusal::Certificate(
            CertificateRefusal::ResumeCertificatesDoNotCoverExactlyTheCertifiedExitEntries,
        )));
    }
    let block_count = plan.blocks.len();
    let mut stop = BTreeSet::new();
    for entry in entries.keys() {
        storage::insert_set(function, budget, &mut stop, *entry)?;
    }
    let ordinary = storage::reach(function, plan.entry, None, &stop, None, budget)?;
    receive!(
        function,
        budget,
        permit,
        ArrayReceiving<'_>,
        9,
        budget.child_reserve_array::<Option<usize>>(function, block_count, "lexical proof domains")
    );
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
        receive!(
            function,
            budget,
            permit,
            ArrayReceiving<'_>,
            9,
            budget.child_reserve_array::<BlockId>(function, count, "lexical resume inventory")
        );
        let mut inventory = Vec::with_capacity(count);
        inventory.push(resume.entry);
        inventory.extend(
            seen.iter()
                .enumerate()
                .filter(|(block, seen)| **seen && *block != resume.entry.0 as usize)
                .map(|(block, _)| BlockId(block as u32)),
        );
        if resume.blocks != inventory {
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::ResumeCertificateIsNotItsEntryFollowedByItsExactReachableBlocks,
            )));
        }
        for block in &inventory {
            let slot = &mut domains[block.0 as usize];
            if ordinary[block.0 as usize] || slot.replace(index).is_some() {
                return Err(permit.refuse(Refusal::Certificate(
                    CertificateRefusal::LexicalProofDomainsOverlap,
                )));
            }
        }
        // A fragment is closed except one authenticated rejoin of a break.
        let leaving_edges = || {
            plan.edges
                .iter()
                .filter(|edge| seen[edge.from.0 as usize] && !seen[edge.to.0 as usize])
        };
        receive!(
            function,
            budget,
            permit,
            ArrayReceiving<'_>,
            9,
            budget.child_reserve_array::<&crate::cleanup_plan::CleanupEdge>(
                function,
                leaving_edges().count(),
                "lexical leaving edges",
            )
        );
        let mut leaving = Vec::with_capacity(leaving_edges().count());
        leaving.extend(leaving_edges());
        match entries[&resume.entry] {
            Entry::Break(lp_index) => {
                let [edge] = leaving.as_slice() else {
                    return Err(permit.refuse(Refusal::Certificate(
                        CertificateRefusal::OwningBreakFragmentDoesNotEndInExactlyOneParentJoin,
                    )));
                };
                let parent = plan.regions[control.loops[lp_index].region.0 as usize].parent;
                if !ordinary[edge.to.0 as usize]
                    || !matches!(edge.condition, EdgeCondition::Always)
                    || plan.blocks[edge.from.0 as usize].terminator
                        != CleanupTerminator::Goto(edge.id)
                    || Some(plan.blocks[edge.to.0 as usize].region) != parent
                {
                    return Err(permit.refuse(Refusal::Certificate(CertificateRefusal::OwningBreakFragmentDoesNotRejoinItsWrapperSParentExactly)));
                }
                storage::insert_set(function, budget, &mut exact_joins, edge.to)?;
            }
            Entry::Return | Entry::Latch(_) => {
                if !leaving.is_empty() {
                    return Err(permit.refuse(Refusal::Certificate(
                        CertificateRefusal::TerminalLexicalFragmentHasAnOutgoingEdge,
                    )));
                }
            }
        }
    }
    if (0..block_count).any(|block| !ordinary[block] && domains[block].is_none()) {
        return Err(permit.refuse(Refusal::Certificate(
            CertificateRefusal::CleanupBlockBelongsToNoLexicalProofDomain,
        )));
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
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::AnEdgeEntersALexicalResumeOutsideItsCertifiedExit,
            )));
        }
    }

    // Per-domain exit shapes, uncertified latches and success exits.
    let mut ordinary_commits = 0usize;
    receive!(
        function,
        budget,
        permit,
        ArrayReceiving<'_>,
        9,
        budget.child_reserve_array::<usize>(
            function,
            control.resumes.len(),
            "lexical resume commits"
        )
    );
    let mut resume_commits = vec![0usize; control.resumes.len()];
    receive!(
        function,
        budget,
        permit,
        ArrayReceiving<'_>,
        9,
        budget.child_reserve_array::<usize>(
            function,
            control.resumes.len(),
            "lexical resume latches"
        )
    );
    let mut resume_latches = vec![0usize; control.resumes.len()];
    receive!(
        function,
        budget,
        permit,
        ArrayReceiving<'_>,
        9,
        budget.child_reserve_array::<usize>(
            function,
            control.resumes.len(),
            "lexical resume cancels"
        )
    );
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
                        return Err(permit.refuse(Refusal::Certificate(
                            CertificateRefusal::OwningBreakFragmentHasNoOwningLatch,
                        )));
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
                receive!(
                    function,
                    budget,
                    permit,
                    VecReceiving<'_, ExitTargetId>,
                    10,
                    budget.child_reserve_vec(
                        function,
                        &mut commit_exits,
                        1,
                        "lexical success exit inventory",
                    )
                );
                commit_exits.push(exit.id);
            }
            (Domain::Return, ExitContinuation::CommitResult { .. }) => {
                resume_commits[domains[exit.from.0 as usize].expect("return domain")] += 1;
                receive!(
                    function,
                    budget,
                    permit,
                    VecReceiving<'_, ExitTargetId>,
                    10,
                    budget.child_reserve_vec(
                        function,
                        &mut commit_exits,
                        1,
                        "lexical success exit inventory",
                    )
                );
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
                    return Err(permit.refuse(Refusal::Certificate(
                        CertificateRefusal::LatchFragmentEndsAtAnotherLoopSLatch,
                    )));
                }
                resume_latches[*index] += 1;
            }
            _ => {
                return Err(permit.refuse(Refusal::Certificate(
                    CertificateRefusal::ExitContinuationIsNotAdmittedInItsLexicalProofDomain,
                )));
            }
        }
    }
    // A terminal residual `?` path and the normal body path share one epilogue.
    if ordinary_commits != usize::from(facts.body_completes || facts.tries > 0) {
        return Err(permit.refuse(Refusal::Certificate(
            CertificateRefusal::OrdinarySuccessExitsDisagreeWithTheBodySNormalCompletion,
        )));
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
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::ResumeProofDomainDoesNotEndInItsExactTerminalExit,
            )));
        }
    }
    if control.function_success_exits != commit_exits
        || !control
            .function_success_exits
            .contains(&plan.regions[0].normal_scope_end)
    {
        return Err(permit.refuse(Refusal::Certificate(
            CertificateRefusal::FunctionSuccessExitsAreNotTheExactOrdinaryAndReturnCommits,
        )));
    }
    if iteration_ends.len()
        != facts
            .loops
            .iter()
            .filter(|fact| fact.body_completes)
            .count()
    {
        return Err(permit.refuse(Refusal::Certificate(
            CertificateRefusal::NormalIterationEndsAreNotOnePerCompletingLoop,
        )));
    }

    validate_normal_ends(function, control, &domains, budget)?;
    let boundaries = crate::cleanup_plan::replay::lexical_boundary::paid::authenticate(
        function, facts, &domains, budget,
    )?;
    Ok(LexicalAuth {
        loops,
        exact_joins,
        wrapper_cancels,
        item_disposals,
        boundaries,
    })
}

fn validate_normal_ends(
    function: &ResolvedFunction,
    control: &crate::cleanup_plan::LexicalControlPlan,
    domains: &[Option<usize>],
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    let permit = Permit::acquire::<()>(function, budget)?;
    let plan = &function.cleanup_plan;
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
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::AbruptOnlyRegionListIsInvalidOrRepeated,
            )));
        }
    }
    for region in plan.regions.iter().skip(1) {
        let unresolved = region.normal_scope_end == UNRESOLVED_NORMAL_END;
        if unresolved != listed.contains(&region.id) {
            return Err(permit.refuse(Refusal::Certificate(
                CertificateRefusal::ProvisionalNormalScopeEndDiffersFromTheAbruptOnlyCertificate,
            )));
        }
        if !unresolved {
            let exit = &plan.exits[region.normal_scope_end.0 as usize];
            if !matches!(exit.continuation, ExitContinuation::Continue(_))
                || exit.leaves_regions != [region.id]
            {
                return Err(permit.refuse(Refusal::Certificate(
                    CertificateRefusal::RegionNormalScopeEndIsNotItsOwnScopeExit,
                )));
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
                        && crate::cleanup_plan::replay::lexical_cert::paid::chains::contains_or_false!(
                            function,
                            exit.from,
                            plan.blocks[failure.from.0 as usize].region,
                            budget.selected(),
                        )
                });
                if !is_return(entry)
                    || !typed_ensures_scope
                    || plan.blocks.iter().any(|block| {
                        block.region == region.id
                            && domains[block.id.0 as usize]
                                .is_some_and(|other| !is_return(control.resumes[other].entry))
                    })
                {
                    return Err(permit.refuse(Refusal::Certificate(
                        CertificateRefusal::RegionNormalScopeEndCrossesItsTypedEnsuresReturnDomains,
                    )));
                }
            }
            continue;
        }
        for exit in &plan.exits {
            if matches!(exit.continuation, ExitContinuation::Continue(_))
                && exit.leaves_regions == [region.id]
                && domains[exit.from.0 as usize].is_none()
            {
                return Err(permit.refuse(Refusal::Certificate(
                    CertificateRefusal::AbruptOnlyRegionHasANormalScopeExit,
                )));
            }
        }
        for edge in &plan.edges {
            if domains[edge.from.0 as usize].is_none()
                && crate::cleanup_plan::replay::lexical_cert::paid::chains::contains!(
                    permit,
                    function,
                    edge.from,
                    region.id,
                    budget.selected()
                )
                && !crate::cleanup_plan::replay::lexical_cert::paid::chains::contains!(
                    permit,
                    function,
                    edge.to,
                    region.id,
                    budget.selected()
                )
                && !lexical_edges.contains(&edge.id)
            {
                return Err(permit.refuse(Refusal::Certificate(
                    CertificateRefusal::AbruptOnlyRegionFlowEscapesWithoutALexicalExit,
                )));
            }
        }
    }
    if plan.regions[0].normal_scope_end == UNRESOLVED_NORMAL_END {
        return Err(permit.refuse(Refusal::Certificate(
            CertificateRefusal::RootRegionHasNoNormalSuccessExit,
        )));
    }
    Ok(())
}
