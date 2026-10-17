use super::*;
use std::rc::Rc;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{measure, Metrics};

fn program(prefix: usize) -> ResolvedProgram {
    let locals = (0..prefix).map(|i| format!("let n{i}={i};")).collect::<String>();
    let source = format!("module fused879; @id(\"f\") fn f(a:bool,b:bool)->i64 {{{locals} let n=if a {{1}}else{{2}}; if b {{n+1}}else{{n+2}}}} @id(\"main\") fn main()->i64 {{f(true,false)}}");
    let parsed = crate::check(&source, "fused879.spx").unwrap();
    crate::hir::resolve(&parsed).unwrap()
}
fn values(function: &ResolvedFunction, width: usize, capacity: usize) -> Vec<Rc<SkeletonObservation>> {
    let mut values = Vec::with_capacity(capacity);
    for _ in 0..width {
        values.push(Rc::new(SkeletonObservation::Boolean { expression: function.body.id.clone(), value: true }));
    }
    values
}
#[test]
fn production_fused_probe_preserves_every_original_take_decision_and_bounds_copy_receivers() {
    let program = program(0);
    let function = &program.functions[0];
    with_limit_usage_refusal(1 << 30, || {
        let mut budget = ReplayBudget::new();
        let mut selected_copy = false;
        for width in [0usize, 1, 2, 16, 42, 96] {
            for capacity in [width, width+1, width*2+1] {
                let values = values(function, width, capacity);
                for target in 0..function.cleanup_plan.blocks.len() {
                    for remaining_edges in [0, 1] {
                        for edge_count in [1, 2, 3] {
                            budget.reserve_bytes(function, 2 * TAKE_CALL_CONTROLS + ADDITIONAL_CONTROLS,
                                "two independent take query receivers").unwrap();
                            let expected = can_take(TakeCandidate { values: &values, remaining_edges, edge_count,
                                budget: &budget, plan: &function.cleanup_plan, target: BlockId(target as u32) });
                            let actual = probe(TakeCandidate { values: &values, remaining_edges, edge_count,
                                budget: &budget, plan: &function.cleanup_plan, target: BlockId(target as u32) });
                            assert_eq!(actual.take, expected, "prefix={width} target={target} capacity={capacity}");
                            if let Some(copy) = actual.copy_capacity {
                                selected_copy = true;
                                let original = physical_storage::replay_growth_capacity(width, width+1).unwrap();
                                assert!(copy >= width+1 && copy < original);
                                assert!((original-copy) * size_of::<Rc<SkeletonObservation>>() > ADDITIONAL_CONTROLS);
                            }
                        }
                    }
                }
            }
        }
        assert!(selected_copy, "the fused copy route must actually be selected");
    });
}

