//! Complete immediate-old/current replay over the same checked fixtures.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

const SOURCE: &str = r#"
module boxed;
@id("f") fn f(flag:bool)->i64 {
    let scratch=bytes_zeroed(3usize);
    let view=bytes_as_slice(scratch);
    if flag && byte_len(view)==3usize {7}else{9}
}
@id("main") fn main()->i64 {f(false)}
"#;

fn program() -> ResolvedProgram {
    let checked = crate::check(SOURCE, "singleton-path-group.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "singleton-path-group.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}
fn replay(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    old: bool,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    if old {
        before_frontier::validate_path_states(
            program, function, &storage, &leaves, budget, None, None,
        )
    } else {
        validate_path_states(program, function, &storage, &leaves, budget, None, None)
    }
}

#[test]
fn final_edge_move_keeps_complete_immediate_work_and_hostile_diagnostic() {
    let program = program();
    for function in &program.functions {
        let original_plan = function.cleanup_plan.clone();
        let mut full_old = ReplayBudget::unobserved();
        replay(&program, function, true, &mut full_old).unwrap();
        let work = MAX_REPLAY_WORK_UNITS - full_old.remaining;
        for limit in [
            0,
            1,
            2,
            7,
            31,
            127,
            work.saturating_sub(1),
            work,
            MAX_REPLAY_WORK_UNITS,
        ] {
            let mut old = ReplayBudget::unobserved();
            let mut current = ReplayBudget::unobserved();
            old.remaining = limit;
            current.remaining = limit;
            let expected = replay(&program, function, true, &mut old);
            let actual = replay(&program, function, false, &mut current);
            assert_eq!(actual, expected, "function={} work={limit}", function.id);
            assert_eq!(current.remaining, old.remaining);
            assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
            assert_eq!(function.cleanup_plan, original_plan);
        }
    }
    // Direct low-level callers retain the old generic behavior even when
    // the enclosing production CFG certificate would reject this cardinality.
    let original = program.functions.iter().find(|f| f.name == "f").unwrap();
    for count in [0, 1, 3] {
        let mut malformed = original.clone();
        let edges = malformed
            .cleanup_plan
            .blocks
            .iter_mut()
            .find_map(|block| {
                if let CleanupTerminator::Branch(edges) = &mut block.terminator {
                    Some(edges)
                } else {
                    None
                }
            })
            .expect("fixture owns a checked two-edge branch");
        let first = edges[0];
        edges.resize(count, first);
        let before = malformed.cleanup_plan.clone();
        let mut old = ReplayBudget::unobserved();
        let mut current = ReplayBudget::unobserved();
        assert_eq!(
            replay(&program, &malformed, false, &mut current),
            replay(&program, &malformed, true, &mut old)
        );
        assert_eq!(current.remaining, old.remaining);
        assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
        assert_eq!(malformed.cleanup_plan, before);
    }
    let mut forged = program
        .functions
        .iter()
        .find(|f| f.name == "f")
        .unwrap()
        .clone();
    let block = forged
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .transitions
                .iter()
                .any(|t| matches!(t, CleanupTransition::Initialize { .. }))
        })
        .expect("original owned fixture initializes storage");
    let index = block
        .transitions
        .iter()
        .position(|t| matches!(t, CleanupTransition::Initialize { .. }))
        .unwrap();
    block
        .transitions
        .insert(index + 1, block.transitions[index].clone());
    let original_plan = forged.cleanup_plan.clone();
    let mut old = ReplayBudget::unobserved();
    let mut current = ReplayBudget::unobserved();
    let expected = replay(&program, &forged, true, &mut old).unwrap_err();
    let actual = replay(&program, &forged, false, &mut current).unwrap_err();
    assert_eq!(expected.code, "SPX-H006");
    assert_eq!(actual, expected); // all six Diagnostic fields, not only its code
    assert_eq!(current.remaining, old.remaining);
    assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
    assert_eq!(forged.cleanup_plan, original_plan);
}

