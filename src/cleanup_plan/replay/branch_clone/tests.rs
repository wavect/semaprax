use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use std::cell::Cell;
use std::rc::Rc;
thread_local! { static ORIGINAL: Cell<bool> = const { Cell::new(false) }; }
pub(super) fn original_selected() -> bool {
    ORIGINAL.with(Cell::get)
}
fn with_original<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ORIGINAL.with(|flag| flag.set(self.0));
        }
    }
    let _restore = Restore(ORIGINAL.with(|flag| flag.replace(true)));
    run()
}
fn program() -> ResolvedProgram {
    let source="module branch_copy; @id(\"f\") fn f(a:bool,b:bool)->i64 {let n=if a {1}else{2};if b {n+1}else{n+2}} @id(\"main\") fn main()->i64 {f(true,false)}";
    let checked = crate::check(source, "branch-copy.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "branch-copy.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}

#[test]
fn full_plan_enumerator_keeps_every_original_path_work_and_materialization() {
    let program = program();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for function in &program.functions {
                let mut old = ReplayBudget::new();
                let mut current = ReplayBudget::new();
                reserve_program_skeleton_work(&program, std::iter::once(function), &mut old).unwrap();
                reserve_program_skeleton_work(&program, std::iter::once(function), &mut current).unwrap();
                assert!(old.pays_allocations() && current.pays_allocations());
                let before = skeleton_materializations();
                let expected = with_original(|| plan_skeleton_paths(function, &mut old)).unwrap();
                let old_count = skeleton_materializations() - before;
                let before = skeleton_materializations();
                let actual = plan_skeleton_paths(function, &mut current).unwrap();
                assert_eq!(skeleton_materializations() - before, old_count);
                assert_eq!(actual, expected);
                assert_eq!(current.remaining, old.remaining);
                assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                drop(expected);
                drop(actual);
                drop(old);
                drop(current);
            }
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}

#[test]
fn clone_and_push_keep_capacity_contents_work_and_drop_with_less_backing() {
    let program = program();
    let function = &program.functions[0];
    for width in [0, 1, 2, 8, 32, 64] {
        for pushes in [1, 3] {
            let values = (0..width).map(Rc::new).collect::<Vec<_>>();
            let next = Rc::new(999usize);
            let observe = |original, limit| {
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        let before = skeleton_materializations();
                        let result = (|| {
                            // One actual clone and exactly this many pushes; lend
                            // their work from the unchanged global caller budget.
                            budget.reserve_skeleton(function, 1 + pushes)?;
                            let mut copy = if original {
                                skeleton_clone(&mut budget, function, &values, "branch clone")?
                            } else {
                                for_push(&mut budget, function, &values, "branch clone")?
                            };
                            for _ in 0..pushes {
                                skeleton_push(
                                    &mut budget,
                                    function,
                                    &mut copy,
                                    Rc::clone(&next),
                                    "branch push",
                                )?;
                            }
                            assert_eq!(copy.len(), width + pushes);
                            assert!(copy[..width]
                                .iter()
                                .zip(&values)
                                .all(|(a, b)| Rc::ptr_eq(a, b)));
                            assert!(copy[width..].iter().all(|value| Rc::ptr_eq(value, &next)));
                            let capacity = copy.capacity();
                            assert_eq!(budget.skeleton_remaining, 0);
                            drop(copy);
                            Ok::<_, Diagnostic>(capacity)
                        })();
                        let capacity = result.as_ref().ok().copied();
                        let work = budget.skeleton_remaining;
                        let materialized = skeleton_materializations() - before;
                        drop(result);
                        drop(budget);
                        (capacity, work, materialized)
                    })
                })
            };
            let ((old, old_allocation), refused, old_paid, first) = observe(true, usize::MAX);
            assert!(!refused && first.is_none());
            assert!(old.0.is_some());
            assert!(!old_allocation.invalid && old_allocation.live == 0);
            assert_eq!(old_allocation.allocated, old_allocation.freed);
            assert!(old_allocation.allocated <= old_paid);
            let ((actual, allocation), refused, exact, first) = observe(false, usize::MAX);
            assert!(!refused && first.is_none());
            assert_eq!(actual, old);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert!(allocation.allocated <= exact);
            assert_eq!(old_paid - exact, width * std::mem::size_of::<Rc<usize>>());
            assert_eq!(
                old_allocation.allocated - allocation.allocated,
                width * std::mem::size_of::<Rc<usize>>()
            );
            for limit in [0, exact - 1, exact] {
                let ((outcome, allocation), refused, paid, first) = observe(false, limit);
                assert!(!allocation.invalid && allocation.live == 0);
                assert_eq!(allocation.allocated, allocation.freed);
                assert!(allocation.allocated <= paid);
                assert_eq!(outcome.0.is_some(), limit == exact);
                if limit == exact {
                    assert!(!refused && first.is_none());
                    assert_eq!(outcome, actual);
                    assert_eq!(paid, exact);
                } else {
                    assert!(refused && first.is_some());
                    if limit == 0 {
                        assert_eq!(allocation.allocated, 0);
                    }
                }
            }
            assert_eq!(Rc::strong_count(&next), 1);
            assert!(values.iter().all(|value| Rc::strong_count(value) == 1));
        }
    }
}

