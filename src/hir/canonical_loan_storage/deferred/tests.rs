//! Full-driver saved final boundary, plus narrow receipt/complete Eq controls.
//! Plan-builder scratch and detailed diagnostics keep their existing payment
//! scope. These observers establish removal, not whole-validator funding.
use super::*;
use crate::test_allocation_observer::measure;

fn fixture() -> ResolvedProgram {
    crate::hir::canonical_loan_storage::tests::fixture()
}
fn saved_validate(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    crate::cleanup_plan::with_coupled(|| {
        crate::hir::byte_partition::refuse_default(program)?;
        crate::hir::agent_validation::validate(program)?;
        crate::hir::inspection::validate_with_static_text(program, None)?;
        crate::loan_plan::validate_program(program)
    })
}
fn same(program: &ResolvedProgram) {
    assert_eq!(crate::hir::validate(program), saved_validate(program));
    let saved_private = (|| {
        crate::hir::agent_validation::validate(program)?;
        let selected = crate::hir::inspection::validate_with_static_text(program, None)?;
        crate::loan_plan::validate_program(program)?;
        Ok(selected)
    })();
    assert_eq!(
        crate::hir::byte_partition::validate_private(program),
        saved_private
    );
}
fn receipt(program: &ResolvedProgram) -> Deferred<'_> {
    let mut receipt = Deferred::new(program);
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        receipt.compare(
            function,
            &crate::loan_plan::build_plan(program, function).unwrap(),
        );
    }
    receipt
}

