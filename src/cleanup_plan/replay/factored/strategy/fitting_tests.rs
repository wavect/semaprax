//! Ordinary fitting admission reuses the held exact comparison; old entry stays independent.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

const NESTED: &str = r#"module factored.coverage_nested;
@id("probe") fn probe(n:i64)->i64 {
 let mut outer=0;
 let mut total=0;
 while outer<n {
  outer=outer+1;
  let mut inner=0;
  while inner<2 {
   inner=inner+1;
   if inner==1 {continue;};
   if n==0 {break;};
   total=total+inner;
   0
  }
  if outer==2 {continue;};
  if total>4 {return total;};
  0
 }
 return total;
}
@id("main") fn main()->i64 {0}
"#;

const OWNED: &str = r#"module factored.coverage_owned;
@id("probe") fn probe(n:i64)->Bytes {
 let payload=if n==0 {bytes_zeroed(1usize)}else{bytes_zeroed(2usize)};
 let mut i=0;
 while i<1 {
  i=i+1;
  if n==1 {return payload;};
  0
 }
 payload
}
@id("main") fn main()->i64 {0}
"#;

const CONTRACTS: &str = r#"module factored.coverage_contracts;
@id("check_loop") fn check_loop(n:i64)->bool {
 let mut i=0;
 while i<n {i=i+1;0}
 return i==n;
}
@id("probe") fn probe(n:i64)->i64
 requires check_loop(1)
 ensures check_loop(2)
{
 let owner="live root";
 let mut i=0;
 while i<1 {
  i=i+1;
  if n==1 {return 11;};
  if n==2 {return 12;};
  0
 }
 13
}
@id("main") fn main()->i64 {0}
"#;

fn same_error(actual: &Diagnostic, expected: &Diagnostic) {
    assert_eq!(actual.code, expected.code);
    assert_eq!(actual.severity, expected.severity);
    assert_eq!(actual.message, expected.message);
    assert_eq!(actual.path, expected.path);
    assert_eq!(actual.span, expected.span);
    assert_eq!(actual.help, expected.help);
}
#[test]
fn shipping_fitting_held_proof_matches_complete_enumerator_and_lexical_forgeries() {
    for source in [
        lexical_plan_tests::WHILE,
        lexical_plan_tests::GENERATED,
        NESTED,
        OWNED,
        CONTRACTS,
    ] {
        let (program, function) = lexical_plan_tests::fixture(source);
        crate::hir::validate(&program).unwrap();
        let original_plan = function.cleanup_plan.clone();
        let mut budget = ReplayBudget::new();
        let candidate = Candidate::reconstruct(&program, &function, &mut budget).unwrap();
        assert_eq!(candidate.strategy, Strategy::Enumerated);
        assert!(matches!(candidate.hir, Census::Known { .. }));
        assert!(matches!(candidate.plan, Census::Known { .. }));
        drop((candidate, budget));
        // None is the actual shipping dispatch, never a forced old enumerator.
        assert_eq!(forced_strategy(), None);
        validate_structure(&program, &function).unwrap();
        with_strategy(Strategy::Enumerated, || {
            validate_structure(&program, &function)
        })
        .unwrap();
        assert_eq!(function.cleanup_plan, original_plan);
        lexical_plan_tests::assert_copy_scope_oracle(source);
        lexical_plan_tests::assert_plan_oracle(&function);
        let mut forged = function.clone();
        assert!(forged.cleanup_plan.exits.pop().is_some());
        let actual = validate_structure(&program, &forged).unwrap_err();
        let expected = with_strategy(Strategy::Enumerated, || {
            validate_structure(&program, &forged)
        })
        .unwrap_err();
        same_error(&actual, &expected);
        assert_eq!(actual.code, "SPX-H006");
        assert_eq!(function.cleanup_plan, original_plan);
    }
}

