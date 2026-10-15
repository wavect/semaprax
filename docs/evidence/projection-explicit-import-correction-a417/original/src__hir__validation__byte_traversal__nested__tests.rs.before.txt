//! Authored controls: no compilation/runtime evidence is claimed by this file.
use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};
use std::cell::{Cell, RefCell};
const SOURCE: &str = include_str!("../../../../../tests/fixtures/nested-byte-slice-traversal.spx");
const ROOTED: &str = r#"module nested.freeze;
@id("nested.main") fn main()->i64 {
 let data=[1u8,2u8]; let mut owned=bytes_copy(array_as_slice(data));
 let whole=bytes_as_slice(owned); let view=byte_range(whole,0usize,1usize);
 for outer in whole { for inner in view { let copied=inner; 0 } let first=outer; 0 }
 0
}
"#;
thread_local! {
    static REQUESTS: RefCell<Option<Vec<usize>>> = const { RefCell::new(None) };
    static FAILED_AT: Cell<Option<usize>> = const { Cell::new(None) };
}
pub(super) fn record_request(bytes: usize) {
    REQUESTS.with(|slot| {
        if let Some(requests) = slot.borrow_mut().as_mut() {
            assert!(
                requests.len() < requests.capacity(),
                "test request inventory is exact/preowned"
            );
            requests.push(bytes);
        }
    });
}
pub(super) fn record_failure() {
    FAILED_AT.with(|slot| {
        if slot.get().is_none() {
            slot.set(Some(current_metrics().allocated));
        }
    });
}
fn resolved(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "nested-bytes.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}
fn function<'a>(program: &'a ResolvedProgram, name: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.name == name)
        .unwrap()
}
fn outer(function: &mut ResolvedFunction) -> &mut ResolvedExpr {
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("function block")
    };
    statements
        .iter_mut()
        .find_map(|statement| match statement {
            ResolvedStatement::Let { binding, value, .. } if binding.name == "#for-bytes" => {
                Some(value)
            }
            _ => None,
        })
        .unwrap()
}
fn authored(region: &mut ResolvedExpr) -> &mut Vec<ResolvedStatement> {
    let ResolvedExprKind::Block { statements, .. } = &mut region.kind else {
        panic!("region")
    };
    let (ResolvedStatement::While { body, .. } | ResolvedStatement::LexicalWhile { body, .. }) =
        &mut statements[2]
    else {
        panic!("loop")
    };
    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("steps")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[1] else {
        panic!("authored")
    };
    let ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
        panic!("authored block")
    };
    statements
}
fn inner(region: &mut ResolvedExpr) -> &mut ResolvedExpr {
    authored(region)
        .iter_mut()
        .find_map(|statement| match statement {
            ResolvedStatement::Let { binding, value, .. } if binding.name == "#for-bytes" => {
                Some(value)
            }
            _ => None,
        })
        .unwrap()
}
#[test]
fn nested_byte_full_source_hir_graph_and_cache_replay_preserves_v1_receiver_independence() {
    let ast = crate::check(SOURCE, "nested-bytes.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    assert_eq!(
        crate::format::canonical(&crate::check(&canonical, "nested-bytes.spx").unwrap()),
        canonical
    );
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
    let program = resolved(SOURCE);
    let wire = crate::cache_codec::encode(&program).unwrap();
    let decoded: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    crate::hir::validate(&decoded).unwrap();
    crate::loan_plan::validate_program(&decoded).unwrap();
    assert_eq!(crate::cache_codec::encode(&decoded).unwrap(), wire);
    let sum = function(&program, "sum");
    assert!(super::super::before_nested::validate_function(&program.declarations, sum).is_err());
    let manual = function(&program, "manual");
    assert_eq!(
        super::super::before_nested::validate_function(&program.declarations, manual),
        super::super::validate_function(&program.declarations, manual)
    );
    // A cache or context clone is a different immutable invocation member.
    let census = Census::acquire(&program, sum).unwrap();
    let row = census.rows[0].shape.root;
    let first = census.region(&decoded, sum, row).err().unwrap();
    assert!(first.message.contains("function differs"));
    let foreign_function = function(&decoded, "sum");
    assert!(Census::acquire(&program, foreign_function).is_err());
}
#[test]
fn nested_byte_ancestor_freezes_cover_every_inner_subtree_and_same_root_after_inner_exit() {
    let source = SOURCE.replace("for inner in right { total", "for inner in left { total");
    let program = resolved(&source);
    let sum = function(&program, "sum");
    let census = Census::acquire(&program, sum).unwrap();
    let outer = census.rows.iter().min_by_key(|row| row.start).unwrap();
    let inner = census.rows.iter().max_by_key(|row| row.start).unwrap();
    let freezes = sum
        .loan_plan
        .loans
        .iter()
        .filter(|loan| loan.cause == crate::loan_plan::LoanCause::TraversalFreeze)
        .collect::<Vec<_>>();
    assert_eq!(freezes.len(), 2);
    assert_eq!(freezes[0].origin.root, freezes[1].origin.root);
    let outer_loan = freezes
        .iter()
        .find(|loan| loan.site == outer.shape.root.id)
        .unwrap();
    let inner_loan = freezes
        .iter()
        .find(|loan| loan.site == inner.shape.root.id)
        .unwrap();
    assert_ne!(outer_loan.id, inner_loan.id);
    let after_inner = sum
        .loan_plan
        .endpoints
        .iter()
        .find(|endpoint| {
            endpoint.point.expression == inner.shape.root.id
                && endpoint.point.phase == crate::loan_plan::LoanPointPhase::After
        })
        .unwrap();
    assert!(after_inner.live_after.contains(&outer_loan.id));
    let after_outer = sum
        .loan_plan
        .endpoints
        .iter()
        .find(|endpoint| {
            endpoint.point.expression == outer.shape.root.id
                && endpoint.point.phase == crate::loan_plan::LoanPointPhase::After
        })
        .unwrap();
    assert!(after_outer.live_before.contains(&outer_loan.id));
    for id in outer.shape.frozen {
        let mut forged = program.clone();
        let sum = forged
            .functions
            .iter_mut()
            .find(|function| function.name == "sum")
            .unwrap();
        let owner = FunctionExecutionId::Monomorphic(sum.id.clone());
        let mut binding = shape::shape(super::tests::outer(sum))
            .unwrap()
            .unwrap()
            .payload
            .clone();
        binding.id = id.clone();
        let value = ResolvedExpr {
            id: ExpressionId::new(&owner, "forged.outer.freeze.assignment"),
            ty: binding.ty.clone(),
            ownership: binding.ownership,
            kind: ResolvedExprKind::Uint8(0),
            span: binding.span,
        };
        let span = binding.span;
        authored(super::tests::inner(super::tests::outer(sum))).push(ResolvedStatement::Assign {
            binding,
            field: None,
            value,
            span,
        });
        let error = super::super::validate_function(&forged.declarations, function(&forged, "sum"))
            .unwrap_err();
        assert!(
            error.message.contains("mutated or rebound"),
            "{}",
            error.message
        );
        assert!(crate::hir::validate(&forged).is_err());
    }
}
#[test]
fn nested_byte_census_uses_exact_owner_and_first_refusal_on_foreign_then_restored_scope() {
    let program = resolved(SOURCE);
    let sum = function(&program, "sum");
    let fixture = Census::acquire(&program, sum).unwrap();
    let root = fixture
        .rows
        .iter()
        .min_by_key(|row| row.start)
        .unwrap()
        .shape
        .root;
    drop(fixture);
    for warm in [false, true] {
        let ((((), metrics), remaining), refused, used, first) =
            with_limit_usage_refusal(usize::MAX, || {
                let mut slot = Slot::new(&program, sum).unwrap();
                if warm {
                    assert!(slot.region(&program, sum, root).unwrap().is_some());
                }
                let remaining = crate::bounded_output::active_remaining().unwrap();
                (
                    measure(|| {
                        let (error, child_refused, child_used, child_first) =
                            with_limit_usage_refusal(0, || {
                                slot.region(&program, sum, root).err().unwrap()
                            });
                        assert!(!child_refused);
                        assert_eq!(child_used, 0);
                        assert_eq!(child_first, None);
                        assert!(error.message.contains("detached"));
                        drop(error);
                        let restored = slot.region(&program, sum, root).err().unwrap();
                        assert!(restored.message.contains("detached"));
                        drop(restored);
                        drop(slot);
                    }),
                    remaining,
                )
            });
        assert!(refused);
        assert_eq!(
            first,
            Some(crate::bounded_output::ReservationRefusal {
                requested: 0,
                remaining,
                floor: 0,
                stage: None
            })
        );
        assert_eq!(used, usize::MAX - remaining);
        assert!(!metrics.invalid);
        assert_eq!(metrics.allocated, 0);
    }
}

