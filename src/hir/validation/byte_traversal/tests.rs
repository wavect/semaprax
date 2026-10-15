//! Owner freeze, provenance and receiver-shape authentication, each exercised
//! against its own independent check rather than through one shared refusal.
use super::*;

type Mutation = (&'static str, fn(&mut ResolvedExpr));
use crate::loan_plan::{LoanCause, LoanPointPhase};

/// An owned, renewable root viewed through a subrange alias: the source
/// owner is mutable, so only the traversal freeze keeps it fixed.
const SOURCE: &str = r#"module app.byte_traversal_freeze;

@id("app.freeze.main")
fn main() -> i64 {
    let data = [1u8, 2u8, 3u8];
    let mut owned = bytes_copy(array_as_slice(data));
    let whole = bytes_as_slice(owned);
    let view = byte_range(whole, 0usize, 2usize);
    let mut total = 0;
    for byte in view { let first = byte; total = total + 1; 0 }
    total
}
"#;

fn resolved() -> ResolvedProgram {
    let ast = crate::parse(SOURCE, std::path::Path::new("byte-traversal-freeze.spx")).unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}

fn statements(function: &mut ResolvedFunction) -> &mut Vec<ResolvedStatement> {
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("function block")
    };
    statements
}

fn owner(program: &ResolvedProgram) -> ResolvedBinding {
    let ResolvedExprKind::Block { statements, .. } = &program.functions[0].body.kind else {
        panic!("function block")
    };
    statements
        .iter()
        .find_map(|statement| match statement {
            ResolvedStatement::Let { binding, .. } if binding.name == "owned" => {
                Some(binding.clone())
            }
            _ => None,
        })
        .unwrap()
}

fn region_mut(function: &mut ResolvedFunction) -> &mut ResolvedExpr {
    statements(function)
        .iter_mut()
        .find_map(|statement| match statement {
            ResolvedStatement::Let { binding, value, .. } if binding.name == "#for-bytes" => {
                Some(value)
            }
            _ => None,
        })
        .unwrap()
}

fn loop_body(region: &mut ResolvedExpr) -> &mut Vec<ResolvedStatement> {
    let ResolvedExprKind::Block { statements, .. } = &mut region.kind else {
        panic!("region block")
    };
    let (ResolvedStatement::While { body, .. } | ResolvedStatement::LexicalWhile { body, .. }) =
        &mut statements[2]
    else {
        panic!("traversal loop")
    };
    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("loop body")
    };
    statements
}

fn authored(region: &mut ResolvedExpr) -> &mut Vec<ResolvedStatement> {
    let ResolvedStatement::Let { value, .. } = &mut loop_body(region)[1] else {
        panic!("authored body")
    };
    let ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
        panic!("authored block")
    };
    statements
}

fn receiver(region: &mut ResolvedExpr) -> &mut ResolvedExpr {
    let ResolvedStatement::Let { value, .. } = &mut loop_body(region)[0] else {
        panic!("item")
    };
    value
}

fn execution(program: &ResolvedProgram) -> FunctionExecutionId {
    FunctionExecutionId::Monomorphic(program.functions[0].id.clone())
}

#[test]
fn owner_assignment_anywhere_in_the_body_is_refused_by_the_census_freeze() {
    let mut program = resolved();
    let owned = owner(&program);
    let execution = execution(&program);
    let assignment = ResolvedStatement::Assign {
        binding: owned.clone(),
        field: None,
        value: ResolvedExpr {
            id: ExpressionId::new(&execution, "forged.renewal"),
            ty: owned.ty.clone(),
            ownership: OwnershipMode::Own,
            kind: ResolvedExprKind::Place(Place {
                root: owned.id.clone(),
                projections: Vec::new(),
            }),
            span: owned.span,
        },
        span: owned.span,
    };
    authored(region_mut(&mut program.functions[0])).insert(0, assignment);
    // The census reads only the authenticated provenance root, never liveness.
    let error = validate_function(&program.declarations, &program.functions[0]).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert!(error.message.contains("owner"), "{}", error.message);
    // Loan planning refuses the same renewal through the traversal freeze.
    let error = crate::loan_plan::build_plan(&program, &program.functions[0]).unwrap_err();
    assert!(
        error.message.contains("active shared loan"),
        "{}",
        error.message
    );
}