#[test]
fn whole_original_enumerator_and_clone_push_helpers_remain_byte_exact() {
    use sha2::{Digest, Sha256};
    let root = main_replay_input(include_str!("../../replay.rs"));
    let packets = [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/branch-clone-capacity-v1/plan_skeleton_paths.before.rs.txt"
            )),
            "fc722eea9f308a5ccae38965e33eeee30f426173c6fea9cb8a364238049ee2e8",
            true,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/branch-clone-capacity-v1/skeleton_clone.before.rs.txt"
            )),
            "8e797ebf316c2951cb3eb66caf2a06c08859b63b4923bcbcc2c37b03cabef97e",
            false,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/branch-clone-capacity-v1/skeleton_push.before.rs.txt"
            )),
            "4fe08ee24ce1cef9aa4802256e2b59c280705157bb7bd44d432c8fb683649023",
            false,
        ),
    ];
    for (original, hash, changed) in packets {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            hash
        );
        let current = if changed {
            assert_eq!(
                original.matches("let mut branch = skeleton_clone(").count(),
                1
            );
            original.replacen(
                "let mut branch = skeleton_clone(",
                "let mut branch = branch_clone::for_push(",
                1,
            )
        } else {
            original.to_owned()
        };
        assert_eq!(root.matches(&current).count(), 1);
    }
}

thread_local! { static TAKES: Cell<usize> = const { Cell::new(0) }; }
pub(super) fn note_take() {
    TAKES.with(|count| count.set(count.get() + 1));
}

#[test]
fn final_prefix_transfer_keeps_full_checked_arithmetic_paths_and_work() {
    let mut source = String::from("module final_prefix; @id(\"f\") fn f(x:i64)->i64 {");
    for index in 0..48 {
        let input = if index == 0 {
            "x".to_owned()
        } else {
            format!("x{}", index - 1)
        };
        source.push_str(&format!("let x{index}={input}+1;"));
    }
    source.push_str("x47} @id(\"main\") fn main()->i64 {f(0)}");
    let checked = crate::check(&source, "final-prefix.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "final-prefix.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    let program = crate::hir::resolve(&checked).unwrap();
    let before_takes = TAKES.with(Cell::get);
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for function in &program.functions {
                let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let before = skeleton_materializations();
                let expected =
                    with_original(|| compact_stage::original_paths(function, &mut old)).unwrap();
                let old_count = skeleton_materializations() - before;
                let before = skeleton_materializations();
                let actual = super::super::plan_skeleton_paths(function, &mut current).unwrap();
                assert_eq!(skeleton_materializations() - before, old_count);
                assert!(compact_stage::paths_match(&actual, &expected));
                assert_eq!(current.remaining, old.remaining);
                assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                drop(actual);
                drop(expected);
                drop(current);
                drop(old);
            }
        })
    });
    assert!(TAKES.with(Cell::get) > before_takes);
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}