#[test]
fn nested_byte_census_funds_exact_short_zero_every_frontier_sticky_and_complete_drop() {
    for (source, name, late) in [(SOURCE, "sum", false), (ROOTED, "main", true)] {
        let program = resolved(source);
        let sum = function(&program, name);
        let fixture = Census::acquire(&program, sum).unwrap();
        let root = fixture
            .rows
            .iter()
            .min_by_key(|row| row.start)
            .unwrap()
            .shape
            .root;
        drop(fixture);
        let operation = || -> Result<(), Diagnostic> {
            if late {
                super::super::validate_function(&program.declarations, sum)
            } else {
                let mut slot = Slot::new(&program, sum)?;
                slot.region(&program, sum, root)?;
                drop(slot);
                Ok(())
            }
        };

        // The recorder is test instrumentation, preowned before the measured route.
        // Its bound follows actual expression/statement shape, not a payment margin.
        let mut expressions = 0usize;
        let mut regions = 0usize;
        let mut bindings = sum.params.len();
        super::super::walk(&sum.body, |expression, authorized| {
            expressions += 1;
            regions += usize::from(shape::shape(expression)?.is_some());
            if let ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
            {
                bindings += statements.len();
            }
            Ok(authorized)
        })
        .unwrap();
        REQUESTS.with(|slot| {
            *slot.borrow_mut() = Some(Vec::with_capacity(
                64 + expressions * 2 + bindings * 2 + regions * 21,
            ))
        });
        let (((), metrics), refused, exact, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                operation().unwrap();
            })
        });
        let requests = REQUESTS.with(|slot| slot.borrow_mut().take().unwrap());
        assert!(!refused);
        assert_eq!(first, None);
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= exact);
        assert_eq!(requests.iter().sum::<usize>(), exact);
        let mut cumulative = 0usize;
        let mut caps = vec![0, exact - 1, exact];
        for request in requests {
            cumulative += request;
            if request != 0 {
                caps.push(cumulative - 1);
            }
        }
        caps.sort_unstable();
        caps.dedup();
        for cap in caps {
            FAILED_AT.with(|slot| slot.set(None));
            let ((failed, metrics), refused, used, first) = with_limit_usage_refusal(cap, || {
                measure(|| {
                    let result = operation();
                    let failed = result.is_err();
                    drop(result);
                    failed
                })
            });
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
            if cap == exact {
                assert!(!failed);
                assert!(!refused);
                assert_eq!(used, exact);
                assert_eq!(first, None);
            } else {
                assert!(failed);
                assert!(refused);
                assert!(first.is_some());
                assert_eq!(
                    metrics.allocated,
                    FAILED_AT.with(Cell::get).unwrap(),
                    "no allocation after first refusal including error/partial owner Drop"
                );
            }
        }
        let ((failed, metrics), refused, used, first) = with_limit_usage_refusal(0, || {
            assert!(!reserve_active_required(1));
            measure(|| {
                let result = operation();
                let failed = result.is_err();
                drop(result);
                failed
            })
        });
        assert!(failed && refused);
        assert_eq!(used, 0);
        assert_eq!(first.unwrap().requested, 1);
        assert_eq!(metrics.allocated, 0);
    }
}