#[test]
fn traversal_freeze_loan_spans_the_whole_region_on_the_resolved_owner() {
    let program = resolved();
    let owned = owner(&program);
    let mut copy = program.clone();
    let region = region_mut(&mut copy.functions[0]).clone();
    let body_id = {
        let mut region = region.clone();
        let ResolvedStatement::Let { value, .. } = &loop_body(&mut region)[1] else {
            panic!("authored body")
        };
        value.id.clone()
    };
    let plan = &program.functions[0].loan_plan;
    let freeze = plan
        .loans
        .iter()
        .find(|loan| loan.cause == LoanCause::TraversalFreeze)
        .expect("byte traversal freeze loan");
    // Through `view` -> `whole` -> `owned`: the subrange alias reaches the root.
    assert_eq!(freeze.origin.root, owned.id);
    assert_eq!(freeze.site, region.id);
    assert_eq!(freeze.start.expression, region.id);
    assert_eq!(freeze.start.phase, LoanPointPhase::Before);
    let live_at = |expression: &ExpressionId, phase: LoanPointPhase| {
        plan.endpoints.iter().any(|endpoint| {
            endpoint.point.expression == *expression
                && endpoint.point.phase == phase
                && endpoint.live_after.contains(&freeze.id)
        })
    };
    // Live after the authored body on every iteration, and until the region ends.
    assert!(live_at(&body_id, LoanPointPhase::After));
    assert!(plan
        .endpoints
        .iter()
        .any(|endpoint| endpoint.point.expression == region.id
            && endpoint.point.phase == LoanPointPhase::After
            && endpoint.live_before.contains(&freeze.id)));

    // Attached evidence without the freeze is forged.
    let mut forged = program.clone();
    forged.functions[0]
        .loan_plan
        .loans
        .retain(|loan| loan.cause != LoanCause::TraversalFreeze);
    assert!(crate::loan_plan::validate_program(&forged).is_err());
}

#[test]
fn detached_view_without_authenticated_provenance_is_refused() {
    let mut program = resolved();
    let execution = execution(&program);
    let detached = ValueId::local(&execution, "forged.view");
    let region = region_mut(&mut program.functions[0]);
    let ResolvedExprKind::Block { statements, .. } = &mut region.kind else {
        panic!("region block")
    };
    let ResolvedStatement::Let { value: count, .. } = &mut statements[0] else {
        panic!("snapshot")
    };
    let ResolvedExprKind::Call { args, .. } = &mut count.kind else {
        panic!("byte_len")
    };
    let ResolvedExprKind::Place(place) = &mut args[0].kind else {
        panic!("view place")
    };
    place.root = detached.clone();
    let ResolvedExprKind::Match { scrutinee, .. } = &mut receiver(region).kind else {
        panic!("receiver")
    };
    let ResolvedExprKind::Call { args, .. } = &mut scrutinee.kind else {
        panic!("byte_get")
    };
    let ResolvedExprKind::Place(place) = &mut args[0].kind else {
        panic!("view place")
    };
    place.root = detached;
    let error = validate_function(&program.declarations, &program.functions[0]).unwrap_err();
    assert!(error.message.contains("provenance"), "{}", error.message);
}

#[test]
fn nested_receiver_in_the_authored_body_is_refused() {
    let mut program = resolved();
    let region = region_mut(&mut program.functions[0]);
    let nested = receiver(region).clone();
    let ResolvedStatement::Let { value, .. } = &mut authored(region)[0] else {
        panic!("authored let")
    };
    *value = nested;
    let error = validate_function(&program.declarations, &program.functions[0]).unwrap_err();
    assert!(error.message.contains("nested"), "{}", error.message);
}