#[test]
fn terminal_prefix_probe_rejects_growth_cycles_and_nonfinal_edges() {
    let program = program();
    let function = &program.functions[0];
    let mut plan = function.cleanup_plan.clone();
    let terminal = plan.blocks.iter().find(|block| {
        matches!(block.terminator, CleanupTerminator::Exit(exit) if matches!(plan.exits[exit.0 as usize].continuation,
            ExitContinuation::ReturnUnit | ExitContinuation::ReturnFailure { .. } | ExitContinuation::CommitResult { .. }))
    }).unwrap().id;
    let mut values = Vec::with_capacity(64);
    for _ in 0..48 {
        values.push(Rc::new(SkeletonObservation::Boolean {
            expression: crate::hir::ExpressionId::from_owned("prefix.test".to_owned()),
            value: true,
        }));
    }
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let probe =
                |plan: &crate::cleanup_plan::CleanupPlan, target, remaining_edges, edge_count| {
                    can_take(TakeCandidate {
                        values: &values,
                        remaining_edges,
                        edge_count,
                        budget: &budget,
                        plan,
                        target,
                    })
                };
            assert!(probe(&plan, terminal, 0, 2));
            assert!(!probe(&plan, terminal, 1, 2));
            assert!(!probe(&plan, terminal, 0, 1));
            assert!(!probe(&plan, BlockId(u32::MAX), 0, 2));
            assert!(!with_original(|| probe(&plan, terminal, 0, 2)));
            let index = terminal.0 as usize;
            let original = plan.blocks[index].terminator.clone();
            plan.blocks[index].terminator = CleanupTerminator::Branch(Vec::new());
            assert!(!probe(&plan, terminal, 0, 2));
            plan.blocks[index].terminator = CleanupTerminator::Goto(EdgeId(u32::MAX));
            assert!(!probe(&plan, terminal, 0, 2));
            plan.blocks[index].terminator =
                CleanupTerminator::Exit(crate::cleanup_plan::ExitTargetId(u32::MAX));
            assert!(!probe(&plan, terminal, 0, 2));
            let old_target = plan.edges[0].to;
            plan.edges[0].to = terminal;
            plan.blocks[index].terminator = CleanupTerminator::Goto(EdgeId(0));
            assert!(!probe(&plan, terminal, 0, 2));
            plan.edges[0].to = old_target;
            plan.blocks[index].terminator = original;
            drop(budget);
        })
    });
    assert!(!refused && first.is_none());
    assert_eq!(paid, 0);
    assert_eq!(allocation.allocated, 0);
    assert!(!allocation.invalid && allocation.live == 0);
}

thread_local! { static GENERIC_TAKES: Cell<usize> = const { Cell::new(0) }; }
pub(super) fn note_generic_take() {
    GENERIC_TAKES.with(|count|count.set(count.get()+1));
}

#[test]
fn joined_generic_three_edge_transfer_keeps_complete_paths_and_original_work() {
    let mut source=String::from("module three_prefix; @id(\"f\") fn f(x:i64)->i64 {");
    for index in 0..48 {
        let value=if index==0 {"x".to_owned()}else{format!("x{}",index-1)};
        source.push_str(&format!("let x{index}={value}+1;"));
    }
    source.push_str("x47} @id(\"main\") fn main()->i64 {f(0)}");
    let checked=crate::check(&source,"three-prefix.spx").unwrap();
    let mut program=crate::hir::resolve(&checked).unwrap();
    let function=program.functions.iter_mut().find(|function|function.id.as_str()=="f").unwrap();
    let block=function.cleanup_plan.blocks.iter().rposition(|block|
        matches!(&block.terminator,CleanupTerminator::Branch(edges) if edges.len()==2)).unwrap();
    let CleanupTerminator::Branch(edges)=&function.cleanup_plan.blocks[block].terminator else {unreachable!()};
    let mut edge=function.cleanup_plan.edges[edges[edges.len()-1].0 as usize].clone();
    let id=EdgeId(u32::try_from(function.cleanup_plan.edges.len()).unwrap());
    edge.id=id;function.cleanup_plan.edges.push(edge);
    let CleanupTerminator::Branch(edges)=&mut function.cleanup_plan.blocks[block].terminator else {unreachable!()};
    edges.push(id);
    let (((),allocation),refused,paid,first)=with_limit_usage_refusal(usize::MAX,||measure(||{
        let mut old=ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut current=ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let before=skeleton_materializations();
        let expected=with_original(||compact_stage::original_paths(function,&mut old)).unwrap();
        let count=skeleton_materializations()-before;
        let before=skeleton_materializations();
        let takes=GENERIC_TAKES.with(Cell::get);
        let actual=super::super::plan_skeleton_paths(function,&mut current).unwrap();
        assert!(GENERIC_TAKES.with(Cell::get)>takes);
        assert!(compact_stage::paths_match(&actual,&expected));
        assert_eq!(skeleton_materializations()-before,count);
        assert_eq!(current.remaining,old.remaining);
        assert_eq!(current.skeleton_remaining,old.skeleton_remaining);
        drop(actual);drop(expected);drop(current);drop(old);
    }));
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live==0 && allocation.allocated==allocation.freed);
    assert!(allocation.allocated<=paid);
}

