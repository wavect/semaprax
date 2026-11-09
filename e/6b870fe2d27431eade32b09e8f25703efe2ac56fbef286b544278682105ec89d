use super::*;
use crate::cleanup_plan::replay::{ReplayBudget, MAX_REPLAY_WORK_UNITS};
use crate::hir::ResolvedProgram;
use sha2::{Digest, Sha256};

const CAP: usize = 64 * 1024 * 1024;
fn fixture() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(
        "module release.scratch; @id(\"f\") fn f(x:i64)->i64 requires x>=0 ensures result>=0 {x+1} @id(\"main\") fn main()->i64 { f(1)+f(2)+f(3)+f(4) }",
        "release.spx").unwrap()).unwrap()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn run(program: &ResolvedProgram, old: bool, work: usize) -> (Option<String>, usize, usize) {
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    let mut budget = ReplayBudget::new();
    budget.remaining = work;
    let result = match crate::cleanup_plan::replay::reserve_program_skeleton_work(
        program,
        std::iter::once(function),
        &mut budget,
    ) {
        Err(error) => Err(error),
        Ok(_) => {
            if old {
                super::super::before_structure_release::validate_structure_with_budget_storage(
                    program,
                    function,
                    &mut budget,
                    None,
                    None,
                )
            } else {
                super::super::ordinary_cross_phase::validate_structure_with_budget_storage(
                    program,
                    function,
                    &mut budget,
                    None,
                    None,
                )
            }
        }
    };
    (
        result.err().map(|d| d.json()),
        budget.remaining,
        budget.skeleton_spent,
    )
}

#[test]
fn full_immediate_receiver_and_exact_release_only_inverse_remain_independent() {
    let before = include_str!("whole.before.txt");
    assert_eq!(before.len(), 86698);
    assert_eq!(
        hash(before.as_bytes()),
        "fa232cdadb3ff7ea321caa04554d8dc2c237928b976af5890f6ed555c276b1ec"
    );
    let compiled = include_str!("before.rs");
    assert_eq!(compiled, &before[..compiled.len()]);
    assert!(before[compiled.len()..].starts_with("#[cfg(test)]"));
    let mut current = include_str!("../ordinary_cross_phase.rs").to_owned();
    for (indent, attempt) in [
        ("                                ", ", attempt"),
        ("        ", ""),
    ] {
        let after = format!("finish_structure!(program, function, budget, helpers, continuations;\n{indent}    statuses, leaves, storage, status_cases, expression_facts{attempt})?;");
        let old = format!("validate_typed_control_skeleton_with_index(\n{indent}    program,\n{indent}    function,\n{indent}    budget,\n{indent}    helpers,\n{indent}    continuations,\n{indent})?;");
        assert_eq!(current.matches(&after).count(), 1);
        current = current.replacen(&after, &old, 1);
    }
    assert_eq!(current.as_bytes(), before.as_bytes());
}

#[test]
fn whole_ordinary_and_funded_receivers_keep_work_and_complete_diagnostics() {
    let program = fixture();
    for work in [0, 1, 2, 5, 16, 64, 256, 4096, MAX_REPLAY_WORK_UNITS] {
        assert_eq!(run(&program, false, work), run(&program, true, work));
        let (old, old_refused, _) =
            crate::bounded_output::with_limit_usage(CAP, || run(&program, true, work));
        let (current, current_refused, _) =
            crate::bounded_output::with_limit_usage(CAP, || run(&program, false, work));
        assert_eq!(current, old);
        assert_eq!(current_refused, old_refused);
    }
    for mode in 0..3 {
        let mut program = fixture();
        let owner = program
            .functions
            .iter_mut()
            .find(|f| f.name == "main")
            .unwrap();
        let body = owner.body.id.clone();
        let mut changed = false;
        for block in &mut owner.cleanup_plan.blocks {
            for transition in &mut block.transitions {
                if let crate::cleanup_plan::CleanupTransition::CallCommit { call, .. } = transition
                {
                    if !changed {
                        *call = if mode == 0 {
                            body.clone()
                        } else {
                            crate::hir::ExpressionId::from_owned(format!("missing.release.{mode}"))
                        };
                        changed = true;
                    }
                }
            }
        }
        assert!(changed);
        if mode == 2 {
            owner.cleanup_plan.schema = "foreign.release";
        }
        let before = format!("{program:?}");
        let old = run(&program, true, MAX_REPLAY_WORK_UNITS);
        let current = run(&program, false, MAX_REPLAY_WORK_UNITS);
        assert!(old.0.is_some());
        assert_eq!(current, old, "all six fields mode={mode}");
        assert_eq!(format!("{program:?}"), before);
    }
}

#[test]
fn independent_release_invoice_and_exact_short_zero_sticky_full_drop() {
    type Owners = (
        StatusSources<'static>,
        Leaves,
        BTreeSet<StorageId>,
        StatusCases<'static>,
        Facts<'static>,
    );
    let payment = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
        + 2 * size_of::<Result<(), Diagnostic>>();
    assert_eq!(RELEASE, 2 * size_of::<Owners>() + payment);
    assert_eq!(
        ATTEMPT,
        2 * size_of::<Attempt<'static, 'static>>() + payment
    );
    let program = fixture();
    let (old_ample, old_refused, _) =
        crate::bounded_output::with_limit_usage(CAP, || run(&program, true, MAX_REPLAY_WORK_UNITS));
    assert!(
        old_ample.0.is_none() && !old_refused,
        "complete immediate-old ample result={old_ample:?}, refused={old_refused}"
    );
    let (ample, refused, paid) = crate::bounded_output::with_limit_usage(CAP, || {
        run(&program, false, MAX_REPLAY_WORK_UNITS)
    });
    assert!(
        ample.0.is_none() && !refused,
        "complete current ample result={ample:?}, refused={refused}"
    );
    assert_eq!(ample, old_ample);
    for cap in [paid, paid - 1, 0] {
        let ((result, retry), refused, used) = crate::bounded_output::with_limit_usage(cap, || {
            let result = run(&program, false, MAX_REPLAY_WORK_UNITS);
            let function = program.functions.iter().find(|f| f.name == "main").unwrap();
            let mut budget = ReplayBudget::new();
            let retry = budget.reserve_bytes(function, 0, "release restored account retry");
            (result, retry)
        });
        if cap == paid {
            assert!(result.0.is_none() && retry.is_ok() && !refused);
            assert_eq!(used, paid);
        } else {
            assert!(result.0.is_some() && retry.is_err() && refused);
        }
        let (_, allocation) = crate::test_allocation_observer::measure(|| {
            drop(crate::bounded_output::with_limit_usage(cap, || {
                run(&program, false, MAX_REPLAY_WORK_UNITS)
            }));
        });
        assert!(!allocation.invalid);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn actual_whole_receiver_reports_physical_storage_without_forecast_credit() {
    let program = fixture();
    for old in [true, false] {
        let (_, allocation) = crate::test_allocation_observer::measure(|| {
            drop(run(&program, old, MAX_REPLAY_WORK_UNITS));
        });
        assert!(!allocation.invalid);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
        eprintln!("ordinary structure last-use old={old} allocation={allocation:?}");
    }
}
