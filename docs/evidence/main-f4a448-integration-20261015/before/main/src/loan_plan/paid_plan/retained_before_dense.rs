//! Complete prior plan builder after moving its actual first paid inventories.
use super::*;

pub(in crate::loan_plan) fn build(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    work: &mut WorkCounter,
    inventory: paid_candidate::Inventory<'_, '_>,
) -> Result<LoanPlan, Diagnostic> {
    let payer = work.allocation;
    // The current inventory additionally carries the nested-byte receiver.
    // This historical builder keeps its own complete prior traversal below.
    let (aliases, bound, ownership, _byte_census) =
        inventory.into_parts(program, function, payer)?;
    let cfg = build_cfg(function, work)?;

    let mut drafts = Vec::<PaidDraft>::new();
    for expression in &cfg.expressions {
        charge(work)?;
        if let Some(draft) = traversal::paid::draft(program, expression, &cfg, work)? {
            storage::push(&mut drafts, draft, payer)?;
        }
        if let Some(draft) = traversal::paid::draft_bytes(expression, &cfg, &aliases, work)? {
            storage::push(&mut drafts, draft, payer)?;
        }
        let view = match &expression.kind {
            ResolvedExprKind::Call { .. } => paid_predicates::string_slot(expression)
                .map(|place| storage::clone_place(place, payer))
                .transpose()?,
            ResolvedExprKind::BorrowPlace { place, .. } => {
                Some(storage::clone_place(place, payer)?)
            }
            ResolvedExprKind::VecFieldRead { args, .. }
                if matches!(
                    expression.ty,
                    crate::hir::ResolvedType::Str | crate::hir::ResolvedType::SliceU8
                ) =>
            {
                args.first()
                    .map(|source| storage::expression_place(source, payer))
                    .transpose()?
                    .flatten()
            }
            ResolvedExprKind::ByteRange { source, .. } => storage::expression_place(source, payer)?,
            ResolvedExprKind::NativeRustImportCall(call)
                if matches!(
                    call.result,
                    crate::hir::ResolvedImportResultKind::BorrowedStr { .. }
                ) =>
            {
                call.args
                    .first()
                    .map(|source| storage::expression_place(source, payer))
                    .transpose()?
                    .flatten()
            }
            ResolvedExprKind::Place(place) if bound.contains_key(&expression.id) => {
                Some(storage::clone_place(place, payer)?)
            }
            _ => None,
        };
        if let Some(place) = view {
            storage::push(
                &mut drafts,
                PaidDraft {
                    site: expression.id.clone(),
                    origin: origin::funded::resolve(
                        &aliases,
                        storage::clone_place(&place, payer)?,
                        work,
                    )?,
                    origin_span: expression.span,
                    parent_root: aliases
                        .contains_key(&place.root)
                        .then(|| place.root.clone()),
                    binding: bound.get(&expression.id).cloned(),
                    start: cfg.node(expression, LoanPointPhase::Before)?,
                    seeds: cfg_directory::Set::new(payer.expect("private funded entry"))?,
                    cause: if expression.ty == ResolvedType::Str {
                        LoanCause::StrView
                    } else {
                        LoanCause::SliceView
                    },
                },
                payer,
            )?;
        }
        for (index, argument) in
            call_arguments::borrowed_call_arguments_paid(program, expression, payer)?
        {
            let place = storage::expression_place(argument, payer)?
                .ok_or_else(|| error("borrowed call lacks an exact place origin"))?;
            storage::push(
                &mut drafts,
                PaidDraft {
                    site: expression.id.clone(),
                    origin: origin::funded::resolve(
                        &aliases,
                        storage::clone_place(&place, payer)?,
                        work,
                    )?,
                    origin_span: argument.span,
                    parent_root: aliases
                        .contains_key(&place.root)
                        .then(|| place.root.clone()),
                    binding: None,
                    start: cfg.node(expression, LoanPointPhase::Before)?,
                    seeds: cfg_directory::singleton_set(
                        cfg.node(expression, LoanPointPhase::After)?,
                        payer,
                    )?,
                    cause: LoanCause::BorrowedCall {
                        argument: u16::try_from(index)
                            .map_err(|_| error("borrowed argument index overflows"))?,
                    },
                },
                payer,
            )?;
        }
        if let ResolvedExprKind::Match {
            mode: crate::hir::ResolvedMatchMode::Borrow,
            scrutinee,
            arms,
        } = &expression.kind
        {
            let place = storage::expression_place(scrutinee, payer)?
                .ok_or_else(|| error("borrow match lacks an exact place origin"))?;
            let origin =
                origin::funded::resolve(&aliases, storage::clone_place(&place, payer)?, work)?;
            for (index, arm) in arms.iter().enumerate() {
                let entry = arm.guard.as_deref().unwrap_or(&arm.value);
                storage::push(
                    &mut drafts,
                    PaidDraft {
                        site: expression.id.clone(),
                        origin: storage::clone_place(&origin, payer)?,
                        origin_span: scrutinee.span,
                        parent_root: aliases
                            .contains_key(&place.root)
                            .then(|| place.root.clone()),
                        binding: None,
                        start: cfg.node(entry, LoanPointPhase::Before)?,
                        seeds: cfg_directory::singleton_set(
                            cfg.node(&arm.value, LoanPointPhase::After)?,
                            payer,
                        )?,
                        cause: LoanCause::MatchBorrow {
                            arm: u16::try_from(index)
                                .map_err(|_| error("match arm index overflows"))?,
                        },
                    },
                    payer,
                )?;
            }
        }
    }

    // Independent HIR scope validation admits this exact result identity only
    // in ensures; it is held owned storage until contract settlement completes.
    drafts.retain(|draft| {
        ownership.get(&draft.origin.root) == Some(&OwnershipMode::Own)
            || provisional_result::root(function) == Some(&draft.origin.root)
    });
    // Borrow-only roots do not need an authenticated loan attachment. Do not
    // retain the transient CFG built to classify candidates: legacy HIR stays
    // heap-identical at this boundary and only real own-root loans carry the
    // endpoint/edge proof.
    if drafts.is_empty() {
        let mut plan = LoanPlan::empty_v1();
        if cfg
            .expressions
            .iter()
            .any(|e| matches!(e.kind, ResolvedExprKind::LexicalExit { .. }))
        {
            plan.schema = LOAN_PLAN_SCHEMA_V3;
        }
        return Ok(plan);
    }
    if drafts.len() > MAX_LOANS_PER_FUNCTION_V1 {
        return Err(error("function exceeds 256 shared loans"));
    }

    let mut binding_loans = cfg_directory::Map::new(payer.expect("private funded entry"))?;
    for (index, draft) in drafts.iter().enumerate() {
        if let Some(binding) = &draft.binding {
            cfg_directory::map_insert(
                &mut binding_loans,
                binding.clone(),
                LoanId(index as u16),
                payer,
            )?;
        }
    }
    for expression in &cfg.expressions {
        let ResolvedExprKind::Place(place) = &expression.kind else {
            continue;
        };
        if let Some(id) = binding_loans.get(&place.root) {
            cfg_directory::set_insert(
                &mut drafts[id.0 as usize].seeds,
                cfg.node(expression, LoanPointPhase::After)?,
                payer,
            )?;
        }
    }
    let match_parents = native_view::match_parents_funded(&cfg.expressions, &drafts, payer)?;
    let mut parents = storage::vector(drafts.len(), payer)?;
    for draft in &drafts {
        let parent = if let Some(root) = &draft.parent_root {
            match resolve_parent_paid(&aliases, &binding_loans, root.clone(), payer)? {
                Some(parent) => Some(parent),
                None => resolve_parent_paid(&aliases, &match_parents, root.clone(), payer)?,
            }
        } else {
            None
        };
        parents.push(parent);
    }
    #[cfg(not(test))]
    let reachable = work::Reachability::build(&cfg, drafts.iter().map(|draft| draft.start), work)?;
    #[cfg(test)]
    let reachable = if work.uncached_reachability {
        work::Reachability::uncached(&cfg, drafts.iter().map(|draft| draft.start), work)?
    } else {
        work::Reachability::build(&cfg, drafts.iter().map(|draft| draft.start), work)?
    };
    let mut live = storage::vector(drafts.len(), payer)?;
    let mut live_pending = Vec::new();
    for (index, draft) in drafts.iter().enumerate() {
        live.push(paid_live::live_nodes_with_pending(
            &cfg,
            LoanId(index as u16),
            draft.start,
            &draft.seeds,
            &reachable,
            work,
            &mut live_pending,
        )?);
    }
    for _ in 0..=drafts.len() {
        let mut changed = false;
        for child in (0..drafts.len()).rev() {
            let Some(parent) = parents[child] else {
                continue;
            };
            let parent = parent.0 as usize;
            let before = live[parent].len();
            let mut seeds = cfg_directory::clone_set(&drafts[parent].seeds, payer)?;
            for node in live[child].iter(payer.expect("private funded entry"))? {
                cfg_directory::set_insert(&mut seeds, *node, payer)?;
            }
            live[parent] = paid_live::live_nodes_with_pending(
                &cfg,
                LoanId(parent as u16),
                drafts[parent].start,
                &seeds,
                &reachable,
                work,
                &mut live_pending,
            )?;
            changed |= live[parent].len() != before;
        }
        if !changed {
            break;
        }
    }
    drop(live_pending);
    reject_cfg_overlaps_paid(program, &cfg, &aliases, &drafts, &live, work)?;

    let (mut edge_live, mut termination_edges) = paid_live::edge_liveness(&cfg, &live, work)?;
    // A contract/body root has no outgoing CFG edge. A synchronous call
    // ending at that root completes on its incoming After edge, rather than
    // needing a nonexistent successor. Only admit this formerly rejected
    // terminal shape; already admitted v1 plans retain identical bytes.
    if termination_edges.iter().any(Vec::is_empty) {
        let mut terminal_roots = cfg_directory::Set::new(payer.expect("private funded entry"))?;
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(function.ensures.iter())
        {
            charge(work)?;
            cfg_directory::set_insert(
                &mut terminal_roots,
                cfg.node(root, LoanPointPhase::After)?,
                payer,
            )?;
        }
        for nodes in &mut live {
            for (index, point) in cfg.points.iter().enumerate() {
                charge(work)?;
                if point.phase == LoanPointPhase::After
                    && cfg.successor_edges(index).is_empty()
                    && terminal_roots.contains(&(index as u16))
                {
                    nodes.remove(&(index as u16), payer.expect("private funded entry"))?;
                }
            }
        }
        for (child, parent) in parents.iter().enumerate() {
            if let Some(parent) = parent {
                for node in live[child].iter(payer.expect("private funded entry"))? {
                    charge(work)?;
                    if !live[parent.0 as usize].contains(node) {
                        return Err(error("terminal loan child escapes its parent lifetime"));
                    }
                }
            }
        }
        // Recompute edge proof only for a previously rejected terminal shape.
        (edge_live, termination_edges) = paid_live::edge_liveness(&cfg, &live, work)?;
    }
    paid_materialize::materialize(
        cfg,
        drafts,
        parents,
        edge_live,
        termination_edges,
        payer.expect("private funded entry"),
    )
}