/// Every partial `Option<u8>` consumer (HIR exhaustiveness, Graph projection)
/// keys on `partial_receiver`. Each single-fact deviation loses the exemption
/// and is refused as an ordinary non-exhaustive or ill-typed match.
#[test]
fn only_the_fully_authenticated_receiver_shape_is_partial() {
    let program = resolved();
    let mut copy = program.clone();
    let original = receiver(region_mut(&mut copy.functions[0])).clone();
    assert!(partial_receiver(&original));
    let mutations: [Mutation; 9] = [
        ("guarded arm", |e| {
            let ResolvedExprKind::Match { arms, .. } = &mut e.kind else {
                unreachable!()
            };
            arms[0].guard = Some(Box::new(arms[0].value.clone()));
        }),
        ("None case", |e| {
            let ResolvedExprKind::Match { arms, .. } = &mut e.kind else {
                unreachable!()
            };
            let ResolvedMatchPattern::Variant { case, .. } = &mut arms[0].pattern else {
                unreachable!()
            };
            *case = DeclarationId::new(crate::prelude::OPTION_NONE_ID);
        }),
        ("foreign field", |e| {
            let ResolvedExprKind::Match { arms, .. } = &mut e.kind else {
                unreachable!()
            };
            let ResolvedMatchPattern::Variant { fields, .. } = &mut arms[0].pattern else {
                unreachable!()
            };
            fields[0].field = DeclarationId::new("core.option.some.other");
        }),
        ("borrow mode", |e| {
            let ResolvedExprKind::Match { mode, .. } = &mut e.kind else {
                unreachable!()
            };
            *mode = ResolvedMatchMode::Borrow;
        }),
        ("vec_get scrutinee", |e| {
            let ResolvedExprKind::Match { scrutinee, .. } = &mut e.kind else {
                unreachable!()
            };
            let ResolvedExprKind::Call { callee, .. } = &mut scrutinee.kind else {
                unreachable!()
            };
            *callee = DeclarationId::new(crate::vec_ops::GET_ID);
        }),
        ("typed byte_get", |e| {
            let ResolvedExprKind::Match { scrutinee, .. } = &mut e.kind else {
                unreachable!()
            };
            let ResolvedExprKind::Call { type_arguments, .. } = &mut scrutinee.kind else {
                unreachable!()
            };
            type_arguments.push(ResolvedType::U8);
        }),
        ("projected index", |e| {
            let ResolvedExprKind::Match { scrutinee, .. } = &mut e.kind else {
                unreachable!()
            };
            let ResolvedExprKind::Call { args, .. } = &mut scrutinee.kind else {
                unreachable!()
            };
            args[1].kind = ResolvedExprKind::Usize(0);
        }),
        ("arm reads a literal", |e| {
            let ResolvedExprKind::Match { arms, .. } = &mut e.kind else {
                unreachable!()
            };
            arms[0].value.kind = ResolvedExprKind::Uint8(0);
        }),
        ("widened result", |e| {
            e.ty = ResolvedType::I64;
        }),
    ];
    for (case, mutate) in mutations {
        let mut forged = program.clone();
        mutate(receiver(region_mut(&mut forged.functions[0])));
        assert!(
            !partial_receiver(receiver(region_mut(&mut forged.functions[0]))),
            "{case}"
        );
        assert!(crate::hir::validate(&forged).is_err(), "{case}");
    }
}

#[test]
fn complete_receiver_census_region_and_discovery_have_no_heap_scratch() {
    let program = resolved();
    // The compiler and its ordinary validation storage are prepared outside
    // this observation. These are the complete NEW independent proof phases,
    // including provenance-root freezing and schema discovery, through return.
    let (((), observed), refused, used) = crate::bounded_output::with_limit_usage(0, || {
        crate::test_allocation_observer::measure(|| {
            for function in &program.functions {
                validate_function(&program.declarations, function).unwrap();
            }
            assert!(program_uses(&program));
        })
    });
    assert!(!refused);
    assert_eq!(used, 0);
    assert!(!observed.invalid);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.freed, 0);

    let ast = crate::parse(
        "module app.plain; @id(\"app.plain.main\") fn main() -> i64 { 0 }",
        std::path::Path::new("plain.spx"),
    )
    .unwrap();
    let plain = crate::hir::resolve(&ast).unwrap();
    let ((), absent) = crate::test_allocation_observer::measure(|| {
        validate_function(&plain.declarations, &plain.functions[0]).unwrap();
        assert!(!program_uses(&plain));
    });
    assert!(!absent.invalid);
    assert_eq!(absent.allocations, 0);
    assert_eq!(absent.live, 0);
}