const TINY: &str = "module tiny; @id(\"main\") fn main()->i64 {0}";
const LEXICAL: &str = r#"module ordinary.lexical;
@id("probe") fn probe(flag:bool)->i64 {
 let key=str"x";let view=str_as_bytes(key);let mut next=0;
 while next<3 {next=next+1;if flag {continue;};if next==2 {return if byte_len(view)==1usize {7}else{0};};0}
 9
}
@id("main") fn main()->i64 {probe(false)}
"#;
const RENEWAL_UNREACHABLE: &str = r#"module ordinary.byte_renewal;
@id("app.main") fn main()->i64 {
 let mut buffer=bytes_zeroed(2usize);
 let untouched=bytes_zeroed(1usize);
 let mut index=0usize;
 while index<2usize {
  if index==0usize {buffer=bytes_set(buffer,index,65u8);0}else{0}
  index=index+1usize;
  0
 }
 return if byte_len(bytes_as_slice(buffer))==2usize && byte_len(bytes_as_slice(untouched))==1usize {7}else{0};
 0
}
"#;
const RENEWAL: &str = r#"module ordinary.byte_renewal;
@id("app.main") fn main()->i64 {
 let mut buffer=bytes_zeroed(2usize);
 let untouched=bytes_zeroed(1usize);
 let mut index=0usize;
 while index<2usize {
  if index==0usize {buffer=bytes_set(buffer,index,65u8);0}else{0}
  index=index+1usize;
  0
 }
 return if byte_len(bytes_as_slice(buffer))==2usize && byte_len(bytes_as_slice(untouched))==1usize {7}else{0};
}
"#;
fn ordinary_replay(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    old: bool,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let storage = validate_slots_paid(function, budget)?;
    let leaves = collect_leaves_paid(function, budget)?;
    let auth = if lexical_cert::requires_lexical(function) {
        let owner = selected_owner::owner(program, function, budget)?;
        let facts = lexical_hir::collect(function, budget)?;
        Some(lexical_cert::authenticate(
            program, function, &owner, &facts, budget,
        )?)
    } else {
        None
    };
    if old {
        before_frontier::validate_path_states(
            program,
            function,
            &storage,
            &leaves,
            budget,
            auth.as_ref(),
            None,
        )
    } else {
        validate_path_states(
            program,
            function,
            &storage,
            &leaves,
            budget,
            auth.as_ref(),
            None,
        )
    }
}
#[test]
fn ordinary_complete_cold_tiny_branch_and_lexical_costs_include_final_drop_and_exact_refusal() {
    // Keep the original invalid fixture as a full diagnostic control. The
    // successful renewal case below keeps its return and every ownership use;
    // only the unreachable trailing expression is absent.
    let errors = crate::check(RENEWAL_UNREACHABLE, "ordinary-frontier.spx").unwrap_err();
    assert_eq!(errors.len(), 1);
    let error = &errors[0];
    assert_eq!(error.code, "SPX-P209");
    assert_eq!(format!("{:?}", error.severity), "Error");
    assert_eq!(
        error.message,
        "code after an unconditional return is unreachable"
    );
    assert_eq!(error.path.as_deref(), Some("ordinary-frontier.spx"));
    let span = error.span.as_ref().unwrap();
    assert_eq!(
        (span.start, span.end, span.line, span.column),
        (390, 391, 12, 2)
    );
    assert_eq!(
        error.help.as_deref(),
        Some("remove the unreachable code or put it on a non-returning guard path")
    );
    for source in [TINY, SOURCE, LEXICAL, RENEWAL] {
        let program =
            crate::hir::resolve(&crate::check(source, "ordinary-frontier.spx").unwrap()).unwrap();
        for function in &program.functions {
            let observe = |old, limit| {
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let mut b = ReplayBudget::new();
                        let result = ordinary_replay(&program, function, old, &mut b);
                        let success = result.is_ok();
                        if let Err(e) = &result {
                            assert_eq!(e.code, "SPX-H006");
                        }
                        drop(result);
                        drop(b);
                        success
                    })
                })
            };
            let old = observe(true, usize::MAX);
            let current = observe(false, usize::MAX);
            assert!(old.0 .0 && current.0 .0 && !old.1 && !current.1);
            eprintln!(
                "ordinary-frontier {} old_invoice={} old={:?} current_invoice={} current={:?}",
                function.id, old.2, old.0 .1, current.2, current.0 .1
            );
            for o in [&old, &current] {
                let a = &o.0 .1;
                assert!(!a.invalid && a.live == 0);
                assert_eq!(a.allocated, a.freed);
                assert!(a.allocated <= o.2);
            }
            for limit in [0, current.2 - 1, current.2] {
                let o = observe(false, limit);
                assert_eq!(o.0 .0, limit == current.2);
                assert_eq!(o.1, limit != current.2);
                assert_eq!(o.3.is_some(), limit != current.2);
                let a = &o.0 .1;
                assert!(!a.invalid && a.live == 0);
                assert_eq!(a.allocated, a.freed);
                assert!(a.allocated <= o.2 && o.2 <= limit);
            }
            let mut old_budget = ReplayBudget::unobserved();
            let mut current_budget = ReplayBudget::unobserved();
            let expected = ordinary_replay(&program, function, true, &mut old_budget);
            assert_eq!(
                ordinary_replay(&program, function, false, &mut current_budget),
                expected
            );
            assert_eq!(current_budget.remaining, old_budget.remaining);
            assert_eq!(
                current_budget.skeleton_remaining,
                old_budget.skeleton_remaining
            );
        }
    }
}
