//! Full owning census and ordinary late-validator parity, including Drop.
use super::*;
#[path = "tests/old.rs"]
mod old;
const SOURCE: &str = r#"module scratch.cfg;
@id("probe") fn probe(flag:bool)->i64 {
 let key=str"abc";let view=str_as_bytes(key);
 if flag {if byte_len(view)==3usize {7}else{8}}else{9}
}
@id("main") fn main()->i64 {probe(false)}
"#;
fn program() -> crate::hir::ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "scratch.spx").unwrap()).unwrap()
}
fn tuple(value: CfgReplayBounds) -> (usize, usize, usize) {
    (value.terminal_paths, value.work, value.skeleton_work)
}
fn compare(function: &ResolvedFunction) {
    let old = old::bounds(function)
        .map(tuple)
        .map_err(|error| (error.code, error.message));
    let mut budget = ReplayBudget::new();
    let work = (
        budget.remaining,
        budget.skeleton_remaining,
        budget.skeleton_spent,
    );
    let new = bounds(function, &mut budget)
        .map(tuple)
        .map_err(|error| (error.code, error.message));
    assert_eq!(old, new);
    assert_eq!(
        work,
        (
            budget.remaining,
            budget.skeleton_remaining,
            budget.skeleton_spent
        )
    );
}
#[test]
fn full_owning_census_preserves_duplicates_hostile_ids_cycles_and_unreachable_blocks() {
    let program = program();
    let function = program
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    compare(function);
    let branch = function
        .cleanup_plan
        .blocks
        .iter()
        .position(|block| matches!(block.terminator, CleanupTerminator::Branch(_)))
        .unwrap();
    let CleanupTerminator::Branch(edges) = &function.cleanup_plan.blocks[branch].terminator else {
        unreachable!()
    };
    let first = edges[0];
    for arity in [0, 1, 2, 3, 17] {
        let mut changed = function.clone();
        changed.cleanup_plan.blocks[branch].terminator =
            CleanupTerminator::Branch(vec![first; arity]);
        compare(&changed);
    }
    let mut hostile = function.clone();
    hostile.cleanup_plan.edges[first.0 as usize].to = BlockId(u32::MAX);
    hostile.cleanup_plan.blocks[branch].terminator =
        CleanupTerminator::Branch(vec![first, EdgeId(u32::MAX)]);
    compare(&hostile);
    let mut cycle = function.clone();
    cycle.cleanup_plan.edges[first.0 as usize].to = BlockId(branch as u32);
    compare(&cycle);
    let mut unreachable = function.clone();
    unreachable
        .cleanup_plan
        .blocks
        .push(function.cleanup_plan.blocks.last().unwrap().clone());
    compare(&unreachable);
    let mut missing = function.clone();
    missing.cleanup_plan.entry = BlockId(u32::MAX);
    compare(&missing);
}
fn late(function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<(), Diagnostic> {
    structural_maps::validate_reference_coverage(function, budget)?;
    structural_control::validate_ordinary_cfg(function, budget)
}
fn old_late(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    super::super::validate_reference_coverage(function)?;
    super::super::validate_reachable_acyclic_cfg(function)
}
#[test]
fn late_reference_before_cfg_errors_preserves_full_original_order_and_work() {
    let program = program();
    let function = program
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    let mut cases = vec![function.clone()];
    let mut repeated = function.clone();
    let branch = repeated
        .cleanup_plan
        .blocks
        .iter()
        .position(|block| matches!(block.terminator, CleanupTerminator::Branch(_)))
        .unwrap();
    let CleanupTerminator::Branch(edges) = &repeated.cleanup_plan.blocks[branch].terminator else {
        unreachable!()
    };
    let first = edges[0];
    repeated.cleanup_plan.blocks[branch].terminator = CleanupTerminator::Branch(vec![first, first]);
    repeated.cleanup_plan.edges[first.0 as usize].to = BlockId(branch as u32);
    cases.push(repeated);
    let mut cycle = function.clone();
    cycle.cleanup_plan.edges[first.0 as usize].to = BlockId(branch as u32);
    cases.push(cycle);
    let mut unreachable = function.clone();
    // A disconnected block duplicates an exit owner, so reference failure must
    // still precede the otherwise observable unreachable-CFG verdict.
    unreachable
        .cleanup_plan
        .blocks
        .push(function.cleanup_plan.blocks.last().unwrap().clone());
    cases.push(unreachable);
    let mut missing_status = function.clone();
    missing_status.cleanup_plan.status_sources.clear();
    cases.push(missing_status);
    for function in &cases {
        let old = old_late(function).map_err(|e| (e.code, e.message));
        let mut budget = ReplayBudget::new();
        let work = (
            budget.remaining,
            budget.skeleton_remaining,
            budget.skeleton_spent,
        );
        let new = late(function, &mut budget).map_err(|e| (e.code, e.message));
        assert_eq!(old, new);
        assert_eq!(
            work,
            (
                budget.remaining,
                budget.skeleton_remaining,
                budget.skeleton_spent
            )
        );
    }
}
#[test]
fn actual_scratch_is_paid_through_drop_exact_short_and_sticky_refusal() {
    let program = program();
    let function = program
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    let original = function.cleanup_plan.clone();
    let run = || {
        let mut budget = ReplayBudget::new();
        let work = (
            budget.remaining,
            budget.skeleton_remaining,
            budget.skeleton_spent,
        );
        bounds(function, &mut budget)?;
        late(function, &mut budget)?;
        assert_eq!(
            work,
            (
                budget.remaining,
                budget.skeleton_remaining,
                budget.skeleton_spent
            )
        );
        Ok::<(), Diagnostic>(())
    };
    let ((wide, observed), refused, used) =
        crate::bounded_output::with_limit_usage(64 * 1024 * 1024, || {
            crate::test_allocation_observer::measure(run)
        });
    wide.unwrap();
    assert!(!refused && !observed.invalid);
    assert!(observed.allocated > 0 && used > 0);
    assert!(observed.allocated <= used && observed.highwater <= used);
    assert_eq!(observed.live, 0);
    let ((exact, observed), refused, exact_used) =
        crate::bounded_output::with_limit_usage(used, || {
            crate::test_allocation_observer::measure(run)
        });
    exact.unwrap();
    assert!(!refused && !observed.invalid);
    assert_eq!(exact_used, used);
    assert!(observed.allocated <= used && observed.highwater <= used);
    assert_eq!(observed.live, 0);
    let ((short, observed), refused, _) = crate::bounded_output::with_limit_usage(used - 1, || {
        crate::test_allocation_observer::measure(|| {
            let mut budget = ReplayBudget::new();
            let result = bounds(function, &mut budget).and_then(|_| late(function, &mut budget));
            assert!(result.is_err());
            assert!(budget.refusal_spent);
            assert!(budget
                .reserve_array::<u8>(function, 0, "sticky scratch")
                .is_err());
            drop(result);
        })
    });
    let () = short;
    assert!(refused && !observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(function.cleanup_plan, original);
}

#[test]
fn independent_literal_removed_directories_equal_actual_old_requests_through_drop() {
    let program = program();
    let function = program
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    assert!(function.cleanup_plan.lexical_control.is_none());
    let plan = &function.cleanup_plan;
    // The old topology directory and terminal directory have exact vec![..;N]
    // constructors. Each owning target constructor is measured independently,
    // including any Result-collect growth; capacity guesses are not the oracle.
    let directories =
        plan.blocks.len() * (std::mem::size_of::<Vec<usize>>() + std::mem::size_of::<bool>());
    let mut target_requests = 0;
    for block in &plan.blocks {
        let (_, metrics) = crate::test_allocation_observer::measure(|| {
            let targets = match &block.terminator {
                CleanupTerminator::Goto(edge) => vec![plan.edges[edge.0 as usize].to.0 as usize],
                CleanupTerminator::Branch(edges) => edges
                    .iter()
                    .map(|edge| {
                        plan.edges
                            .get(edge.0 as usize)
                            .map(|edge| edge.to.0 as usize)
                            .ok_or(())
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap(),
                CleanupTerminator::Exit(exit) => match &plan.exits[exit.0 as usize].continuation {
                    ExitContinuation::Continue(edge)
                    | ExitContinuation::LoopBreak { edge, .. }
                    | ExitContinuation::FunctionReturn { edge, .. } => {
                        vec![plan.edges[edge.0 as usize].to.0 as usize]
                    }
                    ExitContinuation::CommitResult { .. }
                    | ExitContinuation::ReturnUnit
                    | ExitContinuation::ReturnFailure { .. } => Vec::new(),
                    ExitContinuation::LoopLatch { .. } => panic!("fixture is nonlexical"),
                },
            };
            drop(targets);
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        target_requests += metrics.allocated;
    }
    let (original, old_metrics) =
        crate::test_allocation_observer::measure(|| old::bounds(function).map(tuple));
    let (current, new_metrics) = crate::test_allocation_observer::measure(|| {
        bounds(function, &mut ReplayBudget::new()).map(tuple)
    });
    assert_eq!(original.unwrap(), current.unwrap());
    assert!(!old_metrics.invalid && !new_metrics.invalid);
    assert_eq!(old_metrics.live, 0);
    assert_eq!(new_metrics.live, 0);
    assert_eq!(
        old_metrics.allocated,
        new_metrics.allocated + directories + target_requests
    );
    // Highwater is reported independently: retained queues may overlap at
    // different points, so cumulative removed requests do not imply its delta.
    assert!(new_metrics.highwater <= new_metrics.allocated);
    assert!(old_metrics.highwater <= old_metrics.allocated);
}
#[test]
fn factored_selector_preserves_semantic_false_but_propagates_paid_refusal() {
    let program = program();
    let function = program
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    for paths in [0, 4096, 4097] {
        assert_eq!(
            factored::selected(function, paths, 0),
            factored::selected_paid(function, paths, 0, &mut ReplayBudget::new()).unwrap()
        );
    }
    let mut cycle = function.clone();
    let branch = cycle
        .cleanup_plan
        .blocks
        .iter()
        .position(|block| matches!(block.terminator, CleanupTerminator::Branch(_)))
        .unwrap();
    let CleanupTerminator::Branch(edges) = &cycle.cleanup_plan.blocks[branch].terminator else {
        unreachable!()
    };
    cycle.cleanup_plan.edges[edges[0].0 as usize].to = BlockId(branch as u32);
    assert!(!factored::selected(&cycle, 4097, 0));
    assert!(!factored::selected_paid(&cycle, 4097, 0, &mut ReplayBudget::new()).unwrap());
    let (wide, refused, used) = crate::bounded_output::with_limit_usage(64 * 1024 * 1024, || {
        factored::selected_paid(function, 4097, 0, &mut ReplayBudget::new())
    });
    assert!(wide.unwrap() && !refused && used > 0);
    let (exact, refused, actual) = crate::bounded_output::with_limit_usage(used, || {
        factored::selected_paid(function, 4097, 0, &mut ReplayBudget::new())
    });
    assert!(exact.unwrap() && !refused);
    assert_eq!(used, actual);
    let (short, refused, _) = crate::bounded_output::with_limit_usage(used - 1, || {
        let mut budget = ReplayBudget::new();
        let result = factored::selected_paid(function, 4097, 0, &mut budget);
        assert!(budget.refusal_spent);
        assert!(budget
            .reserve_array::<u8>(function, 0, "selector sticky")
            .is_err());
        result
    });
    assert!(short.is_err() && refused);
}

#[test]
fn lexical_latch_return_and_continue_preserve_full_owning_census_and_late_checks() {
    let source = r#"module scratch.lexical;
@id("probe") fn probe(flag:bool)->i64 {
 let key=str"x";let view=str_as_bytes(key);let mut next=0;
 while next<3 {next=next+1;if flag {continue;};if next==2 {return if byte_len(view)==1usize {7}else{0};};0}
 9
}
@id("main") fn main()->i64 {probe(false)}
"#;
    let program =
        crate::hir::resolve(&crate::check(source, "scratch-lexical.spx").unwrap()).unwrap();
    let function = program
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    assert!(function.cleanup_plan.lexical_control.is_some());
    compare(function);
    let mut budget = ReplayBudget::new();
    assert_eq!(
        old_late(function).map_err(|e| (e.code, e.message)),
        late(function, &mut budget).map_err(|e| (e.code, e.message))
    );
}
#[test]
fn saturating_product_census_preserves_original_work_and_path_ceiling() {
    let decisions = "if flag {1}else{2};".repeat(26);
    let source=format!("module scratch.product; @id(\"probe\") fn probe(flag:bool)->i64 {{ let key=str\"x\"; {decisions} if byte_len(str_as_bytes(key))==1usize {{7}}else{{0}} }} @id(\"main\") fn main()->i64 {{probe(false)}}");
    let program =
        crate::hir::resolve(&crate::check(&source, "scratch-product.spx").unwrap()).unwrap();
    let function = program
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    compare(function);
    let original = old::bounds(function).unwrap();
    assert_eq!(original.work, MAX_REPLAY_WORK_UNITS + 1);
    assert_eq!(original.terminal_paths, MAX_REPLAY_PATHS + 1);
}

#[test]
fn skeleton_preflight_does_not_reclassify_funding_refusal_as_zero_allowance() {
    let program = program();
    let function = program
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    let (result, refused, _) = crate::bounded_output::with_limit_usage(0, || {
        super::super::skeleton_work_upper(&program, function)
    });
    assert!(result.is_err() && refused);
}

#[test]
fn local_capacity_refusal_keeps_exact_diagnostic_without_global_poisoning() {
    let program = program();
    let function = program
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    let ((), refused, used) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        assert!(!crate::bounded_output::active_refused());
        let mut budget = ReplayBudget::new();
        // Exercise the actual ordinary payer overflow boundary without an enclosing
        // quota failure or attempting an enormous physical allocation.
        let mut values = Vec::<u64>::new();
        let error = budget
            .reserve_vec(function, &mut values, usize::MAX, "injected local capacity")
            .unwrap_err();
        assert!(budget.refusal_spent);
        assert!(!crate::bounded_output::active_refused());
        let expected = (error.code, error.message.clone());
        let result = match enumerating_result(Err(error), &budget) {
            Err(error) => error,
            Ok(_) => panic!("local physical refusal became an enumeration fallback"),
        };
        assert_eq!((result.code, result.message), expected);
        assert!(values.is_empty() && values.capacity() == 0);
        // A genuine semantic failure retains the full old zero-allowance fallback.
        let semantic = replay_error(function, "cleanup CFG contains a cycle");
        assert!(enumerating_result(Err(semantic), &ReplayBudget::new())
            .unwrap()
            .is_none());
        // The same enclosing payer remains usable after both local outcomes.
        assert!(crate::bounded_output::reserve_active_required(1));
    });
    // Layout overflow and zero-size authentication construct no backing; only
    // the final successful reservation consumes the enclosing account.
    assert!(!refused);
    assert_eq!(used, 1);
}