#[test]
fn production_fused_enumerator_matches_the_complete_predecessor_paths_work_and_materializations() {
    for prefix in [0, 42, 96] {
        let program = program(prefix);
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(1 << 29, || measure(|| {
            for function in &program.functions {
                let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let begin = skeleton_materializations();
                let expected = before::plan_skeleton_paths(function, &mut old).unwrap();
                let old_events = skeleton_materializations()-begin;
                let begin = skeleton_materializations();
                let actual = crate::cleanup_plan::replay::plan_skeleton_paths(function, &mut current).unwrap();
                assert_eq!(actual, expected);
                assert_eq!(skeleton_materializations()-begin, old_events);
                assert_eq!(current.remaining, old.remaining);
                assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                drop((actual, expected)); drop(current); drop(old);
            }
        }));
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid); assert_eq!(allocation.live,0); assert_eq!(allocation.allocated,allocation.freed);
        assert!(allocation.allocated <= paid && allocation.highwater <= paid);
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Failure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8;1024],usize),
    path: Option<([u8;1024],usize)>,
    span: Option<crate::ast::Span>,
    help: Option<([u8;1024],usize)>,
}
fn text(value: &str) -> ([u8;1024],usize) {
    assert!(value.len()<=1024); let mut bytes=[0;1024]; bytes[..value.len()].copy_from_slice(value.as_bytes()); (bytes,value.len())
}
fn failure(value: Diagnostic) -> Failure {
    Failure { code:value.code,severity:value.severity,message:text(&value.message),path:value.path.as_deref().map(text),span:value.span,help:value.help.as_deref().map(text) }
}
#[derive(Debug, Eq, PartialEq)]
struct Outcome { result:Result<usize,Failure>, remaining:usize, skeleton:usize, events:usize, sticky:Option<Failure> }
fn observe(old:bool,function:&ResolvedFunction,work:usize,limit:usize,floor:usize) -> (Outcome,Metrics,bool,usize) {
    let ((outcome,allocation),refused,paid,_)=with_limit_usage_refusal(limit,||measure(||{
        let mut budget=ReplayBudget::with_skeleton_limit(work);
        assert!(crate::bounded_output::set_active_floor(floor));
        let begin=skeleton_materializations();
        let result=if old { before::plan_skeleton_paths(function,&mut budget) } else { crate::cleanup_plan::replay::plan_skeleton_paths(function,&mut budget) };
        let result=result.map(|paths| {let n=paths.len();drop(paths);n}).map_err(failure);
        let events=skeleton_materializations()-begin;
        let remaining=budget.remaining; let skeleton=budget.skeleton_remaining;
        crate::bounded_output::clear_active_floor();
        let sticky=if result.is_err() {
            let bytes=crate::bounded_output::active_remaining();
            let error=budget.reserve_bytes(function,0,"fused879 sticky refusal").err().unwrap();
            assert_eq!(crate::bounded_output::active_remaining(),bytes);
            Some(failure(error))
        } else {None};
        drop(budget);
        Outcome {result,remaining,skeleton,events,sticky}
    }));
    assert!(!allocation.invalid);assert_eq!(allocation.live,0);assert_eq!(allocation.allocated,allocation.freed);
    (outcome,allocation,refused,paid)
}
#[test]
fn production_fused_clone_keeps_original_work_failures_and_actual_exact_short_floor_drop() {
    for prefix in [0,42,96] {
        let program=program(prefix);let function=&program.functions[0];
        let (old,old_allocation,old_refused,old_paid)=observe(true,function,MAX_REPLAY_WORK_UNITS,1<<29,0);
        let (current,current_allocation,current_refused,current_paid)=observe(false,function,MAX_REPLAY_WORK_UNITS,1<<29,0);
        assert_eq!(old,current);assert!(!old_refused && !current_refused);
        assert!(old_allocation.allocated<=old_paid && current_allocation.allocated<=current_paid);
        let spent=MAX_REPLAY_WORK_UNITS-current.skeleton;
        for work in [0,1,spent/2,spent-1,spent] {
            assert_eq!(observe(true,function,work,1<<29,0).0,observe(false,function,work,1<<29,0).0,"all six original work failure fields");
        }
        let (exact,_,refused,paid)=observe(false,function,MAX_REPLAY_WORK_UNITS,current_paid,0);
        assert_eq!(exact,current);assert!(!refused);assert_eq!(paid,current_paid);
        for limit in [0,1,current_paid/2,current_paid-1] {
            let (refused,_,_,_)=observe(false,function,MAX_REPLAY_WORK_UNITS,limit,0);
            assert_eq!(refused.result.as_ref().err().unwrap().code,"SPX-H006");assert!(refused.sticky.is_some());
        }
        let (floored,_,refused,_)=observe(false,function,MAX_REPLAY_WORK_UNITS,current_paid,1);
        assert!(floored.result.is_err() && floored.sticky.is_some() && refused);
        eprintln!("OPT879 production-only prefix={prefix}: old/current paid={old_paid}/{current_paid}, allocated={}/{}, highwater={}/{}",old_allocation.allocated,current_allocation.allocated,old_allocation.highwater,current_allocation.highwater);
    }
}
