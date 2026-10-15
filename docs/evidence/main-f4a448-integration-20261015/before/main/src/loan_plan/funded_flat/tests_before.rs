use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::hir::DeclarationId;
use crate::test_allocation_observer::measure;

#[test]
fn funded_flat_receiver_keeps_complete_saved_plan_and_current_member_boundary() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    let function = &program.functions[index];
    let old = saved_planner::build_plan(&program, function).unwrap();
    let (result, refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        build(&program, function, &owner)
    });
    assert!(!refused);
    let actual = match result {
        Ok(plan) => plan,
        Err(_) => panic!("intended funded flat planner positive refused"),
    };
    assert_eq!(actual, old);
    assert!(!actual.loans.is_empty());
    // String-slot discovery must preserve the independent allocating predicate
    // while avoiding its temporary Vec<String> type constructor.
    let slot_ast = crate::check(
        r#"module slots;
@id("slots.inspect") fn inspect(values:own Vec<string>)->i64 {
 let text=vec_str_at(values,0usize);
 str_len_bytes(text)
}
@id("slots.main") fn main()->i64 {0}
"#,
        "slots.spx",
    )
    .unwrap();
    let slot_program = crate::hir::resolve(&slot_ast).unwrap();
    let slot_function = slot_program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "slots.inspect")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &slot_function.body.kind else {
        panic!("slot body");
    };
    let ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!("slot declaration");
    };
    let expected = crate::vec_ops::string_slot::place(value).unwrap();
    let ((), observed) = measure(|| {
        for _ in 0..32 {
            assert!(std::ptr::eq(
                paid_predicates::string_slot(value).unwrap(),
                expected
            ));
        }
    });
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
    let (_, old_queries) = measure(|| {
        for _ in 0..32 {
            assert!(crate::vec_ops::string_slot::place(value).is_some());
        }
    });
    assert_eq!(old_queries.allocations, 32);
    assert_eq!(
        old_queries.allocated,
        32 * std::mem::size_of::<ResolvedType>()
    );
    assert_eq!(old_queries.freed, old_queries.allocated);
    assert_eq!(old_queries.live, 0);
    for attack in 0..4 {
        let mut forged = value.clone();
        let ResolvedExprKind::Call {
            args,
            instance,
            type_arguments,
            ..
        } = &mut forged.kind
        else {
            panic!("slot call");
        };
        match attack {
            0 => args[0].ownership = OwnershipMode::Value,
            1 => args[1].ty = ResolvedType::I64,
            2 => type_arguments.clear(),
            3 => {
                *instance = Some(crate::hir::FunctionInstanceId::derive(
                    &DeclarationId::new("foreign.instance"),
                    &[],
                ))
            }
            _ => unreachable!(),
        }
        assert!(crate::vec_ops::string_slot::place(&forged).is_none());
        assert!(paid_predicates::string_slot(&forged).is_none());
    }
    let foreign = function.clone();
    let (result, refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        build(&program, &foreign, &owner)
    });
    assert!(!refused);
    match result {
        Err(PlannerError::Semantic(error)) => assert_eq!(
            error.message,
            "funded loan planner requires the exact current function"
        ),
        _ => panic!("foreign member was not rejected before planner storage"),
    }
}

#[test]
fn empty_semantic_diagnostic_is_not_a_physical_tag_or_paid_report() {
    let ((), refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        match classify::<()>(Err(storage::marker()), &owner) {
            Err(PlannerError::Semantic(error)) => {
                assert_eq!(error.code, "SPX-H006");
                assert!(error.message.is_empty());
            }
            _ => panic!("semantic empty diagnostic acquired physical meaning"),
        }
        assert_eq!(owner.first(), None);
    });
    assert!(!refused);
    let setup = std::mem::size_of::<CfgReservation>() + 46;
    // The independent literal is checked against the dedicated message length
    // by the owning report module; this is not an identity-message allowance.
    let ((), refused, _) = with_limit_usage(setup, || {
        let owner = CfgReservation::acquire().unwrap();
        let first = owner.reserve(1).unwrap_err();
        match classify::<()>(Err(storage::marker()), &owner) {
            Err(PlannerError::Physical(reason)) => assert_eq!(reason, first),
            _ => panic!("actual typed physical failure was rendered semantically"),
        }
        assert_eq!(owner.reserve(0), Err(first));
        drop(owner.diagnostic());
    });
    assert!(refused);
}

