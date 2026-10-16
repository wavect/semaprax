use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let source = "module branch_inline; @id(\"f\") fn f(a:bool,b:bool)->i64 {let n=if a {1}else{2};if b {n+1}else{n+2}} @id(\"main\") fn main()->i64 {f(true,false)}";
    let checked = crate::check(source, "branch-inline.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "branch-inline.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}

#[derive(Debug, Eq, PartialEq)]
struct Failure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 1024], usize),
    path: Option<([u8; 1024], usize)>,
    span: Option<crate::ast::Span>,
    help: Option<([u8; 1024], usize)>,
}
fn text(value: &str) -> ([u8; 1024], usize) {
    let mut bytes = [0; 1024];
    assert!(value.len() <= bytes.len());
    bytes[..value.len()].copy_from_slice(value.as_bytes());
    (bytes, value.len())
}
fn failure(value: Diagnostic) -> Failure {
    let result = Failure {
        code: value.code,
        severity: value.severity,
        message: text(&value.message),
        path: value.path.as_deref().map(text),
        span: value.span,
        help: value.help.as_deref().map(text),
    };
    drop(value);
    result
}
fn enumerate(
    old: bool,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    if old {
        before_inline::plan_skeleton_paths(function, budget)
    } else {
        super::super::plan_skeleton_paths(function, budget)
    }
}

#[test]
fn inline_branch_keeps_complete_paths_work_and_all_six_refusal_fields() {
    let program = program();
    let function = &program.functions[0];
    let (spent, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut old = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut actual = ReplayBudget::with_skeleton_limit(usize::MAX);
        assert!(old.pays_allocations() && actual.pays_allocations());
        let expected = enumerate(true, function, &mut old).unwrap();
        let current = enumerate(false, function, &mut actual).unwrap();
        assert_eq!(current, expected);
        assert_eq!(actual.skeleton_spent, old.skeleton_spent);
        assert_eq!(actual.skeleton_remaining, old.skeleton_remaining);
        assert_eq!(actual.remaining, old.remaining);
        let spent = actual.skeleton_spent;
        assert!(spent > 1);
        drop((current, expected, actual, old));
        spent
    });
    assert!(!refused && first.is_none());
    for work in [0, 1, spent - 1, spent] {
        let observe = |old| {
            with_limit_usage_refusal(usize::MAX, || {
                let mut budget = ReplayBudget::with_skeleton_limit(work);
                let before = skeleton_materializations();
                let result = enumerate(old, function, &mut budget);
                let outcome = match result {
                    Ok(paths) => {
                        let len = paths.len();
                        drop(paths);
                        Ok(len)
                    }
                    Err(error) => Err(failure(error)),
                };
                let outcome = (
                    outcome,
                    budget.skeleton_remaining,
                    budget.skeleton_spent,
                    skeleton_materializations() - before,
                );
                drop(budget);
                outcome
            })
        };
        let (old, old_refused, _, old_first) = observe(true);
        let (actual, refused, _, first) = observe(false);
        assert_eq!(actual, old);
        assert_eq!(refused, old_refused);
        assert_eq!(first.is_some(), old_first.is_some());
        assert_eq!(actual.0.is_ok(), work == spent);
    }
}

#[test]
fn inline_whole_enumerator_reports_cost_and_exact_short_zero_complete_drop() {
    let program = program();
    let function = &program.functions[0];
    let observe = |old, limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let result = enumerate(old, function, &mut budget);
                let outcome = match result {
                    Ok(paths) => {
                        let len = paths.len();
                        drop(paths);
                        Ok(len)
                    }
                    Err(error) => Err(failure(error)),
                };
                let spent = budget.skeleton_spent;
                drop(budget);
                (outcome, spent)
            })
        })
    };
    let ((old, old_alloc), old_refused, old_paid, old_first) = observe(true, usize::MAX);
    let ((actual, allocation), refused, exact, first) = observe(false, usize::MAX);
    eprintln!("branch_inline old_paid={old_paid} current_paid={exact} old_allocation={old_alloc:?} current_allocation={allocation:?}");
    assert!(!old_refused && old_first.is_none() && !refused && first.is_none());
    assert!(actual.0.is_ok());
    assert_eq!(actual, old);
    assert!(!allocation.invalid && allocation.live == 0);
    assert!(!old_alloc.invalid && old_alloc.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert_eq!(old_alloc.allocated, old_alloc.freed);
    assert_eq!(allocation.allocated, old_alloc.allocated);
    assert!(allocation.allocated <= exact && old_alloc.allocated <= old_paid);
    assert!(exact < old_paid); // Actual absent receivers, never a cap or heap credit.
    for limit in [0, exact - 1, exact] {
        let ((outcome, allocation), refused, paid, first) = observe(false, limit);
        assert_eq!(outcome.0.is_ok(), limit == exact);
        assert_eq!(refused, limit != exact);
        assert_eq!(first.is_some(), limit != exact);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
    }
    assert_complete_old_sources();
}

// Complete immediate old source is checked after all measured closures.
fn assert_complete_old_sources() {
    use sha2::{Digest, Sha256};
    let original = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/branch-inline-receiving-next-808-20261016/plan_skeleton_paths.before.rs.txt"));
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "89282c1f26c14e65ced0f50b7dbfada34175fd4ca6aed96d57a43147a8e818e0"
    );
    let compiled = include_str!("before_inline.rs");
    let prefix = "//! Complete immediate old enumerator, using its unchanged fused helper.\nuse super::super::*;\nuse super::super::branch_clone;\n\n";
    assert_eq!(
        compiled,
        format!(
            "{prefix}{}",
            original.replacen(
                "fn plan_skeleton_paths(",
                "pub(super) fn plan_skeleton_paths(",
                1
            )
        )
    );
    let current = include_str!("../branch_clone.rs");
    for original in [
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/branch-inline-receiving-next-808-20261016/controls.before.rs.txt")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/branch-inline-receiving-next-808-20261016/with_observation.before.rs.txt")),
    ] {
        assert_eq!(current.matches(original).count(), 1);
    }
}