fn retarget(expression: &mut ResolvedExpr, site: &ExpressionId) -> usize {
    match &mut expression.kind {
        ResolvedExprKind::LexicalExit {
            kind: crate::ast::LexicalExitKind::Break | crate::ast::LexicalExitKind::Continue,
            target: ControlTarget::Loop { site: actual, .. },
            ..
        } => {
            *actual = site.clone();
            1
        }
        ResolvedExprKind::Block { statements, tail } => {
            let mut count = retarget(tail, site);
            for statement in statements {
                count += match statement {
                    ResolvedStatement::While {
                        condition, body, ..
                    }
                    | ResolvedStatement::LexicalWhile {
                        condition, body, ..
                    } => retarget(condition, site) + retarget(body, site),
                    ResolvedStatement::Let { value, .. }
                    | ResolvedStatement::Assign { value, .. }
                    | ResolvedStatement::CopyDiscard { value, .. } => retarget(value, site),
                    ResolvedStatement::Unsafe { body, .. } => retarget(body, site),
                };
            }
            count
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => retarget(condition, site) + retarget(then_branch, site) + retarget(else_branch, site),
        _ => 0,
    }
}
#[test]
fn nested_byte_while_bridge_and_inner_only_exits_replay_exact_nearest_targets() {
    let program = resolved(SOURCE);
    for name in ["exits", "while_bridge"] {
        let function = function(&program, name);
        let owner = FunctionExecutionId::Monomorphic(function.id.clone());
        crate::hir::lexical_exit::validate_function(&program, function, &owner).unwrap();
        assert!(Census::acquire(&program, function).is_ok());
    }
    let mut forged = program.clone();
    let exits = forged
        .functions
        .iter_mut()
        .find(|function| function.name == "exits")
        .unwrap();
    let region = super::tests::outer(exits);
    let ResolvedExprKind::Block { statements, .. } = &region.kind else {
        panic!("outer region")
    };
    let (ResolvedStatement::While { condition, .. }
    | ResolvedStatement::LexicalWhile { condition, .. }) = &statements[2]
    else {
        panic!("outer loop")
    };
    let site = condition.id.clone();
    assert_eq!(
        retarget(region, &site),
        2,
        "both inner Break/Continue are kept"
    );
    let exits = function(&forged, "exits");
    let owner = FunctionExecutionId::Monomorphic(exits.id.clone());
    let error = crate::hir::lexical_exit::validate_function(&forged, exits, &owner).unwrap_err();
    assert_eq!(
        error,
        Diagnostic::io(
            "SPX-H006",
            "lexical exit kind, value or exact nearest target is inconsistent"
        )
    );
    assert!(crate::hir::validate(&forged).is_err());
    // An ordinary Vec accumulator is unrelated to byte receiver authority.
    // Source-for nesting remains closed for Vec; ordinary safe primitives do not.
    let accumulated = SOURCE.replacen("let mut total = 0;", "let mut total = 0; let mut accumulated = vec_with_capacity<i64>(6usize);", 1)
        .replacen("for inner in right { total", "for inner in right { accumulated = vec_push<i64>(accumulated, i64_from_u8(inner)); total", 1);
    let accumulated = resolved(&accumulated);
    super::super::validate_function(&accumulated.declarations, function(&accumulated, "sum"))
        .unwrap();
}
#[test]
fn nested_byte_same_root_release_does_not_allow_outer_move_or_renewal() {
    resolved(ROOTED);
    for source in [
        ROOTED.replace("let first=outer;", "let first=outer; let stolen=owned;"),
        ROOTED.replace(
            "let first=outer;",
            "let first=outer; owned=bytes_copy(array_as_slice(data));",
        ),
        ROOTED
            .replace("for outer in whole", "for outer in view")
            .replace("for inner in view", "for inner in whole")
            .replace("let first=outer;", "let first=outer; let stolen=owned;"),
    ] {
        let ast = crate::parse(&source, "nested-freeze.spx").unwrap();
        let diagnostics = crate::verify::verify(&ast);
        assert!(
            diagnostics.iter().any(|error| error.code == "SPX-T265"),
            "{diagnostics:?}"
        );
        assert!(crate::hir::resolve(&ast).is_err());
    }
}