#[test]
fn actual_flat_adjacency_and_fixed_queue_arrays_have_exact_short_drop_payment() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    let function = &program.functions[index];
    let cfg = build_cfg(function, &mut WorkCounter::new(MAX_LOAN_PLAN_WORK_V1)).unwrap();
    let starts = [0_u16, 1, 2];
    let mut legacy_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
    let legacy = work::Reachability::build(&cfg, starts.into_iter(), &mut legacy_work).unwrap();
    // Exact actual allocations: two offsets, one predecessor entry array,
    // one mask word per point, two u16 links and one membership byte.
    let arrays =
        2 * (cfg.points.len() + 1) * 2 + cfg.edges.len() * 2 + cfg.points.len() * (8 + 2 + 2 + 1);
    let setup = std::mem::size_of::<CfgReservation>() + 46;
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let owner = CfgReservation::acquire().unwrap();
                let result = (|| {
                    let adjacency = adjacency::Adjacency::build_with_storage(
                        cfg.points.len(),
                        &cfg.edges,
                        Some(&owner),
                    )?;
                    let mut counted = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
                    counted.allocation = Some(&owner);
                    let reachable =
                        work::Reachability::build(&cfg, starts.into_iter(), &mut counted)?;
                    assert_eq!(counted.used, legacy_work.used);
                    for loan in 0..3 {
                        for node in 0..cfg.points.len() {
                            assert_eq!(
                                reachable.contains(LoanId(loan), node as u16),
                                legacy.contains(LoanId(loan), node as u16)
                            );
                        }
                    }
                    drop(reachable);
                    drop(adjacency);
                    Ok::<_, Diagnostic>(())
                })();
                match classify(result, &owner) {
                    Ok(()) => {
                        drop(owner);
                        true
                    }
                    Err(PlannerError::Physical(reason)) => {
                        assert_eq!(owner.reserve(0), Err(reason));
                        assert_eq!(owner.reserve(1), Err(reason));
                        drop(owner.diagnostic());
                        false
                    }
                    Err(PlannerError::Semantic(_)) => panic!("unexpected semantic failure"),
                }
            })
        })
    };
    let ((success, observed), refused, used, first) = run(setup + arrays);
    assert!(success);
    assert!(!refused);
    assert_eq!(used, setup + arrays);
    assert_eq!(first, None);
    assert_eq!(observed.allocations, 8); // text and seven actual arrays
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.allocated, 46 + arrays);
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
    let ((success, observed), refused, _, first) = run(setup + arrays - 1);
    assert!(!success);
    assert!(refused);
    assert_eq!(first.unwrap().requested, cfg.points.len()); // last bool array
    assert_eq!(observed.allocations, 7);
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
}

#[test]
fn complete_projected_and_lexical_planner_uses_one_paid_owner_through_drop() {
    const PROJECTED: &str = include_str!("../../../tests/fixtures/nested-collection-records.spx");
    const STRINGS: &str =
        include_str!("../../../tests/fixtures/nested-collection-string-record.spx");
    const LEXICAL: &str = r#"module app.paid_lexical;
@id("row") record Row { @id("row.left") left:i64, @id("row.right") right:i64, }
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{left:7,right:9});
 let mut total=0;
 for borrow row at index in rows {
  total=total+row.left+row.right+i64_from_usize(index);
  if total<0 {continue;} else {0}
  0
 }
 total
}"#;
    for (source, name, lexical) in [
        (PROJECTED, "collection.inspect", false),
        (STRINGS, "collection.inspect", false),
        (LEXICAL, "app.main", true),
    ] {
        let ast = crate::check(source, "paid-planner.spx").unwrap();
        let program = crate::hir::resolve(&ast).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == name)
            .unwrap();
        assert_eq!(
            crate::hir::lexical_exit::requires(function).unwrap(),
            lexical
        );
        if lexical {
            // Lower only the receiver's test work counter, not production
            // limits. Both routes execute the real BorrowRecord preflight.
            let legacy_owner = crate::hir::lexical_exit::owner(&program, function).unwrap();
            let mut old_work = WorkCounter::new(0);
            let old_error = crate::hir::lexical_exit::validate_function_with(
                &program,
                function,
                &legacy_owner,
                &mut |program, region| {
                    traversal::inspect_counted(program, region, &mut old_work)
                        .map(|proof| proof.is_some())
                },
            )
            .unwrap_err();
            assert_eq!(old_work.used, 1);
            assert_eq!(old_error.code, "SPX-H006");
            assert_eq!(
                old_error.message,
                "loan analysis exceeds 1,000,000 checked work"
            );
            assert_eq!(old_error.span, None);
            assert_eq!(old_error.help, None);
            let ((), refused, _) = with_limit_usage(usize::MAX, || {
                let payer = CfgReservation::acquire().unwrap();
                let mut new_work = WorkCounter::new(0);
                new_work.allocation = Some(&payer);
                match lexical_preflight(&program, function, &payer, &mut |program, region| {
                    traversal::paid::inspect_counted(program, region, &mut new_work)
                        .map(|proof| proof.is_some())
                }) {
                    Err(PlannerError::Semantic(actual)) => assert_eq!(actual, old_error),
                    _ => panic!("lexical work refusal changed its semantic boundary"),
                }
                assert_eq!(new_work.used, 1);
                assert_eq!(payer.first(), None);
            });
            assert!(!refused);
        }
        let old = saved_planner::build_plan(&program, function).unwrap();
        if lexical {
            // The lexical fixture owns its `rows` root: real loans exist.
            assert!(!old.loans.is_empty());
        } else {
            // The whole-projection fixtures inspect through `borrow Report`,
            // which has no own root: the saved planner invents no loans and
            // the funded planner must agree (checked by `assert_paid_owner_through_drop`).
            assert!(old.loans.is_empty());
        }
        assert_paid_owner_through_drop(&program, function, &old);
    }
}