#[derive(Debug, Eq, PartialEq)]
struct ProbeFailure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 1024], usize),
    span: Option<crate::ast::Span>,
    path: Option<([u8; 1024], usize)>,
    help: Option<([u8; 1024], usize)>,
}
fn probe_text(value: &str) -> ([u8; 1024], usize) {
    assert!(value.len() <= 1024);
    let mut bytes = [0; 1024];
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    (bytes, value.len())
}
fn probe_failure(error: &Diagnostic) -> ProbeFailure {
    ProbeFailure {
        code: error.code, severity: error.severity,
        message: probe_text(&error.message), span: error.span,
        path: error.path.as_deref().map(probe_text),
        help: error.help.as_deref().map(probe_text),
    }
}
#[derive(Debug, Eq, PartialEq)]
struct ProbeOutcome {
    failure: Option<ProbeFailure>,
    ordinary_work: usize,
    skeleton_work: usize,
}
fn observe_generic_probe(function: &ResolvedFunction, limit: usize, floor: usize)
    -> ((ProbeOutcome, crate::test_allocation_observer::Metrics), bool, usize,
        Option<crate::bounded_output::ReservationRefusal>, Option<(&'static str,u32)>)
{
    let (result,refused,paid,first,site)=crate::bounded_output::with_limit_usage_refusal_site(limit,||measure(||{
        assert!(crate::bounded_output::set_active_floor(floor));
        let mut budget=ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let result=super::super::plan_skeleton_paths(function,&mut budget);
        let failure=match result {
            Ok(paths)=>{drop(paths);None}
            Err(error)=>{
                let first=probe_failure(&error);
                let paid=crate::bounded_output::active_remaining();
                let work=(budget.remaining,budget.skeleton_remaining);
                assert!(budget.reserve_bytes(function,0,"generic probe sticky zero").is_err());
                assert_eq!(crate::bounded_output::active_remaining(),paid);
                assert_eq!((budget.remaining,budget.skeleton_remaining),work);
                assert_eq!(probe_failure(&error),first);
                drop(error);Some(first)
            }
        };
        let result=ProbeOutcome {failure,ordinary_work:budget.remaining,skeleton_work:budget.skeleton_remaining};
        drop(budget);crate::bounded_output::clear_active_floor();result
    }));
    assert!(!result.1.invalid && result.1.live==0 && result.1.allocated==result.1.freed);
    assert!(result.1.allocated<=paid);
    (result,refused,paid,first,site.map(|location|(location.file(),location.line())))
}

#[test]
fn real_generic_probe_receiving_frontier_exact_short_zero_floor_is_sticky_and_drops_completely() {
    let mut source=String::from("module physical_prefix; @id(\"f\") fn f(x:i64)->i64 {");
    for index in 0..48 {
        let value=if index==0 {"x".to_owned()}else{format!("x{}",index-1)};
        source.push_str(&format!("let x{index}={value}+1;"));
    }
    source.push_str("x47} @id(\"main\") fn main()->i64 {f(0)}");
    let checked=crate::check(&source,"physical-prefix.spx").unwrap();
    let mut program=crate::hir::resolve(&checked).unwrap();
    let function=program.functions.iter_mut().find(|function|function.id.as_str()=="f").unwrap();
    let last=function.cleanup_plan.blocks.iter().rposition(|block|
        matches!(&block.terminator,CleanupTerminator::Branch(edges) if edges.len()==2)).unwrap();
    // Preserve the checked status branches and their real observations. A Goto
    // does not emit a Status observation, and selecting its first edge can
    // terminate before this final branch. Only append the third final edge.
    let CleanupTerminator::Branch(edges)=&function.cleanup_plan.blocks[last].terminator else {unreachable!()};
    let mut edge=function.cleanup_plan.edges[edges[edges.len()-1].0 as usize].clone();
    let id=EdgeId(u32::try_from(function.cleanup_plan.edges.len()).unwrap());
    edge.id=id;function.cleanup_plan.edges.push(edge);
    let CleanupTerminator::Branch(edges)=&mut function.cleanup_plan.blocks[last].terminator else {unreachable!()};
    edges.push(id);
    let before=GENERIC_TAKES.with(Cell::get);
    let (success,refused,_,first,_)=observe_generic_probe(function,usize::MAX,0);
    assert!(success.0.failure.is_none() && !refused && first.is_none());
    assert!(GENERIC_TAKES.with(Cell::get)>before);
    let target_line=u32::try_from(include_str!("../../replay.rs").lines()
        .position(|line|line.contains("budget.reserve_bytes(function, branch_clone::TAKE_CALL_CONTROLS,")).unwrap()+1).unwrap();
    // Walk actual reservation receipts, never an old bill or guessed offset.
    // Each rerun admits precisely the preceding refused request. The first
    // target receipt therefore identifies this new caller's real prepayment.
    let mut limit=0;
    let mut found=None;
    for _ in 0..100_000 {
        let (outcome,refused,paid,first,site)=observe_generic_probe(function,limit,0);
        assert!(refused && outcome.0.failure.is_some());
        let first=first.unwrap();
        if first.requested==TAKE_CALL_CONTROLS
            && site.is_some_and(|(path,line)|path.ends_with("cleanup_plan/replay.rs") && line==target_line) {
            found=Some(paid);break;
        }
        let next=paid.checked_add(first.requested).unwrap();
        assert!(next>limit,"actual reservation staircase must advance");limit=next;
    }
    let prefix=found.expect("the real three-edge caller must expose its receiving frontier");
    let end=prefix.checked_add(TAKE_CALL_CONTROLS).unwrap();
    let (exact,_,paid,_,_)=observe_generic_probe(function,end,0);
    assert!(paid>=end,"the exact real request is admitted before any later refusal");
    drop(exact);
    for (limit,floor,remaining) in [(prefix,0,0),(end-1,0,TAKE_CALL_CONTROLS-1),(end,1,TAKE_CALL_CONTROLS)] {
        let (first,refused,paid,receipt,site)=observe_generic_probe(function,limit,floor);
        assert!(refused && first.0.failure.is_some());
        assert_eq!(paid,prefix);
        let receipt=receipt.unwrap();
        assert_eq!((receipt.requested,receipt.remaining,receipt.floor),(TAKE_CALL_CONTROLS,remaining,floor));
        assert!(site.is_some_and(|(path,line)|path.ends_with("cleanup_plan/replay.rs") && line==target_line));
        let (again,again_refused,again_paid,again_receipt,again_site)=observe_generic_probe(function,limit,floor);
        assert_eq!(again.0,first.0,"all six Diagnostic fields and both sticky work accounts");
        assert_eq!(again.1,first.1,"actual allocation count, cumulative bytes, peak and whole Drop");
        assert_eq!((again_refused,again_paid,again_receipt,again_site),(refused,paid,Some(receipt),site));
    }
}

thread_local! { static PLAN_COPIES: Cell<usize> = const { Cell::new(0) }; }
pub(super) fn note_plan_copy() {
    PLAN_COPIES.with(|count| count.set(count.get() + 1));
}

mod main_capacity {
use super::*;
use crate::cleanup_plan::replay::plan_skeleton_paths;

#[test]
fn next_fork_capacity_keeps_every_full_arithmetic_path_and_original_work() {
    let mut source = String::from("module next_fork; @id(\"f\") fn f(x:i64)->i64 {");
    for index in 0..96 {
        let input = if index == 0 {
            "x".to_owned()
        } else {
            format!("x{}", index - 1)
        };
        source.push_str(&format!("let x{index}={input}+1;"));
    }
    source.push_str("x95} @id(\"main\") fn main()->i64 {f(0)}");
    let checked = crate::check(&source, "next-fork.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "next-fork.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    let program = crate::hir::resolve(&checked).unwrap();
    let before_copies = PLAN_COPIES.with(Cell::get);
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for function in &program.functions {
                let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let before = skeleton_materializations();
                let expected =
                    with_original(|| compact_stage::original_paths(function, &mut old)).unwrap();
                let old_count = skeleton_materializations() - before;
                let before = skeleton_materializations();
                let actual = plan_skeleton_paths(function, &mut current).unwrap();
                assert_eq!(skeleton_materializations() - before, old_count);
                assert!(compact_stage::paths_match(&actual, &expected));
                assert_eq!(current.remaining, old.remaining);
                assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                drop(actual);
                drop(expected);
                drop(current);
                drop(old);
            }
        })
    });
    assert!(PLAN_COPIES.with(Cell::get) > before_copies);
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}

