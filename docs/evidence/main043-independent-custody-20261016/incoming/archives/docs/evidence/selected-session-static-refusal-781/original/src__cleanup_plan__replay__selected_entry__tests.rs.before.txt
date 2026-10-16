//! The module owner retains one index and one ledger through the final Drop.
use super::*;

const SOURCE: &str = r#"module lexical.session;
@id("leaf") fn leaf(n:i64)->i64 { if n==0 {0}else{n} }
@id("first") fn first(n:i64)->i64 {
    let mut i:i64=0;
    while i<n { i=leaf(i)+1; if i==2 {continue;}else{0} 0 }
    return i;
}
@id("second") fn second(n:i64)->i64 {
    let mut i:i64=0;
    while i<n { i=leaf(i)+1; if i==3 {continue;}else{0} 0 }
    return i;
}
@id("main") fn main()->i64 {0}
"#;
fn program() -> ResolvedProgram {
    let program =
        crate::hir::resolve(&crate::check(SOURCE, "lexical-session.spx").unwrap()).unwrap();
    for member in members(&program) {
        assert!(member.cleanup_plan.lexical_control.is_some());
    }
    program
}
fn members(program: &ResolvedProgram) -> [&ResolvedFunction; 2] {
    ["first", "second"].map(|id| {
        program
            .functions()
            .iter()
            .find(|f| f.id.as_str() == id)
            .unwrap()
    })
}
fn run(
    program: &dyn ReplayContext,
    members: [&ResolvedFunction; 2],
    work: usize,
    bytes: usize,
) -> Result<(usize, usize), Diagnostic> {
    let mut budget = ReplayBudget::with_physical(members[0], bytes)?;
    budget.remaining = work;
    let mut session = Session::from_budget(program, members[0], budget)?;
    let index = session.indexed.storage_identity();
    for member in members {
        session.validate(member)?;
        assert_eq!(
            session.indexed.storage_identity(),
            index,
            "one retained callee inventory"
        );
    }
    let spent = work.checked_sub(session.budget.remaining).unwrap();
    let charged = session.budget.physical.as_ref().unwrap().charged();
    drop(session);
    Ok((spent, charged))
}
fn measured(
    program: &ResolvedProgram,
    work: usize,
    bytes: usize,
) -> (
    Result<(usize, usize), Diagnostic>,
    crate::test_allocation_observer::Metrics,
) {
    let context = primitive_fixture::Context::new(program);
    crate::test_allocation_observer::measure(|| run(&context, members(program), work, bytes))
}

#[test]
fn two_lexical_members_share_one_index_and_exact_whole_drop_ledgers() {
    let program = program();
    let before = members(&program).map(|f| f.cleanup_plan.clone());
    for member in members(&program) {
        validate_structure(&program, member).unwrap();
    }
    let (wide, allocation) = measured(&program, MAX_REPLAY_WORK_UNITS, 1 << 26);
    let (work, bytes) = wide.unwrap();
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.highwater <= bytes);
    assert_eq!(allocation.live, 0);
    let (exact, allocation) = measured(&program, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.highwater <= bytes);
    assert_eq!(allocation.live, 0);
    assert_eq!(
        measured(&program, work - 1, bytes).0.unwrap_err().code,
        "SPX-H006"
    );
    assert_eq!(
        measured(&program, work, bytes - 1).0.unwrap_err().code,
        "SPX-H006"
    );
    assert_eq!(members(&program).map(|f| f.cleanup_plan.clone()), before);
}

#[test]
fn malformed_later_certificate_body_and_target_cannot_reuse_an_earlier_lease() {
    for attack in 0..3 {
        let mut program = program();
        let second = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "second")
            .unwrap();
        let wrong = ExpressionId::new(
            &crate::hir::FunctionExecutionId::Monomorphic(second.id.clone()),
            "forged.later.member",
        );
        match attack {
            0 => {
                second
                    .cleanup_plan
                    .lexical_control
                    .as_mut()
                    .unwrap()
                    .exits
                    .iter_mut()
                    .find(|e| e.kind == crate::ast::LexicalExitKind::Continue)
                    .unwrap()
                    .site = wrong
            }
            1 => second.body.id = wrong,
            2 => {
                let certificate = second
                    .cleanup_plan
                    .lexical_control
                    .as_mut()
                    .unwrap()
                    .exits
                    .iter_mut()
                    .find(|e| e.kind == crate::ast::LexicalExitKind::Continue)
                    .unwrap();
                let crate::hir::ControlTarget::Loop { site, .. } = &mut certificate.target else {
                    panic!("loop")
                };
                *site = wrong;
            }
            _ => unreachable!(),
        }
        let context = primitive_fixture::Context::new(&program);
        let [first, second] = members(&program);
        let mut session = Session::new(&context, first, PhysicalReservation::new(1 << 26)).unwrap();
        session.validate(first).unwrap();
        assert_eq!(session.validate(second).unwrap_err().code, "SPX-H006");
        let remaining = session.budget.remaining;
        assert_eq!(session.validate(first).unwrap_err().code, "SPX-H006");
        assert_eq!(
            session.budget.remaining, remaining,
            "sticky refusal cannot restart the allowance"
        );
        assert_eq!(session.finish().err().unwrap().code, "SPX-H006");
    }
}

mod recovering;

#[path = "tests/setup.rs"]
mod setup;

#[path = "tests/paid.rs"]
mod paid;

#[path = "tests/fact_errors.rs"]
mod fact_errors;
mod structural_errors;

mod transition_errors;