#[test]
fn deferred_complete_plans_preserve_saved_full_validator_forgery_and_error_priority() {
    let program = fixture();
    same(&program);
    assert!(!program.function_instances.is_empty());
    let first = program
        .functions
        .iter()
        .position(|f| !f.loan_plan.loans.is_empty())
        .unwrap();
    // Each attached field is mutated independently. Full Eq is necessary:
    // projected ID/origin/liveness storage alone omits several of these fields.
    for mutation in 0..18 {
        let mut forged = program.clone();
        let plan = &mut forged.functions[first].loan_plan;
        match mutation {
            0 => plan.schema = "forged",
            1 => {
                plan.loans.pop();
            }
            2 => plan.loans[0].id = crate::loan_plan::LoanId(255),
            3 => {
                plan.loans[0].site = crate::hir::ExpressionId::from_owned("forged.site".to_owned())
            }
            4 => plan.loans[0].origin.root = crate::hir::ValueId::new("forged.root".to_owned()),
            5 => plan.loans[0].parent = Some(crate::loan_plan::LoanId(255)),
            6 => plan.loans[0].start.phase = crate::loan_plan::LoanPointPhase::After,
            7 => plan.loans[0].ends.clear(),
            8 => plan.loans[0].end_edges.clear(),
            9 => plan.loans[0].cause = crate::loan_plan::LoanCause::StrView,
            10 => {
                plan.endpoints.pop();
            }
            11 => plan.endpoints[0].point.phase = crate::loan_plan::LoanPointPhase::After,
            12 => plan.endpoints[0].starts = Box::new([crate::loan_plan::LoanId(255)]),
            13 => plan.endpoints[0].kills = Box::new([crate::loan_plan::LoanId(255)]),
            14 => plan.endpoints[0].live_before = Box::new([crate::loan_plan::LoanId(255)]),
            15 => plan.endpoints[0].live_after = Box::new([crate::loan_plan::LoanId(255)]),
            16 => {
                plan.edges.pop();
            }
            17 => plan.edges[0].live = Box::new([crate::loan_plan::LoanId(255)]),
            _ => unreachable!(),
        }
        assert_ne!(plan, &program.functions[first].loan_plan);
        let direct = receipt(&forged).finish(&forged).unwrap_err();
        assert_eq!(
            direct,
            crate::loan_plan::validate_program(&forged).unwrap_err()
        );
        same(&forged);
    }
    let mut several = program.clone();
    for function in &mut several.functions {
        function.loan_plan.schema = "forged";
    }
    for instance in &mut several.function_instances {
        instance.function.loan_plan.schema = "forged";
    }
    assert_eq!(
        receipt(&several).finish(&several),
        crate::loan_plan::validate_program(&several)
    );
    same(&several);
    let mut instance_only = program.clone();
    instance_only.function_instances[0]
        .function
        .loan_plan
        .schema = "forged.instance";
    assert_eq!(
        receipt(&instance_only).finish(&instance_only),
        crate::loan_plan::validate_program(&instance_only)
    );
    same(&instance_only);
    let closures = crate::hir::resolve(
        &crate::check(
            r#"module loan.closure_storage;
@id("snapshot") fn snapshot()->i64 {
 let mut captured=7;
 let callback=fn(value:i64)->i64 {captured+value};
 captured=100;
 callback(5)
}
@id("shadow") fn shadow(captured:i64)->i64 {
 let callback=fn(captured:i64)->i64 {captured+1};
 callback(41)
}
@id("main") fn main()->i64 {snapshot()+shadow(99)}
"#,
            "canonical-loan-closure-storage.spx",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(crate::hir::closure::inventory(&closures).len(), 2);
    same(&closures);
    // Existing earlier identity/constructor/cleanup errors must still win over
    // an already observed attached mismatch, including an instance mismatch.
    let mut earlier = program.clone();
    earlier.functions[first].loan_plan.schema = "forged";
    earlier.entrypoint = crate::hir::DeclarationId::new("missing.entry");
    same(&earlier);
    assert!(!crate::hir::validate(&earlier)
        .unwrap_err()
        .message
        .contains("forged shared-loan"));
    let mut cleanup = program.clone();
    cleanup.functions[first].loan_plan.schema = "forged";
    cleanup.functions[first].cleanup_plan.schema = "forged.cleanup";
    same(&cleanup);
    assert!(!crate::hir::validate(&cleanup)
        .unwrap_err()
        .message
        .contains("forged shared-loan"));
    // The second own-root body is independently rebuilt after the first
    // mismatch. A genuine duplicate CFG identity must still be selected first.
    let mut constructor = program.clone();
    constructor.functions[first].loan_plan.schema = "forged";
    let second = constructor
        .functions
        .iter()
        .enumerate()
        .skip(first + 1)
        .find(|(_, f)| !f.loan_plan.loans.is_empty())
        .map(|(i, _)| i)
        .unwrap();
    let child_id = {
        let crate::hir::ResolvedExprKind::Block { statements, .. } =
            &constructor.functions[second].body.kind
        else {
            panic!("authored block")
        };
        let crate::hir::ResolvedStatement::Let { value, .. } = &statements[0] else {
            panic!("authored view binding")
        };
        value.id.clone()
    };
    constructor.functions[second].body.id = child_id;
    let builder_error =
        crate::loan_plan::build_plan(&constructor, &constructor.functions[second]).unwrap_err();
    assert_eq!(
        builder_error.message,
        "CFG contains a duplicate expression identity"
    );
    same(&constructor);
    assert_eq!(
        crate::hir::validate(&constructor).unwrap_err(),
        builder_error
    );
    let mut later = program.clone();
    later.functions[first].loan_plan.schema = "forged";
    later.functions.last_mut().unwrap().body.ty = crate::hir::ResolvedType::Bool;
    same(&later);
    assert!(!crate::hir::validate(&later)
        .unwrap_err()
        .message
        .contains("forged shared-loan"));
}

#[test]
fn deferred_receipt_seals_exact_subject_and_allocates_no_carrier_or_plan_payload() {
    let program = fixture();
    let equal_clone = program.clone();
    assert!(receipt(&program).finish(&equal_clone).is_err());
    assert!(receipt(&equal_clone).finish(&program).is_err());
    let plans = program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .map(|function| crate::loan_plan::build_plan(&program, function).unwrap())
        .collect::<Vec<_>>();
    let (_, observed) = measure(|| {
        let mut deferred = Deferred::new(&program);
        for (function, rebuilt) in program
            .functions
            .iter()
            .chain(
                program
                    .function_instances
                    .iter()
                    .map(|instance| &instance.function),
            )
            .zip(&plans)
        {
            deferred.compare(function, rebuilt);
        }
        deferred.finish(&program).unwrap();
    });
    assert_eq!(
        std::mem::size_of::<Deferred<'_>>(),
        2 * std::mem::size_of::<usize>()
    );
    assert_eq!(
        (
            observed.allocations,
            observed.reallocations,
            observed.allocated,
            observed.live
        ),
        (0, 0, 0, 0)
    );
    assert!(!observed.invalid);
    let mut forged = program.clone();
    forged.functions[0].loan_plan.schema = "forged";
    // The exact detailed legacy text is constructed outside the active owner.
    // Neither old nor deferred diagnostic rendering claims paid storage.
    let expected = Diagnostic::io(
        "SPX-H006",
        format!(
            "function `{}` has forged shared-loan evidence",
            forged.functions[0].id
        ),
    );
    for limit in [0usize, 1] {
        let deferred = receipt(&forged);
        let ((), failed, used, first) =
            crate::bounded_output::with_limit_usage_refusal(limit, || {
                assert_eq!(crate::bounded_output::active_remaining(), Some(limit));
                let original = crate::loan_plan::forged_evidence(&forged.functions[0]);
                let actual = deferred.finish(&forged).unwrap_err();
                assert_eq!(original, expected);
                assert_eq!(actual, expected);
                drop((original, actual));
                assert_eq!(crate::bounded_output::active_remaining(), Some(limit));
                assert!(crate::bounded_output::reserve_active_required(0));
            });
        assert_eq!((failed, used, first), (false, 0, None));
    }
}

#[test]
fn complete_invocation_removes_exact_saved_final_rebuild_and_preserves_sticky_budget_drop() {
    let program = fixture();
    let (_, final_build) = measure(|| crate::loan_plan::validate_program(&program).unwrap());
    let (_, old) = measure(|| saved_validate(&program).unwrap());
    let (_, new) = measure(|| crate::hir::validate(&program).unwrap());
    assert!(final_build.allocated > 0);
    assert_eq!(old.allocated, new.allocated + final_build.allocated);
    assert_eq!(old.allocations, new.allocations + final_build.allocations);
    assert_eq!(
        old.reallocations,
        new.reallocations + final_build.reallocations
    );
    for observed in [final_build, old, new] {
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    let (old_result, old_failed, old_used) =
        crate::bounded_output::with_limit_usage(usize::MAX, || saved_validate(&program));
    let (new_result, new_failed, new_used) =
        crate::bounded_output::with_limit_usage(usize::MAX, || crate::hir::validate(&program));
    let (_, extra_failed, extra_used) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        crate::loan_plan::validate_program(&program).unwrap()
    });
    assert_eq!(old_result, new_result);
    assert!(!old_failed && !new_failed && !extra_failed);
    assert_eq!(old_used, new_used + extra_used);
    let (exact, failed, used) =
        crate::bounded_output::with_limit_usage(new_used, || crate::hir::validate(&program));
    assert!(exact.is_ok());
    assert!(!failed);
    assert_eq!(used, new_used);
    let (((short, retry), observed), failed, _, first) =
        crate::bounded_output::with_limit_usage_refusal(new_used - 1, || {
            measure(|| {
                let short = crate::hir::validate(&program);
                let rejected = short.is_err();
                drop(short);
                let retry = crate::bounded_output::reserve_active_required(0);
                (rejected, retry)
            })
        });
    assert!(short && failed && !retry);
    assert!(first.is_some());
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}

#[path = "tests/selected.rs"]
mod selected;