#[test]
fn next_fork_clone_keeps_future_capacity_and_exact_refusal_frontiers() {
    let program = program();
    let function = &program.functions[0];
    let mut plan = function.cleanup_plan.clone();
    for block in &mut plan.blocks[..3] {
        block.transitions.clear();
    }
    plan.blocks[0].terminator = CleanupTerminator::Goto(EdgeId(0));
    plan.edges[0].to = BlockId(1);
    plan.blocks[1].terminator = CleanupTerminator::Branch(vec![EdgeId(1), EdgeId(2)]);
    plan.edges[1].to = BlockId(2);
    plan.edges[2].to = BlockId(2);
    plan.blocks[2].terminator = CleanupTerminator::Exit(plan.exits[0].id);
    plan.exits[0].continuation = ExitContinuation::ReturnUnit;
    for width in [42usize, 64, 128] {
        let values: Vec<_> = (0..width)
            .map(|_| {
                Rc::new(SkeletonObservation::Boolean {
                    expression: crate::hir::ExpressionId::from_owned("next.capacity".to_owned()),
                    value: true,
                })
            })
            .collect();
        let next = Rc::clone(&values[0]);
        assert_eq!(plan_capacity(&plan, BlockId(0), width), Some(width + 3));
        let observe = |current, limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    let before = skeleton_materializations();
                    let result = (|| {
                        let mut copy = if current {
                            for_plan_push(
                                &mut budget,
                                function,
                                &values,
                                "next fork clone",
                                &plan,
                                BlockId(0),
                            )?
                        } else {
                            for_push(&mut budget, function, &values, "next fork clone")?
                        };
                        let capacity = copy.capacity();
                        if current {
                            assert_eq!(capacity, width + 3);
                        }
                        for at in 0..3 {
                            if at == 2 {
                                assert!(can_take(TakeCandidate {
                                    values: &copy,
                                    remaining_edges: 0,
                                    edge_count: 2,
                                    budget: &budget,
                                    plan: &plan,
                                    target: BlockId(2),
                                }));
                            }
                            skeleton_push(
                                &mut budget,
                                function,
                                &mut copy,
                                Rc::clone(&next),
                                "future push",
                            )?;
                            assert_eq!(copy.capacity(), capacity);
                        }
                        assert!(copy[..width]
                            .iter()
                            .zip(&values)
                            .all(|(left, right)| Rc::ptr_eq(left, right)));
                        assert!(copy[width..].iter().all(|value| Rc::ptr_eq(value, &next)));
                        drop(copy);
                        Ok::<_, Diagnostic>(())
                    })();
                    let state = (
                        result.is_ok(),
                        budget.remaining,
                        budget.skeleton_remaining,
                        skeleton_materializations() - before,
                    );
                    drop(result);
                    drop(budget);
                    state
                })
            })
        };
        let ((old, old_heap), refused, old_paid, first) = observe(false, usize::MAX);
        assert!(!refused && first.is_none() && old.0);
        let ((current, heap), refused, exact, first) = observe(true, usize::MAX);
        assert!(!refused && first.is_none());
        assert_eq!(current, old);
        assert!(exact < old_paid && heap.allocated < old_heap.allocated);
        for (metrics, paid) in [(old_heap, old_paid), (heap, exact)] {
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
            assert!(metrics.allocated <= paid);
        }
        for limit in [0, exact - 1, exact] {
            let ((state, metrics), refused, paid, first) = observe(true, limit);
            assert_eq!(state.0, limit == exact);
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_some(), limit != exact);
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
            assert!(metrics.allocated <= paid);
        }
    }
    plan.blocks[2].terminator = CleanupTerminator::Branch(vec![EdgeId(1), EdgeId(2)]);
    assert_eq!(plan_capacity(&plan, BlockId(0), 128), None);
    plan.blocks[0].terminator = CleanupTerminator::Goto(EdgeId(u32::MAX));
    assert_eq!(plan_capacity(&plan, BlockId(0), 128), None);
}

}