#[test]
fn ordinary_fitting_whole_owner_costs_exact_short_work_bytes_and_drop_are_actual() {
    let (program, function) = lexical_plan_tests::fixture(lexical_plan_tests::WHILE);
    let original_plan = function.cleanup_plan.clone();
    // Source/fixture preparation is excluded identically. Every comparison,
    // census, canonical receiver or old enumerator and its Drop is included.
    let observe = |old: bool, work: usize, bytes: usize| {
        with_limit_usage_refusal(bytes, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                budget.remaining = work;
                let result = (|| {
                    reserve_program_skeleton_work(
                        &program,
                        std::iter::once(&function),
                        &mut budget,
                    )?;
                    if old {
                        with_strategy(Strategy::Enumerated, || {
                            validate_structure_with_budget(&program, &function, &mut budget, None)
                        })
                    } else {
                        validate_structure_with_budget(&program, &function, &mut budget, None)
                    }
                })();
                let accepted = result.is_ok();
                let error = result.as_ref().err().map(|error| error.code);
                let used = work - budget.remaining;
                let held = budget.skeleton_remaining;
                let spent = budget.skeleton_spent;
                if result.is_err() && budget.refusal_spent {
                    let retry = budget
                        .reserve_bytes(&function, 0, "fitting retry")
                        .unwrap_err();
                    assert_eq!(retry.code, "SPX-H006");
                    assert_eq!(retry.severity, crate::diagnostic::Severity::Error);
                    assert!(retry.message.is_empty());
                    assert!(retry.path.is_none() && retry.span.is_none() && retry.help.is_none());
                    drop(retry);
                }
                drop((result, budget));
                (accepted, error, used, held, spent)
            })
        })
    };
    let ((old, old_alloc), old_refused, old_paid, old_first) =
        observe(true, MAX_REPLAY_WORK_UNITS, usize::MAX);
    let ((current, current_alloc), refused, paid, first) =
        observe(false, MAX_REPLAY_WORK_UNITS, usize::MAX);
    assert!(old.0 && current.0 && !old_refused && !refused);
    assert!(old_first.is_none() && first.is_none());
    for allocation in [old_alloc, current_alloc] {
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
    assert!(old_alloc.allocated <= old_paid && current_alloc.allocated <= paid);
    assert!(current.2 > 0 && paid > 0);
    eprintln!("fitting whole old paid={old_paid} allocations={} allocated={} peak={} work={} held={} skeleton_spent={}; current paid={paid} allocations={} allocated={} peak={} work={} held={} skeleton_spent={}",old_alloc.allocations,old_alloc.allocated,old_alloc.highwater,old.2,old.3,old.4,current_alloc.allocations,current_alloc.allocated,current_alloc.highwater,current.2,current.3,current.4);
    // The legacy upper remains held; the alternative proof spends ordinary free
    // work rather than taking this bank or inventing a fresh allowance.
    assert!(current.3 > old.3);
    for (work, bytes, accepted) in [
        (current.2, paid, true),
        (current.2 - 1, paid, false),
        (0, paid, false),
        (current.2, paid - 1, false),
        (current.2, 0, false),
    ] {
        let ((outcome, allocation), refused, reserved, _) = observe(false, work, bytes);
        assert_eq!(outcome.0 && !refused, accepted, "work={work} bytes={bytes}");
        if !accepted && !outcome.0 {
            assert_eq!(outcome.1, Some("SPX-H006"));
        }
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        if accepted {
            assert!(allocation.allocated <= reserved);
        }
    }
    assert_eq!(function.cleanup_plan, original_plan);
    // Keep the existing whole-module old/new work ledgers distinct and cumulative.
    let run_module = |old: bool, work: usize| {
        let mut budget = ReplayBudget::new();
        budget.remaining = work;
        let result = (|| {
            reserve_program_skeleton_work(&program, program.functions(), &mut budget)?;
            for function in program.functions() {
                if old {
                    with_strategy(Strategy::Enumerated, || {
                        validate_structure_with_budget(&program, function, &mut budget, None)
                    })?;
                } else {
                    validate_structure_with_budget(&program, function, &mut budget, None)?;
                }
            }
            Ok::<_, Diagnostic>(work - budget.remaining)
        })();
        drop(budget);
        result
    };
    for old in [true, false] {
        let spent = run_module(old, MAX_REPLAY_WORK_UNITS).unwrap();
        assert_eq!(run_module(old, spent).unwrap(), spent);
        assert_eq!(run_module(old, spent - 1).unwrap_err().code, "SPX-H006");
    }
}