#[test]
fn owning_root_projected_vec_and_string_loans_use_one_paid_owner_through_drop() {
    const PROJECTED: &str = include_str!("../../../tests/fixtures/nested-collection-records.spx");
    const STRINGS: &str =
        include_str!("../../../tests/fixtures/nested-collection-string-record.spx");
    // `app.main` owns `report`; borrowing its `items` field is a projected
    // Vec/string loan on a real own root.
    for (source, anchor, probe) in [
        (
            PROJECTED,
            "let first=inspect(report);",
            "let count=vec_len<Row>(report.items);let first=inspect(report);",
        ),
        (
            STRINGS,
            "let count=inspect(report);",
            "let probe=vec_len<string>(report.items);let count=inspect(report);",
        ),
    ] {
        assert_eq!(source.matches(anchor).count(), 1);
        let ast = crate::check(&source.replace(anchor, probe), "paid-planner.spx").unwrap();
        let program = crate::hir::resolve(&ast).unwrap();
        let inspect = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "collection.inspect")
            .unwrap();
        // The original borrow parameter still has no own root and no loans.
        assert!(saved_planner::build_plan(&program, inspect)
            .unwrap()
            .loans
            .is_empty());
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        assert!(!crate::hir::lexical_exit::requires(function).unwrap());
        let old = saved_planner::build_plan(&program, function).unwrap();
        assert!(!old.loans.is_empty());
        assert!(old
            .loans
            .iter()
            .any(|loan| !loan.origin.projections.is_empty()));
        assert_paid_owner_through_drop(&program, function, &old);
    }
}

/// Full-plan equality and exact/short payment, refusal and Drop checks for one
/// function, shared by every fixture above.
fn assert_paid_owner_through_drop(
    program: &crate::hir::ResolvedProgram,
    function: &crate::hir::ResolvedFunction,
    old: &LoanPlan,
) {
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            // Only the already established Budget/Rc context is excluded.
            measure(|| {
                let owner = CfgReservation::acquire().unwrap();
                match build(program, function, &owner) {
                    Ok(plan) => {
                        assert_eq!(&plan, old);
                        drop(plan);
                        drop(owner);
                        true
                    }
                    Err(PlannerError::Physical(reason)) => {
                        assert_eq!(owner.reserve(0), Err(reason));
                        assert_eq!(owner.reserve(1), Err(reason));
                        let pointer = owner.text_pointer();
                        let report = owner.diagnostic();
                        assert_eq!(report.message.as_ptr(), pointer);
                        drop(report);
                        false
                    }
                    Err(PlannerError::Semantic(_)) => {
                        panic!("checked positive planner selected semantic failure")
                    }
                }
            })
        })
    };
    let ((success, observed), refused, exact, first) = run(usize::MAX);
    assert!(success);
    assert!(!refused);
    assert_eq!(first, None);
    assert!(observed.allocations > 8);
    assert!(observed.allocated <= exact);
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
    let ((success, repeated), refused, used, first) = run(exact);
    assert!(success);
    assert!(!refused);
    assert_eq!(used, exact);
    assert_eq!(first, None);
    assert_eq!(repeated.allocated, observed.allocated);
    assert_eq!(repeated.allocations, observed.allocations);
    assert_eq!(repeated.freed, repeated.allocated);
    assert_eq!(repeated.live, 0);
    assert!(!repeated.invalid);
    let ((success, short), refused, _, first) = run(exact - 1);
    assert!(!success);
    assert!(refused);
    assert!(first.is_some());
    assert_eq!(short.freed, short.allocated);
    assert_eq!(short.live, 0);
    assert!(!short.invalid);
}
